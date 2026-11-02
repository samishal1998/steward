//! Bounded local discovery. Only decoded native APIs become live router candidates.
use super::*;
use crate::{error::Diagnostic, inventory, model::Scope};
use std::{collections::BTreeSet, time::Instant};

const MAX_CANDIDATES: usize = 64;

pub fn scan() -> Result<Value> {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut candidates = Vec::new();
    let mut diagnostics = Vec::new();
    let mut files = vec![
        (Engine::Caddy, PathBuf::from("/etc/caddy/Caddyfile")),
        (Engine::Caddy, PathBuf::from("/etc/caddy/caddy.json")),
        (Engine::Traefik, PathBuf::from("/etc/traefik/traefik.yml")),
        (Engine::Traefik, PathBuf::from("/etc/traefik/traefik.yaml")),
        (Engine::Traefik, PathBuf::from("/etc/traefik/traefik.json")),
    ];
    for prefix in ["/opt/homebrew/etc", "/usr/local/etc"] {
        files.push((Engine::Caddy, PathBuf::from(prefix).join("Caddyfile")));
        files.push((
            Engine::Traefik,
            PathBuf::from(prefix).join("traefik/traefik.yml"),
        ));
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let config_root = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|h| h.join(".config")));
    if let Some(root) = config_root {
        for (engine, name) in [
            (Engine::Caddy, "caddy/Caddyfile"),
            (Engine::Caddy, "caddy/caddy.json"),
            (Engine::Traefik, "traefik/traefik.yaml"),
            (Engine::Traefik, "traefik/traefik.yml"),
            (Engine::Traefik, "traefik/traefik.json"),
        ] {
            files.push((engine, root.join(name)));
        }
    }
    if let Ok(output) = executor::run(
        Command::new("ps").args(["-ax", "-o", "pid=,comm="]),
        Duration::from_secs(1),
        1024 * 1024,
    ) {
        for line in String::from_utf8_lossy(&output.stdout).lines().take(10000) {
            if candidates.len() >= 24 || Instant::now() >= deadline {
                break;
            }
            let Some((pid, executable)) = line.trim().split_once(char::is_whitespace) else {
                continue;
            };
            let name = Path::new(executable.trim())
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            let engine = match name {
                "caddy" => Engine::Caddy,
                "traefik" => Engine::Traefik,
                _ => continue,
            };
            if !pid.bytes().all(|b| b.is_ascii_digit()) {
                continue;
            }
            let args = process_args(pid);
            let path = flag(&args, &["--config", "--configfile"]);
            let mut config_path = None;
            if let Some(path) = path {
                let path = PathBuf::from(path);
                #[cfg(target_os = "linux")]
                let path = if path.is_absolute() {
                    path
                } else {
                    std::fs::read_link(format!("/proc/{pid}/cwd"))
                        .map(|cwd| cwd.join(&path))
                        .unwrap_or(path)
                };
                if path.is_absolute() {
                    files.push((engine, path.clone()));
                    config_path = Some(path);
                }
            }
            let endpoint = if let Some(path) = &config_path {
                file_endpoint(engine, path)
            } else if engine == Engine::Traefik
                && (args
                    .iter()
                    .any(|s| s.eq_ignore_ascii_case("--api.insecure"))
                    || flag(&args, &["--api.insecure"]).is_some_and(|s| s == "true"))
            {
                local_endpoint(flag(&args, &["--entrypoints.traefik.address"]).unwrap_or(":8080"))
            } else {
                local_endpoint(if engine == Engine::Caddy {
                    "localhost:2019"
                } else {
                    "localhost:8080"
                })
            };
            candidates.push(json!({"engine":engine,"pid":pid,"path":config_path,"source":"native_process","endpoint":endpoint,"runtime_confirmed":false,"write_authorized":false}));
        }
    }
    let mut seen_files = BTreeSet::new();
    for (engine, path) in files {
        if candidates.len() >= MAX_CANDIDATES || Instant::now() >= deadline {
            break;
        }
        if !path.is_file() || !seen_files.insert(path.clone()) {
            continue;
        }
        let endpoint = file_endpoint(engine, &path);
        candidates.push(json!({"engine":engine,"path":path,"source":"native_config","endpoint":endpoint,"runtime_confirmed":false,"write_authorized":false}));
    }
    docker_candidates(&mut candidates, &mut diagnostics, home.as_deref(), deadline);
    // Prefer explicit config/process endpoints to conventional ports within the scan budget.
    for (engine, endpoint) in [
        (Engine::Caddy, "http://127.0.0.1:2019"),
        (Engine::Traefik, "http://127.0.0.1:8080"),
    ] {
        candidates.push(json!({"engine":engine,"endpoint":endpoint,"source":"default_api","runtime_confirmed":false,"write_authorized":false}));
    }
    let mut seen = BTreeSet::new();
    let mut found = Vec::new();
    let mut inventories = Vec::new();
    let mut retained = 0;
    for mut candidate in candidates.into_iter().take(MAX_CANDIDATES) {
        let engine: Engine =
            serde_json::from_value(candidate["engine"].clone()).expect("candidate engine");
        let endpoint = candidate["endpoint"].as_str().and_then(local_endpoint);
        let key = format!(
            "{}:{}",
            candidate["engine"],
            endpoint
                .as_deref()
                .unwrap_or_else(|| candidate["path"].as_str().unwrap_or_else(|| {
                    candidate["container"]
                        .as_str()
                        .unwrap_or_else(|| candidate["pid"].as_str().unwrap_or("unknown"))
                }))
        );
        if !seen.insert(key.clone()) {
            continue;
        }
        let id = format!(
            "detected-{}-{}",
            if engine == Engine::Caddy {
                "caddy"
            } else {
                "traefik"
            },
            &transport::fingerprint(&key)[..8]
        );
        candidate["binding"] = json!(id);
        if let Some(endpoint) = endpoint {
            candidate["endpoint"] = json!(endpoint);
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                diagnostics.push(json!(Diagnostic::new(
                    "discovery.deadline",
                    "Local scan reached its 10-second budget; rescan to retry remaining candidates"
                )));
                break;
            }
            let binding = Binding {
                id,
                engine,
                endpoint,
                auth_file: None,
                snapshot_file: None,
            };
            match transport::probe(&binding, remaining.min(Duration::from_secs(2))).and_then(
                |responses| {
                    if engine == Engine::Caddy
                        && !responses.values.get("/config/").is_some_and(|v| {
                            v.is_object()
                                && (v.as_object().is_some_and(|o| o.is_empty())
                                    || ["apps", "admin", "logging", "storage"]
                                        .iter()
                                        .any(|key| v.get(key).is_some()))
                        })
                    {
                        return Err(Diagnostic::new(
                            "discovery.not_native",
                            "Endpoint did not return a native Caddy configuration",
                        ));
                    }
                    inventory::decode(&binding, Scope::User, responses)
                },
            ) {
                Ok(observed) if retained + observed.retained_bytes <= 32 * 1024 * 1024 => {
                    retained += observed.retained_bytes;
                    candidate["runtime_confirmed"] = json!(true);
                    candidate["route_count"] = json!(observed.routes.len());
                    inventories.push(json!(observed));
                }
                Ok(_) => diagnostics.push(json!(Diagnostic::new(
                    "discovery.budget",
                    "Detected inventory exceeded the 32 MiB scan budget"
                ))),
                Err(error) => {
                    // An unused conventional port is absence, not a broken configured router.
                    if candidate["source"] == "default_api" && error.code == "connection.failed" {
                        continue;
                    }
                    candidate["diagnostics"] = json!([error]);
                    if candidate["source"] == "default_api"
                        && candidate["diagnostics"][0]["code"] != "connection.denied"
                    {
                        continue;
                    }
                }
            }
        }
        found.push(candidate);
    }
    Ok(
        json!({"candidates":found,"inventories":inventories,"complete":diagnostics.is_empty(),"diagnostics":diagnostics}),
    )
}

fn process_args(pid: &str) -> Vec<String> {
    #[cfg(target_os = "linux")]
    if let Ok(bytes) = config::read_bounded(Path::new(&format!("/proc/{pid}/cmdline")), 16384) {
        return bytes
            .split(|b| *b == 0)
            .filter(|s| !s.is_empty())
            .map(|s| String::from_utf8_lossy(s).into_owned())
            .collect();
    }
    executor::run(
        Command::new("ps").args(["-p", pid, "-o", "command="]),
        Duration::from_secs(1),
        16384,
    )
    .ok()
    .and_then(|o| shlex::split(&String::from_utf8_lossy(&o.stdout)))
    .unwrap_or_default()
}
fn flag<'a>(args: &'a [String], names: &[&str]) -> Option<&'a str> {
    for (i, arg) in args.iter().enumerate() {
        let (name, inline) = arg
            .split_once('=')
            .map_or((arg.as_str(), None), |(a, b)| (a, Some(b)));
        if names.contains(&name.to_ascii_lowercase().as_str()) {
            return inline.or_else(|| {
                args.get(i + 1)
                    .filter(|s| !s.starts_with('-'))
                    .map(String::as_str)
            });
        }
    }
    None
}
fn file_endpoint(engine: Engine, path: &Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }
    if engine == Engine::Caddy {
        let bytes = config::read_bounded(path, config::MAX_CONFIG).ok()?;
        if let Ok(value) = config::strict_json(&bytes) {
            return if value.pointer("/admin/disabled") == Some(&json!(true)) {
                None
            } else {
                local_endpoint(
                    value
                        .pointer("/admin/listen")
                        .and_then(Value::as_str)
                        .unwrap_or("localhost:2019"),
                )
            };
        }
        if path.extension().is_some_and(|s| s == "json") {
            return None;
        }
        let text = std::str::from_utf8(&bytes).ok()?;
        let mut lines = text
            .lines()
            .take(128)
            .map(str::trim)
            .filter(|s| !s.is_empty() && !s.starts_with('#'));
        if lines.next() == Some("{") {
            for line in lines {
                if line.trim() == "}" {
                    break;
                }
                let words: Vec<_> = line.split_whitespace().take(3).collect();
                if words.first() == Some(&"admin") {
                    return words.get(1).and_then(|s| local_endpoint(s));
                }
            }
        }
        return local_endpoint("localhost:2019");
    }
    let value: Value = config::load(path, config::MAX_CONFIG).ok()?;
    match engine {
        Engine::Traefik if value.pointer("/api/insecure") == Some(&json!(true)) => local_endpoint(
            value
                .pointer("/entryPoints/traefik/address")
                .and_then(Value::as_str)
                .unwrap_or(":8080"),
        ),
        _ => None,
    }
}
fn local_endpoint(address: &str) -> Option<String> {
    if address == "off" || address.contains(['{', '}', '\n', '\r']) {
        return None;
    }
    if let Some(path) = address.strip_prefix("unix://") {
        return Path::new(path).is_absolute().then(|| address.into());
    }
    if let Some(path) = address.strip_prefix("unix//") {
        return Some(format!("unix:///{}", path.trim_start_matches('/')));
    }
    let address = if address.starts_with(':') {
        format!("127.0.0.1{address}")
    } else {
        address.into()
    };
    let mut url = reqwest::Url::parse(&if address.contains("://") {
        address
    } else {
        format!("http://{address}")
    })
    .ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    match url.host_str()? {
        "0.0.0.0" => {
            url.set_host(Some("127.0.0.1")).ok()?;
        }
        "[::]" => {
            url.set_host(Some("[::1]")).ok()?;
        }
        "localhost" if url.scheme() == "http" => {
            url.set_host(Some("127.0.0.1")).ok()?;
        }
        "localhost" | "127.0.0.1" | "[::1]" => {}
        _ => return None,
    }
    if url.port() == Some(0) {
        return None;
    }
    Some(url.to_string().trim_end_matches('/').into())
}

fn docker_candidates(
    candidates: &mut Vec<Value>,
    diagnostics: &mut Vec<Value>,
    home: Option<&Path>,
    deadline: Instant,
) {
    let socket = std::env::var("DOCKER_HOST")
        .ok()
        .filter(|s| s.starts_with("unix:///"))
        .or_else(|| {
            [
                Some(PathBuf::from("/var/run/docker.sock")),
                home.map(|h| h.join(".docker/run/docker.sock")),
            ]
            .into_iter()
            .flatten()
            .find(|p| p.exists())
            .map(|p| format!("unix://{}", p.display()))
        });
    let Some(socket) = socket else {
        return;
    };
    let Ok(output) = executor::run(
        Command::new("docker").args([
            "--host",
            &socket,
            "container",
            "ls",
            "--format",
            "{{json .}}",
        ]),
        Duration::from_secs(2),
        1024 * 1024,
    ) else {
        return;
    };
    if !output.status.success() {
        diagnostics.push(json!(Diagnostic::new(
            "discovery.docker_unavailable",
            "Local Docker inventory could not be read"
        )));
        return;
    }
    for line in String::from_utf8_lossy(&output.stdout).lines().take(128) {
        if candidates.len() >= MAX_CANDIDATES - 2 || Instant::now() >= deadline {
            break;
        }
        let Ok(row) = config::strict_json(line.as_bytes()) else {
            continue;
        };
        let image = row["Image"].as_str().unwrap_or("");
        let engine = if image.contains("caddy") {
            Engine::Caddy
        } else if image.contains("traefik") {
            Engine::Traefik
        } else {
            continue;
        };
        let id = row["ID"].as_str().unwrap_or("");
        if id.is_empty() || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        let Ok(output) = executor::run(
            Command::new("docker").args(["--host", &socket, "inspect", id]),
            Duration::from_secs(1),
            config::MAX_CONFIG as u64,
        ) else {
            continue;
        };
        let Ok(native) = config::strict_json(&output.stdout) else {
            continue;
        };
        let port = if engine == Engine::Caddy {
            "2019/tcp"
        } else {
            "8080/tcp"
        };
        let published = native[0]["NetworkSettings"]["Ports"][port]
            .as_array()
            .and_then(|a| a.first());
        let endpoint = published
            .and_then(|p| p["HostPort"].as_str())
            .and_then(|p| local_endpoint(&format!("127.0.0.1:{p}")));
        candidates.push(json!({"engine":engine,"container":id,"name":row["Names"],"image":image,"source":"docker_container","endpoint":endpoint,"runtime_confirmed":false,"write_authorized":false}));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovery_only_probes_local_endpoints_and_reads_explicit_admin_configuration() {
        assert_eq!(
            local_endpoint(":22019").as_deref(),
            Some("http://127.0.0.1:22019")
        );
        assert_eq!(
            local_endpoint("0.0.0.0:8080").as_deref(),
            Some("http://127.0.0.1:8080")
        );
        assert_eq!(
            local_endpoint("unix//tmp/caddy.sock").as_deref(),
            Some("unix:///tmp/caddy.sock")
        );
        for input in [
            "https://example.com",
            "http://user:password@localhost:2019",
            "off",
            "{$ADMIN}",
            ":0",
        ] {
            assert!(local_endpoint(input).is_none(), "{input}");
        }
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("caddy.json");
        std::fs::write(&path, r#"{"admin":{"listen":"localhost:29123"}}"#).unwrap();
        assert_eq!(
            file_endpoint(Engine::Caddy, &path).as_deref(),
            Some("http://127.0.0.1:29123")
        );
        std::fs::write(&path, r#"{"admin":{"disabled":true}}"#).unwrap();
        assert!(file_endpoint(Engine::Caddy, &path).is_none());
        let path = root.path().join("native-config");
        std::fs::write(&path, r#"{"admin":{"listen":":29234"}}"#).unwrap();
        assert_eq!(
            file_endpoint(Engine::Caddy, &path).as_deref(),
            Some("http://127.0.0.1:29234")
        );
        std::fs::write(&path, "# Caddy global options\n{\n admin :29345\n}\n").unwrap();
        assert_eq!(
            file_endpoint(Engine::Caddy, &path).as_deref(),
            Some("http://127.0.0.1:29345")
        );
        assert_eq!(
            flag(&["--configFile=/tmp/config.yaml".into()], &["--configfile"]),
            Some("/tmp/config.yaml")
        );
    }
}
