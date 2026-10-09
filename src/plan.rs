use std::collections::BTreeMap;

use crate::{
    config::exact_host,
    error::{Diagnostic, Result},
    model::{Inventory, Manifest, Plan, PlannedRoute},
    transport::fingerprint,
};

pub fn build(manifest: &Manifest, inventories: &[Inventory]) -> Result<Plan> {
    let mut actions = Vec::new();
    let mut diagnostics = Vec::new();
    let mut sources = BTreeMap::new();
    for inventory in inventories {
        if inventory.scope != manifest.scope {
            return Err(Diagnostic::new(
                "scope.mismatch",
                "Manifest and binding scopes must match; cross-scope composition is not implicit",
            ));
        }
        sources.insert(
            format!("{}:binding", inventory.binding),
            inventory.binding_fingerprint.clone(),
        );
        for source in &inventory.sources {
            sources.insert(
                format!("{}:{}", inventory.binding, source.path),
                source.fingerprint.clone(),
            );
        }
        for observed in &inventory.routes {
            sources.insert(
                format!("{}:{}", inventory.binding, observed.reference),
                fingerprint(&observed.native),
            );
        }
    }
    for route in &manifest.routes {
        let inventory = inventories
            .iter()
            .find(|i| i.binding == route.proxy)
            .ok_or_else(|| {
                Diagnostic::new(
                    "binding.missing",
                    "Route references a binding absent from the explicit configuration",
                )
                .at(&route.id)
            })?;
        let mut conflicts = Vec::new();
        if !inventory.runtime_confirmed {
            conflicts.push(
                Diagnostic::new(
                    "discovery.offline",
                    "A saved snapshot cannot establish current conflict absence",
                )
                .at(&route.id),
            );
        }
        if !inventory.complete {
            conflicts.push(
                Diagnostic::new(
                    "discovery.partial",
                    "Unresolved native configuration prevents a complete conflict assessment",
                )
                .at(&route.id),
            );
        }
        for observed in &inventory.routes {
            // Disabled/error routes still reserve configured names; status is shown separately.
            let overlaps = observed.any_host
                || observed.hosts.is_empty()
                || observed
                    .hosts
                    .iter()
                    .any(|h| host_overlap(h, &route.hostname));
            if overlaps {
                conflicts.push(Diagnostic::new("route.external_overlap",format!("Existing native route {} has overlapping or unresolved host coverage; ownership is external",safe(&observed.native_id))).at(&route.id));
            }
        }
        for other in &manifest.routes {
            if other.id != route.id
                && other.proxy == route.proxy
                && other.hostname == route.hostname
            {
                conflicts.push(
                    Diagnostic::new(
                        "route.request_collision",
                        "Two requested route IDs use the same hostname on one binding",
                    )
                    .at(&route.id),
                );
            }
        }
        actions.push(PlannedRoute {
            resource: route.id.clone(),
            binding: route.proxy.clone(),
            hostname: route.hostname.clone(),
            upstream: route.upstream.url.clone(),
            tls: route.tls.mode.clone(),
            proposed_action: if route.enabled {
                "propose_owned_route"
            } else {
                "retain_disabled_definition"
            },
            conflicts,
        });
    }
    // A read channel is not proof of source authority. This is explicitly not an apply-ready plan.
    diagnostics.push(Diagnostic::new("capability.apply_unavailable","Journaled writes, ownership receipts, and compatibility gates are not implemented; this plan cannot be applied"));
    if !manifest.routes.is_empty() {
        diagnostics.push(Diagnostic::new("setup.authority_unverified","The selected runtime APIs do not prove an approved authoritative file/watcher integration"));
        diagnostics.push(Diagnostic::new("network.context_unverified","Upstream reachability has not been verified from the selected proxy runtime; no upstream probe was attempted"));
    }
    let normalized = serde_json::json!({"schema_version":manifest.schema_version,"scope":manifest.scope,"routes":manifest.routes});
    Ok(Plan {
        schema_version: "1",
        scope: manifest.scope,
        manifest_fingerprint: fingerprint(&normalized),
        actions,
        source_fingerprints: sources,
        applicable: false,
        diagnostics,
    })
}

fn host_overlap(pattern: &str, wanted: &str) -> bool {
    if let Ok(exact) = exact_host(pattern) {
        return exact == wanted;
    }
    if let Some(suffix) = pattern.strip_prefix("*.") {
        return wanted.ends_with(&format!(".{}", suffix.to_ascii_lowercase()));
    }
    true
}

pub fn safe(text: &str) -> String {
    text.chars()
        .take(512)
        .flat_map(|c| c.escape_default())
        .collect()
}
