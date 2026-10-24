//! Explicit proxy artifact and native-service lifecycle; source/configuration is preserved.
use crate::{
    composition, config,
    error::{Diagnostic, Result},
    executor,
    managed::{self, Authority, Connection, Store},
    model::{Binding, Engine, Scope, TlsMode},
    services::{self, ServiceCommand, Spec},
};
use clap::Args;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256, Sha512};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Args)]
pub struct SetupArgs {
    pub id: String,
    #[arg(long, value_enum)]
    pub engine: Engine,
    #[arg(
        long,
        help = "Existing executable for setup; install downloads an upstream release instead"
    )]
    pub binary: Option<PathBuf>,
    #[arg(long)]
    pub version: Option<String>,
    #[arg(long, default_value = "127.0.0.1:8088")]
    pub listen: String,
    #[arg(long, default_value = "127.0.0.1:22019")]
    pub admin: String,
    #[arg(long, value_enum, default_value = "user")]
    pub scope: Scope,
    #[arg(long, value_enum, default_value = "http")]
    pub tls: TlsMode,
    #[arg(long)]
    pub acme_email: Option<String>,
    #[arg(long)]
    pub ca_file: Option<PathBuf>,
    #[arg(long, requires = "private_key")]
    pub certificate: Option<PathBuf>,
    #[arg(long, requires = "certificate")]
    pub private_key: Option<PathBuf>,
}

#[derive(Clone, Deserialize, Serialize)]
struct Installation {
    schema_version: String,
    id: String,
    engine: Engine,
    version: Option<String>,
    binary: PathBuf,
    binary_digest: String,
    downloaded: bool,
    service: Spec,
    connection: Connection,
}
#[derive(Deserialize, Serialize)]
struct Upgrade {
    installation: Installation,
    before_digest: String,
    after_digest: String,
    backup: PathBuf,
    phase: String,
}

fn error(code: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message)
}
fn io_error(_: std::io::Error) -> Diagnostic {
    error(
        "setup.io",
        "Proxy installation storage could not be accessed",
    )
}

fn lifecycle_lock(root: &Path, id: &str) -> Result<File> {
    config::resource_id(id)?;
    drop(Store::open(root.to_path_buf())?);
    let path = root.join(format!("proxy-{id}.lock"));
    managed::regular(&path)?;
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path).map_err(io_error)?;
    file.try_lock()
        .map_err(|_| error("setup.locked", "Another lifecycle command holds this proxy"))?;
    Ok(file)
}
fn saved(path: &Path, value: &impl Serialize) -> Result<()> {
    managed::publish(
        path,
        Some(
            &serde_json::to_string_pretty(value)
                .map_err(|_| error("setup.state", "Cannot serialize installation receipt"))?,
        ),
    )
}
fn checksum(path: &Path) -> Result<String> {
    let mut file = File::open(path).map_err(io_error)?;
    let mut h = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(io_error)?;
        if n == 0 {
            break;
        }
        h.update(&buffer[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}

fn download(url: &str, path: &Path, limit: u64) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(5))
        .https_only(true)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|_| error("setup.download", "Cannot create verified download client"))?;
    let mut response = client
        .get(url)
        .send()
        .map_err(|_| error("setup.download", "Artifact download failed"))?;
    if !response.status().is_success() || response.content_length().is_some_and(|n| n > limit) {
        return Err(error(
            "setup.download",
            "Upstream artifact unavailable or oversized",
        ));
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(io_error)?;
    let mut total = 0;
    let mut bytes = [0; 65536];
    loop {
        let n = response.read(&mut bytes).map_err(io_error)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > limit {
            return Err(error(
                "setup.limit",
                "Artifact exceeds bounded download size",
            ));
        }
        file.write_all(&bytes[..n]).map_err(io_error)?;
    }
    file.sync_all().map_err(io_error)
}

fn artifact(engine: Engine, version: &str, destination: &Path) -> Result<()> {
    if version.is_empty()
        || version.len() > 32
        || !version.bytes().all(|b| b.is_ascii_digit() || b == b'.')
    {
        return Err(error(
            "setup.version",
            "Specify a numeric upstream version such as 2.11.7 or 3.7.14",
        ));
    }
    let (name, repo, marker) = match engine {
        Engine::Caddy => ("caddy", "caddyserver/caddy", ""),
        Engine::Traefik => ("traefik", "traefik/traefik", "v"),
    };
    let os = match std::env::consts::OS {
        "linux" => "linux",
        "macos" => {
            if engine == Engine::Caddy {
                "mac"
            } else {
                "darwin"
            }
        }
        _ => {
            return Err(error(
                "setup.platform",
                "Native artifacts support Linux/macOS",
            ));
        }
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        _ => {
            return Err(error(
                "setup.platform",
                "Native artifacts support x86_64/ARM64",
            ));
        }
    };
    let parent = destination
        .parent()
        .ok_or_else(|| error("setup.path", "Missing binary parent"))?;
    let temp = tempfile::tempdir_in(parent).map_err(io_error)?;
    let filename = format!("{name}_{marker}{version}_{os}_{arch}.tar.gz");
    let base = format!("https://github.com/{repo}/releases/download/v{version}");
    let archive = temp.path().join("archive.tar.gz");
    let catalog = temp.path().join("checksums");
    download(
        &format!("{base}/{name}_{marker}{version}_checksums.txt"),
        &catalog,
        128 * 1024,
    )?;
    let text = String::from_utf8(config::read_bounded(&catalog, 128 * 1024)?)
        .map_err(|_| error("setup.checksum", "Invalid upstream checksum file"))?;
    let expected: Vec<_> = text
        .lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            let hash = w.next()?;
            if w.next() == Some(filename.as_str()) {
                Some(hash)
            } else {
                None
            }
        })
        .collect();
    if expected.len() != 1 {
        return Err(error(
            "setup.checksum",
            "Archive checksum missing or ambiguous",
        ));
    }
    download(&format!("{base}/{filename}"), &archive, 100 * 1024 * 1024)?;
    let mut sha256 = Sha256::new();
    let mut sha512 = Sha512::new();
    let mut file = File::open(&archive).map_err(io_error)?;
    let mut bytes = [0; 65536];
    loop {
        let n = file.read(&mut bytes).map_err(io_error)?;
        if n == 0 {
            break;
        }
        sha256.update(&bytes[..n]);
        sha512.update(&bytes[..n]);
    }
    let actual = if expected[0].len() == 128 {
        format!("{:x}", sha512.finalize())
    } else {
        format!("{:x}", sha256.finalize())
    };
    if actual != expected[0] {
        return Err(error(
            "setup.checksum",
            "Upstream archive checksum mismatch",
        ));
    }
    executor::checked(
        "tar",
        &[
            "-xzf".as_ref(),
            archive.as_os_str(),
            "-C".as_ref(),
            temp.path().as_os_str(),
            name.as_ref(),
        ],
    )?;
    let binary = temp.path().join(name);
    let metadata = managed::regular(&binary)?
        .ok_or_else(|| error("setup.archive", "Executable missing from archive"))?;
    if metadata.len() > 256 * 1024 * 1024 {
        return Err(error(
            "setup.archive",
            "Executable exceeds extraction budget",
        ));
    }
    let reported = executor::checked(&binary, &["version".as_ref()])?;
    let reported = String::from_utf8_lossy(&reported);
    if !reported.contains(version) {
        return Err(error(
            "setup.version",
            "Downloaded executable reports another version",
        ));
    }
    if destination.exists() {
        return Err(error("setup.exists", "Destination binary already exists"));
    }
    fs::rename(binary, destination).map_err(io_error)?;
    File::open(parent)
        .and_then(|f| f.sync_all())
        .map_err(io_error)
}

pub fn setup(root: PathBuf, args: &SetupArgs, install: bool) -> Result<Value> {
    let _guard = lifecycle_lock(&root, &args.id)?;
    config::resource_id(&args.id)?;
    let listen: std::net::SocketAddr = args
        .listen
        .parse()
        .map_err(|_| error("setup.listener", "Use an explicit IP:port listener"))?;
    let admin: std::net::SocketAddr = args.admin.parse().map_err(|_| {
        error(
            "setup.admin",
            "Use an explicit loopback IP:port admin listener",
        )
    })?;
    if !admin.ip().is_loopback() || admin.port() == 0 || listen.port() == 0 || admin == listen {
        return Err(error(
            "setup.listener",
            "Use distinct nonzero listener ports and a loopback admin API",
        ));
    }
    let directory = root.join("proxies").join(&args.id);
    {
        let store = Store::open(root.clone())?;
        if store
            .book
            .connections
            .iter()
            .any(|c| c.binding.id == args.id)
            || directory.exists()
        {
            return Err(error(
                "setup.exists",
                "Proxy ID/installation directory already exists",
            ));
        }
    }
    fs::create_dir_all(&directory).map_err(io_error)?;
    let binary = if install {
        if args.binary.is_some() {
            return Err(error(
                "setup.arguments",
                "install downloads a release; use setup for an existing --binary",
            ));
        }
        let version = args
            .version
            .as_deref()
            .ok_or_else(|| error("setup.version", "install requires an explicit --version"))?;
        let binary = directory.join(if args.engine == Engine::Caddy {
            "caddy"
        } else {
            "traefik"
        });
        artifact(args.engine, version, &binary)?;
        binary
    } else {
        managed::executable(
            args.binary
                .as_deref()
                .ok_or_else(|| error("setup.binary", "setup requires --binary"))?,
        )?
    };
    let dynamic = directory.join("dynamic");
    fs::create_dir(&dynamic).map_err(io_error)?;
    let startup = directory.join("proxy.json");
    let mut native = match args.engine {
        Engine::Caddy => {
            json!({"admin":{"listen":args.admin},"apps":{"http":{"servers":{"web":{"listen":[args.listen],"automatic_https":{"disable":true},"routes":[]}}}}})
        }
        Engine::Traefik => {
            json!({"entryPoints":{"web":{"address":args.listen},"traefik":{"address":args.admin}},"api":{"insecure":true},"providers":{"file":{"directory":dynamic,"watch":true}}})
        }
    };
    let mut tls = None;
    if !matches!(args.tls, TlsMode::Http) {
        let mut settings = managed::ProxyTls::default();
        match args.engine {
            Engine::Caddy => {
                native["apps"]["http"]["https_port"] = json!(listen.port());
                native["apps"]["http"]["servers"]["web"]["automatic_https"] =
                    json!({"disable_redirects":true});
                native["apps"]["http"]["servers"]["web"]["tls_connection_policies"] = json!([{}]);
                if matches!(args.tls, TlsMode::Internal) {
                    native["apps"]["pki"] =
                        json!({"certificate_authorities":{"local":{"install_trust":false}}});
                    settings.ca_file =
                        Some(directory.join("data/caddy/pki/authorities/local/root.crt"));
                }
            }
            Engine::Traefik => match args.tls {
                TlsMode::Public => {
                    let email = args
                        .acme_email
                        .as_ref()
                        .filter(|s| s.contains('@') && !s.chars().any(char::is_control))
                        .ok_or_else(|| {
                            error(
                                "setup.acme",
                                "Traefik public TLS setup requires --acme-email",
                            )
                        })?;
                    native["certificatesResolvers"] = json!({"steward":{"acme":{"email":email,"storage":directory.join("acme.json"),"tlsChallenge":{}}}});
                    settings.cert_resolver = Some("steward".into());
                }
                TlsMode::Internal => {
                    if args.ca_file.is_none()
                        || args.certificate.is_none()
                        || args.private_key.is_none()
                    {
                        return Err(error(
                            "setup.tls",
                            "Traefik internal TLS requires --ca-file, --certificate and --private-key",
                        ));
                    }
                    settings.ca_file = args.ca_file.clone();
                    settings.certificate = args.certificate.clone();
                    settings.private_key = args.private_key.clone();
                }
                TlsMode::Http => {}
            },
        }
        tls = Some(settings);
    }
    saved(&startup, &native)?;
    let environment = std::collections::BTreeMap::from([
        (
            "XDG_DATA_HOME".into(),
            directory.join("data").to_string_lossy().into_owned(),
        ),
        (
            "XDG_CONFIG_HOME".into(),
            directory.join("config").to_string_lossy().into_owned(),
        ),
    ]);
    let service = Spec {
        id: format!("proxy-{}", args.id),
        scope: args.scope,
        executable: binary.clone(),
        args: match args.engine {
            Engine::Caddy => vec![
                "run".into(),
                "--config".into(),
                startup.to_string_lossy().into_owned(),
            ],
            Engine::Traefik => vec![format!("--configFile={}", startup.display())],
        },
        working_directory: Some(directory.clone()),
        environment,
        enabled: true,
        restart: true,
        ready_url: Some(format!(
            "http://{admin}{}",
            if args.engine == Engine::Caddy {
                "/config/"
            } else {
                "/api/overview"
            }
        )),
    };
    let connection = Connection {
        binding: Binding {
            id: args.id.clone(),
            engine: args.engine,
            endpoint: format!("http://{admin}"),
            auth_file: None,
            snapshot_file: None,
        },
        authority: match args.engine {
            Engine::Caddy => Authority::CaddyJson {
                file: startup.clone(),
                server: "web".into(),
                binary: binary.clone(),
            },
            Engine::Traefik => Authority::TraefikDirectory {
                startup,
                directory: dynamic,
                entrypoint: "web".into(),
                managed_file: None,
            },
        },
        tls,
    };
    let installation = Installation {
        schema_version: "1".into(),
        id: args.id.clone(),
        engine: args.engine,
        version: args.version.clone(),
        binary_digest: checksum(&binary)?,
        binary,
        downloaded: install,
        service: service.clone(),
        connection: connection.clone(),
    };
    saved(&directory.join("installation.json"), &installation)?;
    services::run(
        root.clone(),
        &ServiceCommand::Add(composition::service_args(&service, true)),
    )?;
    let result = Store::open(root.clone())?.connect(connection);
    if let Err(e) = result {
        let _ = services::run(root, &ServiceCommand::Remove { id: service.id });
        return Err(Diagnostic::new("setup.connection",format!("Proxy service created but connection rejected: {}; generated inputs retained for inspection",e.code)).with_changes());
    }
    Ok(
        json!({"proxy":args.id,"version":args.version,"service":service.id,"directory":directory,"source_preserved":true}),
    )
}

fn installation(root: &Path, id: &str) -> Result<(PathBuf, Installation)> {
    config::resource_id(id)?;
    let directory = root.join("proxies").join(id);
    let record: Installation =
        config::load(&directory.join("installation.json"), config::MAX_CONFIG)?;
    config::version(&record.schema_version)?;
    if record.id != id {
        return Err(error("setup.identity", "Installation receipt ID mismatch"));
    }
    Ok((directory, record))
}

pub fn uninstall(root: PathBuf, id: &str) -> Result<Value> {
    let _guard = lifecycle_lock(&root, id)?;
    let (directory, record) = installation(&root, id)?;
    if directory.join("upgrade.json").exists() {
        return Err(error(
            "setup.pending",
            "Recover the pending upgrade before uninstalling",
        ));
    }
    {
        let store = Store::open(root.clone())?;
        if store.book.routes.iter().any(|r| r.spec.proxy == id) {
            return Err(error(
                "setup.in_use",
                "Remove owned routes or explicitly detach them before uninstalling",
            ));
        }
    }
    if checksum(&record.binary)? != record.binary_digest {
        return Err(error(
            "setup.drift",
            "Proxy executable changed outside Steward",
        ));
    }
    if services::saved_specs(&root)?
        .iter()
        .any(|s| s.id == record.service.id)
    {
        services::run(
            root.clone(),
            &ServiceCommand::Remove {
                id: record.service.id,
            },
        )?;
    }
    let mut store = Store::open(root)?;
    if store.book.connections.iter().any(|c| c.binding.id == id) {
        store.remove_connection(id, false)?;
    }
    if record.downloaded {
        fs::remove_file(&record.binary).map_err(io_error)?;
    }
    fs::remove_file(directory.join("installation.json")).map_err(io_error)?;
    Ok(
        json!({"proxy":id,"configuration_and_certificates_retained":directory,"binary_removed":record.downloaded}),
    )
}

pub fn upgrade(root: PathBuf, id: &str, version: &str) -> Result<Value> {
    let guard = lifecycle_lock(&root, id)?;
    let (directory, mut record) = installation(&root, id)?;
    if !record.downloaded {
        return Err(error(
            "setup.external",
            "Existing external executables must be upgraded by their original installer",
        ));
    }
    if checksum(&record.binary)? != record.binary_digest {
        return Err(error("setup.drift", "Executable changed outside Steward"));
    }
    let pending = directory.join("upgrade.json");
    if pending.exists() {
        return Err(error(
            "setup.pending",
            "Recover the pending proxy upgrade first",
        ));
    }
    let staged = directory.join("upgrade-binary");
    artifact(record.engine, version, &staged)?;
    let backup = directory.join("previous-binary");
    if backup.exists() {
        return Err(error(
            "setup.backup",
            "Previous executable backup already exists",
        ));
    }
    fs::copy(&record.binary, &backup).map_err(io_error)?;
    File::open(&backup)
        .and_then(|f| f.sync_all())
        .map_err(io_error)?;
    let mut journal = Upgrade {
        installation: record.clone(),
        before_digest: record.binary_digest.clone(),
        after_digest: checksum(&staged)?,
        backup,
        phase: "prepared".into(),
    };
    saved(&pending, &journal)?;
    services::run(
        root.clone(),
        &ServiceCommand::Stop {
            id: record.service.id.clone(),
        },
    )?;
    fs::rename(&staged, &record.binary).map_err(io_error)?;
    journal.phase = "published".into();
    saved(&pending, &journal)?;
    if services::run(
        root.clone(),
        &ServiceCommand::Start {
            id: record.service.id.clone(),
        },
    )
    .is_err()
    {
        drop(guard);
        recover(root, id)?;
        return Err(error(
            "setup.compensated",
            "Upgraded proxy failed readiness; previous executable restored",
        ));
    }
    record.binary_digest = journal.after_digest;
    record.version = Some(version.into());
    saved(&directory.join("installation.json"), &record)?;
    fs::remove_file(&pending).map_err(io_error)?;
    fs::remove_file(journal.backup).map_err(io_error)?;
    Ok(json!({"proxy":id,"version":version,"configuration_preserved":true}))
}

pub fn recover(root: PathBuf, id: &str) -> Result<Value> {
    let _guard = lifecycle_lock(&root, id)?;
    let (directory, _) = installation(&root, id)?;
    let path = directory.join("upgrade.json");
    let journal: Upgrade = config::load(&path, config::MAX_CONFIG)?;
    let current = checksum(&journal.installation.binary)?;
    if current != journal.before_digest && current != journal.after_digest {
        return Err(error(
            "setup.drift",
            "Executable drift blocks upgrade recovery",
        ));
    }
    if checksum(&journal.backup)? != journal.before_digest {
        return Err(error("setup.backup", "Backup checksum changed"));
    }
    services::run(
        root.clone(),
        &ServiceCommand::Stop {
            id: journal.installation.service.id.clone(),
        },
    )?;
    let mut restored = tempfile::NamedTempFile::new_in(&directory).map_err(io_error)?;
    std::io::copy(
        &mut File::open(&journal.backup).map_err(io_error)?,
        &mut restored,
    )
    .map_err(io_error)?;
    restored
        .as_file()
        .set_permissions(
            fs::metadata(&journal.backup)
                .map_err(io_error)?
                .permissions(),
        )
        .map_err(io_error)?;
    restored.as_file().sync_all().map_err(io_error)?;
    restored
        .persist(&journal.installation.binary)
        .map_err(|e| io_error(e.error))?;
    services::run(
        root,
        &ServiceCommand::Start {
            id: journal.installation.service.id.clone(),
        },
    )?;
    saved(&directory.join("installation.json"), &journal.installation)?;
    fs::remove_file(path).map_err(io_error)?;
    fs::remove_file(journal.backup).map_err(io_error)?;
    Ok(json!({"proxy":id,"phase":"compensated"}))
}
