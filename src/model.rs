use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::Diagnostic;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Engine {
    Caddy,
    Traefik,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    User,
    System,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema_version: String,
    pub scope: Scope,
    pub proxies: Vec<Binding>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub id: String,
    pub engine: Engine,
    pub endpoint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_file: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: String,
    pub scope: Scope,
    #[serde(default)]
    pub routes: Vec<RouteSpec>,
    #[serde(default)]
    pub services: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RouteSpec {
    pub id: String,
    pub proxy: String,
    pub hostname: String,
    pub upstream: Upstream,
    pub enabled: bool,
    pub tls: TlsPolicy,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Upstream {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TlsPolicy {
    pub mode: TlsMode,
}

#[derive(Debug, Clone, Deserialize, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum TlsMode {
    Public,
    Internal,
    Http,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteObservation {
    pub binding: String,
    pub native_id: String,
    pub reference: String,
    pub origin: String,
    pub source: &'static str,
    pub observed_at: String,
    pub runtime_confirmed: bool,
    pub native_status: String,
    pub ownership: &'static str,
    pub hosts: Vec<String>,
    pub any_host: bool,
    pub paths: Vec<String>,
    pub targets: Vec<String>,
    pub entrypoints: Vec<String>,
    pub tls_configured: Option<bool>,
    pub matcher_coverage: &'static str,
    pub native_fingerprint: String,
    // Raw evidence remains available to the planner without leaking native secrets into output.
    #[serde(skip_serializing)]
    pub native: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSnapshot {
    pub schema_version: String,
    pub binding: String,
    pub engine: Engine,
    pub captured_at: String,
    pub responses: BTreeMap<String, Value>,
}

#[derive(Debug, Serialize)]
pub struct Source {
    pub path: String,
    pub fingerprint: String,
    pub etag: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Inventory {
    pub schema_version: &'static str,
    pub binding: String,
    pub engine: Engine,
    pub scope: Scope,
    pub binding_fingerprint: String,
    pub source: &'static str,
    pub observed_at: String,
    pub runtime_confirmed: bool,
    pub atomic_snapshot: bool,
    pub controller: &'static str,
    pub runtime: &'static str,
    pub probe_network_context: &'static str,
    pub write_authorized: bool,
    pub complete: bool,
    pub sources: Vec<Source>,
    pub routes: Vec<RouteObservation>,
    pub diagnostics: Vec<Diagnostic>,
    #[serde(skip_serializing)]
    pub retained_bytes: usize,
}

#[derive(Debug, Serialize)]
pub struct Plan {
    pub schema_version: &'static str,
    pub scope: Scope,
    pub manifest_fingerprint: String,
    pub actions: Vec<PlannedRoute>,
    pub source_fingerprints: BTreeMap<String, String>,
    pub applicable: bool,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Serialize)]
pub struct PlannedRoute {
    pub resource: String,
    pub binding: String,
    pub hostname: String,
    pub upstream: String,
    pub tls: TlsMode,
    pub proposed_action: &'static str,
    pub conflicts: Vec<Diagnostic>,
}
