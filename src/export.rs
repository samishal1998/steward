//! Versioned, redacted observations for the optional AppTrail file provider.
use crate::{
    error::{Diagnostic, Result},
    inventory,
    managed::{self, Store},
    model::Scope,
    services::{self, ServiceCommand},
    transport,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub fn inventory_export(root: PathBuf, output: Option<&Path>) -> Result<Value> {
    let book = { Store::open(root.clone())?.book.clone() };
    let mut resources = Vec::new();
    let mut sources = Vec::new();
    let mut complete = true;
    for connection in &book.connections {
        let observed = inventory::inspect(&connection.binding, Scope::User);
        let source_complete = observed
            .as_ref()
            .is_ok_and(|i| i.complete && i.runtime_confirmed);
        complete &= source_complete;
        sources.push(json!({"id":connection.binding.id,"kind":"proxy","complete":source_complete}));
        for receipt in book
            .routes
            .iter()
            .filter(|r| r.spec.proxy == connection.binding.id)
        {
            let mut observed_owned = false;
            if let Ok(value) = &observed {
                observed_owned = value
                    .routes
                    .iter()
                    .any(|r| r.hosts == [receipt.spec.hostname.clone()] && r.runtime_confirmed);
            }
            let url = if receipt.spec.enabled {
                managed::public_url(connection, &receipt.spec).ok()
            } else {
                None
            };
            resources.push(json!({"kind":"route","id":receipt.spec.id,"binding":receipt.spec.proxy,"scope":"user","name":receipt.spec.id,"enabled":receipt.spec.enabled,"ownership":"owned_definition","observed":if observed_owned{"present"}else if source_complete{"absent"}else{"unknown"},"public_url":url}));
        }
    }
    for spec in services::saved_specs(&root)? {
        let observed = services::run(
            root.clone(),
            &ServiceCommand::Inspect {
                id: spec.id.clone(),
            },
        );
        let available = observed.is_ok();
        complete &= available;
        sources.push(json!({"id":spec.id,"kind":"service","complete":available}));
        resources.push(json!({"kind":"service","id":spec.id,"binding":"","scope":spec.scope,"name":spec.id,"enabled":spec.enabled,"ownership":"owned_definition","observed":observed.ok().map_or("unknown",|(v,_)|if v["running"]==true{"running"}else{"stopped"}),"public_url":null}));
    }
    let value = json!({"schema_version":"1","kind":"steward.inventory","host_id":book.owner,"revision":transport::fingerprint(&book),"observed_at":transport::now().parse::<u64>().unwrap_or_default(),"complete":complete,"sources":sources,"resources":resources});
    if let Some(path) = output {
        if !path.is_absolute() {
            return Err(Diagnostic::new(
                "export.path",
                "Export path must be absolute",
            ));
        }
        if path.starts_with(&root) {
            return Err(Diagnostic::new(
                "export.path",
                "Export outside the private state directory; share only the redacted export",
            ));
        }
        managed::publish(
            path,
            Some(&serde_json::to_string_pretty(&value).expect("inventory JSON")),
        )?;
    }
    Ok(value)
}
