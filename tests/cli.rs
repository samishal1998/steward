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
fn unavailable_service_and_apply_do_not_load_bad_config() {
    for args in [
        vec!["service", "list", "--json"],
        vec!["apply", "--yes", "--json"],
    ] {
        let output = binary()
            .args(["--config", "/missing/never-read.yaml"])
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["changed"], false);
        assert!(
            value["diagnostics"][0]["code"]
                .as_str()
                .unwrap()
                .starts_with("capability.")
        );
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
