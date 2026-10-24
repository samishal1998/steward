//! Completion reads only the bounded local state snapshot, never opens/creates a Store.
use crate::{Cli, CommandFactory, config, managed};
use clap_complete::{ArgValueCandidates, CompletionCandidate};
use std::{ffi::OsStr, path::PathBuf};

fn book() -> Option<managed::Book> {
    let args: Vec<_> = std::env::args_os().take(256).collect();
    if args.iter().any(|a| {
        a == "--ssh"
            || a == "--container"
            || a.to_str()
                .is_some_and(|a| a.starts_with("--ssh=") || a.starts_with("--container="))
    }) {
        return None;
    }
    let explicit = args
        .windows(2)
        .rev()
        .find(|w| w[0] == "--state-dir")
        .map(|w| PathBuf::from(&w[1]))
        .or_else(|| {
            args.iter()
                .rev()
                .find_map(|s| s.to_str()?.strip_prefix("--state-dir=").map(PathBuf::from))
        });
    let root = explicit.or_else(|| managed::default_root().ok())?;
    let path = root.join("state.json");
    if std::fs::symlink_metadata(&path)
        .ok()?
        .file_type()
        .is_symlink()
    {
        return None;
    }
    config::load(&path, config::MAX_CONFIG).ok()
}

fn proxies() -> Vec<CompletionCandidate> {
    book().map_or_else(Vec::new, |b| {
        b.connections
            .into_iter()
            .filter(|c| config::resource_id(&c.binding.id).is_ok())
            .map(|c| CompletionCandidate::new(c.binding.id))
            .collect()
    })
}

fn routes() -> Vec<CompletionCandidate> {
    let args: Vec<_> = std::env::args_os().take(256).collect();
    let proxy = args
        .windows(2)
        .rev()
        .find(|w| w[0] == "--proxy")
        .map(|w| w[1].clone());
    book().map_or_else(Vec::new, |b| {
        b.routes
            .into_iter()
            .filter(|r| {
                config::resource_id(&r.spec.id).is_ok()
                    && proxy
                        .as_ref()
                        .is_none_or(|p| OsStr::new(&r.spec.proxy) == p)
            })
            .map(|r| CompletionCandidate::new(r.spec.id))
            .collect()
    })
}

pub fn command() -> clap::Command {
    let mut command = Cli::command();
    for group in command.get_subcommands_mut() {
        let name = group.get_name().to_owned();
        for sub in group.get_subcommands_mut() {
            let action = sub.get_name().to_owned();
            if sub.get_arguments().any(|a| a.get_id() == "proxy") {
                *sub = sub
                    .clone()
                    .mut_arg("proxy", |a| a.add(ArgValueCandidates::new(proxies)));
            }
            if sub.get_arguments().any(|a| a.get_id() == "id") && action != "add" {
                if name == "proxies" {
                    *sub = sub
                        .clone()
                        .mut_arg("id", |a| a.add(ArgValueCandidates::new(proxies)));
                }
                if name == "routes" {
                    *sub = sub
                        .clone()
                        .mut_arg("id", |a| a.add(ArgValueCandidates::new(routes)));
                }
            }
        }
    }
    command
}
