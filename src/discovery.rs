//! Explicit bounded discovery. Candidates are evidence, never automatic write authority.
use crate::{
    config,
    error::Result,
    executor,
    model::{Binding, Engine},
    transport,
};

pub fn inspect_file(
    path: &Path,
    engine: Engine,
    binary: &Path,
    id: &str,
) -> Result<crate::model::Inventory> {
    let root: Value = if engine == Engine::Caddy && path.extension().is_none_or(|e| e != "json") {
        config::strict_json(&executor::checked(
            binary,
            &[
                "adapt".as_ref(),
                "--config".as_ref(),
                path.as_os_str(),
                "--adapter".as_ref(),
                "caddyfile".as_ref(),
            ],
        )?)?
    } else {
        config::load(path, config::MAX_CONFIG)?
    };
    let mut values = std::collections::BTreeMap::new();
    if engine == Engine::Caddy {
        values.insert("/config/".into(), root.clone());
    } else {
        let empty = serde_json::Map::new();
        let routers = root
            .pointer("/http/routers")
            .and_then(Value::as_object)
            .unwrap_or(&empty);
        let services = root
            .pointer("/http/services")
            .and_then(Value::as_object)
            .unwrap_or(&empty);
        let mut entries = std::collections::BTreeSet::new();
        let mut rows = Vec::new();
        for (name, router) in routers {
            let mut row = router.clone();
            row["name"] = json!(format!("{name}@file"));
            row["provider"] = json!("file");
            row["status"] = json!("configured");
            if let Some(list) = row["entryPoints"].as_array() {
                for entry in list {
                    if let Some(e) = entry.as_str() {
                        entries.insert(e.to_owned());
                    }
                }
            }
            rows.push(row);
        }
        values.insert("/api/http/routers".into(), json!(rows));
        values.insert(
            "/api/entrypoints".into(),
            json!(
                entries
                    .into_iter()
                    .map(|name| json!({"name":name}))
                    .collect::<Vec<_>>()
            ),
        );
        values.insert(
            "/api/http/services".into(),
            json!(
                services
                    .iter()
                    .map(|(name, value)| {
                        let mut row = value.clone();
                        row["name"] = json!(format!("{name}@file"));
                        row
                    })
                    .collect::<Vec<_>>()
            ),
        );
    }
    let binding = Binding {
        id: id.into(),
        engine,
        endpoint: "http://127.0.0.1".into(),
        auth_file: None,
        snapshot_file: None,
    };
    let responses = transport::Responses {
        values,
        sources: vec![crate::model::Source {
            path: path.to_string_lossy().into_owned(),
            fingerprint: transport::fingerprint(&root),
            etag: None,
        }],
        observed_at: transport::now(),
        live: false,
    };
    let mut observed = crate::inventory::decode(&binding, crate::model::Scope::User, responses)?;
    observed.source = "native_file";
    for route in &mut observed.routes {
        route.source = "native_file";
        route.reference = format!("{}#{}", path.display(), route.reference);
    }
    Ok(observed)
}
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

pub fn discover(docker: Option<&str>, probe: bool) -> Result<Value> {
    let mut candidates = Vec::new();
    let mut diagnostics = Vec::new();
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut files = vec![
        (Engine::Caddy, PathBuf::from("/etc/caddy/Caddyfile")),
        (Engine::Traefik, PathBuf::from("/etc/traefik/traefik.yml")),
        (Engine::Traefik, PathBuf::from("/etc/traefik/traefik.yaml")),
    ];
    if let Some(home) = home {
        files.extend([
            (Engine::Caddy, home.join(".config/caddy/Caddyfile")),
            (Engine::Traefik, home.join(".config/traefik/traefik.yaml")),
        ]);
    }
    for (engine, path) in files {
        if path.is_file() {
            candidates.push(json!({"engine":engine,"source":"known_native_path","path":path,"runtime_confirmed":false,"write_authorized":false}));
        }
    }
    if let Ok(output) = executor::run(
        Command::new("ps").args(["-ax", "-o", "pid=,comm="]),
        Duration::from_secs(5),
        1024 * 1024,
    ) {
        for line in String::from_utf8_lossy(&output.stdout).lines().take(10000) {
            let mut words = line.split_whitespace();
            let pid = words.next().unwrap_or("");
            let name = words
                .next()
                .and_then(|s| Path::new(s).file_name())
                .and_then(|s| s.to_str())
                .unwrap_or("");
            if matches!(name, "caddy" | "traefik") {
                candidates.push(json!({"engine":name,"pid":pid,"source":"native_process","write_authorized":false}));
            }
        }
    }
    if probe {
        for (engine, endpoint) in [
            (Engine::Caddy, "http://127.0.0.1:2019"),
            (Engine::Traefik, "http://127.0.0.1:8080"),
        ] {
            let binding = Binding {
                id: "candidate".into(),
                engine,
                endpoint: endpoint.into(),
                auth_file: None,
                snapshot_file: None,
            };
            match transport::fetch(&binding){Ok(r)=>candidates.push(json!({"engine":engine,"endpoint":endpoint,"source":"runtime_api","runtime_confirmed":r.live,"write_authorized":false})),Err(e)=>diagnostics.push(json!(e)),}
        }
    }
    if let Some(endpoint) = docker {
        config::validate_endpoint(endpoint)?;
        let mut command = Command::new("docker");
        command.args([
            "--host",
            endpoint,
            "container",
            "ls",
            "--all",
            "--no-trunc",
            "--format",
            "{{json .}}",
        ]);
        match executor::run(&mut command,Duration::from_secs(15),4*1024*1024){
            Ok(output) if output.status.success()=>{
                let text=String::from_utf8_lossy(&output.stdout);let rows:Vec<_>=text.lines().take(257).collect();
                if rows.len()>256 {diagnostics.push(json!({"code":"discovery.truncated","message":"Docker candidates limited to 256"}));}
                for row in rows.into_iter().take(256){let value=config::strict_json(row.as_bytes())?;let Some(id)=value["ID"].as_str()else{continue;};if !id.bytes().all(|c|c.is_ascii_hexdigit()){continue;}
                    let output=executor::run(Command::new("docker").args(["--host",endpoint,"inspect",id]),Duration::from_secs(10),1024*1024)?;
                    if !output.status.success(){diagnostics.push(json!({"code":"discovery.docker_inspect","container":id}));continue;}
                    let native=config::strict_json(&output.stdout)?;let labels=native[0].pointer("/Config/Labels").and_then(Value::as_object);
                    let mut known=serde_json::Map::new();if let Some(labels)=labels{for (key,value) in labels{
                        let traefik=key.starts_with("traefik.http.routers.")&&(key.ends_with(".rule")||key.ends_with(".entrypoints")||key.ends_with(".service"));
                        let caddy=key=="caddy"||key.starts_with("caddy_")&&!key.contains('.');
                         if (traefik||caddy)&&value.as_str().is_some_and(|s|s.len()<=2048 && !s.chars().any(char::is_control)){
                             let safe=if traefik&&key.ends_with(".rule") {crate::rules::interpret(value.as_str().unwrap_or("")).map_or_else(|_|json!({"coverage":"unsupported","value":"redacted"}),|m|json!({"hosts":m.hosts,"paths":m.paths,"any_host":m.any_host}))}else{value.clone()};
                             known.insert(key.clone(),safe);
                         }
                    }}
                    let image=native[0].pointer("/Config/Image").and_then(Value::as_str).unwrap_or("");
                    if !known.is_empty()||image.contains("caddy")||image.contains("traefik"){candidates.push(json!({"container":id,"image":image,"source":"docker_known_labels","labels":known,"runtime_confirmed":false,"write_authorized":false,"network_context":"container","endpoint":endpoint}));}
                }
            },
            _=>diagnostics.push(json!({"code":"discovery.docker_unavailable","message":"Docker enumeration failed; not an empty inventory"})),
        }
    }
    Ok(json!({"candidates":candidates,"complete":diagnostics.is_empty(),"diagnostics":diagnostics}))
}
