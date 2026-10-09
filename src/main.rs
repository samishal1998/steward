mod config;
mod error;
mod inventory;
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

use clap::{CommandFactory, Parser, Subcommand};
use serde::Serialize;
use serde_json::{Value, json};

use error::{Diagnostic, Result};
use model::Inventory;

#[derive(Parser)]
#[command(
    version,
    about = "Explicit read-only proxy inventory and route planning",
    long_about = "Steward foundation: inspect Caddy/Traefik and preview route intent. Management, bootstrap, native service control, and TUI are pending compatibility and recovery milestones. No command in this build changes infrastructure."
)]
struct Cli {
    #[arg(long, global = true)]
    config: Option<PathBuf>,
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
    #[command(subcommand)]
    Proxy(ProxyCommand),
    #[command(subcommand)]
    Route(RouteCommand),
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
    Inspect { id: String },
}
#[derive(Subcommand)]
enum RouteCommand {
    List,
    Inspect {
        #[arg(long)]
        proxy: String,
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
    let manifest = if let Some(Command::Plan { file }) = &cli.command {
        Some(config::load_manifest(file)?)
    } else {
        None
    };
    let path=cli.config.as_ref().ok_or_else(||Diagnostic::new("config.required","Pass --config with explicit version-1 proxy bindings; there is no implicit discovery or setup"))?;
    let mut config = config::load_config(path)?;
    // Snapshot paths are relative to the profile, not the invoking shell's directory.
    for binding in &mut config.proxies {
        if let Some(snapshot) = &binding.snapshot_file
            && !std::path::Path::new(snapshot).is_absolute()
        {
            binding.snapshot_file = Some(
                path.parent()
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
            Ok(value) => inventories.push(value),
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
                .find(|r| &r.native_id == id)
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
        writeln!(out, "Steward: {} (no changes)", envelope.status)?;
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
                changed: false,
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
