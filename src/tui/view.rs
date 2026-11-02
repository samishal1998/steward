//! Lazy-style persistent panels and modal workflows, clipped to terminal cells.
use super::*;
use crossterm::{
    queue,
    style::{Color, Print, SetForegroundColor},
    terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate},
};
use std::io::Write;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tone {
    Normal,
    Muted,
    Accent,
    Selected,
    Error,
}
#[derive(PartialEq, Eq)]
struct Paint {
    x: u16,
    y: u16,
    text: String,
    tone: Tone,
}
#[derive(PartialEq, Eq)]
pub(super) struct Frame {
    size: (u16, u16),
    paint: Vec<Paint>,
    cursor: Option<(u16, u16)>,
}
#[derive(Clone, Copy, Default)]
pub(super) struct Rect {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}
pub(super) struct Layout {
    pub panels: [Rect; 4],
    pub inspector: Rect,
    pub activity: Rect,
}

pub(super) fn layout(app: &App, size: (u16, u16)) -> Layout {
    let w = usize::from(size.0).saturating_sub(1);
    let h = usize::from(size.1).saturating_sub(5);
    let mut out = Layout {
        panels: [Rect::default(); 4],
        inspector: Rect::default(),
        activity: Rect::default(),
    };
    if app.zoom {
        out.inspector = Rect { x: 0, y: 2, w, h };
        return out;
    }
    if w >= 71 && h >= 15 {
        let left = (w * 36 / 100).clamp(27, 42);
        let mut y = 2;
        for tab in Tab::ALL {
            let height = if tab == app.tab { h - 9 } else { 3 };
            out.panels[tab.index()] = Rect {
                x: 0,
                y,
                w: left,
                h: height,
            };
            y += height;
        }
        let activity = (h / 4).clamp(4, 8);
        out.inspector = Rect {
            x: left,
            y: 2,
            w: w - left,
            h: h - activity,
        };
        out.activity = Rect {
            x: left,
            y: 2 + h - activity,
            w: w - left,
            h: activity,
        };
    } else {
        let list = (h / 3).max(3);
        out.panels[app.tab.index()] = Rect {
            x: 0,
            y: 2,
            w,
            h: list,
        };
        out.inspector = Rect {
            x: 0,
            y: 2 + list,
            w,
            h: h.saturating_sub(list),
        };
    }
    out
}
impl Frame {
    fn put(&mut self, x: usize, y: usize, width: usize, text: &str, tone: Tone) {
        if y >= self.size.1 as usize || x >= self.size.0 as usize {
            return;
        }
        let width = width.min((self.size.0 as usize).saturating_sub(x + 1));
        let text = clip(text, width);
        let padding = width.saturating_sub(text.width());
        self.paint.push(Paint {
            x: x as u16,
            y: y as u16,
            text: format!("{text}{}", " ".repeat(padding)),
            tone,
        });
    }
    fn block(&mut self, r: Rect, title: &str, focused: bool) {
        if r.w < 2 || r.h < 2 {
            return;
        }
        let tone = if focused { Tone::Accent } else { Tone::Muted };
        let (tl, tr, bl, br, h, v) = if focused {
            ("┏", "┓", "┗", "┛", "━", "┃")
        } else {
            ("┌", "┐", "└", "┘", "─", "│")
        };
        let title = clip(title, r.w.saturating_sub(4));
        self.put(
            r.x,
            r.y,
            r.w,
            &format!(
                "{tl} {title} {}{tr}",
                h.repeat((r.w - 4).saturating_sub(title.width()))
            ),
            tone,
        );
        for y in r.y + 1..r.y + r.h - 1 {
            self.put(r.x, y, r.w, &format!("{v}{}{v}", " ".repeat(r.w - 2)), tone);
        }
        self.put(
            r.x,
            r.y + r.h - 1,
            r.w,
            &format!("{bl}{}{br}", h.repeat(r.w - 2)),
            tone,
        );
    }
    pub fn draw(&self) -> io::Result<()> {
        let mut out = io::stdout().lock();
        queue!(
            out,
            BeginSynchronizedUpdate,
            cursor::Hide,
            terminal::Clear(terminal::ClearType::All)
        )?;
        for p in &self.paint {
            queue!(
                out,
                cursor::MoveTo(p.x, p.y),
                ResetColor,
                SetAttribute(Attribute::Reset)
            )?;
            match p.tone {
                Tone::Normal => {}
                Tone::Muted => queue!(out, SetAttribute(Attribute::Dim))?,
                Tone::Accent => queue!(
                    out,
                    SetForegroundColor(Color::Cyan),
                    SetAttribute(Attribute::Bold)
                )?,
                Tone::Selected => queue!(out, SetAttribute(Attribute::Reverse))?,
                Tone::Error => queue!(
                    out,
                    SetForegroundColor(Color::Red),
                    SetAttribute(Attribute::Bold)
                )?,
            }
            queue!(out, Print(&p.text))?;
        }
        queue!(out, ResetColor, SetAttribute(Attribute::Reset))?;
        if let Some((x, y)) = self.cursor {
            queue!(out, cursor::MoveTo(x, y), cursor::Show)?;
        }
        queue!(out, EndSynchronizedUpdate)?;
        out.flush()
    }
}
fn clean(text: &str) -> String {
    text.chars()
        .take(32768)
        .flat_map(|c| {
            if c.is_control() {
                c.escape_default().collect::<Vec<_>>()
            } else {
                vec![c]
            }
        })
        .collect()
}
pub(super) fn clip(text: &str, width: usize) -> String {
    let text = clean(text);
    if text.width() <= width {
        return text;
    }
    if width == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in text.chars() {
        let cells = c.width().unwrap_or(0);
        if used + cells > width - 1 {
            break;
        }
        out.push(c);
        used += cells;
    }
    out.push('…');
    out
}
fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut out = Vec::new();
    for line in text.lines() {
        let mut row = String::new();
        let mut used = 0;
        for c in clean(line).chars() {
            let cells = c.width().unwrap_or(0);
            if used + cells > width && !row.is_empty() {
                out.push(std::mem::take(&mut row));
                used = 0;
            }
            if cells <= width {
                row.push(c);
                used += cells;
            }
            if out.len() >= 4095 {
                return out;
            }
        }
        out.push(row);
    }
    out
}
fn strings(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => "unknown".into(),
        Value::Array(a) => {
            let mut s = a
                .iter()
                .take(8)
                .map(|v| {
                    v.as_str()
                        .map_or_else(|| clip(&v.to_string(), 80), |s| clip(s, 160))
                })
                .collect::<Vec<_>>()
                .join(", ");
            if a.len() > 8 {
                s.push_str(&format!(" (+{} more)", a.len() - 8));
            }
            s
        }
        _ => value.to_string(),
    }
}
pub(super) fn cells(tab: Tab, row: &Value) -> Vec<String> {
    match tab {
        Tab::Proxies => vec![
            text(row, "binding").into(),
            text(row, "engine").into(),
            row["route_count"].as_u64().map_or_else(
                || {
                    row["routes"]
                        .as_array()
                        .map_or("?".into(), |a| a.len().to_string())
                },
                |n| n.to_string(),
            ),
            if row["_detected"] == true {
                "detected"
            } else if row["_unavailable"] == true {
                "unavailable"
            } else if row["runtime_confirmed"] == true {
                "live"
            } else {
                "offline"
            }
            .into(),
            text(row, "endpoint").into(),
        ],
        Tab::Routes => vec![
            row["hosts"]
                .as_array()
                .filter(|a| !a.is_empty())
                .map_or_else(|| text(row, "native_id").into(), |_| strings(&row["hosts"])),
            strings(&row["targets"]),
            text(row, "binding").into(),
            text(row, "native_status").into(),
        ],
        Tab::Services => vec![
            text(row, "id").into(),
            match row["running"].as_bool() {
                Some(true) => "running",
                Some(false) => "stopped",
                None => "unknown",
            }
            .into(),
            strings(&row["enabled"]),
            text(row, "scope").into(),
        ],
        Tab::Operations => vec![
            text(row, "id").into(),
            text(row, "kind").into(),
            if row["service"].is_string() {
                text(row, "service")
            } else {
                text(row, "proxy")
            }
            .into(),
            text(row, "phase").into(),
        ],
    }
}
fn describe(value: &Value, prefix: &str, depth: usize, out: &mut Vec<String>) {
    if out.len() >= 4094 {
        return;
    }
    if depth > 8 {
        out.push(format!("{prefix}(nested content; switch to JSON)"));
        return;
    }
    if value["kind"] == "steward.route-plan" {
        out.push("REVIEW PLANNED ROUTE CHANGES".into());
        out.push(format!("applicable: {}", strings(&value["applicable"])));
        if let Some(steps) = value["steps"].as_array() {
            for step in steps.iter().take(16) {
                out.push(format!("Router: {}", text(step, "proxy")));
                if let Some(routes) = step["routes"].as_array() {
                    for route in routes.iter().take(128) {
                        out.push(format!(
                            "  UPSERT {}: {} -> {}",
                            text(route, "id"),
                            text(route, "hostname"),
                            text(&route["upstream"], "url")
                        ));
                        out.push(format!(
                            "    TLS {} | enabled {}",
                            text(&route["tls"], "mode"),
                            strings(&route["enabled"])
                        ));
                    }
                }
            }
            if steps.is_empty() {
                out.push("No route changes in this plan.".into());
            }
        }
        out.push(String::new());
        out.push("Existing routes are retained; apply does not prune.".into());
        out.push("A opens apply for the saved plan. Fingerprints are checked again.".into());
        out.push("v shows the full plan JSON and fingerprints.".into());
        return;
    }
    match value {
        Value::Object(map) => {
            let priority = [
                "id",
                "binding",
                "engine",
                "hosts",
                "targets",
                "native_status",
                "running",
                "enabled",
                "scope",
                "ownership",
                "phase",
                "service",
                "proxy",
                "endpoint",
                "runtime_confirmed",
                "source",
                "route_count",
                "executable",
                "working_directory",
                "ready_url",
            ];
            for (key, value) in priority.iter().filter_map(|k| map.get_key_value(*k)).chain(
                map.iter()
                    .filter(|(k, _)| !k.starts_with('_') && !priority.contains(&k.as_str())),
            ) {
                if out.len() >= 4094 {
                    break;
                }
                let label = key.replace('_', " ");
                match value {
                    Value::Array(a) if key == "routes" => {
                        out.push(format!("{prefix}routes: {} (Routes panel / JSON)", a.len()))
                    }
                    Value::Object(_) => {
                        out.push(format!("{prefix}{label}:"));
                        describe(value, &format!("{prefix}  "), depth + 1, out);
                    }
                    Value::Array(a) if a.iter().any(Value::is_object) => {
                        out.push(format!("{prefix}{label}:"));
                        describe(value, &format!("{prefix}  "), depth + 1, out);
                    }
                    _ => out.push(format!("{prefix}{label}: {}", strings(value))),
                }
            }
        }
        Value::Array(a) => {
            for (i, v) in a.iter().enumerate() {
                if out.len() >= 4094 {
                    break;
                }
                out.push(format!("{prefix}#{}", i + 1));
                describe(v, prefix, depth + 1, out);
            }
        }
        Value::String(s) => out.extend(s.lines().take(4094 - out.len()).map(str::to_owned)),
        Value::Null => {}
        _ => out.push(strings(value)),
    }
}
struct Limited(Vec<u8>);
impl Write for Limited {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        if self.0.len() + b.len() > 512 * 1024 {
            return Err(io::Error::other("display limit"));
        }
        self.0.extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub(super) fn detail_lines(details: &Details, width: usize) -> Vec<String> {
    let mut text = Vec::new();
    for d in details.diagnostics.iter().take(128) {
        text.push(format!(
            "{}{}",
            d.code,
            d.resource
                .as_ref()
                .map_or(String::new(), |r| format!(" [{r}]"))
        ));
        text.extend(d.message.lines().take(64).map(str::to_owned));
        text.push(String::new());
    }
    if details.raw {
        let mut output = Limited(Vec::new());
        let mut data = details.data.clone();
        if let Some(map) = data.as_object_mut() {
            map.retain(|k, _| !k.starts_with('_'));
        }
        let truncated = serde_json::to_writer_pretty(&mut output, &data).is_err();
        text.extend(
            String::from_utf8_lossy(&output.0)
                .lines()
                .take(4094)
                .map(str::to_owned),
        );
        if truncated {
            text.push(
                "Display limited to 512 KiB; inspect through the CLI for full output.".into(),
            );
        }
    } else {
        describe(&details.data, "", 0, &mut text);
    }
    if text.is_empty() {
        text.push("No additional data.".into());
    }
    let mut lines = Vec::new();
    for text in text {
        for line in wrap(&text, width) {
            if lines.len() >= 4095 {
                lines.push("Display limit reached; use the CLI for full output.".into());
                return lines;
            }
            lines.push(line);
        }
    }
    lines
}
fn editor_window(editor: &Editor, width: usize) -> (String, usize) {
    let mut start = 0;
    while editor.text[start..editor.cursor].width() >= width.max(1) {
        start += editor.text[start..]
            .chars()
            .next()
            .map_or(1, char::len_utf8);
    }
    (
        clip(&editor.text[start..], width),
        editor.text[start..editor.cursor].width(),
    )
}
pub(super) fn inspector_details(app: &App) -> Details {
    let mut details=if app.mode==InspectMode::Result {app.result.clone()}
        else if app.inspected.as_ref().is_some_and(|(t,key)|*t==app.tab&&app.selected().map(identity).as_ref()==Some(key)) && (app.mode==InspectMode::Logs)==(app.inspector_kind==InspectMode::Logs) {app.inspector.clone()}
        else {None}.unwrap_or_else(||Details{title:format!("{} overview",app.tab.name()),data:app.selected().cloned().unwrap_or_else(||Value::String(if app.mode==InspectMode::Logs {"Select a managed service and press L to follow native logs."}else{"Routers are detected automatically.\n\nc  Connect an existing API\nI  Install a router\nS  Set up an existing executable\np  Search all actions and workflows\n\nEnter a router to browse its routes.\n0 shows routes from all routers."}.into())),diagnostics:app.diagnostics.clone(),status:if app.mode==InspectMode::Logs {"Waiting for logs"}else{"Current observation"}.into(),changed:false,scroll:0,raw:false});
    details.raw = app.mode == InspectMode::Json;
    details.scroll = app.inspect_scroll;
    details
}
pub(super) fn modal_width(width: usize) -> usize {
    width.saturating_sub(6).min(94)
}

pub(super) fn modal_height(screen: &Screen, height: usize) -> usize {
    let desired = match screen {
        Screen::Form(form) => form.fields.len() + 8,
        Screen::Actions { actions, query, .. } => {
            actions
                .iter()
                .filter(|a| action_matches(a, &query.text))
                .count()
                + 5
        }
        Screen::Confirm(_) => 10,
        Screen::Files { .. } => 22,
        _ => 28,
    };
    desired.clamp(10, 28).min(height.saturating_sub(4))
}

pub(super) fn frame(app: &App, size: (u16, u16)) -> Frame {
    let (w, h) = (usize::from(size.0), usize::from(size.1));
    let mut f = Frame {
        size,
        paint: Vec::new(),
        cursor: None,
    };
    if w < 48 || h < 14 {
        f.put(
            0,
            0,
            w,
            if app.quit_after_work {
                "Finishing change..."
            } else {
                "Terminal too small"
            },
            Tone::Accent,
        );
        f.put(0, 2, w, "Resize to at least 48 x 14", Tone::Normal);
        f.put(0, h.saturating_sub(1), w, "q quit", Tone::Normal);
        return f;
    }
    f.put(
        1,
        0,
        w - 3,
        &format!(
            "STEWARD  {}  │  LOCAL  │  auto {}",
            env!("CARGO_PKG_VERSION"),
            if app.auto_refresh { "3s" } else { "paused" }
        ),
        Tone::Accent,
    );
    f.put(
        1,
        1,
        w - 3,
        &format!(
            "{}  │  {}",
            app.scope.as_deref().unwrap_or("All routers"),
            app.root.display()
        ),
        Tone::Muted,
    );
    let l = layout(app, size);
    for tab in Tab::ALL {
        let r = l.panels[tab.index()];
        if r.h == 0 {
            continue;
        }
        let p = app.panel(tab);
        let focused = app.focus == Focus::Resources && tab == app.tab;
        f.block(
            r,
            &format!(
                "[{}] {}  {}{}",
                tab.index() + 1,
                tab.name(),
                p.visible.len(),
                if p.filter.is_empty() {
                    String::new()
                } else {
                    format!(" /{}", p.filter)
                }
            ),
            focused,
        );
        let rows = r.h.saturating_sub(2);
        let offset = p.selection.saturating_sub(rows.saturating_sub(1));
        if p.visible.is_empty() {
            let msg = if !p.diagnostics.is_empty() {
                "Unavailable · i diagnostics"
            } else if p.updated.is_none() {
                "Loading..."
            } else {
                match tab {
                    Tab::Proxies => "No routers detected · c connect",
                    Tab::Routes => "No routes · n add",
                    Tab::Services => "No services · n add",
                    Tab::Operations => "No operations yet",
                }
            };
            f.put(
                r.x + 1,
                r.y + 1,
                r.w - 2,
                if !p.filter.is_empty() {
                    "No matches · Esc clears"
                } else {
                    msg
                },
                Tone::Muted,
            );
        }
        for (n, index) in p.visible.iter().enumerate().skip(offset).take(rows) {
            let row = &p.rows[*index];
            let c = cells(tab, row);
            let label = match tab {
                Tab::Proxies => format!(
                    "{} {} {}",
                    if row["_detected"] == true { "?" } else { "●" },
                    if row["_detected"] == true {
                        text(row, "engine")
                    } else {
                        text(row, "binding")
                    },
                    if row["_detected"] == true {
                        text(row, "endpoint")
                    } else {
                        &c[3]
                    }
                ),
                Tab::Routes => format!(
                    "{} {}",
                    if row["native_status"] == "disabled" {
                        "○"
                    } else {
                        "·"
                    },
                    c[0]
                ),
                Tab::Services => format!(
                    "{} {}",
                    if row["running"] == true { "●" } else { "○" },
                    c[0]
                ),
                Tab::Operations => format!("{} {}", c[0], c[3]),
            };
            f.put(
                r.x + 1,
                r.y + 1 + n - offset,
                r.w - 2,
                &format!("{} {label}", if n == p.selection { "›" } else { " " }),
                if focused && n == p.selection {
                    Tone::Selected
                } else {
                    Tone::Normal
                },
            );
        }
    }
    let r = l.inspector;
    f.block(r, "[5] Inspector", app.focus == Focus::Inspector);
    if r.h >= 4 {
        let modes = [
            ("Overview", InspectMode::Overview),
            ("Logs", InspectMode::Logs),
            ("JSON", InspectMode::Json),
            ("Result", InspectMode::Result),
        ];
        f.put(
            r.x + 1,
            r.y + 1,
            r.w - 2,
            &modes
                .iter()
                .map(|(name, mode)| {
                    if *mode == app.mode {
                        format!("[{name}]")
                    } else {
                        name.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join("  "),
            Tone::Accent,
        );
        let details = inspector_details(app);
        let lines = detail_lines(&details, r.w.saturating_sub(4));
        let available = r.h.saturating_sub(5);
        let offset = if app.mode == InspectMode::Logs && app.follow_logs {
            lines.len().saturating_sub(available)
        } else {
            details.scroll.min(lines.len().saturating_sub(available))
        };
        f.put(
            r.x + 1,
            r.y + 2,
            r.w - 2,
            &format!(
                "{} · {}{}",
                details.title,
                details.status,
                if details.changed { " · changed" } else { "" }
            ),
            if details.status == "rejected" {
                Tone::Error
            } else {
                Tone::Muted
            },
        );
        for (i, line) in lines.iter().skip(offset).take(available).enumerate() {
            f.put(r.x + 2, r.y + 3 + i, r.w - 4, line, Tone::Normal);
        }
        if r.h > 5 {
            f.put(
                r.x + 1,
                r.y + r.h - 2,
                r.w - 2,
                &format!(
                    "{}-{}/{}  v JSON  L logs  z zoom{}",
                    offset + 1,
                    (offset + available).min(lines.len()),
                    lines.len(),
                    if app.mode == InspectMode::Logs {
                        if app.follow_logs {
                            " · following"
                        } else {
                            " · paused (f)"
                        }
                    } else {
                        ""
                    }
                ),
                Tone::Muted,
            );
        }
    }
    let r = l.activity;
    if r.h > 0 {
        f.block(r, "[6] Activity", app.focus == Focus::Activity);
        let rows = r.h - 2;
        let mut lines: Vec<String> = app.activity.iter().cloned().collect();
        if let Some(p) = &app.pending {
            lines.push(format!(
                "RUNNING {} ({}s)",
                p.label,
                p.started.elapsed().as_secs()
            ));
        }
        if let Some(q) = &app.queued {
            lines.push(format!("QUEUED {}", q.label));
        }
        if lines.is_empty() {
            lines.push("Ready · automatic discovery and refresh enabled".into());
        }
        let start = lines.len().saturating_sub(rows + app.activity_scroll);
        for (i, line) in lines.iter().skip(start).take(rows).enumerate() {
            f.put(
                r.x + 1,
                r.y + 1 + i,
                r.w - 2,
                line,
                if line.starts_with("RUNNING") {
                    Tone::Accent
                } else {
                    Tone::Muted
                },
            );
        }
    }
    let status = if app.quit_after_work {
        app.notice.clone()
    } else if let Some(q) = &app.queued {
        format!("Queued: {}", q.label)
    } else if let Some(p) = &app.pending {
        format!("{}...", p.label)
    } else if !app.diagnostics.is_empty() {
        format!("! {} · i diagnostics", app.diagnostics[0])
    } else {
        app.notice.clone()
    };
    f.put(1, h - 3, w - 3, &status, Tone::Muted);
    let keys = match app.focus {
        Focus::Inspector => "j/k scroll  [/] view  v JSON  L logs  f follow  z zoom  Esc back",
        Focus::Activity => "j/k scroll activity  Enter last result  Esc back",
        Focus::Resources => match app.tab {
            Tab::Proxies => "Enter routes  c connect  e edit  I install  S setup  d rescan",
            Tab::Routes => "Enter inspect  n add  e edit  a actions  / filter  0 all routers",
            Tab::Services => "Enter inspect  n add  e edit  a actions  L follow logs",
            Tab::Operations => "Enter inspect  a recover/rollback  / filter",
        },
    };
    f.put(1, h - 2, w - 3, keys, Tone::Normal);
    f.put(
        1,
        h - 1,
        w - 3,
        if w >= 72 {
            "1-6/Tab/h/l panels  p actions  Space auto  ? help  q quit"
        } else {
            "1-6 panels  p actions  ? help  q quit"
        },
        Tone::Accent,
    );
    if let Screen::Prompt { editor, search, .. } = &app.screen {
        let (shown, col) = editor_window(editor, w - 6);
        f.put(
            1,
            h - 2,
            w - 3,
            if *search {
                "Filter current panel · Enter keep · Esc cancel"
            } else {
                "Steward command · Enter run · Up history · Esc cancel"
            },
            Tone::Accent,
        );
        f.put(
            1,
            h - 1,
            w - 3,
            &format!("{} {shown}", if *search { "/" } else { ":" }),
            Tone::Normal,
        );
        f.cursor = Some(((3 + col) as u16, (h - 1) as u16));
    }
    if matches!(app.screen, Screen::Browse | Screen::Prompt { .. }) {
        return f;
    }
    let mw = modal_width(w);
    let mh = modal_height(&app.screen, h);
    let r = Rect {
        x: (w - mw) / 2,
        y: (h - mh) / 2,
        w: mw,
        h: mh,
    };
    match &app.screen {
        Screen::Form(form) => {
            f.block(r, form.title, true);
            let slots = r.h.saturating_sub(7).max(1);
            let offset = form.selected.saturating_sub(slots - 1);
            let label_width = if r.w >= 65 { 22 } else { 16 };
            for (i, field) in form.fields.iter().enumerate().skip(offset).take(slots) {
                let label = clip(field.label, label_width - 1);
                let (shown, col) =
                    editor_window(&field.editor, r.w.saturating_sub(label_width + 6));
                let y = r.y + 2 + i - offset;
                f.put(
                    r.x + 2,
                    y,
                    r.w - 4,
                    &format!(
                        "{label}{}[ {shown} ]",
                        " ".repeat(label_width - label.width())
                    ),
                    if i == form.selected {
                        Tone::Selected
                    } else {
                        Tone::Normal
                    },
                );
                if i == form.selected
                    && field.choices.is_empty()
                    && !app.pending.as_ref().is_some_and(|p| p.mutating)
                {
                    f.cursor = Some(((r.x + label_width + 4 + col) as u16, y as u16));
                }
            }
            for (i, line) in wrap(form.fields[form.selected].hint, r.w - 4)
                .iter()
                .take(2)
                .enumerate()
            {
                f.put(r.x + 2, r.y + r.h - 5 + i, r.w - 4, line, Tone::Muted);
            }
            f.put(
                r.x + 2,
                r.y + 1,
                r.w - 4,
                if form.error.is_empty() {
                    "Tab fields · arrows choices · Ctrl-U clear"
                } else {
                    &form.error
                },
                if form.error.is_empty() {
                    Tone::Muted
                } else {
                    Tone::Error
                },
            );
            f.put(
                r.x + 2,
                r.y + r.h - 3,
                r.w - 4,
                &format!(
                    "Field {}/{} · Ctrl-O browse files",
                    form.selected + 1,
                    form.fields.len()
                ),
                Tone::Muted,
            );
            f.put(
                r.x + 2,
                r.y + r.h - 2,
                r.w - 4,
                "Ctrl-S submit  Esc cancel",
                Tone::Accent,
            );
        }
        Screen::Actions {
            actions,
            selected,
            query,
        } => {
            f.block(r, "Action palette · type to search", true);
            let (shown, col) = editor_window(query, r.w - 6);
            f.put(
                r.x + 2,
                r.y + 1,
                r.w - 4,
                &format!("/ {shown}"),
                Tone::Normal,
            );
            f.cursor = Some(((r.x + 4 + col) as u16, (r.y + 1) as u16));
            let visible: Vec<_> = actions
                .iter()
                .filter(|a| action_matches(a, &query.text))
                .collect();
            let slots = r.h - 5;
            let offset = selected.saturating_sub(slots - 1);
            for (i, a) in visible.iter().enumerate().skip(offset).take(slots) {
                f.put(
                    r.x + 2,
                    r.y + 3 + i - offset,
                    r.w - 4,
                    a.label,
                    if i == *selected {
                        Tone::Selected
                    } else if a.destructive {
                        Tone::Error
                    } else {
                        Tone::Normal
                    },
                );
            }
            if visible.is_empty() {
                f.put(
                    r.x + 2,
                    r.y + 3,
                    r.w - 4,
                    "No matching actions",
                    Tone::Muted,
                );
            }
            f.put(
                r.x + 2,
                r.y + r.h - 2,
                r.w - 4,
                "Up/Down select  Enter run  Esc close",
                Tone::Accent,
            );
        }
        Screen::Confirm(action) => {
            f.block(r, "Confirm selected action", true);
            f.put(r.x + 2, r.y + 2, r.w - 4, action.label, Tone::Error);
            if let Some(row) = app.selected() {
                f.put(
                    r.x + 2,
                    r.y + 4,
                    r.w - 4,
                    &cells(app.tab, row).join(" · "),
                    Tone::Normal,
                );
            }
            f.put(
                r.x + 2,
                r.y + 6,
                r.w - 4,
                "Native ownership and drift checks apply.",
                Tone::Muted,
            );
            f.put(
                r.x + 2,
                r.y + r.h - 2,
                r.w - 4,
                "Enter execute  Esc cancel",
                Tone::Accent,
            );
        }
        Screen::Files {
            directory,
            entries,
            selected,
            ..
        } => {
            f.block(r, "Select a file or directory", true);
            f.put(
                r.x + 2,
                r.y + 1,
                r.w - 4,
                &directory.display().to_string(),
                Tone::Muted,
            );
            let slots = r.h - 5;
            let offset = selected.saturating_sub(slots - 1);
            for (i, (name, dir)) in entries.iter().enumerate().skip(offset).take(slots) {
                f.put(
                    r.x + 2,
                    r.y + 3 + i - offset,
                    r.w - 4,
                    &format!("{name}{}", if *dir { "/" } else { "" }),
                    if i == *selected {
                        Tone::Selected
                    } else {
                        Tone::Normal
                    },
                );
            }
            f.put(
                r.x + 2,
                r.y + r.h - 2,
                r.w - 4,
                "Enter open  Bksp parent  Ctrl-S directory  Esc back",
                Tone::Accent,
            );
        }
        Screen::Details(details) => {
            f.block(r, &details.title, true);
            let lines = detail_lines(details, r.w - 4);
            let slots = r.h - 5;
            let offset = details.scroll.min(lines.len().saturating_sub(slots));
            f.put(
                r.x + 2,
                r.y + 1,
                r.w - 4,
                &format!(
                    "{} · Changes made: {}",
                    details.status,
                    if details.changed {
                        "yes / possible"
                    } else {
                        "no"
                    }
                ),
                Tone::Muted,
            );
            for (i, line) in lines.iter().skip(offset).take(slots).enumerate() {
                f.put(r.x + 2, r.y + 2 + i, r.w - 4, line, Tone::Normal);
            }
            f.put(
                r.x + 2,
                r.y + r.h - 2,
                r.w - 4,
                "j/k/PgUp/PgDn scroll  Esc back  v JSON",
                Tone::Accent,
            );
        }
        _ => {}
    }
    f
}
pub(super) const HELP: &str = "WORKSPACE\n1 Routers · 2 Routes · 3 Services · 4 Operations\n5 Inspector · 6 Activity\nTab / Shift-Tab / h / l move focus\nj/k or arrows select; PgUp/PgDn, Home/End navigate\nEnter router scopes Routes; 0 returns to all routers\nEnter resource fetches an inspection\n[ / ] cycle inspector views; v toggles JSON; z zooms\nL follows selected service logs; f pauses/resumes follow\nSpace pauses/resumes automatic refresh; R refreshes/rescans\n\nWORKFLOWS\np / Ctrl-P searchable action palette\na selected-resource actions; n new; e edit\nc connect detected or existing router\nI install router; S setup existing executable\nP create saved plan; A apply a reviewed plan\nu bring stack up; D bring stack down\nd rescan local routers; i full diagnostics\n/ filter current panel; : parsed command; ? help\n\nFORMS\nTab/Shift-Tab fields; Left/Right choices\nCtrl-O file picker; Ctrl-U clear; Ctrl-S submit\n\nEscape closes overlays or returns to resource panels.\nq / Ctrl-C quits; an active checked write finishes first.\nDetected resources are observations until connected.\nExternal routes require explicit adoption before writes.";

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frames_stay_inside_cells_and_preserve_unicode() {
        let mut app = App::new("/tmp/日本語/state".into());
        app.rows =
            vec![json!({"binding":"edge","engine":"caddy","endpoint":"http://127.0.0.1:2019"})];
        app.refilter();
        for size in [
            (160, 50),
            (120, 30),
            (80, 24),
            (48, 14),
            (20, 5),
            (1, 1),
            (0, 0),
        ] {
            for p in frame(&app, size).paint {
                assert!(p.y < size.1);
                assert!(usize::from(p.x) + p.text.width() < usize::from(size.0));
                assert!(!p.text.chars().any(char::is_control));
            }
        }
        assert_eq!(clip("照片text", 5), "照片…");
        assert_eq!(clip("café", 4), "café");
        assert!(!clip("bad\x1b[2J", 40).contains('\x1b'));
    }
    #[test]
    fn service_operations_and_unknown_states_are_not_blank_or_false() {
        assert_eq!(
            cells(
                Tab::Operations,
                &json!({"id":"1234","kind":"service","service":"api","phase":"verified"})
            )[2],
            "api"
        );
        assert_eq!(
            cells(Tab::Services, &json!({"id":"api","running":null}))[1],
            "unknown"
        );
    }
}
