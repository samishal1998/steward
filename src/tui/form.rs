//! Small editors and guided inputs; submission still goes through Clap and run().
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Clone, Default)]
pub(super) struct Editor {
    pub text: String,
    pub cursor: usize,
}

impl Editor {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            cursor: text.len(),
            text,
        }
    }

    pub fn insert(&mut self, value: &str) {
        let value: String = value.chars().filter(|c| !c.is_control()).collect();
        if self.text.len() + value.len() <= 4096 {
            self.text.insert_str(self.cursor, &value);
            self.cursor += value.len();
        }
    }

    pub fn key(&mut self, key: KeyEvent) {
        let previous = || {
            self.text[..self.cursor]
                .char_indices()
                .last()
                .map_or(0, |(i, _)| i)
        };
        let next = || {
            self.cursor
                + self.text[self.cursor..]
                    .chars()
                    .next()
                    .map_or(0, char::len_utf8)
        };
        match key.code {
            KeyCode::Left => self.cursor = previous(),
            KeyCode::Right => self.cursor = next(),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.text.len(),
            KeyCode::Backspace if self.cursor > 0 => {
                let start = previous();
                self.text.drain(start..self.cursor);
                self.cursor = start;
            }
            KeyCode::Delete if self.cursor < self.text.len() => {
                self.text.drain(self.cursor..next());
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.text.drain(..self.cursor);
                self.cursor = 0;
            }
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.insert(&c.to_string())
            }
            _ => {}
        }
    }
}

#[derive(Clone)]
pub(super) struct Field {
    pub label: &'static str,
    pub hint: &'static str,
    pub editor: Editor,
    pub choices: &'static [&'static str],
}

impl Field {
    pub(super) fn text(label: &'static str, hint: &'static str, value: &str) -> Self {
        Self {
            label,
            hint,
            editor: Editor::new(value),
            choices: &[],
        }
    }
    pub(super) fn choice(
        label: &'static str,
        hint: &'static str,
        choices: &'static [&'static str],
    ) -> Self {
        Self {
            label,
            hint,
            editor: Editor::new(choices[0]),
            choices,
        }
    }
    pub fn key(&mut self, key: KeyEvent) {
        if self.choices.is_empty() {
            self.editor.key(key);
        } else if matches!(
            key.code,
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
        ) {
            let i = self
                .choices
                .iter()
                .position(|c| *c == self.editor.text)
                .unwrap_or(0);
            let delta = if key.code == KeyCode::Left {
                self.choices.len() - 1
            } else {
                1
            };
            self.editor = Editor::new(self.choices[(i + delta) % self.choices.len()]);
        }
    }
}

#[derive(Clone)]
enum Kind {
    Connect,
    UpdateConnection,
    Route,
    EditRoute,
    Service,
    EditService(Vec<String>),
    Workflow {
        prefix: Vec<String>,
        arguments: Vec<Argument>,
    },
}

#[derive(Clone)]
pub(super) enum Argument {
    Positional,
    Required(&'static str),
    Optional(&'static str),
    Switch(&'static str),
}

#[derive(Clone)]
pub(super) struct Form {
    pub title: &'static str,
    pub fields: Vec<Field>,
    pub selected: usize,
    pub error: String,
    kind: Kind,
}

impl Form {
    pub fn workflow(title: &'static str, prefix: &[&str], fields: Vec<(Field, Argument)>) -> Self {
        let (fields, arguments) = fields.into_iter().unzip();
        Self {
            title,
            fields,
            selected: 0,
            error: String::new(),
            kind: Kind::Workflow {
                prefix: prefix.iter().map(|s| s.to_string()).collect(),
                arguments,
            },
        }
    }

    pub fn detected(row: &serde_json::Value) -> Self {
        let mut form = Self::connect();
        let get = |key| row[key].as_str().unwrap_or("");
        form.fields[0].editor = Editor::new(format!("{}-local", get("engine")));
        form.fields[1].editor = Editor::new(get("engine"));
        form.fields[2].editor = Editor::new(get("endpoint"));
        form
    }

    pub fn connection(row: &serde_json::Value) -> Self {
        let mut form = Self::connect();
        form.title = "Edit router connection";
        form.kind = Kind::UpdateConnection;
        let binding = &row["binding"];
        let authority = &row["authority"];
        let tls = &row["tls"];
        for (i, value) in [
            (0, &binding["id"]),
            (1, &binding["engine"]),
            (2, &binding["endpoint"]),
            (8, &binding["auth_file"]),
            (9, &tls["ca_file"]),
            (10, &tls["cert_resolver"]),
            (11, &tls["certificate"]),
            (12, &tls["private_key"]),
        ] {
            form.fields[i].editor = Editor::new(value.as_str().unwrap_or(""));
        }
        let kind = authority["kind"].as_str().unwrap_or("read_only");
        form.fields[3].editor = Editor::new(match kind {
            "caddy_json" => "caddy-json",
            "caddy_file" if authority["controller"] == true => "docker-controller",
            "caddy_file" => "caddyfile",
            "traefik_directory" => "traefik-directory",
            _ => "read-only",
        });
        for (i, value) in [
            (
                4,
                authority.get("file").or_else(|| authority.get("startup")),
            ),
            (
                5,
                authority
                    .get("server")
                    .or_else(|| authority.get("listen"))
                    .or_else(|| authority.get("entrypoint")),
            ),
            (6, authority.get("directory")),
            (7, authority.get("binary")),
        ] {
            if let Some(value) = value.and_then(serde_json::Value::as_str) {
                form.fields[i].editor = Editor::new(value);
            }
        }
        form
    }

    pub fn edit_service(spec: &crate::services::Spec) -> Self {
        let mut form = Self::service();
        form.title = "Edit native service";
        // Preserve environment values without putting them in the activity log or a public result.
        form.kind = Kind::EditService(
            spec.environment
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect(),
        );
        for (i, value) in [
            (0, spec.id.clone()),
            (1, spec.executable.display().to_string()),
            (
                2,
                spec.args
                    .iter()
                    .map(|s| format!("'{}'", s.replace('\'', "'\\''")))
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
            (
                3,
                spec.working_directory
                    .as_ref()
                    .map_or(String::new(), |p| p.display().to_string()),
            ),
            (
                4,
                if spec.scope == crate::model::Scope::User {
                    "user"
                } else {
                    "system"
                }
                .into(),
            ),
            (5, spec.ready_url.clone().unwrap_or_default()),
            (9, spec.ready_timeout.to_string()),
            (6, if spec.enabled { "yes" } else { "no" }.into()),
            (7, if spec.restart { "yes" } else { "no" }.into()),
        ] {
            form.fields[i].editor = Editor::new(value);
        }
        form
    }
    pub fn connect() -> Self {
        Self {
            title: "Connect a proxy",
            kind: Kind::Connect,
            selected: 0,
            error: String::new(),
            fields: vec![
                Field::text(
                    "Connection ID",
                    "A short name, e.g. edge. This is a saved connection, not a new proxy process.",
                    "",
                ),
                Field::choice(
                    "Engine",
                    "Left/Right or Space changes the engine.",
                    &["caddy", "traefik"],
                ),
                Field::text(
                    "API endpoint",
                    "Caddy commonly uses http://127.0.0.1:2019; Traefik http://127.0.0.1:8080.",
                    "http://127.0.0.1:2019",
                ),
                Field::choice(
                    "Authority",
                    "Read-only connects the API. File authority enables checked route changes.",
                    &[
                        "read-only",
                        "caddy-json",
                        "caddyfile",
                        "docker-controller",
                        "traefik-directory",
                    ],
                ),
                Field::text(
                    "Startup file",
                    "Absolute native startup file; required for file authority.",
                    "",
                ),
                Field::text(
                    "Listener",
                    "Caddy server name, Caddyfile listen address, or Traefik entrypoint.",
                    "",
                ),
                Field::text(
                    "Watched directory",
                    "Traefik only: existing watched dynamic-config directory.",
                    "",
                ),
                Field::text(
                    "Caddy executable",
                    "Validator in the proxy's environment; PATH name or absolute path.",
                    "caddy",
                ),
                Field::text(
                    "Auth file",
                    "Optional authorization header file; enter a path, not a secret.",
                    "",
                ),
                Field::text(
                    "CA file",
                    "Optional PEM CA path for verified internal HTTPS.",
                    "",
                ),
                Field::text(
                    "Cert resolver",
                    "Optional existing Traefik public certificate resolver.",
                    "",
                ),
                Field::text(
                    "Certificate",
                    "Optional Traefik certificate path, paired with a private-key path.",
                    "",
                ),
                Field::text(
                    "Private-key file",
                    "Path only. Key contents are not displayed.",
                    "",
                ),
            ],
        }
    }

    pub fn route(proxy: &str, existing: Option<(&str, &str, &str)>) -> Self {
        let (id, host, upstream) = existing.unwrap_or(("", "", "http://127.0.0.1:3000"));
        let mut fields = vec![
            Field::text("Route ID", "An owned route ID, e.g. photos.", id),
            Field::text(
                "Proxy",
                "Saved connection ID from the Proxies view; must have file authority.",
                proxy,
            ),
            Field::text("Hostname", "Hostname only, e.g. photos.localhost.", host),
            Field::text(
                "Upstream",
                "Backend URL reachable from the proxy's network context.",
                upstream,
            ),
        ];
        if existing.is_none() {
            fields.push(Field::choice("TLS", "HTTP is unencrypted. Internal/public HTTPS requires configured certificate authority.", &["http", "internal", "public"]));
        }
        Self {
            title: if existing.is_some() {
                "Update a route"
            } else {
                "Add a route"
            },
            fields,
            kind: if existing.is_some() {
                Kind::EditRoute
            } else {
                Kind::Route
            },
            selected: 0,
            error: String::new(),
        }
    }

    pub fn service() -> Self {
        Self {
            title: "Add a native service",
            kind: Kind::Service,
            selected: 0,
            error: String::new(),
            fields: vec![
                Field::text(
                    "Service ID",
                    "A short service name. systemd/launchd will supervise the process.",
                    "",
                ),
                Field::text("Executable", "Existing absolute executable path.", ""),
                Field::text(
                    "Arguments",
                    "Shell-style quoting splits arguments; no shell expansion or shell execution.",
                    "",
                ),
                Field::text(
                    "Working directory",
                    "Optional existing absolute directory.",
                    "",
                ),
                Field::choice(
                    "Scope",
                    "User or system scope; existing OS permissions apply.",
                    &["user", "system"],
                ),
                Field::text(
                    "Readiness URL",
                    "Optional HTTP readiness endpoint, e.g. http://127.0.0.1:3000/.",
                    "",
                ),
                Field::choice(
                    "Autostart",
                    "Enable startup with the native supervisor.",
                    &["no", "yes"],
                ),
                Field::choice(
                    "Restart on exit",
                    "Let the native supervisor restart this service.",
                    &["no", "yes"],
                ),
                Field::choice(
                    "Start now",
                    "Start after registration and verify readiness.",
                    &["no", "yes"],
                ),
                Field::text(
                    "Ready timeout",
                    "Readiness deadline in seconds (1–300).",
                    "30",
                ),
            ],
        }
    }

    pub fn arguments(&self) -> Result<Vec<String>, String> {
        let v = |i: usize| self.fields[i].editor.text.trim().to_owned();
        let required = |i: usize| -> Result<String, String> {
            let value = v(i);
            if value.is_empty() {
                Err(format!("{} is required", self.fields[i].label))
            } else {
                Ok(value)
            }
        };
        let args = match &self.kind {
            Kind::Connect | Kind::UpdateConnection => {
                let mut args = vec![
                    "proxies".into(),
                    if matches!(self.kind, Kind::UpdateConnection) {
                        "update"
                    } else {
                        "add"
                    }
                    .into(),
                    required(0)?,
                    "--engine".into(),
                    v(1),
                    "--endpoint".into(),
                    required(2)?,
                ];
                let authority = v(3);
                if (authority.starts_with("caddy") || authority == "docker-controller")
                    && v(1) != "caddy"
                    || authority == "traefik-directory" && v(1) != "traefik"
                {
                    return Err("Authority must match the selected engine".into());
                }
                if authority != "read-only" {
                    args.extend([
                        "--config-file".into(),
                        required(4)?,
                        "--listener".into(),
                        required(5)?,
                    ]);
                    if authority == "traefik-directory" {
                        args.extend(["--directory".into(), required(6)?]);
                    }
                    if matches!(authority.as_str(), "caddyfile" | "docker-controller") {
                        args.push("--caddyfile".into());
                    }
                    if authority == "docker-controller" {
                        args.push("--docker-controller".into());
                    }
                    if v(1) == "caddy" {
                        args.extend(["--caddy-bin".into(), required(7)?]);
                    }
                } else if (4..=6).any(|i| !v(i).is_empty()) {
                    return Err(
                        "Select file authority to use startup/listener/directory fields".into(),
                    );
                }
                for (i, flag) in [
                    (8, "--auth-file"),
                    (9, "--ca-file"),
                    (10, "--cert-resolver"),
                    (11, "--certificate"),
                    (12, "--private-key"),
                ] {
                    if !v(i).is_empty() {
                        args.extend([flag.into(), v(i)]);
                    }
                }
                args
            }
            Kind::Route | Kind::EditRoute => {
                let edit = matches!(self.kind, Kind::EditRoute);
                let mut args = vec![
                    "routes".into(),
                    if edit { "update" } else { "add" }.into(),
                    required(0)?,
                    "--proxy".into(),
                    required(1)?,
                    "--host".into(),
                    required(2)?,
                    "--upstream".into(),
                    required(3)?,
                ];
                if !edit {
                    args.extend(["--tls".into(), v(4)]);
                }
                args
            }
            Kind::Service | Kind::EditService(_) => {
                let mut args = vec![
                    "services".into(),
                    if matches!(self.kind, Kind::EditService(_)) {
                        "update"
                    } else {
                        "add"
                    }
                    .into(),
                    required(0)?,
                    "--executable".into(),
                    required(1)?,
                    "--scope".into(),
                    v(4),
                    "--ready-timeout".into(),
                    required(9)?,
                ];
                for (i, flag) in [(3, "--working-directory"), (5, "--ready-url")] {
                    if !v(i).is_empty() {
                        args.extend([flag.into(), v(i)]);
                    }
                }
                for (i, flag) in [(6, "--enabled"), (7, "--restart"), (8, "--start")] {
                    if v(i) == "yes" {
                        args.push(flag.into());
                    }
                }
                if let Kind::EditService(environment) = &self.kind {
                    for value in environment {
                        args.extend(["--env".into(), value.clone()]);
                    }
                }
                let tail = shlex::split(&self.fields[2].editor.text)
                    .ok_or("Arguments contain an unclosed quote")?;
                if !tail.is_empty() {
                    args.push("--".into());
                    args.extend(tail);
                }
                args
            }
            Kind::Workflow { prefix, arguments } => {
                let mut args = prefix.clone();
                for (i, argument) in arguments.iter().enumerate() {
                    match argument {
                        Argument::Positional => args.push(required(i)?),
                        Argument::Required(flag) => args.extend([(*flag).into(), required(i)?]),
                        Argument::Optional(flag) if !v(i).is_empty() => {
                            args.extend([(*flag).into(), v(i)])
                        }
                        Argument::Switch(flag) if v(i) == "yes" => args.push((*flag).into()),
                        _ => {}
                    }
                }
                args
            }
        };
        Ok(args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Cli, Command, services::ServiceCommand};
    use clap::Parser;
    use std::path::PathBuf;

    #[test]
    fn service_editor_preserves_private_environment_and_readiness_policy() {
        let spec: crate::services::Spec = serde_json::from_value(serde_json::json!({"id":"api","scope":"user","executable":"/bin/true","args":["two words"],"working_directory":null,"environment":{"TOKEN":"private-value"},"enabled":true,"restart":true,"ready_url":"http://127.0.0.1:3000/","ready_timeout":47})).unwrap();
        let form = Form::edit_service(&spec);
        assert!(
            form.fields
                .iter()
                .all(|f| !f.editor.text.contains("private-value"))
        );
        let cli =
            Cli::try_parse_from(std::iter::once("steward".into()).chain(form.arguments().unwrap()))
                .unwrap();
        let Some(Command::Service(ServiceCommand::Update(args))) = cli.command else {
            panic!("expected service update")
        };
        assert_eq!(args.ready_timeout, 47);
        assert_eq!(args.args, ["two words"]);
        assert_eq!(args.environment, ["TOKEN=private-value"]);
        assert!(args.enabled && args.restart);
    }

    #[test]
    fn editors_keep_utf8_boundaries_and_cap_paste() {
        let mut editor = Editor::new("a照片z");
        editor.key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        editor.key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        editor.insert("é");
        assert_eq!(editor.text, "a照éz");
        editor.key(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE));
        assert_eq!(editor.text, "a照é");
        editor.insert(&"x".repeat(4097));
        assert_eq!(editor.text, "a照é");
    }

    #[test]
    fn forms_use_cli_validation_and_literal_argument_boundaries() {
        let mut connect = Form::connect();
        connect.fields[0].editor = Editor::new("edge");
        connect.fields[4].editor = Editor::new("/tmp/startup.json");
        assert!(
            connect
                .arguments()
                .unwrap_err()
                .contains("Select file authority")
        );
        connect.fields[3].editor = Editor::new("caddy-json");
        connect.fields[5].editor = Editor::new("edge");
        let cli = Cli::try_parse_from(
            std::iter::once("steward".into()).chain(connect.arguments().unwrap()),
        )
        .unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Proxy(crate::ProxyCommand::Add(_)))
        ));

        let mut service = Form::service();
        service.fields[0].editor = Editor::new("api");
        service.fields[1].editor = Editor::new("/tmp/my executable");
        service.fields[2].editor = Editor::new("'two words' '$(not-a-shell-command)' --flag");
        let cli = Cli::try_parse_from(
            std::iter::once("steward".into()).chain(service.arguments().unwrap()),
        )
        .unwrap();
        let Some(Command::Service(ServiceCommand::Add(args))) = cli.command else {
            panic!("service form must produce a service command")
        };
        assert_eq!(args.executable, PathBuf::from("/tmp/my executable"));
        assert_eq!(args.args, ["two words", "$(not-a-shell-command)", "--flag"]);
    }
}
