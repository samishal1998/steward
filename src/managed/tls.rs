use super::*;

pub(super) fn validate_tls(connection: &Connection, spec: &RouteSpec) -> Result<()> {
    let encrypted = !matches!(spec.tls.mode, TlsMode::Http);
    match &connection.authority {
        Authority::CaddyJson { file, server, .. } => {
            let root: Value = config::load(file, config::MAX_CONFIG)?;
            let server = &root["apps"]["http"]["servers"][server];
            let native_tls = server["tls_connection_policies"]
                .as_array()
                .is_some_and(|p| !p.is_empty());
            if encrypted != native_tls
                || (encrypted
                    && server.pointer("/automatic_https/disable") == Some(&Value::Bool(true)))
                || (!encrypted
                    && server.pointer("/automatic_https/disable") != Some(&Value::Bool(true)))
            {
                return Err(error(
                    "tls.listener",
                    "Route TLS mode must match the explicitly configured server's TLS listener and automatic HTTPS policy",
                ));
            }
        }
        Authority::TraefikDirectory {
            startup,
            entrypoint,
            ..
        } => {
            let root: Value = config::load(startup, config::MAX_CONFIG)?;
            if !encrypted
                && root
                    .pointer(&format!("/entryPoints/{entrypoint}/http/tls"))
                    .is_some()
            {
                return Err(error(
                    "tls.listener",
                    "HTTP route cannot use an entrypoint with default TLS",
                ));
            }
            if encrypted {
                let tls = connection.tls.as_ref().ok_or_else(|| {
                    error(
                        "tls.configuration",
                        "Register TLS verification/issuance settings on this connection",
                    )
                })?;
                match spec.tls.mode {
                    TlsMode::Public => {
                        let resolver = tls.cert_resolver.as_deref().ok_or_else(|| {
                            error(
                                "tls.resolver",
                                "Pass --cert-resolver when registering this connection",
                            )
                        })?;
                        config::resource_id(resolver)?;
                        if root["certificatesResolvers"].get(resolver).is_none() {
                            return Err(error(
                                "tls.resolver",
                                "Resolver is absent from the authoritative startup configuration",
                            ));
                        }
                    }
                    TlsMode::Internal => {
                        for path in [&tls.certificate, &tls.private_key] {
                            let path = path.as_ref().ok_or_else(|| error("tls.certificate","Register --certificate and --private-key for internal Traefik TLS"))?;
                            absolute(path)?;
                            regular(path)?.ok_or_else(|| {
                                error("tls.certificate", "Certificate/key input is missing")
                            })?;
                        }
                    }
                    TlsMode::Http => {}
                }
            }
        }
        Authority::CaddyFile { .. } => {}
        Authority::ReadOnly => {
            return Err(error(
                "capability.read_only",
                "Read-only connection cannot issue certificates",
            ));
        }
    }
    if matches!(spec.tls.mode, TlsMode::Internal) {
        let ca = connection.tls.as_ref().and_then(|t| t.ca_file.as_ref()).ok_or_else(|| error("tls.trust","Register an absolute --ca-file for verified internal TLS; system trust is never changed"))?;
        absolute(ca)?;
        reqwest::Certificate::from_pem(&config::read_bounded(ca, 128 * 1024)?)
            .map_err(|_| error("tls.trust", "CA file must contain a PEM certificate"))?;
    }
    Ok(())
}

fn policy(receipt: &Receipt) -> Value {
    json!({"subjects":[receipt.spec.hostname],"issuers":[{"module":if matches!(receipt.spec.tls.mode,TlsMode::Internal) { "internal" } else { "acme" }}]})
}

pub(super) fn tls_candidate(
    connection: &Connection,
    root: &mut Value,
    old: &[&Receipt],
    wanted: &[&Receipt],
) -> Result<()> {
    for r in wanted {
        validate_tls(connection, &r.spec)?;
    }
    let old_tls: Vec<_> = old
        .iter()
        .filter(|r| r.spec.enabled && !matches!(r.spec.tls.mode, TlsMode::Http))
        .collect();
    let new_tls: Vec<_> = wanted
        .iter()
        .filter(|r| r.spec.enabled && !matches!(r.spec.tls.mode, TlsMode::Http))
        .collect();
    if old_tls.is_empty() && new_tls.is_empty() {
        return Ok(());
    }
    let mut policies = root
        .pointer("/apps/tls/automation/policies")
        .cloned()
        .unwrap_or_else(|| json!([]));
    let entries = policies
        .as_array_mut()
        .ok_or_else(|| error("tls.policy", "Native automation policies must be an array"))?;
    for receipt in &old_tls {
        let expected = policy(receipt);
        let count = entries.iter().filter(|p| *p == &expected).count();
        if count != 1 {
            return Err(error(
                "ownership.drift",
                "Owned TLS automation policy no longer matches its receipt",
            ));
        }
        entries.retain(|p| p != &expected);
    }
    for receipt in new_tls {
        if entries.iter().any(|p| {
            p["subjects"].as_array().is_some_and(|names| {
                names.iter().any(|n| {
                    n.as_str()
                        .is_some_and(|n| n == receipt.spec.hostname || n.contains('*'))
                })
            })
        }) {
            return Err(error(
                "tls.external_policy",
                "An external certificate policy overlaps the requested host",
            ));
        }
        entries.insert(0, policy(receipt));
    }
    if root["apps"].get("tls").is_none() {
        root["apps"]["tls"] = json!({});
    }
    if root["apps"]["tls"].get("automation").is_none() {
        root["apps"]["tls"]["automation"] = json!({});
    }
    if !root["apps"]["tls"].is_object() || !root["apps"]["tls"]["automation"].is_object() {
        return Err(error(
            "tls.policy",
            "Invalid native TLS automation structure",
        ));
    }
    root["apps"]["tls"]["automation"]["policies"] = policies;
    Ok(())
}

pub(super) fn traefik_tls_document(
    connection: &Connection,
    receipts: &[&Receipt],
    entrypoint: &str,
) -> Result<Value> {
    let mut document = traefik_document(receipts, entrypoint);
    for r in receipts.iter().filter(|r| r.spec.enabled) {
        validate_tls(connection, &r.spec)?;
        match r.spec.tls.mode {
            TlsMode::Public => {
                document["http"]["routers"][&r.native_id]["tls"] = json!({"certResolver":connection.tls.as_ref().and_then(|t| t.cert_resolver.clone())})
            }
            TlsMode::Internal => {
                let tls = connection.tls.as_ref().expect("validated TLS");
                document["http"]["routers"][&r.native_id]["tls"] = json!({});
                document["tls"] = json!({"certificates":[{"certFile":tls.certificate,"keyFile":tls.private_key}]});
            }
            TlsMode::Http => {}
        }
    }
    Ok(document)
}
