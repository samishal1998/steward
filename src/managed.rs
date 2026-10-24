//! Explicit local file authority, receipts, durable history and recoverable operations.
mod adoption;
mod caddyfile;
mod history;
mod manifest;
mod tls;
use crate::{
    config,
    error::{Diagnostic, Result},
    inventory,
    model::{Binding, Config, Engine, RouteSpec, Scope, TlsMode},
    transport::{self, fingerprint},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tls::{tls_candidate, traefik_tls_document, validate_tls};

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    pub binding: Binding,
    pub authority: Authority,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<ProxyTls>,
}

#[derive(Clone, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ProxyTls {
    pub ca_file: Option<PathBuf>,
    pub cert_resolver: Option<String>,
    pub certificate: Option<PathBuf>,
    pub private_key: Option<PathBuf>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Authority {
    ReadOnly,
    CaddyFile {
        file: PathBuf,
        binary: PathBuf,
        listen: String,
        #[serde(default)]
        controller: bool,
    },
    CaddyJson {
        file: PathBuf,
        server: String,
        binary: PathBuf,
    },
    TraefikDirectory {
        startup: PathBuf,
        directory: PathBuf,
        entrypoint: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        managed_file: Option<PathBuf>,
    },
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub spec: RouteSpec,
    pub native_id: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Book {
    pub schema_version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub revision: String,
    pub owner: String,
    pub connections: Vec<Connection>,
    pub routes: Vec<Receipt>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    schema_version: String,
    id: String,
    phase: String,
    connection: Connection,
    source: PathBuf,
    before: Option<String>,
    after: Option<String>,
    old: Book,
    desired: Book,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reverses: Option<String>,
    #[serde(default)]
    native_before: Option<Value>,
    #[serde(default)]
    native_after: Option<Value>,
}

pub struct Store {
    pub root: PathBuf,
    pub book: Book,
    _lock: File,
}

fn error(code: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message)
}
fn io_error(_: std::io::Error) -> Diagnostic {
    error(
        "state.io",
        "Could not access or durably publish Steward's state/input",
    )
}

pub fn executable(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        return fs::canonicalize(path).map_err(io_error);
    }
    if path.components().count() != 1 {
        return Err(error(
            "input.executable",
            "Use an absolute executable path or a PATH command name",
        ));
    }
    for directory in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        let candidate = directory.join(path);
        if candidate.is_file() {
            return fs::canonicalize(candidate).map_err(io_error);
        }
    }
    Err(error(
        "input.executable",
        "Caddy validator was not found; pass --caddy-bin /absolute/path",
    ))
}

pub(crate) fn lock_file(path: &Path) -> Result<File> {
    regular(path)?;
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path).map_err(io_error)?;
    file.try_lock().map_err(|_| {
        error(
            "operation.locked",
            "Another writer holds this state/input scope",
        )
    })?;
    Ok(file)
}

fn input_lock(connection: &Connection) -> Result<File> {
    let path = match &connection.authority {
        Authority::ReadOnly => {
            return Err(error(
                "capability.read_only",
                "This connection is inventory-only; register explicit file authority for writes",
            ));
        }
        Authority::CaddyJson { file, .. } | Authority::CaddyFile { file, .. } => {
            let parent = file
                .parent()
                .ok_or_else(|| error("authority.path", "Missing startup parent"))?;
            let canonical = fs::canonicalize(parent).map_err(io_error)?.join(
                file.file_name()
                    .ok_or_else(|| error("authority.path", "Missing startup filename"))?,
            );
            parent.join(format!(".steward-{}.lock", &fingerprint(&canonical)[..16]))
        }
        Authority::TraefikDirectory { directory, .. } => directory.join(".steward.lock"),
    };
    lock_file(&path)
}

pub fn default_root() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("STEWARD_STATE_DIR") {
        return absolute(Path::new(&path));
    }
    if let Some(path) = std::env::var_os("XDG_STATE_HOME") {
        return absolute(&PathBuf::from(path).join("steward"));
    }
    let home =
        std::env::var_os("HOME").ok_or_else(|| error("state.home", "Set HOME or --state-dir"))?;
    absolute(&PathBuf::from(home).join(".local/state/steward"))
}

fn absolute(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(error(
            "input.path",
            "Use an absolute path without parent traversal",
        ));
    }
    Ok(path.to_path_buf())
}

pub(crate) fn regular(path: &Path) -> Result<Option<fs::Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if meta.nlink() != 1 {
                    return Err(error(
                        "input.links",
                        "Managed inputs must not have hard links",
                    ));
                }
            }
            if !meta.is_file() || meta.file_type().is_symlink() {
                return Err(error(
                    "input.type",
                    "Managed inputs must be regular files, not symlinks",
                ));
            }
            Ok(Some(meta))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io_error(e)),
    }
}

fn text(path: &Path) -> Result<Option<String>> {
    if regular(path)?.is_none() {
        return Ok(None);
    }
    let bytes = config::read_bounded(path, config::MAX_CONFIG)?;
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| error("input.encoding", "Managed configuration must be UTF-8"))
}

pub(crate) fn publish(path: &Path, value: Option<&str>) -> Result<()> {
    let existing = regular(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| error("input.path", "Input must have a parent directory"))?;
    if let Some(value) = value {
        let mut stage = tempfile::NamedTempFile::new_in(parent).map_err(io_error)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            stage
                .as_file()
                .set_permissions(
                    existing.map_or_else(|| fs::Permissions::from_mode(0o600), |m| m.permissions()),
                )
                .map_err(io_error)?;
        }
        stage.write_all(value.as_bytes()).map_err(io_error)?;
        stage.as_file().sync_all().map_err(io_error)?;
        stage.persist(path).map_err(|e| io_error(e.error))?;
    } else if existing.is_some() {
        fs::remove_file(path).map_err(io_error)?;
    }
    File::open(parent)
        .and_then(|f| f.sync_all())
        .map_err(io_error)
}

fn encoded(value: &impl Serialize) -> Result<String> {
    serde_json::to_string_pretty(value)
        .map_err(|_| error("state.serialize", "Could not encode managed state"))
}

impl Store {
    pub fn open(root: PathBuf) -> Result<Self> {
        absolute(&root)?;
        if let Ok(meta) = fs::symlink_metadata(&root) {
            if !meta.is_dir() || meta.file_type().is_symlink() {
                return Err(error("state.type", "State directory must not be a symlink"));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::{MetadataExt, PermissionsExt};
                if meta.permissions().mode() & 0o077 != 0
                    || meta.uid() != unsafe { libc::geteuid() }
                {
                    return Err(error(
                        "state.permissions",
                        "Existing state directory must be private (chmod 700) and owned by the current user",
                    ));
                }
            }
        } else {
            fs::create_dir_all(&root).map_err(io_error)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).map_err(io_error)?;
            }
        }
        let lock = lock_file(&root.join("lock"))?;
        let state = root.join("state.json");
        let book = if regular(&state)?.is_some() {
            let book: Book = config::load(&state, config::MAX_CONFIG)?;
            config::version(&book.schema_version)?;
            if book.connections.len() > 16
                || book.routes.len() > 128
                || book.owner.len() != 16
                || !book.owner.bytes().all(|c| c.is_ascii_hexdigit())
            {
                return Err(error(
                    "state.invalid",
                    "Stored state exceeds its schema bounds",
                ));
            }
            let mut bindings = std::collections::BTreeSet::new();
            for connection in &book.connections {
                config::resource_id(&connection.binding.id)?;
                config::validate_endpoint(&connection.binding.endpoint)?;
                if !bindings.insert(&connection.binding.id) {
                    return Err(error(
                        "state.invalid",
                        "Duplicate connection ID in stored state",
                    ));
                }
                if let Some(auth) = &connection.binding.auth_file {
                    absolute(Path::new(auth))?;
                }
            }
            let mut routes = std::collections::BTreeSet::new();
            for receipt in &book.routes {
                let mut spec = receipt.spec.clone();
                config::validate_route(&mut spec)?;
                if fingerprint(&spec) != fingerprint(&receipt.spec)
                    || !bindings.contains(&receipt.spec.proxy)
                    || !routes.insert((&receipt.spec.proxy, &receipt.spec.id))
                {
                    return Err(error(
                        "state.invalid",
                        "Stored route identity/binding/normalization is inconsistent",
                    ));
                }
            }
            book
        } else {
            let owner = fingerprint(&(
                root.to_string_lossy(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos(),
                std::process::id(),
            ));
            Book {
                schema_version: "1".into(),
                revision: String::new(),
                owner: owner[..16].into(),
                connections: Vec::new(),
                routes: Vec::new(),
            }
        };
        Ok(Self {
            root,
            book,
            _lock: lock,
        })
    }

    pub fn config(&self) -> Config {
        Config {
            schema_version: "1".into(),
            scope: Scope::User,
            proxies: self
                .book
                .connections
                .iter()
                .map(|c| c.binding.clone())
                .collect(),
        }
    }

    fn save(&self, book: &Book) -> Result<()> {
        let value = encoded(book)?;
        if value.len() > config::MAX_CONFIG {
            return Err(error("state.limit", "Managed state exceeds 1 MiB"));
        }
        publish(&self.root.join("state.json"), Some(&value))
    }

    fn journal(&self) -> Result<Option<Journal>> {
        let path = self.root.join("operation.json");
        if regular(&path)?.is_none() {
            return Ok(None);
        }
        let journal: Journal = config::load(&path, 8 * config::MAX_CONFIG)?;
        config::version(&journal.schema_version)?;
        Ok(Some(journal))
    }

    fn record(&self, journal: &Journal) -> Result<()> {
        if let Some(previous) = self.journal()?
            && previous.id != journal.id
        {
            let archived = self
                .root
                .join("operations")
                .join(format!("{}.json", previous.id));
            if regular(&archived)?.is_none() {
                self.prepare_history(&previous)?;
                publish(&archived, Some(&encoded(&previous)?))?;
            }
        }
        let value = encoded(journal)?;
        if value.len() > 8 * config::MAX_CONFIG {
            return Err(error("state.limit", "Operation journal exceeds 8 MiB"));
        }
        self.prepare_history(journal)?;
        // The active pointer is durable before the historical copy. Recovery can
        // repair a stale history entry from this pointer after interrupted writes.
        publish(&self.root.join("operation.json"), Some(&value))?;
        publish(
            &self
                .root
                .join("operations")
                .join(format!("{}.json", journal.id)),
            Some(&value),
        )
    }

    fn ready(&self) -> Result<()> {
        if let Some(journal) = self.journal()?
            && !matches!(journal.phase.as_str(), "verified" | "compensated")
        {
            return Err(error(
                "operation.pending",
                "Recover the unfinished operation before changing managed state",
            )
            .at(journal.id));
        }
        Ok(())
    }

    pub fn connect(&mut self, connection: Connection) -> Result<Value> {
        self.ready()?;
        config::resource_id(&connection.binding.id)?;
        config::validate_endpoint(&connection.binding.endpoint)?;
        if self.book.connections.len() >= 16
            || self
                .book
                .connections
                .iter()
                .any(|c| c.binding.id == connection.binding.id)
        {
            return Err(error(
                "binding.exists",
                "Proxy ID already exists, or the 16-binding limit was reached",
            ));
        }
        self.validate_connection(&connection)?;
        let mut book = self.book.clone();
        book.revision = new_revision();
        book.connections.push(connection);
        self.save(&book)?;
        self.book = book;
        Ok(json!({"proxy": self.book.connections.last(), "scope": "user"}))
    }

    fn validate_connection(&self, connection: &Connection) -> Result<()> {
        config::resource_id(&connection.binding.id)?;
        config::validate_endpoint(&connection.binding.endpoint)?;
        if let Some(auth) = &connection.binding.auth_file {
            absolute(Path::new(auth))?;
        }
        if !matches!(connection.authority, Authority::ReadOnly) {
            authority(connection)?;
        }
        let observations = transport::fetch(&connection.binding)?;
        if let Authority::CaddyFile {
            file,
            binary,
            controller,
            ..
        } = &connection.authority
        {
            let source = text(file)?.ok_or_else(|| error("authority.file", "Caddyfile missing"))?;
            let adapted = caddyfile::adapt(binary, file, &source)?;
            if !controller && adapted != observations.values["/config/"] {
                return Err(error(
                    "authority.mismatch",
                    "Caddyfile plus imports does not match loaded native configuration",
                ));
            }
        }
        if let Authority::CaddyJson { file, .. } = &connection.authority {
            let source = config::load::<Value>(file, config::MAX_CONFIG)?;
            if source != observations.values["/config/"] {
                return Err(error(
                    "authority.mismatch",
                    "The JSON startup file does not match Caddy's loaded configuration",
                ));
            }
            if observations.sources[0].etag.is_none() {
                return Err(error(
                    "authority.etag",
                    "Caddy must support conditional configuration updates",
                ));
            }
        }
        inventory::decode(&connection.binding, Scope::User, observations)?;
        Ok(())
    }

    pub fn update_connection(&mut self, connection: Connection) -> Result<Value> {
        self.ready()?;
        let index = self
            .book
            .connections
            .iter()
            .position(|c| c.binding.id == connection.binding.id)
            .ok_or_else(|| error("binding.missing", "Proxy is not registered"))?;
        if self
            .book
            .routes
            .iter()
            .any(|r| r.spec.proxy == connection.binding.id)
            && fingerprint(&self.book.connections[index]) != fingerprint(&connection)
        {
            return Err(error(
                "binding.in_use",
                "Remove or explicitly detach owned routes before changing their authority",
            ));
        }
        self.validate_connection(&connection)?;
        let mut next = self.book.clone();
        next.revision = new_revision();
        next.connections[index] = connection.clone();
        self.save(&next)?;
        self.book = next;
        Ok(json!({"proxy":connection}))
    }

    pub fn remove_connection(&mut self, id: &str, detach: bool) -> Result<Value> {
        self.ready()?;
        if !self.book.connections.iter().any(|c| c.binding.id == id) {
            return Err(error("binding.missing", "Proxy is not registered"));
        }
        let owned = self
            .book
            .routes
            .iter()
            .filter(|r| r.spec.proxy == id)
            .count();
        if owned > 0 && !detach {
            return Err(error(
                "binding.in_use",
                "Remove owned routes first, or use proxies detach to explicitly relinquish ownership without changing native inputs",
            ));
        }
        let mut next = self.book.clone();
        next.revision = new_revision();
        next.connections.retain(|c| c.binding.id != id);
        next.routes.retain(|r| r.spec.proxy != id);
        self.save(&next)?;
        self.book = next;
        Ok(json!({"proxy":id,"detached_routes":owned,"native_inputs_changed":false}))
    }
}

fn authority(connection: &Connection) -> Result<String> {
    config::validate_endpoint(&connection.binding.endpoint)?;
    if connection.binding.snapshot_file.is_some() {
        return Err(error(
            "authority.offline",
            "A saved snapshot cannot authorize live writes",
        ));
    }
    let endpoint = config::http_url(&connection.binding.endpoint)?;
    if !endpoint
        .host_str()
        .is_some_and(|h| matches!(h, "localhost" | "127.0.0.1" | "[::1]"))
    {
        return Err(error(
            "authority.local",
            "File-backed writes require a same-host loopback API endpoint",
        ));
    }
    match &connection.authority {
        Authority::CaddyFile {
            file,
            binary,
            listen,
            ..
        } => {
            absolute(file)?;
            absolute(binary)?;
            regular(file)?.ok_or_else(|| error("authority.file", "Caddyfile must exist"))?;
            if connection.binding.engine != Engine::Caddy {
                return Err(error(
                    "authority.engine",
                    "Caddyfile requires a Caddy connection",
                ));
            }
            listener(listen)
        }
        Authority::ReadOnly => Err(error(
            "capability.read_only",
            "This connection is inventory-only; register explicit file authority for writes",
        )),
        Authority::CaddyJson {
            file,
            server,
            binary,
        } => {
            if connection.binding.engine != Engine::Caddy {
                return Err(error(
                    "authority.engine",
                    "Caddy JSON authority requires a Caddy binding",
                ));
            }
            absolute(file)?;
            absolute(binary)?;
            config::resource_id(server)?;
            regular(file)?.ok_or_else(|| {
                error(
                    "authority.file",
                    "The complete Caddy JSON startup file must exist",
                )
            })?;
            let source: Value = config::load(file, config::MAX_CONFIG)?;
            let server = source
                .pointer(&format!("/apps/http/servers/{server}"))
                .ok_or_else(|| {
                    error(
                        "authority.server",
                        "Selected Caddy server is absent from the startup file",
                    )
                })?;
            listener(
                server
                    .get("listen")
                    .and_then(Value::as_array)
                    .filter(|a| a.len() == 1)
                    .and_then(|a| a[0].as_str())
                    .ok_or_else(|| {
                        error(
                            "authority.listener",
                            "Select a server with one explicit local HTTP listener",
                        )
                    })?,
            )
        }
        Authority::TraefikDirectory {
            startup,
            directory,
            entrypoint,
            managed_file,
        } => {
            if connection.binding.engine != Engine::Traefik {
                return Err(error(
                    "authority.engine",
                    "Watched-directory authority requires a Traefik binding",
                ));
            }
            absolute(startup)?;
            absolute(directory)?;
            if let Some(file) = managed_file {
                absolute(file)?;
                if file.parent() != Some(directory.as_path()) {
                    return Err(error(
                        "authority.file",
                        "Adopted file must be directly inside the watched directory",
                    ));
                }
                regular(file)?;
            }
            config::resource_id(entrypoint)?;
            regular(startup)?.ok_or_else(|| {
                error(
                    "authority.file",
                    "The Traefik static startup file must exist",
                )
            })?;
            let meta = fs::symlink_metadata(directory).map_err(io_error)?;
            if !meta.is_dir() || meta.file_type().is_symlink() {
                return Err(error(
                    "authority.directory",
                    "Watched directory must exist and must not be a symlink",
                ));
            }
            let source: Value = config::load(startup, config::MAX_CONFIG)?;
            if source
                .pointer("/providers/file/directory")
                .and_then(Value::as_str)
                != directory.to_str()
                || source.pointer("/providers/file/watch") == Some(&Value::Bool(false))
            {
                return Err(error(
                    "authority.provider",
                    "Startup file must enable the exact watched directory",
                ));
            }
            let entry = source
                .pointer(&format!("/entryPoints/{entrypoint}"))
                .ok_or_else(|| {
                    error(
                        "authority.listener",
                        "Selected entrypoint is absent from startup configuration",
                    )
                })?;
            listener(
                entry
                    .get("address")
                    .and_then(Value::as_str)
                    .ok_or_else(|| error("authority.listener", "Entrypoint needs an address"))?,
            )
        }
    }
}

fn listener(address: &str) -> Result<String> {
    let (host, port) = address.rsplit_once(':').ok_or_else(|| {
        error(
            "authority.listener",
            "Use a local TCP listener with an explicit port",
        )
    })?;
    let port: u16 = port
        .trim_end_matches("/tcp")
        .parse()
        .map_err(|_| error("authority.listener", "Invalid listener port"))?;
    if port == 0
        || !matches!(
            host,
            "" | "127.0.0.1" | "localhost" | "[::]" | "[::1]" | "0.0.0.0"
        )
    {
        return Err(error(
            "authority.listener",
            "Writable mode requires a reachable same-host listener",
        ));
    }
    Ok(format!(
        "http://{}:{port}",
        if matches!(host, "[::1]" | "[::]") {
            "[::1]"
        } else {
            "127.0.0.1"
        }
    ))
}

pub(crate) fn public_url(connection: &Connection, spec: &RouteSpec) -> Result<String> {
    let native = authority(connection)?;
    let port = config::http_url(&native)?
        .port()
        .ok_or_else(|| error("authority.port", "Missing listener port"))?;
    let scheme = if matches!(spec.tls.mode, TlsMode::Http) {
        "http"
    } else {
        "https"
    };
    let suffix = if (scheme == "http" && port == 80) || (scheme == "https" && port == 443) {
        String::new()
    } else {
        format!(":{port}")
    };
    Ok(format!("{scheme}://{}{suffix}", spec.hostname))
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "action", content = "data", rename_all = "snake_case")]
pub enum Edit {
    Add(RouteSpec),
    Upsert {
        routes: Vec<RouteSpec>,
    },
    Set {
        id: String,
        hostname: Option<String>,
        upstream: Option<String>,
        enabled: Option<bool>,
    },
    Remove(String),
}

fn own<'a>(book: &'a Book, proxy: &str) -> Vec<&'a Receipt> {
    book.routes
        .iter()
        .filter(|r| r.spec.proxy == proxy)
        .collect()
}

fn caddy_route(receipt: &Receipt) -> Result<Value> {
    let url = config::http_url(&receipt.spec.upstream.url)?;
    let host = url
        .host_str()
        .ok_or_else(|| error("route.upstream", "Missing upstream host"))?;
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.into()
    };
    let port = url
        .port_or_known_default()
        .ok_or_else(|| error("route.upstream", "Missing upstream port"))?;
    let mut handler =
        json!({"handler":"reverse_proxy","upstreams":[{"dial":format!("{host}:{port}")}]});
    if url.scheme() == "https" {
        handler["transport"] = json!({"protocol":"http","tls":{}});
    }
    Ok(
        json!({"@id":receipt.native_id,"match":[{"host":[receipt.spec.hostname]}],"handle":[handler],"terminal":true}),
    )
}

fn traefik_document(receipts: &[&Receipt], entrypoint: &str) -> Value {
    let mut routers = serde_json::Map::new();
    let mut services = serde_json::Map::new();
    for receipt in receipts.iter().filter(|r| r.spec.enabled) {
        routers.insert(receipt.native_id.clone(), json!({"rule":format!("Host(`{}`)",receipt.spec.hostname),"entryPoints":[entrypoint],"service":receipt.native_id}));
        services.insert(
            receipt.native_id.clone(),
            json!({"loadBalancer":{"servers":[{"url":receipt.spec.upstream.url}]}}),
        );
    }
    json!({"http":{"routers":routers,"services":services}})
}

fn candidate(
    connection: &Connection,
    old: &Book,
    desired: &Book,
    before: Option<&str>,
) -> Result<Option<String>> {
    let previous = own(old, &connection.binding.id);
    let wanted = own(desired, &connection.binding.id);
    match &connection.authority {
        Authority::CaddyFile { .. } => caddyfile::candidate(connection, old, desired, before),
        Authority::ReadOnly => Err(error(
            "capability.read_only",
            "Inventory-only connections cannot publish route inputs",
        )),
        Authority::CaddyJson { server, .. } => {
            let before =
                before.ok_or_else(|| error("authority.file", "Caddy startup file disappeared"))?;
            let mut root = config::strict_json(before.as_bytes())?;
            let server = root
                .pointer_mut(&format!("/apps/http/servers/{server}"))
                .and_then(Value::as_object_mut)
                .ok_or_else(|| error("authority.server", "Caddy server disappeared"))?;
            let routes = server
                .entry("routes")
                .or_insert_with(|| json!([]))
                .as_array_mut()
                .ok_or_else(|| error("authority.routes", "Caddy routes must be an array"))?;
            for receipt in &previous {
                let found: Vec<_> = routes
                    .iter()
                    .filter(|r| r.get("@id").and_then(Value::as_str) == Some(&receipt.native_id))
                    .collect();
                if receipt.spec.enabled {
                    if found.len() != 1 || found[0] != &caddy_route(receipt)? {
                        return Err(error(
                            "ownership.drift",
                            "Owned Caddy route no longer matches its receipt",
                        ));
                    }
                } else if !found.is_empty() {
                    return Err(error(
                        "ownership.drift",
                        "A disabled owned route appeared in native configuration",
                    ));
                }
            }
            for receipt in &wanted {
                if !previous.iter().any(|r| r.native_id == receipt.native_id)
                    && routes
                        .iter()
                        .any(|r| r.get("@id").and_then(Value::as_str) == Some(&receipt.native_id))
                {
                    return Err(error(
                        "ownership.external",
                        "An unowned native route already uses the requested ID",
                    ));
                }
            }
            routes.retain(|r| {
                !previous
                    .iter()
                    .any(|p| r.get("@id").and_then(Value::as_str) == Some(&p.native_id))
            });
            for receipt in wanted.iter().filter(|r| r.spec.enabled) {
                routes.push(caddy_route(receipt)?);
            }
            tls_candidate(connection, &mut root, &previous, &wanted)?;
            Ok(Some(encoded(&root)?))
        }
        Authority::TraefikDirectory { entrypoint, .. } => {
            let old_active = previous.iter().any(|r| r.spec.enabled);
            if old_active {
                let existing = before.ok_or_else(|| {
                    error("ownership.drift", "Owned Traefik aggregate disappeared")
                })?;
                if config::strict_json(existing.as_bytes())?
                    != traefik_tls_document(connection, &previous, entrypoint)?
                {
                    return Err(error(
                        "ownership.drift",
                        "Owned Traefik aggregate was edited outside Steward",
                    ));
                }
            } else if before.is_some() {
                return Err(error(
                    "ownership.external",
                    "Steward's aggregate path is already occupied by an unowned file",
                ));
            }
            if wanted.iter().any(|r| r.spec.enabled) {
                Ok(Some(encoded(&traefik_tls_document(
                    connection, &wanted, entrypoint,
                )?)?))
            } else {
                Ok(None)
            }
        }
    }
}

fn check_conflicts(connection: &Connection, book: &Book, wanted: &[&Receipt]) -> Result<()> {
    let observed = inventory::inspect(&connection.binding, Scope::User)?;
    let listener = match &connection.authority {
        Authority::CaddyFile { listen, .. } => listen.clone(),
        Authority::ReadOnly => {
            return Err(error(
                "capability.read_only",
                "Inventory-only connections cannot publish route inputs",
            ));
        }
        Authority::CaddyJson { file, .. } => {
            let root: Value = config::load(file, config::MAX_CONFIG)?;
            let server = match &connection.authority {
                Authority::CaddyJson { server, .. } => server,
                _ => unreachable!(),
            };
            root.pointer(&format!("/apps/http/servers/{server}/listen/0"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned()
        }
        Authority::TraefikDirectory { entrypoint, .. } => entrypoint.clone(),
    };
    if matches!(connection.authority, Authority::CaddyJson { .. }) && !observed.complete {
        return Err(error(
            "route.coverage",
            "Caddy inventory is partial; a live write cannot establish absence of conflicts",
        ));
    }
    for route in observed
        .routes
        .iter()
        .filter(|r| r.entrypoints.is_empty() || r.entrypoints.contains(&listener))
    {
        let ours = own(book, &connection.binding.id).iter().any(|r| {
            (matches!(connection.authority, Authority::CaddyFile { .. })
                && caddyfile::route_matches(connection, route, r))
                || route.native_id
                    == match connection.binding.engine {
                        Engine::Caddy => format!("{}:0", r.native_id),
                        Engine::Traefik => format!("{}@file", r.native_id),
                    }
        });
        if ours {
            continue;
        }
        for receipt in wanted.iter().filter(|r| r.spec.enabled) {
            if route.matcher_coverage == "unresolved"
                || route.any_host
                || route.hosts.is_empty()
                || route.hosts.iter().any(|h| {
                    config::exact_host(h).is_ok_and(|h| h == receipt.spec.hostname)
                        || h.strip_prefix("*.")
                            .is_some_and(|s| receipt.spec.hostname.ends_with(&format!(".{s}")))
                })
            {
                return Err(error(
                    "route.external_overlap",
                    "Existing external or unresolved route overlaps the selected listener/host",
                )
                .at(route.native_id.clone()));
            }
        }
    }
    for (index, receipt) in wanted.iter().filter(|r| r.spec.enabled).enumerate() {
        if wanted
            .iter()
            .filter(|r| r.spec.enabled)
            .skip(index + 1)
            .any(|r| r.spec.hostname == receipt.spec.hostname)
        {
            return Err(error(
                "route.request_collision",
                "Two owned routes request the same hostname",
            ));
        }
    }
    Ok(())
}

fn validate_caddy(binary: &Path, source: &Path, candidate: &str) -> Result<()> {
    let parent = source
        .parent()
        .ok_or_else(|| error("authority.path", "Missing startup parent"))?;
    let mut stage = tempfile::Builder::new()
        .suffix(".json")
        .tempfile_in(parent)
        .map_err(io_error)?;
    stage.write_all(candidate.as_bytes()).map_err(io_error)?;
    stage.as_file().sync_all().map_err(io_error)?;
    let mut child = Command::new(binary)
        .args(["validate", "--config"])
        .arg(stage.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| {
            error(
                "validation.executable",
                "Could not run the explicitly selected Caddy executable",
            )
        })?;
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match child.try_wait().map_err(io_error)? {
            Some(status) if status.success() => return Ok(()),
            Some(_) => {
                return Err(error(
                    "validation.rejected",
                    "Caddy rejected the complete candidate configuration; no input was published",
                ));
            }
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error(
                    "validation.timeout",
                    "Caddy validation exceeded 15 seconds",
                ));
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    }
}

fn caddy_current(connection: &Connection) -> Result<(Value, String)> {
    let replies = transport::fetch(&connection.binding)?;
    let etag = replies
        .sources
        .first()
        .and_then(|s| s.etag.clone())
        .ok_or_else(|| {
            error(
                "authority.etag",
                "Caddy did not provide a conditional-update ETag",
            )
        })?;
    Ok((replies.values["/config/"].clone(), etag))
}

fn native_matches(
    connection: &Connection,
    book: &Book,
    expected_source: Option<&str>,
) -> Result<bool> {
    match &connection.authority {
        Authority::CaddyFile { controller, .. } => {
            if !controller {
                let expected = caddyfile::native(
                    connection,
                    expected_source.ok_or_else(|| error("authority.file", "Caddyfile missing"))?,
                )?;
                return Ok(caddy_current(connection)?.0 == expected);
            }
            let observed = inventory::inspect(&connection.binding, Scope::User)?;
            Ok(own(book, &connection.binding.id).iter().all(|r| {
                let count = observed
                    .routes
                    .iter()
                    .filter(|o| caddyfile::route_matches(connection, o, r))
                    .count();
                if r.spec.enabled {
                    count == 1
                } else {
                    count == 0
                }
            }))
        }
        Authority::ReadOnly => Err(error(
            "capability.read_only",
            "Inventory-only connections cannot verify a write",
        )),
        Authority::CaddyJson { .. } => {
            let (value, _) = caddy_current(connection)?;
            Ok(value
                == config::strict_json(
                    expected_source
                        .ok_or_else(|| {
                            error("state.journal", "Caddy journal lacks candidate input")
                        })?
                        .as_bytes(),
                )?)
        }
        Authority::TraefikDirectory { entrypoint, .. } => {
            let replies = transport::fetch(&connection.binding)?;
            let routers = replies.values["/api/http/routers"]
                .as_array()
                .ok_or_else(|| error("discovery.invalid_response", "Expected router array"))?;
            let services = replies.values["/api/http/services"]
                .as_array()
                .ok_or_else(|| error("discovery.invalid_response", "Expected service array"))?;
            let expected = own(book, &connection.binding.id);
            let prefix = format!("steward-{}-{}-", book.owner, connection.binding.id);
            let actual: Vec<_> = routers
                .iter()
                .filter(|r| {
                    r["name"].as_str().is_some_and(|n| {
                        n.starts_with(&prefix)
                            || expected
                                .iter()
                                .any(|r| n == format!("{}@file", r.native_id))
                    })
                })
                .collect();
            if actual.len() != expected.iter().filter(|r| r.spec.enabled).count() {
                return Ok(false);
            }
            for receipt in expected.iter().filter(|r| r.spec.enabled) {
                let name = format!("{}@file", receipt.native_id);
                let Some(router) = actual.iter().find(|r| r["name"] == name) else {
                    return Ok(false);
                };
                if router["rule"] != format!("Host(`{}`)", receipt.spec.hostname)
                    || router["status"] != "enabled"
                    || router["entryPoints"] != json!([entrypoint])
                    || router
                        .get("middlewares")
                        .is_some_and(|v| !v.is_null() && v != &json!([]))
                    || (router
                        .get("tls")
                        .is_some_and(|v| !v.is_null() && v != &Value::Bool(false))
                        == matches!(receipt.spec.tls.mode, TlsMode::Http))
                    || !matches!(router["service"].as_str(), Some(s) if s == receipt.native_id || s == name)
                {
                    return Ok(false);
                }
                let Some(service) = services.iter().find(|s| s["name"] == name) else {
                    return Ok(false);
                };
                if service.pointer("/loadBalancer/servers/0/url")
                    != Some(&json!(receipt.spec.upstream.url))
                    || !service
                        .pointer("/loadBalancer/servers")
                        .and_then(Value::as_array)
                        .is_some_and(|s| s.len() == 1)
                {
                    return Ok(false);
                }
            }
            Ok(true)
        }
    }
}

fn wait_native(connection: &Connection, book: &Book, source: Option<&str>) -> Result<()> {
    let deadline = Instant::now()
        + Duration::from_secs(
            if matches!(
                connection.authority,
                Authority::CaddyFile {
                    controller: true,
                    ..
                }
            ) {
                70
            } else {
                10
            },
        );
    loop {
        if native_matches(connection, book, source)? {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(error(
                "activation.unverified",
                "Owned routes did not activate within 10 seconds",
            ));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn probe(connection: &Connection, receipts: &[&Receipt]) -> Result<()> {
    let base = authority(connection)?;
    let address = config::http_url(&base)?
        .host_str()
        .unwrap_or("")
        .trim_matches(['[', ']'])
        .parse::<std::net::IpAddr>()
        .map_err(|_| error("verification.address", "Invalid local verification address"))?;
    let port = config::http_url(&base)?
        .port()
        .ok_or_else(|| error("verification.port", "Missing listener port"))?;
    let deadline = Instant::now() + Duration::from_secs(60);
    for receipt in receipts.iter().filter(|r| r.spec.enabled) {
        let tls = !matches!(receipt.spec.tls.mode, TlsMode::Http);
        let mut builder = reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(4))
            .resolve(
                &receipt.spec.hostname,
                std::net::SocketAddr::new(address, port),
            );
        if tls && let Some(ca) = connection.tls.as_ref().and_then(|t| t.ca_file.as_ref()) {
            let cert = reqwest::Certificate::from_pem(&config::read_bounded(ca, 128 * 1024)?)
                .map_err(|_| error("tls.trust", "Invalid CA PEM"))?;
            builder = builder.add_root_certificate(cert);
        }
        let client = builder.build().map_err(|_| {
            error(
                "verification.client",
                "Could not construct verified routing client",
            )
        })?;
        loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or_else(|| {
                    error(
                        "verification.timeout",
                        "Routing verification exceeded its 15-second aggregate deadline",
                    )
                })?;
            let response = client
                .get(format!(
                    "{}://{}:{port}/",
                    if tls { "https" } else { "http" },
                    receipt.spec.hostname
                ))
                .header("Host", &receipt.spec.hostname)
                .timeout(remaining.min(Duration::from_secs(4)))
                .send();
            if response.is_ok_and(|r| (200..400).contains(&r.status().as_u16())) {
                break;
            }
            if !tls || Instant::now() >= deadline {
                return Err(error(
                    "verification.routing",
                    "Verified route request failed; check backend, certificate trust, hostname and issuance",
                ));
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }
    Ok(())
}

impl Store {
    pub fn edit(&mut self, proxy: &str, edit: Edit) -> Result<Value> {
        self.edit_tracked(proxy, edit, |_| Ok(()))
    }

    pub fn edit_tracked(
        &mut self,
        proxy: &str,
        edit: Edit,
        checkpoint: impl FnOnce(&str) -> Result<()>,
    ) -> Result<Value> {
        let (journal, id) = self.prepare_edit(proxy, edit)?;
        checkpoint(&journal.id)?;
        self.commit_edit(journal, &id)
    }

    fn prepare_edit(&self, proxy: &str, edit: Edit) -> Result<(Journal, String)> {
        self.ready()?;
        config::resource_id(proxy)?;
        let connection = self
            .book
            .connections
            .iter()
            .find(|c| c.binding.id == proxy)
            .cloned()
            .ok_or_else(|| {
                error(
                    "binding.missing",
                    "Register this proxy with proxies add first",
                )
            })?;
        authority(&connection)?;
        let _input_lock = input_lock(&connection)?;
        let mut desired = self.book.clone();
        let id = match edit {
            Edit::Upsert { routes } => {
                if routes.len() > 128 {
                    return Err(error("input.limit", "At most 128 managed routes"));
                }
                let mut ids = std::collections::BTreeSet::new();
                for mut spec in routes {
                    if spec.proxy != proxy || !ids.insert(spec.id.clone()) {
                        return Err(error(
                            "plan.routes",
                            "Plan routes must have unique IDs on the selected proxy",
                        ));
                    }
                    config::validate_route(&mut spec)?;
                    validate_tls(&connection, &spec)?;
                    if let Some(existing) = desired
                        .routes
                        .iter_mut()
                        .find(|r| r.spec.proxy == proxy && r.spec.id == spec.id)
                    {
                        existing.spec = spec;
                    } else {
                        if desired.routes.len() >= 128 {
                            return Err(error("input.limit", "At most 128 managed routes"));
                        }
                        let native_id = format!("steward-{}-{proxy}-{}", desired.owner, spec.id);
                        desired.routes.push(Receipt { spec, native_id });
                    }
                }
                "manifest".into()
            }
            Edit::Add(mut spec) => {
                spec.proxy = proxy.into();
                config::validate_route(&mut spec)?;
                validate_tls(&connection, &spec)?;
                if desired.routes.len() >= 128
                    || desired
                        .routes
                        .iter()
                        .any(|r| r.spec.proxy == proxy && r.spec.id == spec.id)
                {
                    return Err(error(
                        "route.exists",
                        "Owned route ID already exists, or the 128-route limit was reached",
                    ));
                }
                let id = spec.id.clone();
                let native_id = format!("steward-{}-{proxy}-{id}", desired.owner);
                desired.routes.push(Receipt { spec, native_id });
                id
            }
            Edit::Set {
                id,
                hostname,
                upstream,
                enabled,
            } => {
                config::resource_id(&id)?;
                let receipt = desired
                    .routes
                    .iter_mut()
                    .find(|r| r.spec.proxy == proxy && r.spec.id == id)
                    .ok_or_else(|| {
                        error(
                            "ownership.required",
                            "Only routes created in this state directory can be changed",
                        )
                    })?;
                if let Some(host) = hostname {
                    receipt.spec.hostname = host;
                }
                if let Some(url) = upstream {
                    receipt.spec.upstream.url = url;
                }
                if let Some(value) = enabled {
                    receipt.spec.enabled = value;
                }
                config::validate_route(&mut receipt.spec)?;
                id
            }
            Edit::Remove(id) => {
                config::resource_id(&id)?;
                if !desired
                    .routes
                    .iter()
                    .any(|r| r.spec.proxy == proxy && r.spec.id == id)
                {
                    return Err(error(
                        "ownership.required",
                        "Only routes created in this state directory can be removed",
                    ));
                }
                desired
                    .routes
                    .retain(|r| r.spec.proxy != proxy || r.spec.id != id);
                id
            }
        };
        let source = match &connection.authority {
            Authority::ReadOnly => {
                return Err(error(
                    "capability.read_only",
                    "Inventory-only connections cannot publish route inputs",
                ));
            }
            Authority::CaddyJson { file, .. } | Authority::CaddyFile { file, .. } => file.clone(),
            Authority::TraefikDirectory {
                directory,
                managed_file,
                ..
            } => managed_file.clone().unwrap_or_else(|| {
                directory.join(format!("steward-{}-{proxy}.yaml", self.book.owner))
            }),
        };
        let before = text(&source)?;
        let after = candidate(&connection, &self.book, &desired, before.as_deref())?;
        let (native_before, native_after) =
            if let Authority::CaddyFile { binary, .. } = &connection.authority {
                let before_native =
                    caddyfile::adapt(binary, &source, before.as_deref().expect("source"))?;
                let after_native =
                    caddyfile::adapt(binary, &source, after.as_deref().expect("candidate"))?;
                validate_caddy(binary, &source, &encoded(&after_native)?)?;
                (Some(before_native), Some(after_native))
            } else {
                (None, None)
            };
        if matches!(connection.authority, Authority::TraefikDirectory { .. })
            && !native_matches(&connection, &self.book, before.as_deref())?
        {
            return Err(error(
                "ownership.runtime_drift",
                "Owned Traefik native definitions differ from the registered receipts",
            ));
        }
        check_conflicts(&connection, &self.book, &own(&desired, proxy))?;
        if let Authority::CaddyJson { binary, .. } = &connection.authority {
            validate_caddy(binary, &source, after.as_deref().expect("Caddy candidate"))?;
        }
        let operation = fingerprint(&(
            transport::now(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            std::process::id(),
        ));
        desired.revision = operation[..16].into();
        let journal = Journal {
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
            native_before,
            native_after,
        };
        Ok((journal, id))
    }

    fn commit_edit(&mut self, mut journal: Journal, id: &str) -> Result<Value> {
        let proxy = journal.connection.binding.id.clone();
        let _input_lock = input_lock(&journal.connection)?;
        self.record(&journal)?;
        let result = self.execute(&mut journal);
        if let Err(failure) = result {
            if journal.phase == "prepared" {
                journal.phase = "compensated".into();
                self.record(&journal)?;
                return Err(failure.at(journal.id));
            }
            if self.compensate(&mut journal).is_ok() {
                return Err(error(
                    "operation.compensated",
                    &format!("{}; original owned input was restored", failure.code),
                )
                .at(journal.id));
            }
            journal.phase = "recovery_required".into();
            let _ = self.record(&journal);
            return Err(error("operation.recovery_required", "A write occurred but activation/compensation could not be verified; inspect operations list and recover").at(journal.id).with_changes());
        }
        self.book = journal.desired;
        let active = own(&self.book, &proxy).iter().any(|r| r.spec.enabled);
        Ok(
            json!({"operation":journal.id,"phase":"verified","proxy":proxy,"route":id,"verification":{"native":true,"routing":if active { json!(true) } else { Value::Null }},"owned_routes":own(&self.book,&proxy)}),
        )
    }

    fn execute(&self, journal: &mut Journal) -> Result<()> {
        if text(&journal.source)? != journal.before {
            return Err(error("operation.stale", "Source changed after preparation"));
        }
        let caddy_etag = if matches!(
            journal.connection.authority,
            Authority::CaddyJson { .. }
                | Authority::CaddyFile {
                    controller: false,
                    ..
                }
        ) {
            let (current, etag) = caddy_current(&journal.connection)?;
            if current != journal_native(journal, false)? {
                return Err(error(
                    "authority.mismatch",
                    "Startup source does not match loaded Caddy configuration",
                ));
            }
            Some(etag)
        } else {
            None
        };
        if text(&journal.source)? != journal.before {
            return Err(error(
                "operation.stale",
                "Source changed during native revalidation",
            ));
        }
        journal.phase = "publishing".into();
        self.record(journal)?;
        publish(&journal.source, journal.after.as_deref())?;
        journal.phase = "published".into();
        self.record(journal)?;
        if let Some(etag) = caddy_etag {
            transport::replace_caddy(
                &journal.connection.binding,
                &journal_native(journal, true)?,
                &etag,
            )?;
        }
        journal.phase = "activation_pending".into();
        self.record(journal)?;
        wait_native(
            &journal.connection,
            &if journal.before == journal.after {
                journal.desired.clone()
            } else {
                verification_book(&journal.desired, &journal.old)
            },
            journal.after.as_deref(),
        )?;
        probe(
            &journal.connection,
            &own(&journal.desired, &journal.connection.binding.id),
        )?;
        self.save(&journal.desired)?;
        journal.phase = "verified".into();
        self.record(journal)
    }

    fn compensate(&self, journal: &mut Journal) -> Result<()> {
        let current = text(&journal.source)?;
        if current != journal.after && current != journal.before {
            return Err(error("operation.drift", "Input drift blocks compensation"));
        }
        if let Authority::CaddyJson { binary, .. }
        | Authority::CaddyFile {
            binary,
            controller: false,
            ..
        } = &journal.connection.authority
        {
            let original = journal_native(journal, false)?;
            let (native, etag) = caddy_current(&journal.connection)?;
            if native != original {
                if native != journal_native(journal, true)? {
                    return Err(error(
                        "operation.drift",
                        "Runtime drift blocks conditional compensation",
                    ));
                }
                validate_caddy(binary, &journal.source, &encoded(&original)?)?;
                transport::replace_caddy(&journal.connection.binding, &original, &etag)?;
            }
        }
        // Recheck immediately before restoring only this operation's authorized input.
        if text(&journal.source)? != current {
            return Err(error("operation.drift", "Source changed during recovery"));
        }
        publish(&journal.source, journal.before.as_deref())?;
        wait_native(
            &journal.connection,
            &if journal.before == journal.after {
                journal.old.clone()
            } else {
                verification_book(&journal.old, &journal.desired)
            },
            journal.before.as_deref(),
        )?;
        self.save(&journal.old)?;
        journal.phase = "compensated".into();
        self.record(journal)
    }

    pub fn recover(&mut self, id: &str) -> Result<Value> {
        let mut journal = self
            .journal()?
            .ok_or_else(|| error("operation.missing", "No retained operation exists"))?;
        if journal.id != id {
            return Err(error(
                "operation.missing",
                "Operation ID does not match the retained journal",
            ));
        }
        if matches!(journal.phase.as_str(), "verified" | "compensated") {
            return Err(error(
                "operation.terminal",
                "This operation is already terminal; recover is only for unfinished writes",
            ));
        }
        let _input_lock = input_lock(&journal.connection)?;
        self.compensate(&mut journal)
            .map_err(|e| e.at(id).with_changes())?;
        self.book = journal.old;
        Ok(json!({"operation":id,"phase":"compensated"}))
    }
}

pub(crate) fn new_revision() -> String {
    fingerprint(&(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        std::process::id(),
    ))[..16]
        .into()
}

fn journal_native(journal: &Journal, after: bool) -> Result<Value> {
    let source = if after {
        &journal.after
    } else {
        &journal.before
    };
    let source = source
        .as_deref()
        .ok_or_else(|| error("state.journal", "Missing native source"))?;
    if matches!(journal.connection.authority, Authority::CaddyFile { .. }) {
        let stored = if after {
            &journal.native_after
        } else {
            &journal.native_before
        };
        let stored = stored
            .as_ref()
            .ok_or_else(|| error("state.journal", "Missing adapted native snapshot"))?;
        if caddyfile::native(&journal.connection, source)? != *stored {
            return Err(error(
                "operation.drift",
                "Imported Caddyfile inputs changed after preparation",
            ));
        }
        Ok(stored.clone())
    } else {
        config::strict_json(source.as_bytes())
    }
}

fn verification_book(expected: &Book, previous: &Book) -> Book {
    let mut book = expected.clone();
    for old in &previous.routes {
        if !book
            .routes
            .iter()
            .any(|r| r.spec.proxy == old.spec.proxy && r.native_id == old.native_id)
        {
            let mut absent = old.clone();
            absent.spec.enabled = false;
            book.routes.push(absent);
        }
    }
    book
}

pub fn qualify_ownership(book: &Book, observed: &mut crate::model::Inventory) {
    let binding = book
        .connections
        .iter()
        .find(|c| c.binding.id == observed.binding);
    let source_intact = binding.is_none_or(|c| {
        if let Authority::CaddyFile { file, .. } = &c.authority {
            text(file).is_ok_and(|body| candidate(c, book, book, body.as_deref()).is_ok())
        } else {
            true
        }
    });
    observed.write_authorized = observed.runtime_confirmed
        && book.connections.iter().any(|c| {
            c.binding.id == observed.binding && !matches!(c.authority, Authority::ReadOnly)
        });
    for route in &mut observed.routes {
        for receipt in own(book, &observed.binding) {
            if binding.is_some_and(|c| caddyfile::route_matches(c, route, receipt)) {
                route.ownership = if receipt.spec.enabled && source_intact {
                    "owned"
                } else {
                    "owned_drifted"
                };
                continue;
            }
            let native_id = match observed.engine {
                Engine::Caddy => format!("{}:0", receipt.native_id),
                Engine::Traefik => format!("{}@file", receipt.native_id),
            };
            if route.native_id != native_id {
                continue;
            }
            let matches = receipt.spec.enabled
                && match observed.engine {
                    Engine::Caddy => caddy_route(receipt).is_ok_and(|r| route.native["route"] == r),
                    Engine::Traefik => {
                        route.native["router"]["rule"]
                            == format!("Host(`{}`)", receipt.spec.hostname)
                            && route.native.pointer("/service/loadBalancer/servers/0/url")
                                == Some(&json!(receipt.spec.upstream.url))
                    }
                };
            route.ownership = if matches { "owned" } else { "owned_drifted" };
        }
    }
}
