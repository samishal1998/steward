//! Keyboard terminal interface over the same checked application commands.
use crate::{
    Cli, Command, OperationCommand, ProxyCommand, RouteCommand, plan, run, services::ServiceCommand,
};
use clap::Parser;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use serde_json::Value;
use std::{
    io::{self, IsTerminal, Write},
    path::PathBuf,
};

struct Terminal;
impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), cursor::Show, LeaveAlternateScreen);
    }
}

pub fn start(state: Option<PathBuf>) -> io::Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(io::Error::other("tui requires an interactive terminal"));
    }
    terminal::enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, cursor::Hide)?;
    let _guard = Terminal;
    let tabs = ["Routes", "Proxies", "Services", "Operations"];
    let mut tab = 0;
    let mut selection = 0usize;
    let mut data = Value::Null;
    let mut message = String::new();
    let mut prompt: Option<String> = None;
    let mut details = false;
    let mut refresh = true;
    loop {
        if refresh {
            let mut cli = base(&state);
            cli.command = Some(match tab {
                0 => Command::Route(RouteCommand::List),
                1 => Command::Proxy(ProxyCommand::List),
                2 => Command::Service(ServiceCommand::List),
                _ => Command::Operations(OperationCommand::List),
            });
            match run(&cli) {
                Ok(result) => {
                    data = result.data;
                    message = format!("{} · {}", result.status, result.diagnostics.len());
                }
                Err(e) => {
                    data = Value::Null;
                    message = e.to_string();
                }
            }
            selection = 0;
            details = false;
            refresh = false;
        }
        let (width, height) = terminal::size()?;
        let width = width.max(1) as usize;
        let rows = height.saturating_sub(5) as usize;
        let content = if details {
            serde_json::to_string_pretty(&data)
                .unwrap_or_default()
                .lines()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        } else {
            data.as_array().map_or_else(
                || vec!["No records. Press : to enter a Steward command.".into()],
                |rows| rows.iter().map(|r| summary(tab, r)).collect(),
            )
        };
        let offset = if selection >= rows && rows > 0 {
            selection - rows + 1
        } else {
            0
        };
        let mut screen = format!(
            "steward  |  {}\r\nTab: view · ↑/↓: select · Enter: inspect · r: refresh · : command · q: quit\r\n",
            tabs.iter()
                .enumerate()
                .map(|(i, t)| if i == tab {
                    format!("[{t}]")
                } else {
                    t.to_string()
                })
                .collect::<Vec<_>>()
                .join("  ")
        );
        for (index, line) in content.iter().enumerate().skip(offset).take(rows) {
            screen.push_str(&format!(
                "{} {}\r\n",
                if index == selection { ">" } else { " " },
                plan::safe(line)
                    .chars()
                    .take(width.saturating_sub(3))
                    .collect::<String>()
            ));
        }
        screen.push_str(&format!(
            "\r\n{}\r\n",
            plan::safe(&message).chars().take(width).collect::<String>()
        ));
        if let Some(input) = &prompt {
            screen.push_str(&format!(
                ":{}",
                plan::safe(input)
                    .chars()
                    .take(width.saturating_sub(1))
                    .collect::<String>()
            ));
        }
        execute!(
            io::stdout(),
            cursor::MoveTo(0, 0),
            terminal::Clear(terminal::ClearType::All)
        )?;
        io::stdout().write_all(screen.as_bytes())?;
        io::stdout().flush()?;
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            break;
        }
        if let Some(input) = &mut prompt {
            match key.code {
                KeyCode::Esc => prompt = None,
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(c) if !c.is_control() && input.len() < 4096 => input.push(c),
                KeyCode::Enter => {
                    let input = prompt.take().unwrap_or_default();
                    let tokens = shlex::split(&input);
                    if let Some(tokens) = tokens {
                        let mut argv = vec!["steward".to_owned()];
                        if let Some(root) = &state {
                            argv.extend([
                                "--state-dir".into(),
                                root.to_string_lossy().into_owned(),
                            ]);
                        }
                        argv.extend(tokens);
                        match Cli::try_parse_from(argv) {
                            Ok(cli)
                                if !matches!(
                                    cli.command,
                                    Some(Command::Tui | Command::Completions { .. })
                                ) && cli.ssh.is_none()
                                    && cli.container.is_none() =>
                            {
                                match run(&cli) {
                                    Ok(result) => {
                                        data = result.data;
                                        message = format!(
                                            "{}; changed={}",
                                            result.status, result.changed
                                        );
                                        details = true;
                                        selection = 0;
                                    }
                                    Err(e) => message = e.to_string(),
                                }
                            }
                            Ok(_) => {
                                message =
                                    "Use a regular local Steward command in this terminal view"
                                        .into()
                            }
                            Err(e) => message = e.to_string(),
                        }
                    } else {
                        message = "Unclosed quote in command".into();
                    }
                }
                _ => {}
            }
            continue;
        }
        match key.code {
            KeyCode::Char('q') => break,
            KeyCode::Tab => {
                tab = (tab + 1) % 4;
                refresh = true;
            }
            KeyCode::BackTab => {
                tab = (tab + 3) % 4;
                refresh = true;
            }
            KeyCode::Up => selection = selection.saturating_sub(1),
            KeyCode::Down => selection = (selection + 1).min(content.len().saturating_sub(1)),
            KeyCode::Char('r') | KeyCode::Esc => refresh = true,
            KeyCode::Char(':') => prompt = Some(String::new()),
            KeyCode::Enter => {
                if !details
                    && let Some(item) = data.as_array().and_then(|r| r.get(selection)).cloned()
                {
                    data = item;
                    details = true;
                    selection = 0;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn base(state: &Option<PathBuf>) -> Cli {
    Cli {
        config: None,
        state_dir: state.clone(),
        json: true,
        command: None,
        ssh: None,
        container: None,
        remote_bin: "steward".into(),
    }
}
fn summary(tab: usize, row: &Value) -> String {
    let text = |key: &str| row[key].as_str().unwrap_or("—").to_owned();
    match tab {
        0 => format!(
            "{}  {}  {}  {}",
            text("binding"),
            text("native_id"),
            row["hosts"],
            text("ownership")
        ),
        1 => format!(
            "{}  {}  {} routes",
            text("binding"),
            text("engine"),
            row["routes"].as_array().map_or(0, Vec::len)
        ),
        2 => format!(
            "{}  running={}  enabled={}",
            text("id"),
            row["running"],
            row["enabled"]
        ),
        _ => format!("{}  {}  {}", text("id"), text("proxy"), text("phase")),
    }
}
