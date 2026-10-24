use super::*;

impl Store {
    pub fn adopt(
        &mut self,
        proxy: &str,
        id: &str,
        native_id: &str,
        expected: &str,
        source: Option<&Path>,
    ) -> Result<Value> {
        self.ready()?;
        config::resource_id(id)?;
        if self.book.routes.len() >= 128
            || self
                .book
                .routes
                .iter()
                .any(|r| r.spec.proxy == proxy && r.spec.id == id)
        {
            return Err(error(
                "route.exists",
                "Route ID already owned or receipt limit reached",
            ));
        }
        let mut connection = self
            .book
            .connections
            .iter()
            .find(|c| c.binding.id == proxy)
            .cloned()
            .ok_or_else(|| error("binding.missing", "Proxy not registered"))?;
        authority(&connection)?;
        let guard = input_lock(&connection)?;
        let observed = inventory::inspect(&connection.binding, Scope::User)?;
        let route = observed
            .routes
            .iter()
            .find(|r| r.native_id == native_id)
            .ok_or_else(|| error("resource.not_found", "Native route not found"))?;
        if route.native_fingerprint != expected {
            return Err(error(
                "adoption.stale",
                "Native fingerprint changed; inspect the route again",
            ));
        }
        if route.matcher_coverage != "literal_subset"
            || route.hosts.len() != 1
            || route.targets.len() != 1
            || route.any_host
            || !route.paths.is_empty()
            || route.tls_configured == Some(true)
        {
            return Err(error(
                "adoption.unsupported",
                "Adoption requires one exact HTTP hostname, one upstream, and no path/rewrite or TLS customization",
            ));
        }
        if self
            .book
            .routes
            .iter()
            .any(|r| match connection.binding.engine {
                Engine::Caddy => format!("{}:0", r.native_id) == native_id,
                Engine::Traefik => format!("{}@file", r.native_id) == native_id,
            })
        {
            return Err(error("adoption.owned", "Native ID is already owned"));
        }
        let mut spec = RouteSpec {
            id: id.into(),
            proxy: proxy.into(),
            hostname: route.hosts[0].clone(),
            upstream: crate::model::Upstream {
                url: match connection.binding.engine {
                    Engine::Caddy => format!("http://{}", route.targets[0]),
                    Engine::Traefik => route.targets[0].clone(),
                },
            },
            enabled: true,
            tls: crate::model::TlsPolicy {
                mode: TlsMode::Http,
            },
        };
        config::validate_route(&mut spec)?;
        let (source, before, after, receipt) = match &mut connection.authority {
            Authority::CaddyFile { .. } => {
                return Err(error(
                    "adoption.source",
                    "Caddyfile routes must be explicitly migrated into the managed section; JSON and dedicated Traefik file adoption are supported",
                ));
            }
            Authority::CaddyJson { file, server, .. } => {
                let before =
                    text(file)?.ok_or_else(|| error("authority.file", "Startup input missing"))?;
                let mut root = config::strict_json(before.as_bytes())?;
                let routes = root
                    .pointer_mut(&format!("/apps/http/servers/{server}/routes"))
                    .and_then(Value::as_array_mut)
                    .ok_or_else(|| {
                        error(
                            "adoption.source",
                            "Selected startup server has no direct routes",
                        )
                    })?;
                let index = routes
                    .iter()
                    .position(|r| r == &route.native["route"])
                    .ok_or_else(|| {
                        error(
                            "adoption.source",
                            "Native route is not a direct definition in the authorized server",
                        )
                    })?;
                let receipt = Receipt {
                    spec,
                    native_id: format!("steward-{}-{proxy}-{id}", self.book.owner),
                };
                let generated = caddy_route(&receipt)?;
                let mut existing = routes[index].clone();
                existing["@id"] = json!(receipt.native_id);
                if existing != generated {
                    return Err(error(
                        "adoption.unsupported",
                        "External route has custom behavior; only an exactly representable route may be adopted",
                    ));
                }
                routes[index] = generated;
                (file.clone(), Some(before), Some(encoded(&root)?), receipt)
            }
            Authority::TraefikDirectory {
                directory,
                entrypoint,
                managed_file,
                ..
            } => {
                if !own(&self.book, proxy).is_empty() {
                    return Err(error(
                        "adoption.scope",
                        "Adopt a dedicated native file into a new connection with no owned routes",
                    ));
                }
                let source = source.ok_or_else(|| error("adoption.source","Pass the exact dedicated --source file containing this file-provider route"))?;
                if source.parent() != Some(directory.as_path()) || !native_id.ends_with("@file") {
                    return Err(error(
                        "adoption.source",
                        "Only dedicated files in the authorized watched directory may be adopted",
                    ));
                }
                let receipt = Receipt {
                    spec,
                    native_id: native_id.trim_end_matches("@file").into(),
                };
                config::resource_id(&receipt.native_id)?;
                let before = text(source)?
                    .ok_or_else(|| error("adoption.source", "Selected file is missing"))?;
                let native_file: Value = config::load(source, config::MAX_CONFIG)?;
                if native_file != traefik_document(&[&receipt], entrypoint) {
                    return Err(error(
                        "adoption.unsupported",
                        "Dedicated file must contain exactly this one representable router and service",
                    ));
                }
                *managed_file = Some(source.into());
                (source.into(), Some(before.clone()), Some(before), receipt)
            }
            Authority::ReadOnly => {
                return Err(error(
                    "adoption.authority",
                    "Register writable native input authority before adoption",
                ));
            }
        };
        let mut desired = self.book.clone();
        *desired
            .connections
            .iter_mut()
            .find(|c| c.binding.id == proxy)
            .expect("registered") = connection.clone();
        desired.routes.push(receipt);
        let operation = fingerprint(&(
            expected,
            transport::now(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
        ));
        desired.revision = operation[..16].into();
        let journal = Journal {
            native_before: None,
            native_after: None,
            schema_version: "1".into(),
            id: operation[..16].into(),
            phase: "prepared".into(),
            connection,
            source,
            before,
            after,
            old: self.book.clone(),
            desired,
            reverses: None,
        };
        if let Authority::CaddyJson { binary, .. } = &journal.connection.authority {
            validate_caddy(
                binary,
                &journal.source,
                journal.after.as_deref().expect("candidate"),
            )?;
        }
        drop(guard);
        self.commit_edit(journal, id)
    }
}
