use std::{
    collections::BTreeMap,
    io::Read,
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use reqwest::{blocking::Client, header::HeaderValue};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    config::{self, MAX_RESPONSE},
    error::{Diagnostic, Result},
    model::{Binding, Engine, NativeSnapshot, Source},
};

pub fn fingerprint(value: &impl serde::Serialize) -> String {
    let bytes = serde_json::to_vec(value).expect("serializable internal model");
    format!("{:x}", Sha256::digest(bytes))
}

pub fn now() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

pub struct Responses {
    pub values: BTreeMap<String, Value>,
    pub sources: Vec<Source>,
    pub observed_at: String,
    pub live: bool,
}

pub fn fetch(binding: &Binding) -> Result<Responses> {
    let paths: &[&str] = match binding.engine {
        Engine::Caddy => &["/config/"],
        Engine::Traefik => &[
            "/api/http/routers",
            "/api/entrypoints",
            "/api/http/services",
        ],
    };
    if let Some(path) = &binding.snapshot_file {
        let snapshot: NativeSnapshot = config::load(Path::new(path), MAX_RESPONSE * 3)?;
        config::version(&snapshot.schema_version)?;
        if snapshot.binding != binding.id
            || snapshot.engine != binding.engine
            || snapshot.captured_at.is_empty()
            || snapshot.captured_at.len() > 128
        {
            return Err(Diagnostic::new(
                "snapshot.mismatch",
                "Snapshot binding, engine, or capture time does not match the explicit profile",
            ));
        }
        for path in paths {
            let value = snapshot.responses.get(*path).ok_or_else(|| {
                Diagnostic::new(
                    "snapshot.incomplete",
                    "Required response is missing from the snapshot",
                )
            })?;
            if serde_json::to_vec(value).map_or(true, |bytes| bytes.len() > MAX_RESPONSE) {
                return Err(Diagnostic::new(
                    "input.too_large",
                    "Snapshot response exceeds 8 MiB",
                ));
            }
        }
        let sources = paths
            .iter()
            .map(|path| Source {
                path: (*path).into(),
                fingerprint: fingerprint(&snapshot.responses[*path]),
                etag: None,
            })
            .collect();
        return Ok(Responses {
            values: snapshot.responses,
            sources,
            observed_at: snapshot.captured_at,
            live: false,
        });
    }
    let mut builder = Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(12));
    let mut base = binding.endpoint.trim_end_matches('/').to_owned();
    if let Some(socket) = binding.endpoint.strip_prefix("unix://") {
        #[cfg(unix)]
        {
            builder = builder.unix_socket(socket);
            base = "http://localhost".into();
        }
        #[cfg(not(unix))]
        {
            let _ = socket;
            return Err(Diagnostic::new(
                "capability.unsupported",
                "Unix sockets are unavailable on this platform",
            ));
        }
    }
    let client = builder.build().map_err(|_| {
        Diagnostic::new(
            "connection.client",
            "Could not construct the verified HTTP client",
        )
    })?;
    let authorization = if let Some(path) = &binding.auth_file {
        let bytes = config::read_bounded(Path::new(path), 8192)?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| {
                Diagnostic::new(
                    "connection.authorization",
                    "Authorization file must be UTF-8",
                )
            })?
            .trim();
        if text.is_empty() || text.chars().any(char::is_control) {
            return Err(Diagnostic::new(
                "connection.authorization",
                "Authorization file must contain one complete header value",
            ));
        }
        let mut header = HeaderValue::from_str(text).map_err(|_| {
            Diagnostic::new(
                "connection.authorization",
                "Invalid Authorization header value",
            )
        })?;
        header.set_sensitive(true);
        Some(header)
    } else {
        None
    };
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut values = BTreeMap::new();
    let mut sources = Vec::new();
    for path in paths {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| Diagnostic::new("connection.timeout", "Inventory deadline exceeded"))?;
        let query = if binding.engine == Engine::Traefik {
            "?per_page=10001"
        } else {
            ""
        };
        let mut request = client
            .get(format!("{base}{path}{query}"))
            .timeout(remaining.min(Duration::from_secs(12)));
        if let Some(value) = &authorization {
            request = request.header("Authorization", value.clone());
        }
        let response = request
            .header("Accept", "application/json")
            .send()
            .map_err(|e| {
                Diagnostic::new(
                    if e.is_timeout() {
                        "connection.timeout"
                    } else {
                        "connection.failed"
                    },
                    "API request failed; verify endpoint reachability, permissions, and TLS trust",
                )
            })?;
        if response.status() != reqwest::StatusCode::OK {
            return Err(Diagnostic::new(if response.status().as_u16() == 401 || response.status().as_u16() == 403 { "connection.denied" } else { "connection.status" },
                format!("API returned HTTP {}; redirects and response error bodies are not followed or displayed", response.status().as_u16())).at(binding.id.clone()));
        }
        let header_size: usize = response
            .headers()
            .iter()
            .map(|(name, value)| name.as_str().len() + value.len())
            .sum();
        if header_size > 32 * 1024 {
            return Err(Diagnostic::new(
                "connection.headers",
                "API headers exceed 32 KiB",
            ));
        }
        if binding.engine == Engine::Traefik
            && let Some(next) = response.headers().get("X-Next-Page")
            && !matches!(next.to_str(), Ok("" | "0" | "1"))
        {
            return Err(Diagnostic::new(
                "discovery.truncated",
                "Traefik returned more pages than the bounded snapshot request; do not treat it as complete",
            ));
        }
        let etag = response
            .headers()
            .get("ETag")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let mut bytes = Vec::new();
        response
            .take((MAX_RESPONSE + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| {
                Diagnostic::new(
                    "connection.body",
                    "Could not read API response within the request deadline",
                )
            })?;
        if bytes.len() > MAX_RESPONSE {
            return Err(Diagnostic::new("input.too_large", "API body exceeds 8 MiB"));
        }
        let value = config::strict_json(&bytes)?;
        sources.push(Source {
            path: (*path).into(),
            fingerprint: fingerprint(&value),
            etag,
        });
        values.insert((*path).into(), value);
    }
    Ok(Responses {
        values,
        sources,
        observed_at: now(),
        live: true,
    })
}
