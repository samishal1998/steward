//! Persistent panel state, background scheduling and keyboard workflows.
use super::*;
use form::{Argument as Arg, Field};

impl App {
    pub(super) fn panel(&self, tab: Tab) -> PanelView<'_> {
        if tab == self.tab {
            PanelView {
                rows: &self.rows,
                visible: &self.visible,
                selection: self.selection,
                filter: &self.filter,
                diagnostics: &self.diagnostics,
                updated: self.updated,
            }
        } else {
            let p = &self.panels[tab.index()];
            PanelView {
                rows: &p.rows,
                visible: &p.visible,
                selection: p.selection,
                filter: &p.filter,
                diagnostics: &p.diagnostics,
                updated: p.updated,
            }
        }
    }
    pub(super) fn stash(&mut self) {
        self.panels[self.tab.index()] = Panel {
            rows: std::mem::take(&mut self.rows),
            visible: std::mem::take(&mut self.visible),
            selection: self.selection,
            filter: std::mem::take(&mut self.filter),
            diagnostics: std::mem::take(&mut self.diagnostics),
            updated: self.updated,
        };
    }
    pub(super) fn restore(&mut self) {
        let p = std::mem::take(&mut self.panels[self.tab.index()]);
        self.rows = p.rows;
        self.visible = p.visible;
        self.selection = p.selection;
        self.filter = p.filter;
        self.diagnostics = p.diagnostics;
        self.updated = p.updated;
        self.refilter();
        self.inspect_scroll = 0;
    }
    pub(super) fn log(&mut self, message: &str) {
        if self.activity.len() == 128 {
            self.activity.pop_front();
        }
        let seconds = crate::transport::now().parse::<u64>().unwrap_or(0) % 86400;
        self.activity.push_back(format!(
            "{:02}:{:02}:{:02}  {}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60,
            view::clip(message, 512)
        ));
        self.activity_scroll = 0;
    }
    pub(super) fn schedule_background(&mut self) -> bool {
        if self.pending.is_some()
            || self.queued.is_some()
            || self.quit_after_work
            || !matches!(self.screen, Screen::Browse)
        {
            return false;
        }
        if self.refresh {
            return false;
        }
        if self.scan_requested
            || self.auto_refresh
                && self
                    .scanned
                    .is_none_or(|t| t.elapsed() >= Duration::from_secs(30))
        {
            self.scan_requested = false;
            self.scanned = Some(Instant::now());
            let mut cli = base(&self.root);
            cli.command = Some(Command::Proxy(ProxyCommand::Discover {
                docker: None,
                probe: true,
            }));
            self.launch(cli, Work::Discover, "Auto-detecting local routers".into());
            return true;
        }
        if self.mode == InspectMode::Logs
            && self.follow_logs
            && self.tab == Tab::Services
            && self
                .logs_updated
                .is_none_or(|t| t.elapsed() >= Duration::from_secs(2))
            && let Some(row) = self.selected()
        {
            let args = vec![
                "services".into(),
                "logs".into(),
                text(row, "id").into(),
                "--lines".into(),
                "100".into(),
            ];
            self.logs_updated = Some(Instant::now());
            self.command(args, Work::Logs, "Service logs");
            return true;
        }
        let tab = if self.auto_refresh
            && self
                .updated
                .is_some_and(|t| t.elapsed() >= Duration::from_secs(3))
        {
            Some(self.tab)
        } else {
            Tab::ALL.into_iter().find(|tab| {
                self.panel(*tab).updated.is_none()
                    || self.auto_refresh
                        && self
                            .panel(*tab)
                            .updated
                            .is_some_and(|t| t.elapsed() >= Duration::from_secs(15))
            })
        };
        if let Some(tab) = tab {
            let mut cli = base(&self.root);
            cli.command = Some(tab.command());
            self.launch(cli, Work::List(tab), format!("Refreshing {}", tab.name()));
            return true;
        }
        false
    }
    pub(super) fn discovered(&mut self, result: Result<Envelope>) {
        match result {
            Ok(result) => {
                let n = result.data["candidates"].as_array().map_or(0, Vec::len);
                self.detected = result.data;
                self.scanned = Some(Instant::now());
                let warnings = self.detected["diagnostics"].as_array().map_or(0, Vec::len);
                self.log(&format!(
                    "Discovery complete: {n} local router candidates; {warnings} scan diagnostics"
                ));
                let active = self.tab;
                for tab in [Tab::Proxies, Tab::Routes] {
                    if self.tab != tab {
                        self.stash();
                        self.tab = tab;
                        self.restore();
                    }
                    self.merge_discovery();
                }
                if self.tab != active {
                    self.stash();
                    self.tab = active;
                    self.restore();
                }
            }
            Err(error) => self.log(&format!("Discovery failed: {error}")),
        }
    }
    pub(super) fn merge_discovery(&mut self) {
        if !matches!(self.tab, Tab::Proxies | Tab::Routes) {
            return;
        }
        let previous = self.selected().map(identity);
        self.rows.retain(|r| r["_detected"] != true);
        if self.tab == Tab::Proxies
            && let Some(candidate) = self.detected["candidates"].as_array().and_then(|a| {
                a.iter()
                    .find(|c| self.scope.as_deref() == c["binding"].as_str())
            })
            && let Some(saved) = self
                .rows
                .iter()
                .find(|r| r["endpoint"] == candidate["endpoint"])
        {
            self.scope = Some(text(saved, "binding").to_owned());
        }
        let stored: Vec<_> = if self.tab == Tab::Proxies {
            self.rows
                .iter()
                .map(|r| text(r, "endpoint").trim_end_matches('/').to_owned())
                .collect()
        } else {
            self.panel(Tab::Proxies)
                .rows
                .iter()
                .filter(|r| r["_detected"] != true)
                .map(|r| text(r, "endpoint").trim_end_matches('/').to_owned())
                .collect()
        };
        if let Some(candidates) = self.detected["candidates"].as_array() {
            for candidate in candidates {
                let endpoint = text(candidate, "endpoint");
                if !endpoint.is_empty()
                    && stored.iter().any(|s| s == endpoint.trim_end_matches('/'))
                {
                    continue;
                }
                if self.tab == Tab::Proxies {
                    let mut row = candidate.clone();
                    row["_detected"] = json!(true);
                    self.rows.push(row);
                } else if let Some(inventory) = self.detected["inventories"]
                    .as_array()
                    .and_then(|a| a.iter().find(|i| i["binding"] == candidate["binding"]))
                    && let Some(routes) = inventory["routes"].as_array()
                {
                    for route in routes {
                        let mut route = route.clone();
                        route["_detected"] = json!(true);
                        route["_endpoint"] = json!(endpoint);
                        self.rows.push(route);
                    }
                }
            }
        }
        if self.tab == Tab::Proxies {
            self.rows.sort_by(|a, b| {
                (
                    a["runtime_confirmed"] != true,
                    a["_detected"] == true,
                    text(a, "binding"),
                )
                    .cmp(&(
                        b["runtime_confirmed"] != true,
                        b["_detected"] == true,
                        text(b, "binding"),
                    ))
            });
        }
        self.refilter();
        if let Some(index) = self
            .visible
            .iter()
            .position(|i| previous.as_ref() == Some(&identity(&self.rows[*i])))
        {
            self.selection = index;
        }
    }
    fn scope_router(&mut self) {
        if self.tab == Tab::Proxies {
            self.scope = self.selected().map(|r| text(r, "binding").to_owned());
            self.switch(Tab::Routes);
        }
    }
    pub(super) fn connect_selected(&mut self) {
        self.screen = Screen::Form(
            self.selected()
                .filter(|r| r["_detected"] == true && self.tab == Tab::Proxies)
                .map_or_else(Form::connect, Form::detected),
        );
    }
    fn edit_selected(&mut self) {
        let Some(row) = self.selected().cloned() else {
            return;
        };
        match self.tab {
            Tab::Proxies if row["_connection"].is_object() => {
                self.screen = Screen::Form(Form::connection(&row["_connection"]))
            }
            Tab::Proxies => self.connect_selected(),
            Tab::Services => self.command(
                vec!["services".into(), "inspect".into(), text(&row, "id").into()],
                Work::EditService,
                "Load service editor",
            ),
            Tab::Routes if row["_managed_id"].is_string() => {
                self.screen = Screen::Form(Form::route(
                    text(&row, "binding"),
                    Some((
                        text(&row, "_managed_id"),
                        text(&row, "_hostname"),
                        text(&row, "_upstream"),
                    )),
                ))
            }
            _ => self.notice = "Select an owned resource to edit.".into(),
        }
    }
    fn show_logs(&mut self) {
        if self.tab != Tab::Services || self.selected().is_none() {
            self.notice = "Select a managed service to follow its native logs.".into();
            return;
        }
        self.mode = InspectMode::Logs;
        self.focus = Focus::Inspector;
        self.logs_updated = None;
        self.follow_logs = true;
    }
    pub(super) fn perform(&mut self, action: Action) {
        if action.destructive {
            self.screen = Screen::Confirm(action);
            return;
        }
        self.screen = Screen::Browse;
        if let Some(form) = action.form {
            self.screen = Screen::Form(form);
            return;
        }
        match action.effect {
            Some(Effect::Inspect) => self.inspect(),
            Some(Effect::Edit) => self.edit_selected(),
            Some(Effect::Logs) => self.show_logs(),
            Some(Effect::Rescan) => self.scan_requested = true,
            Some(Effect::Pause) => self.auto_refresh = !self.auto_refresh,
            Some(Effect::Zoom) => {
                self.zoom = !self.zoom;
                self.focus = Focus::Inspector;
            }
            Some(Effect::Diagnostics) => {
                self.screen = Screen::Details(Details {
                    title: "Inventory diagnostics".into(),
                    data: self.detected["diagnostics"].clone(),
                    diagnostics: self.diagnostics.clone(),
                    status: "Diagnostics".into(),
                    changed: false,
                    scroll: 0,
                    raw: false,
                })
            }
            None => self.command(action.args, Work::Command, action.label),
        }
    }
    fn palette(&mut self) {
        let mut choices = self
            .selected()
            .map_or_else(Vec::new, |r| actions(self.tab, r));
        if self.selected().is_some()
            && matches!(self.tab, Tab::Proxies | Tab::Services | Tab::Routes)
        {
            choices.push(Action {
                label: "Edit selected resource",
                effect: Some(Effect::Edit),
                ..Default::default()
            });
        }
        if self.tab == Tab::Services {
            choices.push(Action {
                label: "Follow service logs",
                effect: Some(Effect::Logs),
                ..Default::default()
            });
        }
        choices.extend(self.workflows());
        self.screen = Screen::Actions {
            actions: choices,
            selected: 0,
            query: Editor::default(),
        };
    }
    pub(super) fn workspace_key(
        &mut self,
        key: KeyEvent,
        width: usize,
        page: usize,
    ) -> Option<bool> {
        if let Screen::Confirm(action) = &self.screen {
            match key.code {
                KeyCode::Enter => {
                    let mut action = action.clone();
                    action.destructive = false;
                    self.perform(action);
                }
                KeyCode::Esc | KeyCode::Char('n') => self.screen = Screen::Browse,
                _ => {}
            }
            return Some(false);
        }
        if matches!(self.screen, Screen::Files { .. }) {
            self.file_key(key, page);
            return Some(false);
        }
        if matches!(self.screen, Screen::Form(_))
            && key.code == KeyCode::Char('o')
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            if !self.pending.as_ref().is_some_and(|p| p.mutating) {
                self.open_files();
            }
            return Some(false);
        }
        if let Screen::Actions {
            actions,
            selected,
            query,
        } = &mut self.screen
        {
            let visible: Vec<_> = actions
                .iter()
                .enumerate()
                .filter(|(_, a)| action_matches(a, &query.text))
                .map(|(i, _)| i)
                .collect();
            match key.code {
                KeyCode::Esc => self.screen = Screen::Browse,
                KeyCode::Up => *selected = selected.saturating_sub(1),
                KeyCode::Down => *selected = (*selected + 1).min(visible.len().saturating_sub(1)),
                KeyCode::Enter => {
                    if let Some(index) = visible.get(*selected) {
                        let action = actions[*index].clone();
                        self.perform(action);
                    }
                }
                _ => {
                    query.key(key);
                    *selected = 0;
                }
            }
            return Some(false);
        }
        if !matches!(self.screen, Screen::Browse) {
            return None;
        }
        if self.pending.as_ref().is_some_and(|p| p.mutating)
            && matches!(
                key.code,
                KeyCode::Char('p' | 'P' | 'A' | 'u' | 'D' | 'I' | 'S' | 'e' | 'n' | 'c' | ':')
            )
        {
            self.notice = "A checked change is running; progress is in Activity.".into();
            return Some(false);
        }
        if key.code == KeyCode::Char('p')
            || key.code == KeyCode::Char('P') && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            self.palette();
            return Some(false);
        }
        match key.code {
            KeyCode::Tab | KeyCode::BackTab | KeyCode::Char('h' | 'l') => {
                let current = match self.focus {
                    Focus::Resources => self.tab.index(),
                    Focus::Inspector => 4,
                    Focus::Activity => 5,
                };
                let backwards = matches!(key.code, KeyCode::BackTab | KeyCode::Char('h'));
                let next = (current + if backwards { 5 } else { 1 }) % 6;
                if next < 4 {
                    self.switch(Tab::ALL[next]);
                } else {
                    self.focus = if next == 4 {
                        Focus::Inspector
                    } else {
                        Focus::Activity
                    };
                }
                return Some(false);
            }
            KeyCode::Char('5') => {
                self.focus = Focus::Inspector;
                return Some(false);
            }
            KeyCode::Char('6') => {
                self.focus = Focus::Activity;
                return Some(false);
            }
            KeyCode::Char('0') => {
                self.scope = None;
                self.refilter();
                return Some(false);
            }
            KeyCode::Char(' ') => {
                self.auto_refresh = !self.auto_refresh;
                return Some(false);
            }
            KeyCode::Char('z') => {
                self.zoom = !self.zoom;
                self.focus = Focus::Inspector;
                return Some(false);
            }
            KeyCode::Char('v') => {
                self.mode = if self.mode == InspectMode::Json {
                    InspectMode::Overview
                } else {
                    InspectMode::Json
                };
                return Some(false);
            }
            KeyCode::Char('[' | ']') => {
                let modes = [
                    InspectMode::Overview,
                    InspectMode::Logs,
                    InspectMode::Json,
                    InspectMode::Result,
                ];
                let i = modes.iter().position(|m| *m == self.mode).unwrap_or(0);
                self.mode = modes[(i + if key.code == KeyCode::Char('[') { 3 } else { 1 }) % 4];
                self.inspect_scroll = 0;
                self.focus = Focus::Inspector;
                if self.mode == InspectMode::Logs {
                    self.logs_updated = None;
                }
                return Some(false);
            }
            KeyCode::Char('L') => {
                self.show_logs();
                return Some(false);
            }
            KeyCode::Char('f') if self.mode == InspectMode::Logs => {
                self.follow_logs = !self.follow_logs;
                return Some(false);
            }
            KeyCode::Char('e') => {
                self.edit_selected();
                return Some(false);
            }
            KeyCode::Char('R') => {
                self.scan_requested = true;
                self.refresh = true;
                return Some(false);
            }
            KeyCode::Char('P' | 'A' | 'u' | 'D' | 'I' | 'S') => {
                let label = match key.code {
                    KeyCode::Char('P') => "Create saved plan",
                    KeyCode::Char('A') => "Apply saved plan",
                    KeyCode::Char('u') => "Bring stack up",
                    KeyCode::Char('D') => "Bring stack down",
                    KeyCode::Char('I') => "Install router",
                    _ => "Set up existing router",
                };
                if let Some(action) = self.workflows().into_iter().find(|a| a.label == label) {
                    self.perform(action);
                }
                return Some(false);
            }
            KeyCode::Enter if self.focus == Focus::Resources && self.tab == Tab::Proxies => {
                self.scope_router();
                return Some(false);
            }
            KeyCode::Esc if self.focus != Focus::Resources => {
                self.focus = Focus::Resources;
                self.zoom = false;
                return Some(false);
            }
            _ => {}
        }
        if self.focus == Focus::Activity {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.activity_scroll =
                        (self.activity_scroll + 1).min(self.activity.len().saturating_sub(1))
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.activity_scroll = self.activity_scroll.saturating_sub(1)
                }
                KeyCode::Home => self.activity_scroll = self.activity.len().saturating_sub(1),
                KeyCode::End => self.activity_scroll = 0,
                KeyCode::Enter => {
                    self.focus = Focus::Inspector;
                    self.mode = InspectMode::Result;
                }
                _ => return None,
            }
            return Some(false);
        }
        if self.focus == Focus::Inspector {
            let area = view::layout(self, (width as u16, (page + 11) as u16)).inspector;
            let page = area.h.saturating_sub(5).max(1);
            let details = view::inspector_details(self);
            let end = view::detail_lines(&details, area.w.saturating_sub(4))
                .len()
                .saturating_sub(page);
            self.inspect_scroll = self.inspect_scroll.min(end);
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.inspect_scroll = self.inspect_scroll.saturating_sub(1);
                    self.follow_logs = false;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.inspect_scroll = (self.inspect_scroll + 1).min(end)
                }
                KeyCode::PageUp => self.inspect_scroll = self.inspect_scroll.saturating_sub(page),
                KeyCode::PageDown => self.inspect_scroll = (self.inspect_scroll + page).min(end),
                KeyCode::Home => self.inspect_scroll = 0,
                KeyCode::End => self.inspect_scroll = end,
                KeyCode::Enter => self.inspect(),
                _ => return None,
            }
            return Some(false);
        }
        None
    }

    fn open_files(&mut self) {
        let Screen::Form(form) = std::mem::replace(&mut self.screen, Screen::Browse) else {
            return;
        };
        let input = PathBuf::from(&form.fields[form.selected].editor.text);
        let directory = if input.is_dir() {
            input
        } else {
            input
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| "/".into()))
        };
        self.set_files(form, directory);
    }
    fn set_files(&mut self, mut form: Form, directory: PathBuf) {
        match std::fs::read_dir(&directory) {
            Ok(entries) => {
                let mut entries: Vec<_> = entries
                    .take(512)
                    .filter_map(|e| e.ok())
                    .map(|e| {
                        (
                            e.file_name().to_string_lossy().into_owned(),
                            e.file_type().is_ok_and(|t| t.is_dir()),
                        )
                    })
                    .collect();
                entries.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
                self.screen = Screen::Files {
                    form,
                    directory,
                    entries,
                    selected: 0,
                };
            }
            Err(_) => {
                form.error = "Cannot read that directory".into();
                self.screen = Screen::Form(form);
            }
        }
    }
    fn file_key(&mut self, key: KeyEvent, page: usize) {
        let Screen::Files {
            mut form,
            directory,
            entries,
            mut selected,
        } = std::mem::replace(&mut self.screen, Screen::Browse)
        else {
            return;
        };
        match key.code {
            KeyCode::Esc => {
                self.screen = Screen::Form(form);
                return;
            }
            KeyCode::Backspace => {
                let parent = directory.parent().unwrap_or(&directory).to_owned();
                self.set_files(form, parent);
                return;
            }
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                form.fields[form.selected].editor = Editor::new(directory.display().to_string());
                self.screen = Screen::Form(form);
                return;
            }
            KeyCode::Enter => {
                if let Some((name, folder)) = entries.get(selected) {
                    let path = directory.join(name);
                    if *folder {
                        self.set_files(form, path);
                    } else {
                        form.fields[form.selected].editor = Editor::new(path.display().to_string());
                        form.error.clear();
                        self.screen = Screen::Form(form);
                    }
                    return;
                }
            }
            KeyCode::Up => selected = selected.saturating_sub(1),
            KeyCode::Down => selected = (selected + 1).min(entries.len().saturating_sub(1)),
            KeyCode::PageUp => selected = selected.saturating_sub(page),
            KeyCode::PageDown => selected = (selected + page).min(entries.len().saturating_sub(1)),
            _ => {}
        }
        self.screen = Screen::Files {
            form,
            directory,
            entries,
            selected,
        };
    }

    pub(super) fn workflows(&self) -> Vec<Action> {
        let make = |label, form| Action {
            label,
            form: Some(form),
            ..Default::default()
        };
        let field =
            |label, hint, default, flag| (Field::text(label, hint, default), Arg::Required(flag));
        let mut result = vec![
            make(
                "Connect router",
                self.selected()
                    .filter(|r| self.tab == Tab::Proxies && r["_detected"] == true)
                    .map_or_else(Form::connect, Form::detected),
            ),
            make(
                "Add route",
                Form::route(self.scope.as_deref().unwrap_or(""), None),
            ),
            make("Add native service", Form::service()),
            make(
                "Discover native services",
                Form::workflow(
                    "Discover native services",
                    &["services", "discover"],
                    vec![(
                        Field::choice(
                            "Scope",
                            "Read external native services in this scope.",
                            &["user", "system"],
                        ),
                        Arg::Required("--scope"),
                    )],
                ),
            ),
            make(
                "Inspect external native service",
                Form::workflow(
                    "Inspect native service",
                    &["services", "inspect-native"],
                    vec![
                        (
                            Field::text(
                                "Native name",
                                "Exact systemd unit or launchd label from discovery.",
                                "",
                            ),
                            Arg::Positional,
                        ),
                        (
                            Field::choice("Scope", "Native manager scope.", &["user", "system"]),
                            Arg::Required("--scope"),
                        ),
                    ],
                ),
            ),
            make(
                "Inspect native configuration file",
                Form::workflow(
                    "Inspect native file",
                    &["proxies", "inspect-file"],
                    vec![
                        field(
                            "Native file",
                            "Read-only offline evidence. Ctrl-O browses files.",
                            "",
                            "--file",
                        ),
                        (
                            Field::choice(
                                "Engine",
                                "Choose the native format.",
                                &["caddy", "traefik"],
                            ),
                            Arg::Required("--engine"),
                        ),
                        (
                            Field::text(
                                "Caddy executable",
                                "Validator for Caddyfile adaptation.",
                                "caddy",
                            ),
                            Arg::Optional("--caddy-bin"),
                        ),
                    ],
                ),
            ),
            make(
                "Create saved plan",
                Form::workflow(
                    "Create saved plan",
                    &["plan"],
                    vec![
                        field(
                            "Manifest file",
                            "Existing JSON/YAML intent; Ctrl-O browses files.",
                            "",
                            "--file",
                        ),
                        field(
                            "Save plan to",
                            "New absolute output file. Inspect the result before applying.",
                            "",
                            "--out",
                        ),
                    ],
                ),
            ),
            make(
                "Apply saved plan",
                Form::workflow(
                    "Apply reviewed plan",
                    &["apply", "--yes"],
                    vec![field(
                        "Plan file",
                        "Saved plan; native/state fingerprints are rechecked before applying.",
                        self.plan.as_ref().and_then(|p| p.to_str()).unwrap_or(""),
                        "--plan",
                    )],
                ),
            ),
            make(
                "Bring stack up",
                Form::workflow(
                    "Bring stack up",
                    &["up"],
                    vec![field(
                        "Stack file",
                        "Services become ready before routes are published. Ctrl-O browses files.",
                        "",
                        "--file",
                    )],
                ),
            ),
            make(
                "Bring stack down",
                Form::workflow(
                    "Bring stack down",
                    &["down"],
                    vec![
                        (
                            Field::text("Stack ID", "Existing composition receipt ID.", ""),
                            Arg::Positional,
                        ),
                        (
                            Field::choice(
                                "Remove services",
                                "Remove only unchanged stack-created services.",
                                &["no", "yes"],
                            ),
                            Arg::Switch("--remove-services"),
                        ),
                    ],
                ),
            ),
            make(
                "Export inventory",
                Form::workflow(
                    "Export inventory",
                    &["export"],
                    vec![field(
                        "Output file",
                        "New absolute redacted inventory file for AppTrail.",
                        "",
                        "--out",
                    )],
                ),
            ),
            Action {
                label: "Doctor / inspect inventory",
                args: vec!["doctor".into()],
                ..Default::default()
            },
            Action {
                label: "Rescan local routers",
                effect: Some(Effect::Rescan),
                ..Default::default()
            },
            Action {
                label: "Pause / resume automatic refresh",
                effect: Some(Effect::Pause),
                ..Default::default()
            },
            Action {
                label: "Zoom inspector",
                effect: Some(Effect::Zoom),
                ..Default::default()
            },
            Action {
                label: "Show diagnostics",
                effect: Some(Effect::Diagnostics),
                ..Default::default()
            },
        ];
        for install in [true, false] {
            let label = if install {
                "Install router"
            } else {
                "Set up existing router"
            };
            result.push(make(
                label,
                Form::workflow(
                    label,
                    &["proxies", if install { "install" } else { "setup" }],
                    vec![
                        (
                            Field::text("Router ID", "New installation and connection ID.", ""),
                            Arg::Positional,
                        ),
                        (
                            Field::choice(
                                "Engine",
                                "Choose the native router.",
                                &["caddy", "traefik"],
                            ),
                            Arg::Required("--engine"),
                        ),
                        field(
                            if install { "Version" } else { "Executable" },
                            if install {
                                "Explicit upstream version, e.g. 2.11.7 or 3.7.14."
                            } else {
                                "Existing native executable. Ctrl-O browses files."
                            },
                            "",
                            if install { "--version" } else { "--binary" },
                        ),
                        field(
                            "Listen",
                            "Explicit frontend IP:port.",
                            "127.0.0.1:8088",
                            "--listen",
                        ),
                        field(
                            "Admin",
                            "Distinct loopback admin IP:port.",
                            "127.0.0.1:22019",
                            "--admin",
                        ),
                        (
                            Field::choice(
                                "Scope",
                                "Native service scope; current OS permissions apply.",
                                &["user", "system"],
                            ),
                            Arg::Required("--scope"),
                        ),
                        (
                            Field::choice(
                                "TLS",
                                "Public/internal HTTPS requires certificate settings.",
                                &["http", "internal", "public"],
                            ),
                            Arg::Required("--tls"),
                        ),
                        (
                            Field::text("ACME email", "Public ACME contact, when required.", ""),
                            Arg::Optional("--acme-email"),
                        ),
                        (
                            Field::text("CA file", "Explicit internal CA trust reference.", ""),
                            Arg::Optional("--ca-file"),
                        ),
                        (
                            Field::text("Certificate", "Traefik certificate file.", ""),
                            Arg::Optional("--certificate"),
                        ),
                        (
                            Field::text("Private-key file", "Traefik key file.", ""),
                            Arg::Optional("--private-key"),
                        ),
                    ],
                ),
            ));
        }
        if let Some(row) = self.selected() {
            if self.tab == Tab::Proxies && row["_detected"] != true {
                let id = text(row, "binding");
                result.push(make(
                    "Upgrade installed router",
                    Form::workflow(
                        "Upgrade router",
                        &["proxies", "upgrade", id],
                        vec![field(
                            "Version",
                            "Explicit upstream version; installation receipt is required.",
                            "",
                            "--version",
                        )],
                    ),
                ));
                result.push(Action {
                    label: "Uninstall managed router",
                    args: vec!["proxies".into(), "uninstall".into(), id.into()],
                    destructive: true,
                    ..Default::default()
                });
                result.push(Action {
                    label: "Recover router installation",
                    args: vec!["proxies".into(), "recover".into(), id.into()],
                    ..Default::default()
                });
            }
            if self.tab == Tab::Routes && row["_detected"] != true && row["_managed_id"].is_null() {
                result.push(make("Adopt inspected external route", Form::workflow("Adopt external route", &["routes", "adopt"], vec![
                    (Field::text("Owned route ID", "New receipt ID for this explicitly adopted route.", ""), Arg::Positional),
                    field("Proxy", "Saved connection with native file authority.", text(row, "binding"), "--proxy"),
                    field("Native ID", "Exact inspected native route identity.", text(row, "native_id"), "--native-id"),
                    field("Fingerprint", "Expected inspected native fingerprint; checked again before adopting.", text(row, "native_fingerprint"), "--expected-fingerprint"),
                    (Field::text("Source file", "Dedicated Traefik file, if required.", ""), Arg::Optional("--source")),
                ])));
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_detected_routers_sort_first_and_survive_background_refresh_selection() {
        let mut app = App::new("/tmp/unused-workspace-test".into());
        app.detected = json!({"candidates":[
            {"binding":"dead","endpoint":"http://127.0.0.1:1","runtime_confirmed":false},
            {"binding":"live-a","endpoint":"http://127.0.0.1:2","runtime_confirmed":true},
            {"binding":"live-b","endpoint":"http://127.0.0.1:3","runtime_confirmed":true}
        ]});
        app.merge_discovery();
        assert_eq!(app.selected().unwrap()["binding"], "live-a");
        app.selection = 1;
        let (sender, receiver) = mpsc::sync_channel(1);
        assert!(
            sender
                .send(Ok(Envelope {
                    schema_version: "1",
                    status: "observed",
                    changed: false,
                    data: json!([]),
                    diagnostics: Vec::new()
                }))
                .is_ok()
        );
        app.pending = Some(Pending {
            receiver,
            work: Work::List(Tab::Proxies),
            label: "test refresh".into(),
            mutating: false,
            started: Instant::now(),
            tab: Tab::Proxies,
            key: None,
            plan: None,
        });
        app.refresh = false;
        app.screen = Screen::Form(Form::connect());
        app.pump();
        assert_eq!(app.selected().unwrap()["binding"], "live-b");
    }
    #[test]
    fn panels_keep_independent_selection_and_filters() {
        let mut app = App::new("/tmp/unused-workspace-test".into());
        app.switch(Tab::Routes);
        app.rows = vec![
            json!({"binding":"edge","native_id":"a","hosts":["a.test"]}),
            json!({"binding":"edge","native_id":"b","hosts":["b.test"]}),
        ];
        app.filter = "b.test".into();
        app.refilter();
        app.switch(Tab::Services);
        app.rows = vec![json!({"id":"api","running":true})];
        app.filter = "api".into();
        app.refilter();
        app.switch(Tab::Routes);
        assert_eq!(app.filter, "b.test");
        assert_eq!(app.selected().unwrap()["native_id"], "b");
        app.switch(Tab::Services);
        assert_eq!(app.filter, "api");
        assert_eq!(app.selected().unwrap()["id"], "api");
    }

    #[test]
    fn workspace_forms_match_the_actual_cli_contract() {
        let app = App::new("/tmp/unused-workspace-test".into());
        let mut count = 0;
        for action in app.workflows() {
            let Some(mut form) = action.form else {
                continue;
            };
            for field in &mut form.fields {
                if !field.editor.text.is_empty() {
                    continue;
                }
                let value = match field.label {
                    "Connection ID" | "Router ID" | "Service ID" | "Route ID" | "Stack ID" => {
                        "demo"
                    }
                    "Proxy" => "edge",
                    "Hostname" => "demo.localhost",
                    "Version" => "2.11.7",
                    "Executable" => "/bin/true",
                    "Manifest file" | "Save plan to" | "Plan file" | "Stack file"
                    | "Output file" | "Native file" => "/tmp/example.json",
                    "Native name" => "demo.service",
                    _ => continue,
                };
                field.editor = Editor::new(value);
            }
            let args = form
                .arguments()
                .unwrap_or_else(|e| panic!("{}: {e}", action.label));
            Cli::try_parse_from(std::iter::once("steward".into()).chain(args))
                .unwrap_or_else(|e| panic!("{}: {e}", action.label));
            count += 1;
        }
        assert_eq!(count, 13);
    }
}
