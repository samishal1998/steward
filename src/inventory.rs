use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::{
    config::MAX_RECORDS,
    error::{Diagnostic, Result},
    model::{Binding, Engine, Inventory, RouteObservation, Scope},
    rules,
    transport::{self, Responses, fingerprint},
};

pub fn inspect(binding: &Binding, scope: Scope) -> Result<Inventory> {
    let responses = transport::fetch(binding)?;
    decode(binding, scope, responses)
}

pub fn decode(binding: &Binding, scope: Scope, responses: Responses) -> Result<Inventory> {
    let mut inventory = Inventory {
        schema_version: "1",
        binding: binding.id.clone(),
        engine: binding.engine,
        scope,
        binding_fingerprint: fingerprint(binding),
        source: if responses.live {
            "runtime_api"
        } else {
            "snapshot_file"
        },
        observed_at: responses.observed_at,
        runtime_confirmed: responses.live,
        atomic_snapshot: false,
        controller: "unknown",
        runtime: "unknown",
        probe_network_context: "calling_process",
        write_authorized: false,
        complete: true,
        sources: responses.sources,
        routes: Vec::new(),
        diagnostics: Vec::new(),
        retained_bytes: 0,
    };
    if !inventory.runtime_confirmed {
        inventory.diagnostics.push(Diagnostic::new(
            "discovery.offline",
            "File observations are not evidence that this configuration is currently loaded",
        ));
    }
    match binding.engine {
        Engine::Traefik => traefik(&responses.values, &mut inventory)?,
        Engine::Caddy => caddy(&responses.values, &mut inventory)?,
    }
    inventory.routes.sort_by(|a, b| {
        a.native_id
            .cmp(&b.native_id)
            .then(a.reference.cmp(&b.reference))
    });
    Ok(inventory)
}

fn array(value: &Value) -> Result<&Vec<Value>> {
    let rows = value.as_array().ok_or_else(|| {
        Diagnostic::new("discovery.invalid_response", "Expected a native JSON array")
    })?;
    if rows.len() > MAX_RECORDS {
        return Err(Diagnostic::new(
            "discovery.too_many_records",
            "Native response exceeds 10,000 records",
        ));
    }
    Ok(rows)
}

fn required<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 65_536)
        .ok_or_else(|| {
            Diagnostic::new(
                "discovery.invalid_response",
                format!("Native object is missing a usable {key} field"),
            )
        })
}

fn strings(value: Option<&Value>) -> Result<Vec<String>> {
    match value {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(rows)) if rows.len() <= MAX_RECORDS => rows
            .iter()
            .map(|v| {
                v.as_str().map(str::to_owned).ok_or_else(|| {
                    Diagnostic::new(
                        "discovery.invalid_response",
                        "Expected a native string list",
                    )
                })
            })
            .collect(),
        _ => Err(Diagnostic::new(
            "discovery.invalid_response",
            "Expected a bounded native string list",
        )),
    }
}

fn diagnostic(inventory: &mut Inventory, code: &str, message: &str, reference: &str) {
    inventory.complete = false;
    inventory
        .diagnostics
        .push(Diagnostic::new(code, message).at(reference));
}

fn retain_native(inventory: &mut Inventory, evidence: Value) -> Result<Value> {
    let bytes = serde_json::to_vec(&evidence)
        .expect("native JSON is serializable")
        .len();
    // ponytail: cap expanded evidence at 32 MiB per binding; share native object
    // references for larger inventories rather than allowing unbounded clones.
    if bytes > 32 * 1024 * 1024 - inventory.retained_bytes {
        return Err(Diagnostic::new(
            "discovery.evidence_budget",
            "Expanded native evidence exceeds 32 MiB for this binding",
        ));
    }
    inventory.retained_bytes += bytes;
    Ok(evidence)
}

fn traefik(values: &BTreeMap<String, Value>, inventory: &mut Inventory) -> Result<()> {
    let routers = array(&values["/api/http/routers"])?;
    let entries = array(&values["/api/entrypoints"])?;
    let services = array(&values["/api/http/services"])?;
    let mut by_service = BTreeMap::new();
    let mut by_entry = BTreeMap::new();
    for entry in entries {
        by_entry.insert(required(entry, "name")?, entry);
    }
    for service in services {
        by_service.insert(required(service, "name")?, service);
    }
    let mut native_ids = std::collections::BTreeSet::new();
    for (index, router) in routers.iter().enumerate() {
        let name = required(router, "name")?;
        if !native_ids.insert(name) {
            return Err(Diagnostic::new(
                "discovery.duplicate_native_id",
                "Traefik returned a duplicate router name",
            ));
        }
        let rule = required(router, "rule")?;
        let reference = format!("/api/http/routers/{index}");
        let parsed = rules::interpret(rule);
        let (matched, mut covered) = match parsed {
            Ok(value) => (value, true),
            Err(_) => {
                diagnostic(
                    inventory,
                    "discovery.unsupported_rule",
                    "Unsupported native matcher; original evidence is retained internally",
                    &reference,
                );
                (rules::Match::default(), false)
            }
        };
        let status = router
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        if !matches!(status, "enabled" | "disabled") {
            diagnostic(
                inventory,
                "discovery.native_status",
                "Native router reports an error or unknown activation status",
                &reference,
            );
            covered = false;
        }
        let using = strings(router.get("using"))?;
        let ep_names = if using.is_empty() {
            strings(router.get("entryPoints"))?
        } else {
            using
        };
        for ep in &ep_names {
            if !by_entry.contains_key(ep.as_str()) {
                covered = false;
                diagnostic(
                    inventory,
                    "discovery.entrypoint_missing",
                    "Router entrypoint is absent from this inventory",
                    &reference,
                );
            }
        }
        let origin = router
            .get("provider")
            .and_then(Value::as_str)
            .or_else(|| name.rsplit_once('@').map(|(_, p)| p))
            .unwrap_or("unknown");
        let service_name = router.get("service").and_then(Value::as_str).unwrap_or("");
        let qualified = if service_name.contains('@') {
            service_name.to_owned()
        } else {
            format!("{service_name}@{origin}")
        };
        let service = by_service
            .get(service_name)
            .or_else(|| by_service.get(qualified.as_str()))
            .copied();
        if service.is_none() {
            covered = false;
            diagnostic(
                inventory,
                "discovery.service_missing",
                "Router service is absent from this inventory",
                &reference,
            );
        }
        let mut targets = Vec::new();
        if let Some(service) = service
            && let Some(servers) = service.pointer("/loadBalancer/servers")
        {
            for server in array(servers)? {
                let target = required(server, "url")?;
                // Backend credentials are never published in inventory or error output.
                targets.push(
                    if reqwest::Url::parse(target).is_ok_and(|u| {
                        u.username().is_empty()
                            && u.password().is_none()
                            && u.query().is_none()
                            && u.fragment().is_none()
                    }) {
                        target.into()
                    } else {
                        "[redacted or unsupported upstream]".into()
                    },
                );
            }
        }
        let tls = router
            .get("tls")
            .map(|v| !v.is_null() && v != &Value::Bool(false));
        let native = retain_native(inventory, json!({"router":router,"service":service}))?;
        inventory.routes.push(RouteObservation {
            binding: inventory.binding.clone(),
            native_id: name.into(),
            reference,
            origin: origin.into(),
            source: inventory.source,
            observed_at: inventory.observed_at.clone(),
            runtime_confirmed: inventory.runtime_confirmed,
            native_status: status.into(),
            ownership: "observed_external",
            hosts: matched.hosts,
            any_host: matched.any_host,
            paths: matched.paths,
            targets,
            entrypoints: ep_names,
            tls_configured: tls,
            matcher_coverage: if covered {
                "literal_subset"
            } else {
                "unresolved"
            },
            native_fingerprint: fingerprint(router),
            native,
        });
    }
    Ok(())
}

fn caddy(values: &BTreeMap<String, Value>, inventory: &mut Inventory) -> Result<()> {
    let root = &values["/config/"];
    if !root.is_object() {
        return Err(Diagnostic::new(
            "discovery.invalid_response",
            "Expected a Caddy configuration object",
        ));
    }
    if !root.as_object().expect("object").is_empty()
        && ["apps", "admin", "logging", "storage"]
            .iter()
            .all(|key| root.get(key).is_none())
    {
        return Err(Diagnostic::new(
            "discovery.invalid_response",
            "Response does not resemble a Caddy configuration",
        ));
    }
    let Some(http) = root.pointer("/apps/http") else {
        return Ok(());
    };
    let servers = http
        .get("servers")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            Diagnostic::new(
                "discovery.invalid_response",
                "Caddy HTTP servers must be an object",
            )
        })?;
    for (name, server) in servers {
        let reference = format!("/config/apps/http/servers/{}", pointer_token(name));
        let listen = strings(server.get("listen"))?;
        let tls = server
            .get("tls_connection_policies")
            .and_then(Value::as_array)
            .map(|v| !v.is_empty());
        if let Some(routes) = server.get("routes") {
            walk_caddy(
                routes,
                &rules::Match::default(),
                true,
                &reference,
                (&listen, tls),
                inventory,
                0,
            )?;
        }
        for field in ["errors", "named_routes"] {
            if server.get(field).is_some_and(|v| !v.is_null()) {
                diagnostic(
                    inventory,
                    "discovery.unsupported_routes",
                    "Additional Caddy route collections require separate interpretation",
                    &format!("{reference}/{field}"),
                );
            }
        }
    }
    Ok(())
}

fn walk_caddy(
    routes: &Value,
    inherited: &rules::Match,
    inherited_coverage: bool,
    parent: &str,
    network: (&[String], Option<bool>),
    inventory: &mut Inventory,
    depth: usize,
) -> Result<()> {
    if depth > 32 {
        return Err(Diagnostic::new(
            "discovery.too_deep",
            "Caddy routes exceed 32 nesting levels",
        ));
    }
    for (index, route) in array(routes)?.iter().enumerate() {
        let reference = format!("{parent}/routes/{index}");
        if !route.is_object() {
            return Err(Diagnostic::new(
                "discovery.invalid_response",
                "Expected a Caddy route object",
            ));
        }
        let mut matched = inherited.clone();
        let mut covered = inherited_coverage;
        if let Some(sets) = route.get("match") {
            let sets = array(sets)?;
            if !sets.is_empty() {
                matched.any_host &= sets.iter().any(|s| s.get("host").is_none());
            }
            // A native route and its inherited matchers are kept, not collapsed into a URL.
            for set in sets {
                let map = set.as_object().ok_or_else(|| {
                    Diagnostic::new(
                        "discovery.invalid_response",
                        "Expected Caddy matcher object",
                    )
                })?;
                for (kind, value) in map {
                    match kind.as_str() {
                        "host" => {
                            let hosts = strings(Some(value))?;
                            if hosts.iter().any(|h| h.contains(['*', '{', '}'])) {
                                covered = false;
                            }
                            matched.hosts.extend(hosts);
                        }
                        "path" => matched.paths.extend(strings(Some(value))?),
                        _ => covered = false,
                    }
                }
            }
            // Multiple OR alternatives with unconstrained hosts need conservative conflict analysis.
            if sets.len() > 1 && sets.iter().any(|s| s.get("host").is_none()) {
                covered = false;
            }
        }
        if !covered {
            diagnostic(
                inventory,
                "discovery.unsupported_matcher",
                "Matcher semantics are partial; native reference and fingerprint retained",
                &reference,
            );
        }
        let handlers = route.get("handle").ok_or_else(|| {
            Diagnostic::new(
                "discovery.invalid_response",
                "Caddy route handlers are missing",
            )
        })?;
        for (handler_index, handler) in array(handlers)?.iter().enumerate() {
            let kind = required(handler, "handler")?;
            let href = format!("{reference}/handle/{handler_index}");
            if kind == "subroute" {
                let nested = handler.get("routes").ok_or_else(|| {
                    Diagnostic::new(
                        "discovery.invalid_response",
                        "Caddy subroute collection is missing",
                    )
                })?;
                walk_caddy(
                    nested,
                    &matched,
                    covered,
                    &href,
                    network,
                    inventory,
                    depth + 1,
                )?;
                continue;
            }
            let mut targets = Vec::new();
            let mut handler_covered =
                covered && matches!(kind, "reverse_proxy" | "file_server" | "static_response");
            if let Some(upstreams) = handler.get("upstreams") {
                for upstream in array(upstreams)? {
                    let dial = required(upstream, "dial")?;
                    if dial.contains(['{', '}']) {
                        handler_covered = false;
                    }
                    targets.push(if dial.contains('@') {
                        "[redacted upstream]".into()
                    } else {
                        dial.into()
                    });
                }
            }
            if handler.get("dynamic").is_some() {
                handler_covered = false;
            }
            if !handler_covered {
                diagnostic(
                    inventory,
                    "discovery.unsupported_handler",
                    "Unknown/dynamic handler retained; conflict coverage is partial",
                    &href,
                );
            }
            let native_id = handler
                .get("@id")
                .or_else(|| route.get("@id"))
                .and_then(Value::as_str)
                .map(|id| format!("{id}:{handler_index}"))
                .unwrap_or_else(|| href.clone());
            let native = retain_native(
                inventory,
                json!({"route":route,"handler":handler,"inherited_hosts":inherited.hosts,"inherited_paths":inherited.paths}),
            )?;
            inventory.routes.push(RouteObservation {
                binding: inventory.binding.clone(),
                native_id,
                reference: href,
                origin: "caddy_configuration".into(),
                source: inventory.source,
                observed_at: inventory.observed_at.clone(),
                runtime_confirmed: inventory.runtime_confirmed,
                native_status: "configured".into(),
                ownership: "observed_external",
                hosts: matched.hosts.clone(),
                any_host: matched.any_host,
                paths: matched.paths.clone(),
                targets,
                entrypoints: network.0.to_vec(),
                tls_configured: network.1,
                matcher_coverage: if handler_covered {
                    "literal_subset"
                } else {
                    "unresolved"
                },
                native_fingerprint: fingerprint(
                    &json!({"match":route.get("match"),"handler":handler}),
                ),
                native,
            });
            if inventory.routes.len() > MAX_RECORDS {
                return Err(Diagnostic::new(
                    "discovery.too_many_records",
                    "Caddy inventory exceeds 10,000 handler records",
                ));
            }
        }
    }
    Ok(())
}

fn pointer_token(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}
