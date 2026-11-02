//! Resource browser over the checked CLI. One bounded worker keeps the terminal responsive.
mod form;
mod view;
mod workspace;

use crate::{
    Cli, Command, Envelope, OperationCommand, ProxyCommand, RouteCommand,
    error::{Diagnostic, Result},
    managed,
    model::Engine,
    run,
    services::ServiceCommand,
};
use clap::Parser;
use crossterm::{
    cursor,
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers,
    },
    execute,
    style::{Attribute, ResetColor, SetAttribute},
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use form::{Editor, Form};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    ffi::OsString,
    io::{self, IsTerminal},
    path::PathBuf,
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::{Duration, Instant},
};

struct Terminal;
impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = execute!(
            io::stdout(),
            terminal::EndSynchronizedUpdate,
            DisableBracketedPaste,
            ResetColor,
            SetAttribute(Attribute::Reset),
            cursor::Show,
            LeaveAlternateScreen
        );
        let _ = terminal::disable_raw_mode();
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Routes,
    Proxies,
    Services,
    Operations,
}
impl Tab {
    const ALL: [Self; 4] = [
        Self::Proxies,
        Self::Routes,
        Self::Services,
        Self::Operations,
    ];
    fn name(self) -> &'static str {
        match self {
            Self::Routes => "Routes",
            Self::Proxies => "Routers",
            Self::Services => "Services",
            Self::Operations => "Operations",
        }
    }
    fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }
    fn command(self) -> Command {
        match self {
            Self::Routes => Command::Route(RouteCommand::List),
            Self::Proxies => Command::Proxy(ProxyCommand::List),
            Self::Services => Command::Service(ServiceCommand::List),
            Self::Operations => Command::Operations(OperationCommand::List),
        }
    }
}

#[derive(Clone, Copy)]
enum Work {
    List(Tab),
    Inspect,
    Command,
    Discover,
    Logs,
    EditService,
}
impl Work {
    fn background(self) -> bool {
        matches!(self, Self::List(_) | Self::Discover | Self::Logs)
    }
}
struct Pending {
    receiver: Receiver<Result<Envelope>>,
    work: Work,
    label: String,
    mutating: bool,
    started: Instant,
    tab: Tab,
    key: Option<String>,
    plan: Option<PathBuf>,
}
struct Queued {
    cli: Cli,
    work: Work,
    label: String,
    tab: Tab,
    key: Option<String>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Resources,
    Inspector,
    Activity,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum InspectMode {
    Overview,
    Json,
    Logs,
    Result,
}
#[derive(Default)]
struct Panel {
    rows: Vec<Value>,
    visible: Vec<usize>,
    selection: usize,
    filter: String,
    diagnostics: Vec<Diagnostic>,
    updated: Option<Instant>,
}
struct PanelView<'a> {
    rows: &'a [Value],
    visible: &'a [usize],
    selection: usize,
    filter: &'a str,
    diagnostics: &'a [Diagnostic],
    updated: Option<Instant>,
}
#[derive(Clone)]
struct Details {
    title: String,
    data: Value,
    diagnostics: Vec<Diagnostic>,
    status: String,
    changed: bool,
    scroll: usize,
    raw: bool,
}
#[derive(Clone, Copy)]
enum Effect {
    Inspect,
    Edit,
    Logs,
    Rescan,
    Pause,
    Zoom,
    Diagnostics,
}
#[derive(Clone, Default)]
struct Action {
    label: &'static str,
    args: Vec<String>,
    form: Option<Form>,
    effect: Option<Effect>,
    destructive: bool,
}
fn action_matches(action: &Action, query: &str) -> bool {
    let label = action.label.to_lowercase();
    query
        .to_lowercase()
        .split_whitespace()
        .all(|word| label.contains(word))
}
enum Screen {
    Browse,
    Details(Details),
    Prompt {
        editor: Editor,
        search: bool,
        original: String,
    },
    Form(Form),
    Actions {
        actions: Vec<Action>,
        selected: usize,
        query: Editor,
    },
    Confirm(Action),
    Files {
        form: Form,
        directory: PathBuf,
        entries: Vec<(String, bool)>,
        selected: usize,
    },
}
struct App {
    root: PathBuf,
    tab: Tab,
    rows: Vec<Value>,
    visible: Vec<usize>,
    selection: usize,
    filter: String,
    screen: Screen,
    diagnostics: Vec<Diagnostic>,
    notice: String,
    pending: Option<Pending>,
    refresh: bool,
    updated: Option<Instant>,
    retry: Option<Form>,
    history: Vec<String>,
    history_index: usize,
    quit_after_work: bool,
    panels: [Panel; 4],
    focus: Focus,
    mode: InspectMode,
    inspector: Option<Details>,
    inspected: Option<(Tab, String)>,
    result: Option<Details>,
    activity: VecDeque<String>,
    activity_scroll: usize,
    scope: Option<String>,
    detected: Value,
    scan_requested: bool,
    scanned: Option<Instant>,
    queued: Option<Queued>,
    auto_refresh: bool,
    zoom: bool,
    follow_logs: bool,
    logs_updated: Option<Instant>,
    plan: Option<PathBuf>,
    inspect_scroll: usize,
    inspector_kind: InspectMode,
}

impl App {
    fn new(root: PathBuf) -> Self {
        Self {
            root,
            tab: Tab::Proxies,
            rows: Vec::new(),
            visible: Vec::new(),
            selection: 0,
            filter: String::new(),
            screen: Screen::Browse,
            diagnostics: Vec::new(),
            notice: "Ready. Press ? for keyboard help.".into(),
            pending: None,
            refresh: true,
            updated: None,
            retry: None,
            history: Vec::new(),
            history_index: 0,
            quit_after_work: false,
            panels: std::array::from_fn(|_| Panel::default()),
            focus: Focus::Resources,
            mode: InspectMode::Overview,
            inspector: None,
            inspected: None,
            result: None,
            activity: VecDeque::new(),
            activity_scroll: 0,
            scope: None,
            detected: json!({}),
            scan_requested: true,
            scanned: None,
            queued: None,
            auto_refresh: true,
            zoom: false,
            follow_logs: true,
            logs_updated: None,
            plan: None,
            inspect_scroll: 0,
            inspector_kind: InspectMode::Overview,
        }
    }
    fn selected(&self) -> Option<&Value> {
        self.visible
            .get(self.selection)
            .and_then(|i| self.rows.get(*i))
    }
    fn refilter(&mut self) {
        let query = self.filter.to_lowercase();
        self.visible = self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                (self.tab != Tab::Routes
                    || self.scope.as_ref().is_none_or(|id| r["binding"] == *id))
                    && (query.is_empty()
                        || format!(
                            "{} {} {}",
                            view::cells(self.tab, r).join(" "),
                            text(r, "native_id"),
                            text(r, "_managed_id")
                        )
                        .to_lowercase()
                        .contains(&query)
                        || ["hosts", "targets", "paths", "endpoint"].iter().any(|key| {
                            match &r[*key] {
                                Value::String(s) => s.to_lowercase().contains(&query),
                                Value::Array(values) => values.iter().any(|v| {
                                    v.as_str()
                                        .is_some_and(|s| s.to_lowercase().contains(&query))
                                }),
                                _ => false,
                            }
                        }))
            })
            .map(|(i, _)| i)
            .collect();
        self.selection = self.selection.min(self.visible.len().saturating_sub(1));
    }
    fn switch(&mut self, tab: Tab) {
        self.stash();
        self.tab = tab;
        self.restore();
        self.screen = Screen::Browse;
        self.retry = None;
        self.focus = Focus::Resources;
        self.mode = InspectMode::Overview;
        self.zoom = false;
        self.refresh = self.updated.is_none();
    }
    fn launch(&mut self, cli: Cli, work: Work, label: String) {
        if self.pending.is_some() {
            self.notice =
                "An operation is still running. Navigation and help remain available.".into();
            return;
        }
        let mutating = !work.background() && !read_only(&cli);
        let plan = match &cli.command {
            Some(Command::Plan { out, .. }) => out.clone(),
            _ => None,
        };
        let (sender, receiver) = mpsc::sync_channel(1);
        thread::spawn(move || {
            let _ = sender.send(execute_work(&cli, work));
        });
        self.pending = Some(Pending {
            receiver,
            work,
            label,
            mutating,
            started: Instant::now(),
            tab: self.tab,
            key: self.selected().map(identity),
            plan,
        });
    }
    fn pump(&mut self) -> bool {
        let received = self.pending.as_ref().map(|p| p.receiver.try_recv());
        let result = match received {
            Some(Ok(result)) => Some(result),
            Some(Err(TryRecvError::Disconnected)) => {
                let mut error = Diagnostic::new(
                    "tui.worker",
                    "The operation worker stopped unexpectedly; inspect Operations before retrying a change",
                );
                error.changed = self.pending.as_ref().is_some_and(|p| p.mutating);
                Some(Err(error))
            }
            _ => None,
        };
        let mut changed = result.is_some();
        if let Some(result) = result {
            let pending = self.pending.take().expect("pending result");
            match pending.work {
                Work::List(tab) => {
                    let active = self.tab;
                    if tab != active {
                        self.stash();
                        self.tab = tab;
                        self.restore();
                    }
                    match result {
                        Ok(result) => {
                            let previous = self.selected().map(identity);
                            self.rows = match result.data {
                                Value::Array(rows) => rows,
                                _ => Vec::new(),
                            };
                            self.diagnostics = result.diagnostics;
                            self.updated = Some(Instant::now());
                            self.merge_discovery();
                            self.refilter();
                            if let Some(i) = self
                                .visible
                                .iter()
                                .position(|i| previous.as_ref() == Some(&identity(&self.rows[*i])))
                            {
                                self.selection = i;
                            }
                        }
                        Err(error) => {
                            self.notice =
                                "Could not refresh. Press i for the full error; r retries.".into();
                            self.diagnostics = vec![error];
                            self.updated = Some(Instant::now());
                        }
                    }
                    if tab != active {
                        self.stash();
                        self.tab = active;
                        self.restore();
                    }
                }
                Work::Discover => self.discovered(result),
                Work::EditService => {
                    match result.and_then(|r| {
                        serde_json::from_value::<crate::services::Spec>(r.data).map_err(|_| {
                            Diagnostic::new("tui.edit", "Could not load the service editing input")
                        })
                    }) {
                        Ok(spec) => self.screen = Screen::Form(Form::edit_service(&spec)),
                        Err(error) => self.show_error(&error.to_string()),
                    }
                }
                Work::Inspect | Work::Command | Work::Logs => {
                    let result = match result {
                        Ok(result) => {
                            self.retry = None;
                            result
                        }
                        Err(error) => Envelope {
                            schema_version: "1",
                            status: "rejected",
                            changed: error.changed,
                            data: Value::Null,
                            diagnostics: vec![error],
                        },
                    };
                    self.notice = format!(
                        "{}: {}{}",
                        pending.label,
                        result.status,
                        if result.changed {
                            " (changes made)"
                        } else {
                            ""
                        }
                    );
                    let details = Details {
                        title: pending.label,
                        data: result.data,
                        diagnostics: result.diagnostics,
                        status: result.status.into(),
                        changed: result.changed,
                        scroll: 0,
                        raw: false,
                    };
                    if matches!(pending.work, Work::Command) {
                        if details.status == "planned" {
                            self.plan = pending.plan;
                        }
                        self.log(&format!(
                            "{}: {}{}",
                            details.title,
                            details.status,
                            if details.changed { " (changed)" } else { "" }
                        ));
                        self.result = Some(details.clone());
                        self.mode = InspectMode::Result;
                        self.focus = Focus::Inspector;
                        self.screen = if details.status == "rejected" {
                            Screen::Details(details)
                        } else {
                            Screen::Browse
                        };
                    } else if self.tab == pending.tab
                        && self.selected().map(identity) == pending.key
                    {
                        self.inspector = Some(details);
                        self.inspected = pending.key.map(|key| (pending.tab, key));
                        self.inspector_kind = if matches!(pending.work, Work::Logs) {
                            InspectMode::Logs
                        } else {
                            InspectMode::Overview
                        };
                        if matches!(pending.work, Work::Logs) {
                            self.logs_updated = Some(Instant::now());
                        }
                    }
                    if matches!(pending.work, Work::Command) {
                        self.refresh = true;
                        self.inspect_scroll = 0;
                        for panel in &mut self.panels {
                            panel.updated = None;
                        }
                    }
                }
            }
        }
        if self.pending.is_none()
            && !self.quit_after_work
            && let Some(job) = self.queued.take()
        {
            self.launch(job.cli, job.work, job.label);
            if let Some(p) = &mut self.pending {
                p.tab = job.tab;
                p.key = job.key;
            }
            changed = true;
        }
        changed |= self.schedule_background();
        if self.pending.is_none() && self.refresh && !self.quit_after_work {
            self.refresh = false;
            let mut cli = base(&self.root);
            cli.command = Some(self.tab.command());
            self.launch(
                cli,
                Work::List(self.tab),
                format!("Loading {}", self.tab.name()),
            );
            changed = true;
        }
        changed
    }
    fn command(&mut self, args: Vec<String>, work: Work, label: &str) {
        if self.queued.is_some() || self.pending.as_ref().is_some_and(|p| p.mutating) {
            self.notice = "A checked action is already running or queued.".into();
            return;
        }
        let mut argv = vec![
            OsString::from("steward"),
            "--state-dir".into(),
            self.root.as_os_str().to_owned(),
        ];
        argv.extend(args.into_iter().map(OsString::from));
        match Cli::try_parse_from(argv) {
            Ok(cli) if cli.command.is_some() && !matches!(cli.command, Some(Command::Tui | Command::Completions { .. })) && cli.ssh.is_none() && cli.container.is_none() => {
                if self.pending.is_some() { self.queued = Some(Queued { cli, work, label: label.into(), tab: self.tab, key: self.selected().map(identity) }); self.notice = format!("Queued: {label}"); }
                else { self.launch(cli, work, label.into()); }
            }
            Ok(_) => self.show_error("Enter a local Steward command. Use another terminal for remote targets or completions."),
            Err(error) => self.show_error(&error.to_string()),
        }
    }
    fn show_error(&mut self, message: &str) {
        self.screen = Screen::Details(Details {
            title: "Command help / error".into(),
            data: Value::Null,
            diagnostics: vec![Diagnostic::new("tui.command", message)],
            status: "Not executed".into(),
            changed: false,
            scroll: 0,
            raw: false,
        });
    }
    fn inspect(&mut self) {
        self.focus = Focus::Inspector;
        self.mode = InspectMode::Overview;
        if let Some(row) = self.selected().cloned() {
            if text(&row, "source") == "saved_receipt" || row["_detected"] == true {
                self.inspected = Some((self.tab, identity(&row)));
                self.inspector_kind = InspectMode::Overview;
                self.inspector = Some(Details {
                    title: "Observed / saved resource".into(),
                    data: row.clone(),
                    diagnostics: Vec::new(),
                    status: if row["runtime_confirmed"] == true {
                        "Detected API observation"
                    } else {
                        "Saved/discovered evidence"
                    }
                    .into(),
                    changed: false,
                    scroll: 0,
                    raw: false,
                });
                return;
            }
            let args = inspection(self.tab, &row);
            let label = format!(
                "Resource details: {}",
                if self.tab == Tab::Routes {
                    text(&row, "native_id")
                } else if self.tab == Tab::Proxies {
                    text(&row, "binding")
                } else {
                    text(&row, "id")
                }
            );
            self.command(args, Work::Inspect, &label);
        }
    }
    fn submit_form(&mut self) {
        let Screen::Form(form) = &mut self.screen else {
            return;
        };
        if self.queued.is_some() || self.pending.as_ref().is_some_and(|p| p.mutating) {
            form.error = "A checked change is already running or queued.".into();
            return;
        }
        match form.arguments() {
            Ok(args) => {
                let title = form.title;
                self.retry = Some(form.clone());
                self.command(args, Work::Command, title);
            }
            Err(error) => form.error = error,
        }
    }
    fn quit(&mut self) -> bool {
        self.queued = None;
        if self.pending.as_ref().is_some_and(|p| p.mutating) {
            self.quit_after_work = true;
            self.notice = "Finishing the checked operation before exiting...".into();
            false
        } else {
            true
        }
    }
    fn key(&mut self, key: KeyEvent, width: usize, page: usize) -> bool {
        if key.kind == KeyEventKind::Release {
            return false;
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            return self.quit();
        }
        if self.quit_after_work {
            return false;
        }
        if let Some(quit) = self.workspace_key(key, width, page) {
            return quit;
        }
        if matches!(self.screen, Screen::Form(_)) {
            if self.queued.is_some() {
                if key.code == KeyCode::Esc {
                    self.queued = None;
                    self.retry = None;
                    self.screen = Screen::Browse;
                    self.notice = "Queued action canceled".into();
                }
                return false;
            }
            if self.pending.as_ref().is_some_and(|p| p.mutating) {
                return false;
            }
            if key.code == KeyCode::Esc {
                self.screen = Screen::Browse;
                self.retry = None;
                return false;
            }
            if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
                self.submit_form();
                return false;
            }
            let Screen::Form(form) = &mut self.screen else {
                unreachable!()
            };
            match key.code {
                KeyCode::Tab | KeyCode::Down => {
                    form.selected = (form.selected + 1) % form.fields.len()
                }
                KeyCode::BackTab | KeyCode::Up => {
                    form.selected = (form.selected + form.fields.len() - 1) % form.fields.len()
                }
                KeyCode::Enter if form.selected + 1 == form.fields.len() => self.submit_form(),
                KeyCode::Enter => form.selected += 1,
                _ => {
                    form.fields[form.selected].key(key);
                    form.error.clear();
                }
            }
            return false;
        }
        if let Screen::Prompt {
            editor,
            search,
            original,
        } = &mut self.screen
        {
            match key.code {
                KeyCode::Esc => {
                    if *search {
                        self.filter = original.clone();
                        self.refilter();
                    }
                    self.screen = Screen::Browse;
                }
                KeyCode::Enter => {
                    let input = editor.text.clone();
                    let search = *search;
                    self.screen = Screen::Browse;
                    if !search && !input.trim().is_empty() {
                        if self.history.len() == 16 {
                            self.history.remove(0);
                        }
                        self.history.push(input.clone());
                        self.history_index = self.history.len();
                        match shlex::split(&input) {
                            Some(mut args) => {
                                if args.first().is_some_and(|s| s == "steward") {
                                    args.remove(0);
                                }
                                self.command(args, Work::Command, "Command result");
                            }
                            None => self.show_error("Unclosed quote in command"),
                        }
                    }
                }
                KeyCode::Up if !*search => {
                    self.history_index = self.history_index.saturating_sub(1);
                    *editor = Editor::new(
                        self.history
                            .get(self.history_index)
                            .cloned()
                            .unwrap_or_default(),
                    );
                }
                KeyCode::Down if !*search => {
                    self.history_index = (self.history_index + 1).min(self.history.len());
                    *editor = Editor::new(
                        self.history
                            .get(self.history_index)
                            .cloned()
                            .unwrap_or_default(),
                    );
                }
                _ => {
                    editor.key(key);
                    if *search {
                        self.filter = editor.text.clone();
                        self.selection = 0;
                        self.refilter();
                    }
                }
            }
            return false;
        }
        let modal_page = view::modal_height(&self.screen, page + 11)
            .saturating_sub(5)
            .max(1);
        if let Screen::Details(details) = &mut self.screen {
            let end = view::detail_lines(details, view::modal_width(width).saturating_sub(4))
                .len()
                .saturating_sub(modal_page);
            details.scroll = details.scroll.min(end);
            match key.code {
                KeyCode::Esc | KeyCode::Enter => {
                    self.screen = self.retry.take().map_or(Screen::Browse, Screen::Form)
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    details.scroll = details.scroll.saturating_sub(1)
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    details.scroll = details.scroll.saturating_add(1).min(end)
                }
                KeyCode::PageUp => details.scroll = details.scroll.saturating_sub(modal_page),
                KeyCode::PageDown => {
                    details.scroll = details.scroll.saturating_add(modal_page).min(end)
                }
                KeyCode::Home => details.scroll = 0,
                KeyCode::End => details.scroll = end,
                KeyCode::Char('v') => {
                    details.raw = !details.raw;
                    details.scroll = 0;
                }
                KeyCode::Char('q') => return self.quit(),
                _ => {}
            }
            return false;
        }
        if self.pending.as_ref().is_some_and(|p| p.mutating)
            && matches!(key.code, KeyCode::Char('c' | 'n' | 'e' | ':'))
        {
            self.notice = "Finish the current operation before starting another.".into();
            return false;
        }
        let previous = self.selected().map(identity);
        match key.code {
            KeyCode::Char('q') => return self.quit(),
            KeyCode::Char(c @ '1'..='4') => self.switch(Tab::ALL[c as usize - '1' as usize]),
            KeyCode::Up | KeyCode::Char('k') => self.selection = self.selection.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                self.selection = (self.selection + 1).min(self.visible.len().saturating_sub(1))
            }
            KeyCode::PageUp => self.selection = self.selection.saturating_sub(page),
            KeyCode::PageDown => {
                self.selection = (self.selection + page).min(self.visible.len().saturating_sub(1))
            }
            KeyCode::Home => self.selection = 0,
            KeyCode::End => self.selection = self.visible.len().saturating_sub(1),
            KeyCode::Enter => self.inspect(),
            KeyCode::Char('r') => self.refresh = true,
            KeyCode::Esc => {
                self.filter.clear();
                self.refilter();
            }
            KeyCode::Char('/') => {
                self.screen = Screen::Prompt {
                    editor: Editor::new(&self.filter),
                    search: true,
                    original: self.filter.clone(),
                }
            }
            KeyCode::Char(':') => {
                self.history_index = self.history.len();
                self.screen = Screen::Prompt {
                    editor: Editor::default(),
                    search: false,
                    original: String::new(),
                };
            }
            KeyCode::Char('?') => {
                self.screen = Screen::Details(Details {
                    title: "Keyboard help".into(),
                    data: Value::String(view::HELP.into()),
                    diagnostics: Vec::new(),
                    status: "Help".into(),
                    changed: false,
                    scroll: 0,
                    raw: false,
                })
            }
            KeyCode::Char('c') => self.connect_selected(),
            KeyCode::Char('n') => {
                self.screen = match self.tab {
                    Tab::Proxies => Screen::Form(Form::connect()),
                    Tab::Services => Screen::Form(Form::service()),
                    Tab::Routes => Screen::Form(Form::route(
                        self.scope
                            .as_deref()
                            .unwrap_or_else(|| self.selected().map_or("", |r| text(r, "binding"))),
                        None,
                    )),
                    Tab::Operations => {
                        self.notice = "Operations are recorded when a checked change runs.".into();
                        Screen::Browse
                    }
                }
            }
            KeyCode::Char('e') if self.tab == Tab::Routes => {
                if let Some(row) = self.selected().filter(|r| r["_managed_id"].is_string()) {
                    self.screen = Screen::Form(Form::route(
                        text(row, "binding"),
                        Some((
                            text(row, "_managed_id"),
                            text(row, "_hostname"),
                            text(row, "_upstream"),
                        )),
                    ));
                } else {
                    self.notice = "Only an owned route can be edited. Inspect external routes before explicit adoption.".into();
                }
            }
            KeyCode::Char('a') => {
                if let Some(row) = self.selected() {
                    self.screen = Screen::Actions {
                        actions: actions(self.tab, row),
                        selected: 0,
                        query: Editor::default(),
                    };
                }
            }
            KeyCode::Char('d') => self.scan_requested = true,
            KeyCode::Char('i') => self.perform(Action {
                effect: Some(Effect::Diagnostics),
                ..Default::default()
            }),
            _ => {}
        }
        if self.selected().map(identity) != previous {
            self.inspect_scroll = 0;
            self.logs_updated = None;
        }
        false
    }
    fn paste(&mut self, text: &str) {
        match &mut self.screen {
            Screen::Actions {
                query, selected, ..
            } => {
                query.insert(text);
                *selected = 0;
            }
            Screen::Prompt { editor, search, .. } => {
                editor.insert(text);
                if *search {
                    self.filter = editor.text.clone();
                    self.selection = 0;
                    self.refilter();
                }
            }
            Screen::Form(form) if self.pending.as_ref().is_none_or(|p| !p.mutating) => {
                let field = &mut form.fields[form.selected];
                if field.choices.is_empty() {
                    field.editor.insert(text);
                    form.error.clear();
                }
            }
            _ => {}
        }
    }
}

pub fn start(state: Option<PathBuf>) -> io::Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(io::Error::other("tui requires an interactive terminal"));
    }
    let root = state
        .map(Ok)
        .unwrap_or_else(managed::default_root)
        .map_err(io::Error::other)?;
    let _guard = Terminal;
    terminal::enable_raw_mode()?;
    execute!(
        io::stdout(),
        EnterAlternateScreen,
        EnableBracketedPaste,
        cursor::Hide
    )?;
    let mut app = App::new(root);
    let mut previous = None;
    let mut dirty = true;
    let mut drawn = Instant::now();
    loop {
        dirty |= app.pump();
        if app.quit_after_work && app.pending.is_none() {
            break;
        }
        let size = terminal::size()?;
        if dirty
            || drawn.elapsed()
                >= if app.pending.is_some() {
                    Duration::from_millis(400)
                } else {
                    Duration::from_secs(1)
                }
        {
            let frame = view::frame(&app, size);
            if previous.as_ref() != Some(&frame) {
                frame.draw()?;
                previous = Some(frame);
            }
            drawn = Instant::now();
            dirty = false;
        }
        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        dirty = true;
        match event::read()? {
            Event::Key(key) if size.0 < 48 || size.1 < 14 => {
                if key.kind != KeyEventKind::Release
                    && (key.code == KeyCode::Char('q')
                        || key.code == KeyCode::Char('c')
                            && key.modifiers.contains(KeyModifiers::CONTROL))
                    && app.quit()
                {
                    break;
                }
            }
            Event::Key(key)
                if app.key(
                    key,
                    size.0 as usize,
                    size.1.saturating_sub(11).max(1) as usize,
                ) =>
            {
                break;
            }
            Event::Paste(text) if size.0 >= 48 && size.1 >= 14 => app.paste(&text),
            Event::Resize(_, _) => previous = None,
            _ => {}
        }
    }
    Ok(())
}

fn base(root: &std::path::Path) -> Cli {
    Cli {
        config: None,
        state_dir: Some(root.to_owned()),
        json: true,
        command: None,
        ssh: None,
        container: None,
        remote_bin: "steward".into(),
    }
}
fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key].as_str().unwrap_or("")
}
fn identity(row: &Value) -> String {
    if let Some(id) = row["_managed_id"].as_str() {
        return format!("{}/managed/{id}", text(row, "binding"));
    }
    format!(
        "{}/{}/{}",
        text(row, "binding"),
        text(row, "native_id"),
        text(row, "id")
    )
}
fn inspection(tab: Tab, row: &Value) -> Vec<String> {
    match tab {
        Tab::Routes => vec![
            "routes",
            "inspect",
            "--proxy",
            text(row, "binding"),
            "--",
            text(row, "native_id"),
        ],
        Tab::Proxies => vec!["proxies", "inspect", text(row, "binding")],
        Tab::Services => vec!["services", "inspect", text(row, "id")],
        Tab::Operations => vec!["operations", "inspect", text(row, "id")],
    }
    .into_iter()
    .map(str::to_owned)
    .collect()
}
fn actions(tab: Tab, row: &Value) -> Vec<Action> {
    let mut actions = vec![Action {
        label: "Inspect resource",
        args: inspection(tab, row),
        effect: Some(Effect::Inspect),
        ..Default::default()
    }];
    let mut add = |label, args: Vec<&str>| {
        actions.push(Action {
            label,
            args: args.into_iter().map(str::to_owned).collect(),
            destructive: label.contains("Remove")
                || label.contains("Detach")
                || label.contains("Roll back"),
            ..Default::default()
        })
    };
    match tab {
        Tab::Routes if row["_managed_id"].is_string() => {
            let (id, proxy) = (text(row, "_managed_id"), text(row, "binding"));
            if row["_enabled"] == false {
                add(
                    "Enable route",
                    vec!["routes", "enable", id, "--proxy", proxy],
                );
            } else {
                add(
                    "Disable route",
                    vec!["routes", "disable", id, "--proxy", proxy],
                );
            }
            add(
                "Remove owned route",
                vec!["routes", "remove", id, "--proxy", proxy],
            );
        }
        Tab::Proxies if row["_detected"] != true => {
            add(
                "Remove connection (refuses owned routes)",
                vec!["proxies", "remove", text(row, "binding")],
            );
            add(
                "Detach and relinquish route ownership",
                vec!["proxies", "detach", text(row, "binding")],
            );
        }
        Tab::Services => {
            let id = text(row, "id");
            for (label, verb) in [
                ("Start service", "start"),
                ("Stop service", "stop"),
                ("Restart service", "restart"),
                ("Enable autostart", "enable"),
                ("Disable autostart", "disable"),
                ("Show recent logs", "logs"),
                ("Remove service", "remove"),
            ] {
                add(label, vec!["services", verb, id]);
            }
        }
        Tab::Operations => {
            if text(row, "phase") == "verified" {
                add(
                    "Roll back checked operation",
                    vec!["operations", "rollback", text(row, "id")],
                );
            } else if text(row, "phase") != "compensated" {
                if text(row, "kind") == "service" {
                    add(
                        "Recover unfinished service operation",
                        vec!["services", "recover"],
                    );
                } else {
                    add(
                        "Recover unfinished route operation",
                        vec!["operations", "recover", text(row, "id")],
                    );
                }
            }
        }
        _ => {}
    }
    actions
}
fn read_only(cli: &Cli) -> bool {
    matches!(
        &cli.command,
        Some(Command::Proxy(
            ProxyCommand::List
                | ProxyCommand::Inspect { .. }
                | ProxyCommand::InspectFile { .. }
                | ProxyCommand::Discover { .. }
        )) | Some(Command::Route(
            RouteCommand::List | RouteCommand::Inspect { .. }
        )) | Some(Command::Service(
            ServiceCommand::List
                | ServiceCommand::Inspect { .. }
                | ServiceCommand::InspectNative { .. }
                | ServiceCommand::Discover { .. }
                | ServiceCommand::Logs { .. }
        )) | Some(Command::Operations(
            OperationCommand::List | OperationCommand::Inspect { .. }
        )) | Some(
            Command::Doctor | Command::Export { out: None } | Command::Plan { out: None, .. }
        )
    )
}

fn execute_work(cli: &Cli, work: Work) -> Result<Envelope> {
    if matches!(work, Work::EditService)
        && let Some(Command::Service(ServiceCommand::Inspect { id })) = &cli.command
    {
        let spec = crate::services::edit_spec(cli.state_dir.as_ref().expect("state root"), id)?;
        return Ok(Envelope {
            schema_version: "1",
            status: "observed",
            changed: false,
            data: json!(spec),
            diagnostics: Vec::new(),
        });
    }
    let result = run(cli);
    let Work::List(tab @ (Tab::Routes | Tab::Proxies)) = work else {
        return result;
    };
    // Keep saved connections and disabled receipts visible when native inventory is
    // unavailable. UI-only fields never alter the CLI's JSON contract or authority.
    let store = managed::Store::open(cli.state_dir.clone().expect("TUI state directory"))?;
    let mut envelope = result.unwrap_or_else(|error| Envelope {
        schema_version: "1",
        status: "partial",
        changed: false,
        data: json!([]),
        diagnostics: vec![error],
    });
    let rows = envelope.data.as_array_mut().expect("list result");
    if tab == Tab::Proxies {
        for connection in &store.book.connections {
            let id = &connection.binding.id;
            if let Some(row) = rows.iter_mut().find(|r| r["binding"] == *id) {
                row["endpoint"] = json!(connection.binding.endpoint);
                row["_connection"] = json!(connection);
            } else {
                rows.push(json!({"binding":id,"engine":connection.binding.engine,"endpoint":connection.binding.endpoint,"complete":false,"runtime_confirmed":false,"_unavailable":true,"_connection":connection}));
            }
        }
        for row in rows.iter_mut() {
            if let Some(routes) = row.get("routes").and_then(Value::as_array) {
                row["route_count"] = json!(routes.len());
            }
            if let Some(map) = row.as_object_mut() {
                map.remove("routes");
            }
        }
    } else {
        for receipt in &store.book.routes {
            let engine = store
                .book
                .connections
                .iter()
                .find(|c| c.binding.id == receipt.spec.proxy)
                .map(|c| c.binding.engine);
            let native = match engine {
                Some(Engine::Caddy) => format!("{}:0", receipt.native_id),
                _ => format!("{}@file", receipt.native_id),
            };
            let matched = rows.iter().position(|r| {
                r["binding"] == receipt.spec.proxy
                    && (r["native_id"] == native
                        || matches!(r["ownership"].as_str(), Some("owned" | "owned_drifted"))
                            && r["hosts"] == json!([receipt.spec.hostname]))
            });
            let index = matched.unwrap_or_else(|| {
                rows.push(json!({"binding":receipt.spec.proxy,"native_id":native,"hosts":[receipt.spec.hostname],"targets":[receipt.spec.upstream.url],"ownership":"owned","source":"saved_receipt","runtime_confirmed":false,"native_status":if receipt.spec.enabled {"missing"} else {"disabled"}}));
                rows.len() - 1
            });
            // A native-ID collision alone does not grant actions over an external route.
            if rows[index]["ownership"] == "owned" {
                rows[index]["_managed_id"] = json!(receipt.spec.id);
                rows[index]["_enabled"] = json!(receipt.spec.enabled);
                rows[index]["_hostname"] = json!(receipt.spec.hostname);
                rows[index]["_upstream"] = json!(receipt.spec.upstream.url);
            }
        }
    }
    Ok(envelope)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_actions_preserve_identity_and_do_not_grant_external_ownership() {
        let mut route =
            json!({"binding":"edge", "native_id":"--external", "ownership":"observed_external"});
        assert_eq!(actions(Tab::Routes, &route).len(), 1);
        let cli = Cli::try_parse_from(
            std::iter::once("steward".into()).chain(inspection(Tab::Routes, &route)),
        )
        .unwrap();
        assert!(
            matches!(cli.command, Some(Command::Route(RouteCommand::Inspect { proxy, id })) if proxy == "edge" && id == "--external")
        );
        route["_managed_id"] = json!("photos");
        route["ownership"] = json!("owned");
        route["_enabled"] = json!(false);
        let owned_identity = identity(&route);
        route["native_id"] = json!("different-native-id-after-disable");
        assert_eq!(identity(&route), owned_identity);
        let items = actions(Tab::Routes, &route);
        let cli =
            Cli::try_parse_from(std::iter::once("steward".into()).chain(items[1].args.clone()))
                .unwrap();
        assert!(
            matches!(cli.command, Some(Command::Route(RouteCommand::Enable { proxy, id })) if proxy == "edge" && id == "photos")
        );

        let mut app = App::new("/tmp/unused-tui-test".into());
        route["source"] = json!("saved_receipt");
        app.rows.push(route);
        app.refilter();
        app.inspect();
        assert!(app.pending.is_none());
        assert!(matches!(app.screen, Screen::Browse));
        assert!(app.focus == Focus::Inspector);
        assert_eq!(
            app.inspector.as_ref().unwrap().data["source"],
            "saved_receipt"
        );
    }
}
