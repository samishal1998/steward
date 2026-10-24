//! General native services. The OS supervisor owns the process, never Steward.
use crate::{
    config,
    error::{Diagnostic, Result},
    executor,
    managed::{self, Store},
    model::Scope,
    transport,
};
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub id: String,
    pub scope: Scope,
    pub executable: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    pub working_directory: Option<PathBuf>,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub restart: bool,
    pub ready_url: Option<String>,
    #[serde(default = "default_ready_timeout")]
    pub ready_timeout: u16,
}

#[derive(Args)]
pub struct ServiceArgs {
    pub id: String,
    #[arg(long, value_enum, default_value = "user")]
    pub scope: Scope,
    #[arg(long)]
    pub executable: PathBuf,
    #[arg(long)]
    pub working_directory: Option<PathBuf>,
    #[arg(long = "env", value_name = "NAME=VALUE")]
    pub environment: Vec<String>,
    #[arg(long)]
    pub enabled: bool,
    #[arg(long)]
    pub restart: bool,
    #[arg(long)]
    pub ready_url: Option<String>,
    #[arg(long,default_value="30",value_parser=clap::value_parser!(u16).range(1..=300))]
    pub ready_timeout: u16,
    #[arg(long)]
    pub start: bool,
    #[arg(last = true)]
    pub args: Vec<String>,
}

#[derive(Subcommand)]
pub enum ServiceCommand {
    #[command(about = "List native services as external observations; no ownership is granted")]
    Discover {
        #[arg(long, value_enum, default_value = "user")]
        scope: Scope,
    },
    InspectNative {
        name: String,
        #[arg(long, value_enum, default_value = "user")]
        scope: Scope,
    },
    List,
    Inspect {
        id: String,
    },
    Add(ServiceArgs),
    Update(ServiceArgs),
    Start {
        id: String,
    },
    Stop {
        id: String,
    },
    Restart {
        id: String,
    },
    Enable {
        id: String,
    },
    Disable {
        id: String,
    },
    Remove {
        id: String,
    },
    Logs {
        id: String,
        #[arg(long,default_value="100",value_parser=clap::value_parser!(u16).range(1..=1000))]
        lines: u16,
    },
    Recover,
    Rollback {
        id: String,
    },
}

#[derive(Clone, Deserialize, Serialize)]
struct Record {
    spec: Spec,
    path: PathBuf,
    label: String,
    fingerprint: String,
}
#[derive(Default, Clone, Deserialize, Serialize)]
struct Registry {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    revision: String,
    #[serde(default)]
    services: Vec<Record>,
}
#[derive(Deserialize, Serialize)]
struct Journal {
    id: String,
    phase: String,
    record: Record,
    before: Option<String>,
    after: Option<String>,
    old: Registry,
    next: Registry,
    was_running: bool,
    #[serde(default)]
    after_running: Option<bool>,
    #[serde(default)]
    before_enabled: Option<bool>,
    #[serde(default)]
    after_enabled: Option<bool>,
    #[serde(default)]
    reverses: Option<String>,
}

fn error(code: &str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message)
}

fn default_ready_timeout() -> u16 {
    30
}
fn io_error(_: std::io::Error) -> Diagnostic {
    error(
        "service.io",
        "Native service state/definition cannot be accessed",
    )
}
fn serialize(value: &impl Serialize) -> Result<String> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|_| error("service.state", "Cannot serialize service state"))?;
    if text.len() > 8 * config::MAX_CONFIG {
        return Err(error("service.limit", "Service journal exceeds 8 MiB"));
    }
    Ok(text)
}
fn load(root: &Path) -> Result<Registry> {
    let path = root.join("services.json");
    if managed::regular(&path)?.is_none() {
        return Ok(Registry::default());
    }
    let registry: Registry = config::load(&path, config::MAX_CONFIG)?;
    if registry.services.len() > 128 {
        return Err(error("service.limit", "At most 128 managed services"));
    }
    Ok(registry)
}

pub fn spec(args: &ServiceArgs) -> Result<Spec> {
    let mut environment = BTreeMap::new();
    for item in &args.environment {
        let (key, value) = item
            .split_once('=')
            .ok_or_else(|| error("service.env", "Use --env NAME=VALUE"))?;
        if environment.insert(key.into(), value.into()).is_some() {
            return Err(error("service.env", "Duplicate environment key"));
        }
    }
    let spec = Spec {
        id: args.id.clone(),
        scope: args.scope,
        executable: args.executable.clone(),
        args: args.args.clone(),
        working_directory: args.working_directory.clone(),
        environment,
        enabled: args.enabled,
        restart: args.restart,
        ready_url: args.ready_url.clone(),
        ready_timeout: args.ready_timeout,
    };
    validate(&spec)?;
    Ok(spec)
}

pub fn validate(spec: &Spec) -> Result<()> {
    if !(1..=300).contains(&spec.ready_timeout) {
        return Err(error(
            "service.readiness",
            "Readiness timeout must be between 1 and 300 seconds",
        ));
    }
    config::resource_id(&spec.id)?;
    if !spec.executable.is_absolute()
        || !spec.executable.is_file()
        || spec.args.len() > 128
        || spec.environment.len() > 128
    {
        return Err(error(
            "service.spec",
            "Use an existing absolute executable, at most 128 arguments/environment entries",
        ));
    }
    if let Some(path) = &spec.working_directory
        && (!path.is_absolute() || !path.is_dir())
    {
        return Err(error(
            "service.directory",
            "Working directory must be an existing absolute directory",
        ));
    }
    for path in std::iter::once(&spec.executable).chain(spec.working_directory.iter()) {
        let text = path.to_string_lossy();
        if text.trim() != text || text.chars().any(char::is_control) {
            return Err(error(
                "service.path",
                "Native paths must not contain control characters or surrounding whitespace",
            ));
        }
    }
    for value in spec.args.iter().chain(spec.environment.values()) {
        if value.len() > 8192 || value.chars().any(|c| c == '\0' || c == '\n' || c == '\r') {
            return Err(error(
                "service.argument",
                "Argument/environment values must be single-line and at most 8 KiB",
            ));
        }
    }
    for key in spec.environment.keys() {
        if key.is_empty()
            || key.len() > 128
            || !key
                .bytes()
                .enumerate()
                .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || (i > 0 && b.is_ascii_digit()))
        {
            return Err(error("service.env", "Invalid environment variable name"));
        }
    }
    if let Some(url) = &spec.ready_url {
        config::http_url(url)?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn systemd_quote(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('%', "%%")
    )
}
#[cfg(target_os = "macos")]
fn xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn definition(record: &Record, root: &Path) -> Result<String> {
    let s = &record.spec;
    #[cfg(target_os = "linux")]
    {
        let mut body = format!(
            "# Steward-owned {}\n[Unit]\nDescription=Steward {}\n[Service]\nType=simple\nExecStart=:{}",
            record.label,
            s.id,
            systemd_quote(&s.executable.to_string_lossy())
        );
        for arg in &s.args {
            body.push(' ');
            body.push_str(&systemd_quote(arg));
        }
        body.push('\n');
        if let Some(path) = &s.working_directory {
            body.push_str(&format!(
                "WorkingDirectory={}\n",
                path.to_string_lossy().replace('%', "%%")
            ));
        }
        for (key, value) in &s.environment {
            body.push_str(&format!(
                "Environment={}\n",
                systemd_quote(&format!("{key}={value}"))
            ));
        }
        body.push_str(&format!(
            "Restart={}\nRestartSec=1\n[Install]\nWantedBy={}\n",
            if s.restart { "on-failure" } else { "no" },
            if s.scope == Scope::User {
                "default.target"
            } else {
                "multi-user.target"
            }
        ));
        let _ = root;
        Ok(body)
    }
    #[cfg(target_os = "macos")]
    {
        let mut body = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict><key>Label</key><string>{}</string><key>ProgramArguments</key><array><string>{}</string>",
            xml(&record.label),
            xml(&s.executable.to_string_lossy())
        );
        for arg in &s.args {
            body.push_str(&format!("<string>{}</string>", xml(arg)));
        }
        body.push_str("</array>");
        if let Some(path) = &s.working_directory {
            body.push_str(&format!(
                "<key>WorkingDirectory</key><string>{}</string>",
                xml(&path.to_string_lossy())
            ));
        }
        body.push_str("<key>EnvironmentVariables</key><dict>");
        for (key, value) in &s.environment {
            body.push_str(&format!(
                "<key>{}</key><string>{}</string>",
                xml(key),
                xml(value)
            ));
        }
        let keep_alive = if s.restart {
            "<dict><key>SuccessfulExit</key><false/></dict>"
        } else {
            "<false/>"
        };
        body.push_str(&format!("</dict><key>RunAtLoad</key><{}/><key>KeepAlive</key>{keep_alive}<key>ThrottleInterval</key><integer>1</integer><key>StandardOutPath</key><string>{}</string><key>StandardErrorPath</key><string>{}</string></dict></plist>\n",s.enabled,xml(&root.join(format!("{}.stdout.log",record.label)).to_string_lossy()),xml(&root.join(format!("{}.stderr.log",record.label)).to_string_lossy())));
        Ok(body)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (record, root);
        Err(error(
            "capability.platform",
            "Native services support Linux/systemd and macOS/launchd",
        ))
    }
}

fn record(root: &Path, owner: &str, spec: Spec) -> Result<Record> {
    let label = format!("steward-{owner}-{}", spec.id);
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| error("service.home", "HOME is required"))?;
    #[cfg(target_os = "linux")]
    let path = if spec.scope == Scope::User {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"))
            .join("systemd/user")
            .join(format!("{label}.service"))
    } else {
        PathBuf::from("/etc/systemd/system").join(format!("{label}.service"))
    };
    #[cfg(target_os = "macos")]
    let path = if spec.scope == Scope::User {
        home.join("Library/LaunchAgents")
            .join(format!("{label}.plist"))
    } else {
        PathBuf::from("/Library/LaunchDaemons").join(format!("{label}.plist"))
    };
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let path = root.join(format!("{label}.unsupported"));
    let mut r = Record {
        spec,
        path,
        label,
        fingerprint: String::new(),
    };
    if !r.path.is_absolute() {
        return Err(error(
            "service.path",
            "Native configuration directory must be absolute",
        ));
    }
    r.fingerprint = transport::fingerprint(&definition(&r, root)?);
    Ok(r)
}

fn manager(record: &Record, args: &[&str]) -> Result<executor::Output> {
    #[cfg(target_os = "linux")]
    {
        let mut command = Command::new("systemctl");
        if record.spec.scope == Scope::User {
            command.arg("--user");
        }
        command.args(args);
        executor::run(&mut command, Duration::from_secs(30), 1024 * 1024)
    }
    #[cfg(target_os = "macos")]
    {
        let _ = record;
        executor::run(
            Command::new("launchctl").args(args),
            Duration::from_secs(30),
            1024 * 1024,
        )
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (record, args);
        Err(error(
            "capability.platform",
            "Native service manager is unavailable",
        ))
    }
}

#[cfg(target_os = "macos")]
fn domain(record: &Record) -> String {
    if record.spec.scope == Scope::System {
        "system".into()
    } else {
        format!("gui/{}", unsafe { libc::geteuid() })
    }
}

fn running(record: &Record) -> Result<bool> {
    #[cfg(target_os = "linux")]
    {
        let result = manager(
            record,
            &[
                "show",
                "--property=ActiveState",
                "--value",
                &format!("{}.service", record.label),
            ],
        )?;
        if !result.status.success() {
            return Err(error("service.manager", "systemd scope unavailable"));
        }
        Ok(String::from_utf8_lossy(&result.stdout).trim() == "active")
    }
    #[cfg(target_os = "macos")]
    {
        let result = manager(
            record,
            &["print", &format!("{}/{}", domain(record), record.label)],
        )?;
        Ok(result.status.success()
            && String::from_utf8_lossy(&result.stdout)
                .lines()
                .map(str::trim)
                .find(|line| line.starts_with("state = "))
                == Some("state = running"))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = record;
        Err(error(
            "capability.platform",
            "Unsupported native service manager",
        ))
    }
}

fn control(record: &Record, action: &str) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        let unit = format!("{}.service", record.label);
        let result = if action == "reload" {
            manager(record, &["daemon-reload"])?
        } else {
            manager(record, &[action, &unit])?
        };
        if !result.status.success() {
            return Err(error(
                "service.manager",
                &format!(
                    "Native service action {action} failed ({}); inspect service status/logs",
                    result.status
                ),
            ));
        }
    }
    #[cfg(target_os = "macos")]
    {
        let target = format!("{}/{}", domain(record), record.label);
        let path = record
            .path
            .to_str()
            .ok_or_else(|| error("service.path", "Native definition path must be UTF-8"))?;
        let result = match action {
            "reload" => return Ok(()),
            "start" | "restart" => {
                if !manager(record, &["enable", &target])?.status.success() {
                    return Err(error(
                        "service.enablement",
                        "Could not enable the explicitly requested job",
                    ));
                }
                if !manager(record, &["print", &target])?.status.success()
                    && !manager(record, &["bootstrap", &domain(record), path])?
                        .status
                        .success()
                {
                    return Err(error("service.manager", "launchd bootstrap failed"));
                }
                if action == "restart" {
                    manager(record, &["kickstart", "-k", &target])?
                } else {
                    manager(record, &["kickstart", &target])?
                }
            }
            "stop" => {
                let result = manager(record, &["bootout", &target])?;
                if !result.status.success()
                    && !manager(record, &["print", &target])?.status.success()
                {
                    return Ok(());
                }
                result
            }
            "enable" | "disable" => manager(record, &[action, &target])?,
            _ => return Err(error("service.action", "Unknown native action")),
        };
        if !result.status.success() {
            return Err(error(
                "service.manager",
                "launchd action failed; inspect service status/logs",
            ));
        }
        if matches!(action, "start" | "restart") && !record.spec.enabled {
            // launchd startup is asynchronous. Restore the persistent disabled
            // override only after the explicitly requested process is ready.
            verify_running(record, true)?;
            if !manager(record, &["disable", &target])?.status.success() {
                return Err(error(
                    "service.enablement",
                    "Could not restore disabled-at-login intent",
                ));
            }
        }
    }
    Ok(())
}

fn verify_running(record: &Record, expected: bool) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(u64::from(record.spec.ready_timeout));
    while running(record)? != expected {
        if Instant::now() > deadline {
            return Err(error(
                "service.activation",
                "Native service did not reach the requested running state",
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    if expected && let Some(url) = &record.spec.ready_url {
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .tls_built_in_root_certs(config::http_url(url)?.scheme() == "https")
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(2))
            .build()
            .map_err(|_| error("service.readiness", "Could not create readiness client"))?;
        loop {
            let response = client.get(url).send();
            if response.as_ref().is_ok_and(|r| r.status().is_success()) {
                break;
            }
            let failure = match response {
                Ok(r) => format!("HTTP {}", r.status().as_u16()),
                Err(e) => {
                    if e.is_timeout() {
                        "request timeout".into()
                    } else if e.is_connect() {
                        "connection failure".into()
                    } else {
                        "request failed".into()
                    }
                }
            };
            if Instant::now() > deadline {
                return Err(error(
                    "service.readiness",
                    &format!(
                        "Service readiness did not return HTTP 2xx before timeout ({failure}); native observation: {}",
                        native_inventory(record.spec.scope, Some(&record.label))
                            .unwrap_or(Value::Null)
                    ),
                ));
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    Ok(())
}

fn enabled(record: &Record) -> Result<Option<bool>> {
    if !record.path.exists() {
        return Ok(None);
    }
    #[cfg(target_os = "linux")]
    {
        let result = manager(
            record,
            &["is-enabled", &format!("{}.service", record.label)],
        )?;
        Ok(match String::from_utf8_lossy(&result.stdout).trim() {
            "enabled" | "enabled-runtime" => Some(true),
            "disabled" => Some(false),
            _ => None,
        })
    }
    #[cfg(target_os = "macos")]
    {
        let result = manager(record, &["print-disabled", &domain(record)])?;
        if !result.status.success() {
            return Err(error(
                "service.enablement",
                "Could not inspect launchd enablement",
            ));
        }
        let text = String::from_utf8_lossy(&result.stdout);
        let disabled = format!("\"{}\" => true", record.label);
        Ok(Some(!text.contains(&disabled)))
    }
}

fn verify_definition(record: &Record) -> Result<String> {
    managed::regular(&record.path)?
        .ok_or_else(|| error("service.drift", "Owned native definition is missing"))?;
    let body = String::from_utf8(config::read_bounded(&record.path, config::MAX_CONFIG)?)
        .map_err(|_| error("service.encoding", "Definition must be UTF-8"))?;
    if transport::fingerprint(&body) != record.fingerprint {
        return Err(error(
            "service.drift",
            "Native definition changed outside Steward",
        ));
    }
    Ok(body)
}

fn record_journal(root: &Path, j: &Journal) -> Result<()> {
    let directory = root.join("service-operations");
    fs::create_dir_all(&directory).map_err(io_error)?;
    // Bounded directory, explicit archival rather than silent deletion.
    if !directory.join(format!("{}.json", j.id)).exists()
        && fs::read_dir(&directory)
            .map_err(io_error)?
            .take(1024)
            .count()
            >= 1024
    {
        return Err(error(
            "service.history_limit",
            "Archive service operation history before adding more entries",
        ));
    }
    let value = serialize(j)?;
    let mut total = 0u64;
    for entry in fs::read_dir(&directory).map_err(io_error)?.take(1025) {
        let path = entry.map_err(io_error)?.path();
        if path.file_stem().and_then(|s| s.to_str()) != Some(&j.id) {
            total += managed::regular(&path)?
                .ok_or_else(|| error("service.history", "History entry disappeared"))?
                .len();
        }
    }
    if total + value.len() as u64 > 256 * 1024 * 1024 {
        return Err(error(
            "service.history_limit",
            "Archive service history before exceeding 256 MiB",
        ));
    }
    managed::publish(&root.join("service-operation.json"), Some(&value))?;
    managed::publish(&directory.join(format!("{}.json", j.id)), Some(&value))
}

fn restore(root: &Path, j: &mut Journal) -> Result<()> {
    let current = if managed::regular(&j.record.path)?.is_some() {
        Some(
            String::from_utf8(config::read_bounded(&j.record.path, config::MAX_CONFIG)?)
                .map_err(|_| error("service.encoding", "Invalid definition"))?,
        )
    } else {
        None
    };
    if current != j.before && current != j.after {
        return Err(error("service.drift", "Definition drift blocks recovery"));
    }
    if running(&j.record)? {
        control(&j.record, "stop")?;
    }
    managed::publish(&j.record.path, j.before.as_deref())?;
    control(&j.record, "reload")?;
    if let Some(old) = j
        .old
        .services
        .iter()
        .find(|r| r.spec.id == j.record.spec.id)
    {
        let mut restored = old.clone();
        restored.spec.enabled = j.before_enabled.unwrap_or(old.spec.enabled);
        control(
            &restored,
            if restored.spec.enabled {
                "enable"
            } else {
                "disable"
            },
        )?;
        if j.was_running {
            control(&restored, "start")?;
            verify_running(&restored, true)?;
        }
    }
    managed::publish(&root.join("services.json"), Some(&serialize(&j.old)?))?;
    j.phase = "compensated".into();
    record_journal(root, j)
}

type Checkpoint<'a> = &'a mut dyn FnMut(&str) -> Result<()>;

pub fn run(root: PathBuf, command: &ServiceCommand) -> Result<(Value, bool)> {
    if let ServiceCommand::Discover { scope } = command {
        return Ok((native_inventory(*scope, None)?, false));
    }
    if let ServiceCommand::InspectNative { name, scope } = command {
        return Ok((native_inventory(*scope, Some(name))?, false));
    }
    run_internal(root, command, None)
}

fn native_inventory(scope: Scope, name: Option<&str>) -> Result<Value> {
    if let Some(name) = name
        && (name.is_empty()
            || name.len() > 256
            || name.starts_with('-')
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.@:-".contains(&b)))
    {
        return Err(error("service.native_id", "Invalid native service name"));
    }
    #[cfg(target_os = "linux")]
    {
        let mut cmd = Command::new("systemctl");
        if scope == Scope::User {
            cmd.arg("--user");
        }
        if let Some(name) = name {
            cmd.args([
                "show",
                "--property=Id,LoadState,ActiveState,SubState,UnitFileState",
                "--",
                name,
            ]);
            let result = executor::run(&mut cmd, Duration::from_secs(10), 1024 * 1024)?;
            if !result.status.success() {
                return Err(error(
                    "service.native_unavailable",
                    "Native service inspection failed",
                ));
            }
            let fields = String::from_utf8_lossy(&result.stdout)
                .lines()
                .filter_map(|line| {
                    line.split_once('=')
                        .map(|(k, v)| (k.to_owned(), v.to_owned()))
                })
                .collect::<BTreeMap<_, _>>();
            Ok(json!({"native":fields,"ownership":"external","scope":scope}))
        } else {
            cmd.args([
                "list-units",
                "--type=service",
                "--all",
                "--no-pager",
                "--output=json",
            ]);
            let result = executor::run(&mut cmd, Duration::from_secs(10), 4 * 1024 * 1024)?;
            if !result.status.success() {
                return Err(error(
                    "service.native_unavailable",
                    "Native service enumeration failed",
                ));
            }
            let native = config::strict_json(&result.stdout)?;
            Ok(json!({"native":native,"ownership":"external","scope":scope}))
        }
    }
    #[cfg(target_os = "macos")]
    {
        if scope == Scope::System && unsafe { libc::geteuid() } != 0 {
            return Err(error(
                "service.privilege",
                "System launchd discovery requires a root invocation",
            ));
        }
        if let Some(name) = name {
            let domain = if scope == Scope::System {
                "system".into()
            } else {
                format!("gui/{}", unsafe { libc::geteuid() })
            };
            let result = executor::run(
                Command::new("launchctl").args(["print", &format!("{domain}/{name}")]),
                Duration::from_secs(10),
                1024 * 1024,
            )?;
            if !result.status.success() {
                return Err(error(
                    "service.native_unavailable",
                    "Native service inspection failed",
                ));
            }
            let mut fields = String::from_utf8_lossy(&result.stdout)
                .lines()
                .filter_map(|line| {
                    let (k, v) = line.trim().split_once(" = ")?;
                    if matches!(k, "state" | "pid" | "last exit code") {
                        Some((k.to_owned(), v.to_owned()))
                    } else {
                        None
                    }
                })
                .fold(BTreeMap::new(), |mut fields, (key, value)| {
                    fields.entry(key).or_insert(value);
                    fields
                });
            if let Some(pid) = fields
                .get("pid")
                .filter(|p| p.bytes().all(|b| b.is_ascii_digit()))
                && let Ok(process) = executor::run(
                    Command::new("ps").args(["-p", pid, "-o", "comm=,state="]),
                    Duration::from_secs(5),
                    4096,
                )
            {
                fields.insert(
                    "process".into(),
                    String::from_utf8_lossy(&process.stdout).trim().into(),
                );
            }
            Ok(json!({"name":name,"native":fields,"ownership":"external","scope":scope}))
        } else {
            let result = executor::run(
                Command::new("launchctl").arg("list"),
                Duration::from_secs(10),
                1024 * 1024,
            )?;
            if !result.status.success() {
                return Err(error(
                    "service.native_unavailable",
                    "Native service enumeration failed",
                ));
            }
            let rows = String::from_utf8_lossy(&result.stdout)
                .lines()
                .skip(1)
                .take(10000)
                .filter_map(|line| {
                    let p = line.split_whitespace().collect::<Vec<_>>();
                    if p.len() == 3 {
                        Some(json!({"pid":p[0],"last_exit":p[1],"name":p[2]}))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            Ok(json!({"native":rows,"ownership":"external","scope":scope}))
        }
    }
}

pub fn run_tracked(
    root: PathBuf,
    command: &ServiceCommand,
    checkpoint: &mut dyn FnMut(&str) -> Result<()>,
) -> Result<(Value, bool)> {
    run_internal(root, command, Some(checkpoint))
}

fn run_internal(
    root: PathBuf,
    command: &ServiceCommand,
    checkpoint: Option<Checkpoint<'_>>,
) -> Result<(Value, bool)> {
    let store = Store::open(root.clone())?;
    let registry = load(&root)?;
    let pending_path = root.join("service-operation.json");
    let pending = if managed::regular(&pending_path)?.is_some() {
        Some(config::load::<Journal>(
            &pending_path,
            8 * config::MAX_CONFIG,
        )?)
    } else {
        None
    };
    if let ServiceCommand::Rollback { id } = command {
        if pending
            .as_ref()
            .is_some_and(|j| !matches!(j.phase.as_str(), "verified" | "compensated"))
        {
            return Err(error(
                "service.pending",
                "Recover the unfinished service operation first",
            ));
        }
        if id.len() != 16 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(error("service.operation", "Invalid operation ID"));
        }
        let mut original: Journal = config::load(
            &root.join("service-operations").join(format!("{id}.json")),
            8 * config::MAX_CONFIG,
        )?;
        if original.phase != "verified"
            || crate::transport::fingerprint(&registry)
                != crate::transport::fingerprint(&original.next)
            || Some(running(&original.record)?) != original.after_running
            || enabled(&original.record)? != original.after_enabled
        {
            return Err(error(
                "service.drift",
                "Service registry/runtime differs from the verified operation",
            ));
        }
        let current = if managed::regular(&original.record.path)?.is_some() {
            Some(
                String::from_utf8(config::read_bounded(
                    &original.record.path,
                    config::MAX_CONFIG,
                )?)
                .map_err(|_| error("service.encoding", "Invalid service definition"))?,
            )
        } else {
            None
        };
        if current != original.after {
            return Err(error(
                "service.drift",
                "Definition changed since the operation",
            ));
        }
        let reverse = transport::fingerprint(&(
            id,
            transport::now(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
        ));
        original.id = reverse[..16].into();
        original.reverses = Some(id.clone());
        original.phase = "prepared".into();
        record_journal(&root, &original)?;
        restore(&root, &mut original).map_err(|e| e.with_changes())?;
        return Ok((
            json!({"operation":original.id,"reverses":id,"phase":"compensated"}),
            true,
        ));
    }
    if matches!(command, ServiceCommand::Recover) {
        let mut j =
            pending.ok_or_else(|| error("service.operation", "No service operation exists"))?;
        if matches!(j.phase.as_str(), "verified" | "compensated") {
            return Err(error("service.operation", "Operation already terminal"));
        }
        restore(&root, &mut j).map_err(|e| e.with_changes())?;
        return Ok((json!({"operation":j.id,"phase":j.phase}), true));
    }
    if matches!(command, ServiceCommand::List) {
        let rows=registry.services.iter().map(|r|json!({"id":r.spec.id,"scope":r.spec.scope,"enabled":r.spec.enabled,"running":running(r).ok(),"native_id":r.label,"ownership":"owned"})).collect::<Vec<_>>();
        return Ok((json!(rows), false));
    }
    let id = match command {
        ServiceCommand::Add(a) | ServiceCommand::Update(a) => &a.id,
        ServiceCommand::Inspect { id }
        | ServiceCommand::Start { id }
        | ServiceCommand::Stop { id }
        | ServiceCommand::Restart { id }
        | ServiceCommand::Enable { id }
        | ServiceCommand::Disable { id }
        | ServiceCommand::Remove { id }
        | ServiceCommand::Logs { id, .. } => id,
        _ => unreachable!(),
    };
    config::resource_id(id)?;
    let old = registry.services.iter().find(|r| r.spec.id == *id).cloned();
    if let ServiceCommand::Inspect { .. } = command {
        let r = old.ok_or_else(|| {
            error(
                "service.missing",
                "Service is not owned by this state directory",
            )
        })?;
        verify_definition(&r)?;
        return Ok((
            json!({"id":id,"scope":r.spec.scope,"native_id":r.label,"enabled":r.spec.enabled,"native_enabled":enabled(&r)?,"running":running(&r)?,"executable":r.spec.executable,"working_directory":r.spec.working_directory,"ready_url":r.spec.ready_url}),
            false,
        ));
    }
    if let ServiceCommand::Logs { lines, .. } = command {
        let r = old.ok_or_else(|| error("service.missing", "Service is not owned"))?;
        return Ok((logs(&root, &r, *lines)?, false));
    }
    if pending.is_some_and(|j| !matches!(j.phase.as_str(), "verified" | "compensated")) {
        return Err(error(
            "service.pending",
            "Recover the unfinished service operation first",
        ));
    }
    let record = match command {
        ServiceCommand::Add(a) => {
            if old.is_some() || registry.services.len() >= 128 {
                return Err(error(
                    "service.exists",
                    "Service exists or service limit reached",
                ));
            }
            record(&root, &store.book.owner, spec(a)?)?
        }
        ServiceCommand::Update(a) => {
            let previous = old
                .as_ref()
                .ok_or_else(|| error("service.missing", "Service is not owned"))?;
            if a.scope != previous.spec.scope {
                return Err(error(
                    "service.scope",
                    "Remove the existing service before changing its scope",
                ));
            }
            record(&root, &store.book.owner, spec(a)?)?
        }
        _ => old.clone().ok_or_else(|| {
            error(
                "service.missing",
                "Service is not owned by this state directory",
            )
        })?,
    };
    let before = if let Some(old) = &old {
        Some(verify_definition(old)?)
    } else {
        if managed::regular(&record.path)?.is_some() {
            return Err(error(
                "service.external",
                "Native definition path already belongs to an external service",
            ));
        }
        None
    };
    let was_running = if old.is_some() {
        running(&record)?
    } else {
        false
    };
    let before_enabled = if old.is_some() {
        enabled(&record)?
    } else {
        None
    };
    if record.spec.scope == Scope::System && unsafe { libc::geteuid() } != 0 {
        return Err(error(
            "service.privilege",
            "System-scope services require an explicitly elevated invocation",
        ));
    }
    if !root.join("state.json").exists() {
        managed::publish(&root.join("state.json"), Some(&serialize(&store.book)?))?;
    }
    let mut next = registry.clone();
    let mut desired = record.clone();
    if matches!(command, ServiceCommand::Enable { .. }) {
        desired.spec.enabled = true;
    }
    if matches!(command, ServiceCommand::Disable { .. }) {
        desired.spec.enabled = false;
    }
    let after = if matches!(command, ServiceCommand::Remove { .. }) {
        None
    } else {
        Some(definition(&desired, &root)?)
    };
    if let Some(body) = &after {
        desired.fingerprint = transport::fingerprint(body);
    }
    next.services.retain(|r| r.spec.id != *id);
    if after.is_some() {
        next.services.push(desired.clone());
    }
    if let Some(parent) = desired.path.parent() {
        fs::create_dir_all(parent).map_err(io_error)?;
    }
    let op = transport::fingerprint(&(
        transport::now(),
        id,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    ));
    let mut journal = Journal {
        id: op[..16].into(),
        phase: "prepared".into(),
        record: desired.clone(),
        before,
        after,
        old: registry,
        next,
        was_running,
        after_running: None,
        before_enabled,
        after_enabled: None,
        reverses: None,
    };
    journal.next.revision = journal.id.clone();
    if serialize(&journal.next)?.len() > config::MAX_CONFIG
        || journal
            .after
            .as_ref()
            .is_some_and(|s| s.len() > config::MAX_CONFIG)
    {
        return Err(error(
            "service.limit",
            "Service registry/definition exceeds 1 MiB",
        ));
    }
    if let Some(checkpoint) = checkpoint {
        checkpoint(&journal.id)?;
    }
    record_journal(&root, &journal)?;
    let result = (|| {
        journal.phase = "publishing".into();
        record_journal(&root, &journal)?;
        if matches!(
            command,
            ServiceCommand::Update(_) | ServiceCommand::Remove { .. }
        ) && was_running
        {
            control(&record, "stop")?;
        }
        if matches!(command, ServiceCommand::Remove { .. }) {
            control(&record, "disable")?;
        }
        managed::publish(&desired.path, journal.after.as_deref())?;
        control(&desired, "reload")?;
        match command {
            ServiceCommand::Start { .. } | ServiceCommand::Restart { .. } => {
                if matches!(command, ServiceCommand::Restart { .. }) {
                    control(&desired, "restart")?;
                } else if !was_running {
                    control(&desired, "start")?;
                }
                verify_running(&desired, true)?;
            }
            ServiceCommand::Stop { .. } => {
                control(&desired, "stop")?;
                verify_running(&desired, false)?;
            }
            ServiceCommand::Add(a) | ServiceCommand::Update(a) => {
                control(
                    &desired,
                    if desired.spec.enabled {
                        "enable"
                    } else {
                        "disable"
                    },
                )?;
                if a.start || was_running {
                    control(&desired, "start")?;
                    verify_running(&desired, true)?;
                }
            }
            ServiceCommand::Enable { .. } | ServiceCommand::Disable { .. } => control(
                &desired,
                if desired.spec.enabled {
                    "enable"
                } else {
                    "disable"
                },
            )?,
            ServiceCommand::Remove { .. } => verify_running(&record, false)?,
            _ => unreachable!(),
        }
        managed::publish(
            &root.join("services.json"),
            Some(&serialize(&journal.next)?),
        )?;
        journal.after_running = Some(running(&desired)?);
        journal.after_enabled = enabled(&desired)?;
        journal.phase = "verified".into();
        record_journal(&root, &journal)
    })();
    if let Err(failure) = result {
        if restore(&root, &mut journal).is_ok() {
            return Err(error(
                "service.compensated",
                &format!(
                    "Native action failed ({}); preceding definition/state restored",
                    failure.message
                ),
            ));
        }
        return Err(error(
            "service.recovery_required",
            "Native service action could not be verified or restored; run services recover",
        )
        .at(journal.id)
        .with_changes());
    }
    Ok((
        json!({"id":id,"operation":journal.id,"phase":"verified","enabled":desired.spec.enabled,"running":running(&desired).ok()}),
        true,
    ))
}

fn logs(root: &Path, record: &Record, lines: u16) -> Result<Value> {
    #[cfg(target_os = "linux")]
    {
        let mut cmd = Command::new("journalctl");
        if record.spec.scope == Scope::User {
            cmd.arg("--user");
        }
        cmd.args([
            "--no-pager",
            "--output=cat",
            "--lines",
            &lines.to_string(),
            "--unit",
            &format!("{}.service", record.label),
        ]);
        let output = executor::run(&mut cmd, Duration::from_secs(10), 1024 * 1024)?;
        if !output.status.success() {
            return Err(error("service.logs", "Native log query failed"));
        }
        let _ = root;
        Ok(json!({"text":String::from_utf8_lossy(&output.stdout),"lines_requested":lines}))
    }
    #[cfg(target_os = "macos")]
    {
        use std::io::{Read, Seek, SeekFrom};
        let mut result = Vec::new();
        for kind in ["stdout", "stderr"] {
            let path = root.join(format!("{}.{kind}.log", record.label));
            if !path.exists() {
                continue;
            }
            managed::regular(&path)?;
            let mut file = fs::File::open(path).map_err(io_error)?;
            let length = file.metadata().map_err(io_error)?.len();
            file.seek(SeekFrom::Start(length.saturating_sub(128 * 1024)))
                .map_err(io_error)?;
            let mut text = String::new();
            file.take(128 * 1024)
                .read_to_string(&mut text)
                .map_err(io_error)?;
            let rows: Vec<_> = text.lines().rev().take(lines as usize).collect();
            result.push(
                json!({"stream":kind,"text":rows.into_iter().rev().collect::<Vec<_>>().join("\n")}),
            );
        }
        Ok(json!(result))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (root, record, lines);
        Err(error("capability.platform", "Native logs unavailable"))
    }
}

pub fn saved_specs(root: &Path) -> Result<Vec<Spec>> {
    Ok(load(root)?.services.into_iter().map(|r| r.spec).collect())
}

pub fn operation_summaries(root: &Path) -> Result<Vec<Value>> {
    let active_path = root.join("service-operation.json");
    let active = if managed::regular(&active_path)?.is_some() {
        Some(config::load::<Journal>(
            &active_path,
            8 * config::MAX_CONFIG,
        )?)
    } else {
        None
    };
    let summary = |j: &Journal| json!({"kind":"service","id":j.id,"service":j.record.spec.id,"phase":j.phase,"reverses":j.reverses});
    let mut output = active.as_ref().map_or_else(Vec::new, |j| vec![summary(j)]);
    let directory = root.join("service-operations");
    if !directory.exists() {
        return Ok(output);
    }
    if fs::symlink_metadata(&directory)
        .map_err(io_error)?
        .file_type()
        .is_symlink()
    {
        return Err(error(
            "service.history",
            "History directory must not be a symlink",
        ));
    }
    let mut count = 0;
    for entry in fs::read_dir(directory).map_err(io_error)? {
        count += 1;
        if count > 1024 {
            return Err(error(
                "service.history_limit",
                "Service history exceeds 1024 entries",
            ));
        }
        let path = entry.map_err(io_error)?.path();
        if active
            .as_ref()
            .is_some_and(|j| path.file_stem().and_then(|s| s.to_str()) == Some(&j.id))
        {
            continue;
        }
        managed::regular(&path)?;
        let j: Journal = config::load(&path, 8 * config::MAX_CONFIG)?;
        output.push(summary(&j));
    }
    Ok(output)
}
