//! Explicit command execution in the selected host/container context.
use crate::{
    error::{Diagnostic, Result},
    executor,
};
use std::{
    io::Write,
    process::{Command, ExitCode},
    time::Duration,
};

pub fn dispatch() -> Result<Option<ExitCode>> {
    let mut selected = None;
    let mut binary = "steward".to_owned();
    let mut forwarded = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--" {
            forwarded.push(arg);
            forwarded.extend(args);
            break;
        }
        let (key, value) = if matches!(arg.as_str(), "--ssh" | "--container" | "--remote-bin") {
            let value = args
                .next()
                .ok_or_else(|| Diagnostic::new("target.argument", "Missing target option value"))?;
            (arg.clone(), Some(value))
        } else if let Some((key, value)) = arg.split_once('=')
            && matches!(key, "--ssh" | "--container" | "--remote-bin")
        {
            (key.to_owned(), Some(value.to_owned()))
        } else {
            (String::new(), None)
        };
        if let Some(value) = value {
            if key == "--remote-bin" {
                binary = value;
            } else {
                if selected.is_some() {
                    return Err(Diagnostic::new(
                        "target.conflict",
                        "Select exactly one SSH or container target",
                    ));
                }
                selected = Some((key == "--ssh", value));
            }
            continue;
        }
        forwarded.push(arg);
    }
    let Some((ssh, target)) = selected else {
        return Ok(None);
    };
    if target.is_empty()
        || target.starts_with('-')
        || target.len() > 256
        || target.chars().any(|c| c.is_control() || c.is_whitespace())
        || binary.is_empty()
        || binary.chars().any(char::is_control)
    {
        return Err(Diagnostic::new(
            "target.invalid",
            "Use an explicit target and executable without control characters",
        ));
    }
    if forwarded.len() > 256 {
        return Err(Diagnostic::new(
            "target.limit",
            "Too many delegated arguments",
        ));
    }
    let mut command = if ssh {
        let mut argv = vec![binary.clone()];
        argv.extend(forwarded);
        let quoted = argv
            .iter()
            .map(|a| format!("'{}'", a.replace('\'', "'\\''")))
            .collect::<Vec<_>>()
            .join(" ");
        let mut c = Command::new("ssh");
        c.args(["-T", "-o", "BatchMode=yes", "--", &target, &quoted]);
        c
    } else {
        let mut c = Command::new("docker");
        c.args(["exec", &target, &binary]);
        c.args(forwarded);
        c
    };
    let output =
        executor::run(&mut command, Duration::from_secs(300), 8 * 1024 * 1024).map_err(|e| {
            if e.code == "process.start" {
                e
            } else {
                e.with_changes()
            }
        })?;
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
