//! Explicit command execution in the selected host/container context.
use crate::{
    Cli,
    error::{Diagnostic, Result},
    executor,
};
use std::{
    io::Write,
    process::{Command, ExitCode},
    time::Duration,
};

pub fn dispatch(cli: &Cli) -> Result<Option<ExitCode>> {
    let Some(target) = cli.ssh.as_ref().or(cli.container.as_ref()) else {
        return Ok(None);
    };
    if target.is_empty()
        || target.starts_with('-')
        || target.len() > 256
        || target.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        return Err(Diagnostic::new(
            "target.invalid",
            "Use an explicit SSH host or Docker container identifier",
        ));
    }
    let mut forwarded = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--" {
            forwarded.push(arg);
            forwarded.extend(args);
            break;
        }
        if matches!(arg.as_str(), "--ssh" | "--container" | "--remote-bin") {
            args.next();
            continue;
        }
        if ["--ssh=", "--container=", "--remote-bin="]
            .iter()
            .any(|p| arg.starts_with(p))
        {
            continue;
        }
        forwarded.push(arg);
    }
    if forwarded.len() > 256 {
        return Err(Diagnostic::new(
            "target.limit",
            "Too many delegated arguments",
        ));
    }
    let mut command = if cli.ssh.is_some() {
        let mut argv = vec![cli.remote_bin.clone()];
        argv.extend(forwarded);
        let quoted = argv
            .iter()
            .map(|a| format!("'{}'", a.replace('\'', "'\\''")))
            .collect::<Vec<_>>()
            .join(" ");
        let mut c = Command::new("ssh");
        c.args(["-T", "-o", "BatchMode=yes", "--", target, &quoted]);
        c
    } else {
        let mut c = Command::new("docker");
        c.args(["exec", target, &cli.remote_bin]);
        c.args(forwarded);
        c
    };
    let output = executor::run(&mut command, Duration::from_secs(300), 8 * 1024 * 1024)
        .map_err(|e| e.with_changes())?;
    std::io::stdout()
        .write_all(&output.stdout)
        .map_err(|_| Diagnostic::new("output.failed", "Could not write target output"))?;
    std::io::stderr()
        .write_all(&output.stderr)
        .map_err(|_| Diagnostic::new("output.failed", "Could not write target diagnostic"))?;
    Ok(Some(ExitCode::from(
        output.status.code().unwrap_or(1).clamp(0, 255) as u8,
    )))
}
