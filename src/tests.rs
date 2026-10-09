use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use serde_json::{Value, json};

use crate::{
    config, inventory,
    model::{Binding, Engine, Manifest, NativeSnapshot, Scope, Source},
    plan, rules,
    transport::{self, Responses},
};

fn fixture(engine: Engine) -> (Binding, Responses) {
    let text = match engine {
        Engine::Traefik => include_str!("../fixtures/traefik/snapshot.json"),
        Engine::Caddy => include_str!("../fixtures/caddy/snapshot.json"),
    };
    let snapshot: NativeSnapshot = serde_json::from_str(text).unwrap();
    let sources = snapshot
        .responses
        .iter()
        .map(|(path, value)| Source {
            path: path.clone(),
            fingerprint: transport::fingerprint(value),
            etag: None,
        })
        .collect();
    let binding = Binding {
        id: snapshot.binding,
        engine,
        endpoint: "http://127.0.0.1:9".into(),
        auth_file: None,
        snapshot_file: None,
    };
    (
        binding,
        Responses {
            values: snapshot.responses,
            sources,
            observed_at: snapshot.captured_at,
            live: false,
        },
    )
}

#[test]
fn traefik_retains_native_routes_aliases_disabled_and_instance_identity() {
    let (binding, responses) = fixture(Engine::Traefik);
    let first = inventory::decode(&binding, Scope::User, responses).unwrap();
    assert!(!first.runtime_confirmed);
    assert!(first.complete);
    assert!(!first.write_authorized);
    assert_eq!(first.routes.len(), 2);
    let photos = first
        .routes
        .iter()
        .find(|r| r.native_id == "photos@file")
        .unwrap();
    assert_eq!(
        photos.hosts,
        vec!["photos.example.test", "pictures.example.test"]
    );
    assert!(!photos.any_host);
    assert_eq!(photos.native["router"]["priority"], 30);
    assert_eq!(photos.targets, vec!["http://photos:3000"]);
    let disabled = first
        .routes
        .iter()
        .find(|r| r.native_id == "old@docker")
        .unwrap();
    assert_eq!(disabled.native_status, "disabled");
    let (mut binding, responses) = fixture(Engine::Traefik);
    binding.id = "another-instance".into();
    let second = inventory::decode(&binding, Scope::User, responses).unwrap();
    assert_ne!(first.binding, second.binding);
    assert_ne!(first.binding_fingerprint, second.binding_fingerprint);
    assert_eq!(second.routes.len(), 2);
}

#[test]
fn caddy_preserves_unknowns_and_original_matchers_without_false_completeness() {
    let (binding, responses) = fixture(Engine::Caddy);
    let observed = inventory::decode(&binding, Scope::User, responses).unwrap();
    assert!(!observed.complete);
    assert_eq!(observed.routes.len(), 2);
    assert!(
        observed
            .diagnostics
            .iter()
            .any(|d| d.code == "discovery.unsupported_handler")
    );
    let reverse = observed
        .routes
        .iter()
        .find(|r| r.targets == vec!["photos:3000"])
        .unwrap();
    assert_eq!(reverse.hosts, vec!["photos.example.test"]);
    assert_eq!(reverse.paths, vec!["/library/*"]);
    assert!(!reverse.any_host);
    assert!(reverse.reference.contains("/handle/0/routes/0/handle/0"));
    let unknown = observed
        .routes
        .iter()
        .find(|r| r.native_id.starts_with("external-special"))
        .unwrap();
    assert_eq!(unknown.matcher_coverage, "unresolved");
    assert_eq!(unknown.native["handler"]["setting"], "synthetic");
    let output = serde_json::to_string(&observed).unwrap();
    assert!(!output.contains("synthetic"));
    assert!(!output.contains("http.request.method"));
}

#[test]
fn rule_interpretation_keeps_unrestricted_or_branches_conservative() {
    assert!(
        !rules::interpret("Host(`a.example.test`) && PathPrefix(`/a`)")
            .unwrap()
            .any_host
    );
    assert!(
        rules::interpret("Host(`a.example.test`) || PathPrefix(`/all`)")
            .unwrap()
            .any_host
    );
    assert!(
        !rules::interpret(
            "(Host(`a.example.test`) || PathPrefix(`/all`)) && Host(`b.example.test`)"
        )
        .unwrap()
        .any_host
    );
    for text in [
        "!Host(`a.example.test`)",
        "HostRegexp(`.*`)",
        "Host(`a.example.test`) && Header(`Authorization`,`private`)",
        "Host()",
    ] {
        assert!(rules::interpret(text).is_err(), "{text}");
    }
    let too_deep = format!(
        "{}Host(`a.example.test`){}",
        "(".repeat(100),
        ")".repeat(100)
    );
    assert!(rules::interpret(&too_deep).is_err());
}

#[test]
fn config_rejects_ambiguous_yaml_json_and_mutation_types() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("input.yaml");
    for text in [
        "schema_version: \"1\"\nschema_version: \"2\"\nscope: user\nproxies: []",
        "schema_version: \"1\"\nscope: user\nproxies: []\n---\nx: y",
        "schema_version: !unexpected \"1\"\nscope: user\nproxies: []",
        "schema_version: \"1\"\nscope: user\nproxies: []\nextra: ignored",
    ] {
        fs::write(&path, text).unwrap();
        assert!(config::load_config(&path).is_err());
    }
    let path = dir.path().join("input.json");
    fs::write(&path,r#"{"schema_version":"1","scope":"user","proxies":[{"id":"edge","engine":"traefik","endpoint":"http://proxy","endpoint":"http://other"}]}"#).unwrap();
    assert!(config::load_config(&path).is_err());
    fs::write(&path,r#"{"schema_version":"1","scope":"user","proxies":[{"id":"edge","engine":"traefik","endpoint":"http://user:secret@proxy"}]}"#).unwrap();
    let err = config::load_config(&path).unwrap_err().to_string();
    assert!(!err.contains("secret"));
    let path = dir.path().join("route.yaml");
    for text in [
        "schema_version: \"1\"\nscope: user\nroutes: [{id: a, proxy: edge, hostname: a.test, enabled: yes, upstream: {url: 'http://localhost:3000'}, tls: {mode: http}}]",
        "schema_version: \"1\"\nscope: user\nroutes: [{id: a, proxy: edge, hostname: a.test, enabled: true, upstream: {url: 'http://localhost:3000/path'}, tls: {mode: http}}]",
        "schema_version: \"1\"\nscope: user\nservices: [{id: worker}]",
    ] {
        fs::write(&path, text).unwrap();
        assert!(config::load_manifest(&path).is_err());
    }
    for name in [
        "*.example.test",
        "https://example.test",
        "-bad.test",
        "127.0.0.1",
        "[::1]",
    ] {
        assert!(config::exact_host(name).is_err());
    }
    assert_eq!(
        config::exact_host("Photos.Example.Test.").unwrap(),
        "photos.example.test"
    );
}

fn manifest(host: &str) -> Manifest {
    serde_json::from_value(json!({"schema_version":"1","scope":"user","routes":[{"id":"new","proxy":"edge","hostname":host,"enabled":true,"upstream":{"url":"http://127.0.0.1:3000"},"tls":{"mode":"http"}}]})).unwrap()
}

#[test]
fn plans_are_deterministic_read_only_and_do_not_adopt_external_names() {
    let (binding, responses) = fixture(Engine::Traefik);
    let observed = inventory::decode(&binding, Scope::User, responses).unwrap();
    let request = manifest("photos.example.test");
    let first = plan::build(&request, &[observed]).unwrap();
    assert!(!first.applicable);
    assert!(
        first.actions[0]
            .conflicts
            .iter()
            .any(|d| d.code == "route.external_overlap")
    );
    assert!(
        first
            .diagnostics
            .iter()
            .any(|d| d.code == "capability.apply_unavailable")
    );
    let (binding, responses) = fixture(Engine::Traefik);
    let observed = inventory::decode(&binding, Scope::User, responses).unwrap();
    let again = plan::build(&request, &[observed]).unwrap();
    assert_eq!(
        serde_json::to_value(&first).unwrap(),
        serde_json::to_value(&again).unwrap()
    );
    let (binding, responses) = fixture(Engine::Traefik);
    let mut observed = inventory::decode(&binding, Scope::User, responses).unwrap();
    observed.scope = Scope::System;
    assert!(plan::build(&request, &[observed]).is_err());
}

struct Mock {
    endpoint: String,
    requests: Arc<Mutex<Vec<String>>>,
    join: Option<thread::JoinHandle<()>>,
}
impl Mock {
    fn new(responses: Vec<String>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let join = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            for response in responses {
                let (mut stream, _) = loop {
                    match listener.accept() {
                        Ok(value) => break value,
                        Err(_) if Instant::now() < deadline => {
                            thread::sleep(Duration::from_millis(5))
                        }
                        Err(_) => return,
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = Vec::new();
                let mut byte = [0];
                while !request.ends_with(b"\r\n\r\n") && request.len() < 32768 {
                    if stream.read(&mut byte).unwrap_or(0) == 0 {
                        break;
                    };
                    request.push(byte[0]);
                }
                captured
                    .lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&request).into_owned());
                let _ = stream.write_all(response.as_bytes());
            }
        });
        Self {
            endpoint,
            requests,
            join: Some(join),
        }
    }
}
impl Drop for Mock {
    fn drop(&mut self) {
        if let Some(join) = self.join.take() {
            join.join().unwrap();
        }
    }
}
fn http_response(status: &str, headers: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}",
        body.len()
    )
}

#[test]
fn live_transport_uses_base_path_auth_and_bounded_traefik_query() {
    let (_, fixture) = fixture(Engine::Traefik);
    let server = Mock::new(
        [
            "/api/http/routers",
            "/api/entrypoints",
            "/api/http/services",
        ]
        .iter()
        .map(|path| {
            http_response(
                "200 OK",
                "X-Next-Page: 1\r\n",
                &fixture.values[*path].to_string(),
            )
        })
        .collect(),
    );
    let dir = tempfile::tempdir().unwrap();
    let secret = dir.path().join("auth");
    fs::write(&secret, "Bearer TEST_ONLY_NEVER_PRINT\n").unwrap();
    let binding = Binding {
        id: "edge".into(),
        engine: Engine::Traefik,
        endpoint: format!("{}/admin/", server.endpoint),
        auth_file: Some(secret.display().to_string()),
        snapshot_file: None,
    };
    let result = inventory::inspect(&binding, Scope::User).unwrap();
    assert!(result.runtime_confirmed);
    for request in server.requests.lock().unwrap().iter() {
        assert!(request.contains("/admin/api/") && request.contains("per_page=10001"));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer test_only_never_print")
        );
    }
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains("TEST_ONLY_NEVER_PRINT")
    );
}

#[test]
fn denied_redirect_truncated_and_oversized_responses_never_become_empty_inventory() {
    for (response, code) in [
        (
            http_response("401 Unauthorized", "", "NEVER_PRINT_ERROR_BODY"),
            "connection.denied",
        ),
        (
            http_response("302 Found", "Location: http://169.254.169.254/\r\n", ""),
            "connection.status",
        ),
        (
            http_response("200 OK", "X-Next-Page: 2\r\n", "[]"),
            "discovery.truncated",
        ),
        (
            http_response("200 OK", "", &"x".repeat(config::MAX_RESPONSE + 1)),
            "input.too_large",
        ),
    ] {
        let server = Mock::new(vec![response]);
        let binding = Binding {
            id: "edge".into(),
            engine: Engine::Traefik,
            endpoint: server.endpoint.clone(),
            auth_file: None,
            snapshot_file: None,
        };
        let err = transport::fetch(&binding).err().unwrap();
        assert_eq!(err.code, code);
        assert!(!err.message.contains("NEVER_PRINT_ERROR_BODY"));
        assert_eq!(server.requests.lock().unwrap().len(), 1);
    }
}

#[test]
fn malformed_native_objects_and_duplicate_ids_fail_closed() {
    let (binding, mut responses) = fixture(Engine::Traefik);
    responses
        .values
        .insert("/api/http/routers".into(), Value::Null);
    assert!(inventory::decode(&binding, Scope::User, responses).is_err());
    let (binding, mut responses) = fixture(Engine::Traefik);
    let routers = responses
        .values
        .get_mut("/api/http/routers")
        .unwrap()
        .as_array_mut()
        .unwrap();
    routers.push(routers[0].clone());
    assert!(inventory::decode(&binding, Scope::User, responses).is_err());
    let (binding, mut responses) = fixture(Engine::Caddy);
    responses
        .values
        .insert("/config/".into(), json!({"ok":true}));
    assert!(inventory::decode(&binding, Scope::User, responses).is_err());
}

#[test]
fn snapshot_version_and_binding_mismatch_are_rejected_without_network() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("snapshot.json");
    let (mut binding, responses) = fixture(Engine::Traefik);
    fs::write(&file,json!({"schema_version":"1","binding":"other","engine":"traefik","captured_at":"fixture","responses":responses.values}).to_string()).unwrap();
    binding.snapshot_file = Some(file.display().to_string());
    assert_eq!(
        transport::fetch(&binding).err().unwrap().code,
        "snapshot.mismatch"
    );
}

#[cfg(unix)]
#[test]
fn unix_admin_socket_is_read_without_host_network_access() {
    use std::os::unix::net::UnixListener;
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("admin.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let (mut stream, _) = loop {
            match listener.accept() {
                Ok(value) => break value,
                Err(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
                Err(_) => panic!("Unix request not received"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = Vec::new();
        let mut byte = [0];
        while !request.ends_with(b"\r\n\r\n") && request.len() < 32768 {
            assert_eq!(stream.read(&mut byte).unwrap(), 1);
            request.push(byte[0]);
        }
        assert!(String::from_utf8_lossy(&request).starts_with("GET /config/ "));
        stream
            .write_all(http_response("200 OK", "", "{}").as_bytes())
            .unwrap();
    });
    let binding = Binding {
        id: "socket".into(),
        engine: Engine::Caddy,
        endpoint: format!("unix://{}", socket.display()),
        auth_file: None,
        snapshot_file: None,
    };
    let observed = inventory::inspect(&binding, Scope::User).unwrap();
    assert!(observed.runtime_confirmed && observed.routes.is_empty());
    server.join().unwrap();
}

#[test]
fn authorization_rotation_is_loaded_for_each_command_and_never_printed() {
    let dir = tempfile::tempdir().unwrap();
    let secret = dir.path().join("auth");
    let server = Mock::new(vec![
        http_response("200 OK", "", "{}"),
        http_response("200 OK", "", "{}"),
    ]);
    let binding = Binding {
        id: "edge".into(),
        engine: Engine::Caddy,
        endpoint: server.endpoint.clone(),
        auth_file: Some(secret.display().to_string()),
        snapshot_file: None,
    };
    for token in ["Bearer FIRST_TEST_SECRET", "Bearer SECOND_TEST_SECRET"] {
        fs::write(&secret, token).unwrap();
        let observed = inventory::inspect(&binding, Scope::User).unwrap();
        assert!(!serde_json::to_string(&observed).unwrap().contains(token));
    }
    let requests = server.requests.lock().unwrap();
    assert!(requests[0].contains("FIRST_TEST_SECRET"));
    assert!(requests[1].contains("SECOND_TEST_SECRET"));
}

#[test]
fn generated_internal_and_error_routes_are_preserved_without_claiming_health() {
    let (binding, mut responses) = fixture(Engine::Traefik);
    responses.values.get_mut("/api/http/routers").unwrap().as_array_mut().unwrap().extend([
        json!({"name":"ping@internal","provider":"internal","rule":"PathPrefix(`/ping`)","service":"ping@internal","using":["web"],"status":"enabled"}),
        json!({"name":"broken@file","provider":"file","rule":"HostRegexp(`.*`)","service":"missing@file","using":["web"],"status":"error","errors":["synthetic"]}),
    ]);
    responses
        .values
        .get_mut("/api/http/services")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .push(json!({"name":"ping@internal","status":"enabled"}));
    let observed = inventory::decode(&binding, Scope::User, responses).unwrap();
    assert_eq!(observed.routes.len(), 4);
    assert!(!observed.complete);
    assert!(
        observed
            .routes
            .iter()
            .any(|r| r.origin == "internal" && r.native_id == "ping@internal")
    );
    let error = observed
        .routes
        .iter()
        .find(|r| r.native_id == "broken@file")
        .unwrap();
    assert_eq!(error.native_status, "error");
    assert_eq!(error.native["router"]["errors"][0], "synthetic");
    assert_eq!(error.ownership, "observed_external");
}
