mod config;
mod error;
mod inventory;
mod managed;
mod model;
mod plan;
mod rules;
#[cfg(test)]
mod tests;
mod transport;

use std::{
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};

use clap::{Args, CommandFactory, Parser, Subcommand};
use serde::Serialize;
use serde_json::{Value, json};

use error::{Diagnostic, Result};
use model::{Binding, Engine, Inventory, RouteSpec, TlsMode, TlsPolicy, Upstream};

#[derive(Parser)]
#[command(
    version,
    about = "Inspect proxies and manage explicitly connected HTTP routes",
    long_about = "Steward: native Caddy/Traefik inventory, route previews, and recoverable HTTP route changes through explicitly connected local file-backed proxies. General native services, TLS setup, Docker-controller writes, and TUI remain planned."
)]
struct Cli {
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[arg(
        long,
        global = true,
        help = "Managed connections, receipts, and journal directory"
    )]
    state_dir: Option<PathBuf>,
    #[arg(
        long,
        global = true,
        help = "Write one versioned JSON result to stdout"
    )]
    json: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    #[command(name = "proxies", visible_alias = "proxy", subcommand)]
    Proxy(ProxyCommand),
    #[command(name = "routes", visible_alias = "route", subcommand)]
    Route(RouteCommand),
    #[command(about = "Generate a shell completion script without loading configuration")]
    Completions {
        shell: clap_complete::Shell,
    },
    #[command(subcommand)]
    Operations(OperationCommand),
    Doctor,
    Plan {
        #[arg(long)]
        file: PathBuf,
    },
    Apply {
        #[arg(long)]
        plan: Option<PathBuf>,
        #[arg(long)]
        yes: bool,
    },
    #[command(subcommand)]
    Service(ServiceCommand),
}

#[derive(Subcommand)]
enum ProxyCommand {
    List,
    Inspect {
        id: String,
    },
    #[command(
        about = "Connect an authorized API; optional file authority enables local HTTP writes"
    )]
    Add(ConnectArgs),
}

#[derive(Args)]
struct ConnectArgs {
    id: String,
    #[arg(long, value_enum)]
    engine: Engine,
    #[arg(long)]
    endpoint: String,
    #[arg(
        long,
        requires = "listener",
        help = "Complete JSON startup file for Caddy, or YAML/JSON static startup file for Traefik"
    )]
    config_file: Option<PathBuf>,
    #[arg(
        long,
        requires = "config_file",
        help = "Caddy server name, or Traefik entrypoint name"
    )]
    listener: Option<String>,
    #[arg(
        long,
        requires = "config_file",
        help = "Existing watched directory (Traefik only)"
    )]
    directory: Option<PathBuf>,
    #[arg(
        long,
        default_value = "caddy",
        help = "Same-environment Caddy validator executable"
    )]
    caddy_bin: PathBuf,
    #[arg(long)]
    auth_file: Option<PathBuf>,
}
#[derive(Subcommand)]
enum RouteCommand {
    List,
    Inspect {
        #[arg(long)]
        proxy: String,
        id: String,
    },
    #[command(about = "Create and activate an owned HTTP route")]
    Add {
        id: String,
        #[arg(long)]
        proxy: String,
        #[arg(long = "host")]
        hostname: String,
        #[arg(long)]
        upstream: String,
        #[arg(
            long,
            value_enum,
            help = "Explicit frontend policy; writable mode currently supports http"
        )]
        tls: TlsMode,
    },
    #[command(about = "Update an owned route and verify activation")]
    Update {
        id: String,
        #[arg(long)]
        proxy: String,
        #[arg(long = "host", required_unless_present = "upstream")]
        hostname: Option<String>,
        #[arg(long, required_unless_present = "hostname")]
        upstream: Option<String>,
    },
    Disable {
        id: String,
        #[arg(long)]
        proxy: String,
    },
    Enable {
        id: String,
        #[arg(long)]
        proxy: String,
    },
    #[command(visible_alias = "rm")]
    Remove {
        id: String,
        #[arg(long)]
        proxy: String,
    },
}

#[derive(Subcommand)]
enum OperationCommand {
    List,
    #[command(
        about = "Compensate an unfinished write while input/runtime fingerprints still match"
    )]
    Recover {
        id: String,
    },
}
#[derive(Subcommand)]
enum ServiceCommand {
    List,
}

#[derive(Serialize)]
struct Envelope {
    schema_version: &'static str,
    status: &'static str,
    changed: bool,
    data: Value,
    diagnostics: Vec<Diagnostic>,
}

fn run(cli: &Cli) -> Result<Envelope> {
    // Capability-only paths are independent: no configuration load, network client,
    // proxy probing, or Apptrail process starts for unavailable service/apply commands.
    if matches!(cli.command, Some(Command::Apply { .. })) {
        return Err(Diagnostic::new(
            "capability.apply_unavailable",
            "Apply is not implemented; no plan was loaded and no mutation was attempted",
        ));
    }
    if matches!(cli.command, Some(Command::Service(_))) {
        return Err(Diagnostic::new(
            "capability.unsupported",
            "General native service management is a later milestone; no proxy or Docker connection was initialized",
        ));
    }
    let managed_root = || {
        cli.state_dir
            .clone()
            .map(Ok)
            .unwrap_or_else(managed::default_root)
    };
    let mutation = match &cli.command {
        Some(Command::Route(RouteCommand::Add {
            id,
            proxy,
            hostname,
            upstream,
            tls,
        })) => Some((
            proxy,
            managed::Edit::Add(RouteSpec {
                id: id.clone(),
                proxy: proxy.clone(),
                hostname: hostname.clone(),
                upstream: Upstream {
                    url: upstream.clone(),
                },
                enabled: true,
                tls: TlsPolicy { mode: tls.clone() },
            }),
        )),
        Some(Command::Route(RouteCommand::Update {
            id,
            proxy,
            hostname,
            upstream,
        })) => Some((
            proxy,
            managed::Edit::Set {
                id: id.clone(),
                hostname: hostname.clone(),
                upstream: upstream.clone(),
                enabled: None,
            },
        )),
        Some(Command::Route(RouteCommand::Disable { id, proxy })) => Some((
            proxy,
            managed::Edit::Set {
                id: id.clone(),
                hostname: None,
                upstream: None,
                enabled: Some(false),
            },
        )),
        Some(Command::Route(RouteCommand::Enable { id, proxy })) => Some((
            proxy,
            managed::Edit::Set {
                id: id.clone(),
                hostname: None,
                upstream: None,
                enabled: Some(true),
            },
        )),
        Some(Command::Route(RouteCommand::Remove { id, proxy })) => {
            Some((proxy, managed::Edit::Remove(id.clone())))
        }
        _ => None,
    };
    if mutation.is_some()
        || matches!(
            cli.command,
            Some(Command::Proxy(ProxyCommand::Add(_))) | Some(Command::Operations(_))
        )
    {
        if cli.config.is_some() {
            return Err(Diagnostic::new(
                "cli.managed_state",
                "Direct writes use stored connections; omit --config and use proxies add",
            ));
        }
        let mut store = managed::Store::open(managed_root()?)?;
        let (data, changed, status) = if let Some((proxy, edit)) = mutation {
            (store.edit(proxy, edit)?, true, "applied")
        } else {
            match &cli.command {
                Some(Command::Proxy(ProxyCommand::Add(args))) => {
                    let authority = if let Some(file) = &args.config_file {
                        match args.engine {
                            Engine::Caddy => {
                                if args.directory.is_some() {
                                    return Err(Diagnostic::new(
                                        "cli.authority",
                                        "--directory is for Traefik",
                                    ));
                                }
                                managed::Authority::CaddyJson {
                                    file: file.clone(),
                                    server: args
                                        .listener
                                        .clone()
                                        .expect("required with config-file"),
                                    binary: managed::executable(&args.caddy_bin)?,
                                }
                            }
                            Engine::Traefik => managed::Authority::TraefikDirectory {
                                startup: file.clone(),
                                entrypoint: args
                                    .listener
                                    .clone()
                                    .expect("required with config-file"),
                                directory: args.directory.clone().ok_or_else(|| {
                                    Diagnostic::new("cli.authority", "Traefik requires --directory")
                                })?,
                            },
                        }
                    } else {
                        managed::Authority::ReadOnly
                    };
                    let connection = managed::Connection {
                        binding: Binding {
                            id: args.id.clone(),
                            engine: args.engine,
                            endpoint: args.endpoint.clone(),
                            auth_file: args
                                .auth_file
                                .as_ref()
                                .map(|p| p.to_string_lossy().into_owned()),
                            snapshot_file: None,
                        },
                        authority,
                    };
                    (store.connect(connection)?, true, "connected")
                }
                Some(Command::Operations(OperationCommand::List)) => {
                    (store.operations()?, false, "observed")
                }
                Some(Command::Operations(OperationCommand::Recover { id })) => {
                    (store.recover(id)?, true, "compensated")
                }
                _ => unreachable!(),
            }
        };
        return Ok(Envelope {
            schema_version: "1",
            status,
            changed,
            data,
            diagnostics: Vec::new(),
        });
    }
    let manifest = if let Some(Command::Plan { file }) = &cli.command {
        Some(config::load_manifest(file)?)
    } else {
        None
    };
    let store = if cli.config.is_none() {
        Some(managed::Store::open(managed_root()?)?)
    } else {
        None
    };
    let mut config = if let Some(path) = &cli.config {
        config::load_config(path)?
    } else {
        store.as_ref().expect("managed read").config()
    };
    // Snapshot paths are relative to the profile, not the invoking shell's directory.
    for binding in &mut config.proxies {
        if let Some(snapshot) = &binding.snapshot_file
            && !std::path::Path::new(snapshot).is_absolute()
        {
            binding.snapshot_file = Some(
                cli.config
                    .as_ref()
                    .expect("snapshot profile")
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("."))
                    .join(snapshot)
                    .to_string_lossy()
                    .into_owned(),
            );
        }
    }
    if let Some(manifest) = &manifest {
        if manifest.scope != config.scope {
            return Err(Diagnostic::new(
                "scope.mismatch",
                "Plan and binding configuration must use the same management scope",
            ));
        }
        for route in &manifest.routes {
            if !config.proxies.iter().any(|p| p.id == route.proxy) {
                return Err(Diagnostic::new(
                    "binding.missing",
                    "Requested proxy binding is not configured",
                )
                .at(&route.id));
            }
        }
    }
    let requested = match &cli.command {
        Some(Command::Proxy(ProxyCommand::Inspect { id })) => Some(id.as_str()),
        Some(Command::Route(RouteCommand::Inspect { proxy, .. })) => Some(proxy.as_str()),
        _ => None,
    };
    if requested.is_some_and(|id| !config.proxies.iter().any(|p| p.id == id)) {
        return Err(Diagnostic::new(
            "binding.missing",
            "Requested proxy binding is not configured",
        ));
    }
    let mut inventories: Vec<Inventory> = Vec::new();
    let mut diagnostics = Vec::new();
    for binding in &config.proxies {
        if requested.is_some_and(|id| id != binding.id) {
            continue;
        }
        if let Some(manifest) = &manifest
            && !manifest.routes.iter().any(|r| r.proxy == binding.id)
        {
            continue;
        }
        match inventory::inspect(binding, config.scope) {
            Ok(mut value) => {
                if let Some(store) = &store {
                    managed::qualify_ownership(&store.book, &mut value);
                }
                let retained: usize = inventories.iter().map(|i| i.retained_bytes).sum();
                if value.retained_bytes > 64 * 1024 * 1024 - retained {
                    diagnostics.push(Diagnostic::new("discovery.evidence_budget","Combined native evidence exceeds 64 MiB; inspect fewer bindings per command").at(&binding.id));
                } else {
                    inventories.push(value);
                }
            }
            Err(err) => diagnostics.push(err.at(&binding.id)),
        }
    }
    if let Some(manifest) = &manifest {
        if !diagnostics.is_empty() {
            return Err(Diagnostic::new(
                "discovery.unavailable",
                "One or more required inventories failed; no plan can establish a safe conflict boundary",
            ));
        }
        let preview = plan::build(manifest, &inventories)?;
        return Ok(Envelope {
            schema_version: "1",
            status: "planned_read_only",
            changed: false,
            data: serde_json::to_value(preview).expect("plan serialization"),
            diagnostics: Vec::new(),
        });
    }
    if inventories.is_empty() && !diagnostics.is_empty() {
        return Err(diagnostics.remove(0));
    }
    let partial = !diagnostics.is_empty()
        || inventories
            .iter()
            .any(|i| !i.complete || !i.runtime_confirmed);
    for value in &inventories {
        diagnostics.extend(value.diagnostics.iter().cloned());
    }
    let data = match &cli.command {
        Some(Command::Route(RouteCommand::List)) => json!(
            inventories
                .iter()
                .flat_map(|i| i.routes.iter())
                .collect::<Vec<_>>()
        ),
        Some(Command::Route(RouteCommand::Inspect { id, .. })) => {
            let route = inventories
                .iter()
                .flat_map(|i| i.routes.iter())
                .find(|r| {
                    &r.native_id == id
                        || store.as_ref().is_some_and(|s| {
                            s.book.routes.iter().any(|owned| {
                                owned.spec.id == *id
                                    && owned.spec.proxy == r.binding
                                    && r.native_id
                                        == match config
                                            .proxies
                                            .iter()
                                            .find(|p| p.id == r.binding)
                                            .map(|p| p.engine)
                                        {
                                            Some(Engine::Caddy) => format!("{}:0", owned.native_id),
                                            _ => format!("{}@file", owned.native_id),
                                        }
                            })
                        })
                })
                .ok_or_else(|| {
                    Diagnostic::new(
                        "resource.not_found",
                        "Native route ID is not in this snapshot",
                    )
                })?;
            json!(route)
        }
        _ => json!(inventories),
    };
    Ok(Envelope {
        schema_version: "1",
        status: if partial { "partial" } else { "observed" },
        changed: false,
        data,
        diagnostics,
    })
}

fn output(envelope: &Envelope, json_mode: bool) -> io::Result<()> {
    let mut out = io::stdout().lock();
    if json_mode {
        serde_json::to_writer_pretty(&mut out, envelope)?;
        writeln!(out)?;
    } else {
        writeln!(
            out,
            "Steward: {} ({})",
            envelope.status,
            if envelope.changed {
                "changes recorded"
            } else {
                "no changes"
            }
        )?;
        // JSON provides a readable escaped native inventory without terminal control injection.
        serde_json::to_writer_pretty(&mut out, &envelope.data)?;
        writeln!(out)?;
        for diagnostic in &envelope.diagnostics {
            eprintln!("{}", plan::safe(&diagnostic.to_string()));
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) {
                let _ = error.print();
                return ExitCode::SUCCESS;
            }
            if std::env::args().any(|a| a == "--json") {
                let result = Envelope {
                    schema_version: "1",
                    status: "rejected",
                    changed: false,
                    data: Value::Null,
                    diagnostics: vec![Diagnostic::new(
                        "cli.usage",
                        "Invalid or missing command arguments; run steward --help",
                    )],
                };
                let _ = output(&result, true);
            } else {
                let _ = error.print();
            }
            return ExitCode::from(2);
        }
    };
    if cli.command.is_none() {
        let mut command = Cli::command();
        let _ = command.print_help();
        println!();
        return ExitCode::SUCCESS;
    }
    if let Some(Command::Completions { shell }) = cli.command {
        // The static command tree emits a small script; buffering keeps generator
        // internals from panicking when a downstream reader closes stdout early.
        let mut script = Vec::new();
        clap_complete::generate(shell, &mut Cli::command(), "steward", &mut script);
        return match io::stdout().write_all(&script) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
            Err(_) => ExitCode::from(1),
        };
    }
    match run(&cli) {
        Ok(envelope) => match output(&envelope, cli.json) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
            Err(_) => ExitCode::from(1),
        },
        Err(error) => {
            let envelope = Envelope {
                schema_version: "1",
                status: "rejected",
                changed: error.changed,
                data: Value::Null,
                diagnostics: vec![error],
            };
            if cli.json {
                let _ = output(&envelope, true);
            } else {
                eprintln!("{}", plan::safe(&envelope.diagnostics[0].to_string()));
            }
            ExitCode::from(2)
        }
    }
}
