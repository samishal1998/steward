use crate::{
    config,
    error::{Diagnostic, Result},
    managed::{self, Edit, Store},
    model::{RouteSpec, Scope},
    services::{self, ServiceArgs, ServiceCommand, Spec},
    transport::fingerprint,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Stack {
    schema_version: String,
    id: String,
    scope: Scope,
    #[serde(default)]
    services: Vec<Spec>,
    #[serde(default)]
    routes: Vec<Route>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Route {
    spec: RouteSpec,
    #[serde(default)]
    depends_on: Vec<String>,
}
#[derive(Deserialize, Serialize)]
struct Receipt {
    schema_version: String,
    stack: Stack,
    created_services: Vec<String>,
    published_routes: Vec<(String, String)>,
    phase: String,
    #[serde(default)]
    pending_service: Option<(String, String, bool)>,
    #[serde(default)]
    pending_route: Option<(String, String, String)>,
}

fn error(code: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message)
}
fn save(path: &Path, receipt: &Receipt) -> Result<()> {
    let text = serde_json::to_string_pretty(receipt)
        .map_err(|_| error("composition.state", "Could not encode stack receipt"))?;
    if text.len() > config::MAX_CONFIG {
        return Err(error("composition.limit", "Stack receipt exceeds 1 MiB"));
    }
    managed::publish(path, Some(&text))
}

pub fn service_args(spec: &Spec, start: bool) -> ServiceArgs {
    ServiceArgs {
        id: spec.id.clone(),
        scope: spec.scope,
        executable: spec.executable.clone(),
        args: spec.args.clone(),
        working_directory: spec.working_directory.clone(),
        environment: spec
            .environment
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect(),
        enabled: spec.enabled,
        restart: spec.restart,
        ready_url: spec.ready_url.clone(),
        start,
    }
}

pub fn up(root: PathBuf, path: &Path) -> Result<Value> {
    let mut stack: Stack = config::load(path, config::MAX_CONFIG)?;
    config::version(&stack.schema_version)?;
    config::resource_id(&stack.id)?;
    if stack.scope != Scope::User || stack.services.len() > 32 || stack.routes.len() > 128 {
        return Err(error(
            "composition.scope",
            "Composition supports user scope, up to 32 services and 128 routes",
        ));
    }
    let mut service_ids = BTreeSet::new();
    for spec in &stack.services {
        services::validate(spec)?;
        if spec.scope != Scope::User || !service_ids.insert(spec.id.clone()) {
            return Err(error(
                "composition.service",
                "Services must have unique user-scope IDs",
            ));
        }
    }
    let mut route_ids = BTreeSet::new();
    for route in &mut stack.routes {
        config::validate_route(&mut route.spec)?;
        let spec = &route.spec;
        if !route_ids.insert((spec.proxy.clone(), spec.id.clone())) {
            return Err(error("composition.route", "Duplicate route ID"));
        }
        for dependency in &route.depends_on {
            if !stack
                .services
                .iter()
                .any(|s| &s.id == dependency && s.ready_url.is_some())
            {
                return Err(error(
                    "composition.readiness",
                    "Every route dependency must name a supplied service with an explicit readiness URL",
                ));
            }
        }
    }
    let directory = root.join("stacks");
    let state = directory.join(format!("{}.json", stack.id));
    drop(Store::open(root.clone())?);
    let _guard = managed::lock_file(&root.join(format!("stack-{}.lock", stack.id)))?;
    let mut retained = Vec::new();
    {
        let _lock = Store::open(root.clone())?;
        if managed::regular(&state)?.is_some() {
            let previous: Receipt = config::load(&state, config::MAX_CONFIG)?;
            if previous.phase != "down" || fingerprint(&previous.stack) != fingerprint(&stack) {
                return Err(error(
                    "composition.exists",
                    "Run down first; changing a retained stack specification requires a new stack ID",
                ));
            }
            retained = previous.created_services;
        }
        fs::create_dir_all(&directory)
            .map_err(|_| error("composition.io", "Could not create stack receipt directory"))?;
    }
    let existing = services::saved_specs(&root)?;
    for spec in &stack.services {
        if let Some(old) = existing.iter().find(|s| s.id == spec.id)
            && fingerprint(old) != fingerprint(spec)
        {
            return Err(error(
                "composition.service_drift",
                "Existing service differs from the stack; update it explicitly first",
            ));
        }
    }
    let mut receipt = Receipt {
        schema_version: "1".into(),
        stack,
        created_services: retained,
        published_routes: Vec::new(),
        phase: "starting".into(),
        pending_service: None,
        pending_route: None,
    };
    save(&state, &receipt)?;
    let execute = (|| {
        for spec in receipt.stack.services.clone() {
            let created = !existing.iter().any(|s| s.id == spec.id);
            let command = if created {
                ServiceCommand::Add(service_args(&spec, true))
            } else {
                ServiceCommand::Start {
                    id: spec.id.clone(),
                }
            };
            services::run_tracked(root.clone(), &command, &mut |operation| {
                if !created
                    && !services::saved_specs(&root)?
                        .iter()
                        .any(|s| s.id == spec.id && fingerprint(s) == fingerprint(&spec))
                {
                    return Err(error(
                        "composition.service_drift",
                        "Service changed during composition preparation",
                    ));
                }
                receipt.pending_service = Some((spec.id.clone(), operation.into(), created));
                save(&state, &receipt)
            })?;
            if created {
                receipt.created_services.push(spec.id.clone());
            }
            receipt.pending_service = None;
            save(&state, &receipt)?;
        }
        receipt.phase = "publishing_routes".into();
        save(&state, &receipt)?;
        for route in receipt.stack.routes.clone() {
            let mut store = Store::open(root.clone())?;
            if store
                .book
                .routes
                .iter()
                .any(|r| r.spec.id == route.spec.id && r.spec.proxy == route.spec.proxy)
            {
                return Err(error(
                    "composition.route_exists",
                    "Stack publication requires a new route ID; existing routes are not taken over",
                ));
            }
            store.edit_tracked(
                &route.spec.proxy,
                Edit::Add(route.spec.clone()),
                |operation| {
                    receipt.pending_route = Some((
                        route.spec.proxy.clone(),
                        route.spec.id.clone(),
                        operation.into(),
                    ));
                    save(&state, &receipt)
                },
            )?;
            receipt
                .published_routes
                .push((route.spec.proxy.clone(), route.spec.id.clone()));
            receipt.pending_route = None;
            save(&state, &receipt)?;
        }
        receipt.phase = "verified".into();
        save(&state, &receipt)
    })();
    if let Err(e) = execute {
        receipt.phase = "partial".into();
        let _ = save(&state, &receipt);
        return Err(Diagnostic::new("composition.partial",format!("{}; stack {} records completed effects; use down after resolving unfinished native operations",e.code,receipt.stack.id)).with_changes());
    }
    Ok(
        json!({"stack":receipt.stack.id,"phase":receipt.phase,"created_services":receipt.created_services,"published_routes":receipt.published_routes}),
    )
}

pub fn down(root: PathBuf, id: &str, remove_services: bool) -> Result<Value> {
    config::resource_id(id)?;
    drop(Store::open(root.clone())?);
    let _guard = managed::lock_file(&root.join(format!("stack-{id}.lock")))?;
    let path = root.join("stacks").join(format!("{id}.json"));
    let mut receipt: Receipt = config::load(&path, config::MAX_CONFIG)?;
    config::version(&receipt.schema_version)?;
    if let Some((id, operation, created)) = receipt.pending_service.clone() {
        let summaries = services::operation_summaries(&root)?;
        let phase = summaries
            .iter()
            .find(|j| j["id"] == operation)
            .and_then(|j| j["phase"].as_str());
        match phase {
            Some("verified") => {
                if created && !receipt.created_services.contains(&id) {
                    receipt.created_services.push(id);
                }
            }
            Some("compensated") => {}
            None if !services::saved_specs(&root)?.iter().any(|s| s.id == id) => {}
            _ => {
                return Err(error(
                    "composition.pending",
                    "Recover the recorded service operation before down",
                ));
            }
        }
        receipt.pending_service = None;
        save(&path, &receipt)?;
    }
    if let Some((proxy, id, operation)) = receipt.pending_route.clone() {
        let store = Store::open(root.clone())?;
        let inspected = store.inspect_operation(&operation);
        match inspected.as_ref().ok().and_then(|v| v["phase"].as_str()) {
            Some("verified") => {
                if !receipt
                    .published_routes
                    .contains(&(proxy.clone(), id.clone()))
                {
                    receipt.published_routes.push((proxy, id));
                }
            }
            Some("compensated") => {}
            None if !store
                .book
                .routes
                .iter()
                .any(|r| r.spec.proxy == proxy && r.spec.id == id) => {}
            _ => {
                return Err(error(
                    "composition.pending",
                    "Recover the recorded route operation before down",
                ));
            }
        }
        receipt.pending_route = None;
        save(&path, &receipt)?;
    }
    while let Some((proxy, id)) = receipt.published_routes.last().cloned() {
        let mut store = Store::open(root.clone())?;
        if let Some(current) = store
            .book
            .routes
            .iter()
            .find(|r| r.spec.proxy == proxy && r.spec.id == id)
        {
            let expected = receipt
                .stack
                .routes
                .iter()
                .find(|r| r.spec.proxy == proxy && r.spec.id == id)
                .ok_or_else(|| error("composition.receipt", "Route receipt is inconsistent"))?;
            if fingerprint(&current.spec) != fingerprint(&expected.spec) {
                return Err(error(
                    "composition.drift",
                    "Route changed after stack publication; not removing newer intent",
                ));
            }
            store.edit(&proxy, Edit::Remove(id))?;
        }
        receipt.published_routes.pop();
        save(&path, &receipt)?;
    }
    if remove_services {
        while let Some(id) = receipt.created_services.last().cloned() {
            if let Some(current) = services::saved_specs(&root)?.iter().find(|s| s.id == id) {
                let expected = receipt
                    .stack
                    .services
                    .iter()
                    .find(|s| s.id == id)
                    .ok_or_else(|| {
                        error("composition.receipt", "Service receipt is inconsistent")
                    })?;
                if fingerprint(current) != fingerprint(expected) {
                    return Err(error(
                        "composition.drift",
                        "Service changed after stack creation; not removing newer intent",
                    ));
                }
                services::run(root.clone(), &ServiceCommand::Remove { id })?;
            }
            receipt.created_services.pop();
            save(&path, &receipt)?;
        }
    }
    receipt.phase = "down".into();
    save(&path, &receipt)?;
    Ok(json!({"stack":id,"phase":"down","services_retained":!remove_services}))
}
