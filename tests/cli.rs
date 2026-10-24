use std::{path::PathBuf, process::Command};

use serde_json::Value;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_steward"))
}
fn config() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/read-only.yaml")
}

#[test]
fn standalone_help_and_version_require_no_proxy_or_configuration() {
    for args in [vec!["--help"], vec!["--version"], vec![]] {
        let result = binary().args(args).output().unwrap();
        assert!(result.status.success());
        assert!(!result.stdout.is_empty());
    }
}

#[test]
fn json_inventory_is_single_document_and_retains_disabled_route() {
    let output = binary()
        .arg("--config")
        .arg(config())
        .args(["route", "list", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schema_version"], "1");
    assert_eq!(value["changed"], false);
    assert_eq!(value["status"], "partial");
    assert!(
        value["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["native_status"] == "disabled")
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn service_and_apply_guard_paths_do_not_load_bad_config() {
    let directory = tempfile::tempdir().unwrap();
    for args in [
        vec!["service", "list", "--json"],
        vec!["apply", "--yes", "--json"],
    ] {
        let output = binary()
            .arg("--state-dir")
            .arg(directory.path().join("state"))
            .args(["--config", "/missing/never-read.yaml"])
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(if args[0] == "apply" { 2 } else { 0 })
        );
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["changed"], false);
        if args[0] == "apply" {
            assert!(
                value["diagnostics"][0]["code"]
                    .as_str()
                    .unwrap()
                    .starts_with("plan.")
            );
        }
    }
}

#[test]
fn usage_errors_in_json_mode_use_versioned_error_envelope() {
    let output = binary()
        .args(["proxy", "inspect", "--json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["diagnostics"][0]["code"], "cli.usage");
    assert!(output.stderr.is_empty());
}

#[test]
fn dry_plan_has_fingerprints_and_never_claims_applicability() {
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/routes-only.yaml");
    let output = binary()
        .arg("--config")
        .arg(config())
        .arg("plan")
        .arg("--file")
        .arg(file)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["status"], "planned_read_only");
    assert_eq!(value["data"]["applicable"], false);
    assert!(
        !value["data"]["source_fingerprints"]
            .as_object()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn an_empty_manifest_initializes_no_proxy_and_never_prunes() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = dir.path().join("empty.json");
    let profile = dir.path().join("unreachable.json");
    std::fs::write(
        &manifest,
        r#"{"schema_version":"1","scope":"user","routes":[],"services":[]}"#,
    )
    .unwrap();
    std::fs::write(&profile, r#"{"schema_version":"1","scope":"user","proxies":[{"id":"edge","engine":"traefik","endpoint":"http://127.0.0.1:1","auth_file":"/never/open/this-file"}]}"#).unwrap();
    let result = binary()
        .arg("--config")
        .arg(profile)
        .arg("plan")
        .arg("--file")
        .arg(manifest)
        .arg("--json")
        .output()
        .unwrap();
    assert!(result.status.success());
    let output: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert!(output["data"]["actions"].as_array().unwrap().is_empty());
    assert_eq!(output["changed"], false);
}

#[test]
fn all_shell_completions_bypass_configuration_and_state() {
    let directory = tempfile::tempdir().unwrap();
    let state = directory.path().join("never-created");
    for shell in ["bash", "zsh", "fish", "elvish", "powershell"] {
        let result = binary()
            .arg("--state-dir")
            .arg(&state)
            .args(["--config", "/never/read", "completions", shell])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stdout).contains("upstream"));
        assert!(!state.exists());
    }
}

#[test]
fn direct_writes_reject_profile_override_before_loading_inputs() {
    let directory = tempfile::tempdir().unwrap();
    let state = directory.path().join("state");
    let result = binary()
        .arg("--state-dir")
        .arg(&state)
        .args([
            "--config",
            "/never/read",
            "routes",
            "add",
            "app",
            "--proxy",
            "edge",
            "--host",
            "app.localhost",
            "--upstream",
            "http://127.0.0.1:3000",
            "--tls",
            "http",
            "--json",
        ])
        .output()
        .unwrap();
    let value: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["diagnostics"][0]["code"], "cli.managed_state");
    assert_eq!(value["changed"], false);
    assert!(!state.exists());
}

#[test]
fn unmanaged_routes_are_not_adopted_or_removed_by_name() {
    let directory = tempfile::tempdir().unwrap();
    let state = directory.path().join("state");
    std::fs::create_dir(&state).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let source = directory.path().join("caddy.json");
    let original = r#"{"apps":{"http":{"servers":{"web":{"listen":["127.0.0.1:12345"],"automatic_https":{"disable":true},"routes":[]}}}}}"#;
    std::fs::write(&source, original).unwrap();
    let book = serde_json::json!({"schema_version":"1","owner":"0123456789abcdef","connections":[{"binding":{"id":"local","engine":"caddy","endpoint":"http://127.0.0.1:1"},"authority":{"kind":"caddy_json","file":source,"server":"web","binary":"/not/run"}}],"routes":[]});
    std::fs::write(state.join("state.json"), serde_json::to_vec(&book).unwrap()).unwrap();
    let result = binary()
        .arg("--state-dir")
        .arg(&state)
        .args(["routes", "remove", "external", "--proxy", "local", "--json"])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    let value: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["diagnostics"][0]["code"], "ownership.required");
    assert_eq!(value["changed"], false);
    assert_eq!(std::fs::read_to_string(&source).unwrap(), original);
    assert!(!state.join("operation.json").exists());
}
