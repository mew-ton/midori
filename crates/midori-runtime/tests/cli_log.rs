//! CLI 経由で `--log-format json` の structured logger 出力を検証する統合
//! テスト。`main()` 内で `logging::init` が動くこと、`logging::warn` 等が
//! 実バイナリの stdout に 1 行 JSON を吐くことを end-to-end で担保する。
//!
//! 単体テスト（`logging.rs::tests`）は `Logger` 値直接で format / filter を
//! 検証するため、本ファイルは実バイナリ spawn 経路（`OnceLock` 経由）に絞る。

use std::io::Write;
use std::path::Path;
use std::process::Command;

/// `<app-data-dir>/plugins/driver-<name>/events.yaml` を持たない app-data-dir
/// を temp dir で作る。startup chain が "Missing → warn" の経路に入る。
fn setup_empty_app_data_dir() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("midori-cli-log-app-")
        .tempdir()
        .expect("tempdir")
}

fn write_tmp_profile(driver: &str) -> tempfile::NamedTempFile {
    let yaml = format!(
        "inputs:\n  - adapter: a.yaml\n    connection: {{ driver: {driver} }}\ntransform: t.yaml\noutputs:\n  - adapter: b.yaml\n    connection: {{ driver: {driver} }}\n"
    );
    let mut file = tempfile::Builder::new()
        .prefix("midori-cli-log-profile-")
        .suffix(".yaml")
        .tempfile()
        .expect("tempfile");
    file.write_all(yaml.as_bytes()).expect("write profile");
    file
}

fn spawn(profile: &Path, app_data_dir: &Path, format: &str) -> std::process::Output {
    let bin = env!("CARGO_BIN_EXE_midori");
    Command::new(bin)
        .args([
            "--log-format",
            format,
            "--app-data-dir",
            app_data_dir.to_str().expect("utf-8"),
            "run",
            profile.to_str().expect("utf-8"),
        ])
        .output()
        .expect("spawn midori")
}

#[test]
fn it_should_emit_warn_log_as_single_json_line_when_events_yaml_is_missing() {
    let app = setup_empty_app_data_dir();
    let profile = write_tmp_profile("midi");
    let output = spawn(profile.path(), app.path(), "json");
    assert!(
        output.status.success(),
        "missing events.yaml should warn + continue, exit success: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf-8");
    let warn_line = stdout
        .lines()
        .find(|line| line.contains("\"level\":\"warn\""))
        .unwrap_or_else(|| panic!("warn line missing in stdout: {stdout}"));
    let value: serde_json::Value =
        serde_json::from_str(warn_line).expect("warn line is valid json");
    assert_eq!(value["type"], "log");
    assert_eq!(value["level"], "warn");
    assert_eq!(value["layer"], "bridge");
    assert_eq!(value["device"], "midi");
    let message = value["message"].as_str().unwrap_or("");
    assert!(
        message.contains("見つかりませんでした"),
        "warn message should describe the missing-schema case: {message}"
    );
}

#[test]
fn it_should_emit_text_format_when_log_format_is_text() {
    let app = setup_empty_app_data_dir();
    let profile = write_tmp_profile("osc");
    let output = spawn(profile.path(), app.path(), "text");
    assert!(output.status.success(), "exit success");
    let stdout = String::from_utf8(output.stdout).expect("utf-8");
    // text 形式は `midori [warn] bridge (osc): ...` の形
    assert!(
        stdout.contains("midori [warn] bridge (osc)"),
        "text-format warn line missing: {stdout}"
    );
    // text 形式は JSON シンボルを含まないこと（JSON 形式と排他）
    assert!(
        !stdout.contains("\"type\":\"log\""),
        "text format should not emit json: {stdout}"
    );
}

#[test]
fn it_should_emit_error_log_when_profile_is_invalid() {
    // profile が存在しない path を渡す → CliError::LoadProfile (Io) → main で
    // logging::error 経由で出力。exit code は失敗。
    let app = setup_empty_app_data_dir();
    let bin = env!("CARGO_BIN_EXE_midori");
    let output = Command::new(bin)
        .args([
            "--log-format",
            "json",
            "--app-data-dir",
            app.path().to_str().expect("utf-8"),
            "run",
            "/nonexistent/midori-cli-log/profile.yaml",
        ])
        .output()
        .expect("spawn");
    assert!(
        !output.status.success(),
        "missing profile must fail: {output:?}"
    );
    let stdout = String::from_utf8(output.stdout).expect("utf-8");
    let error_line = stdout
        .lines()
        .find(|line| line.contains("\"level\":\"error\""))
        .unwrap_or_else(|| panic!("error line missing: {stdout}"));
    let value: serde_json::Value =
        serde_json::from_str(error_line).expect("error line is valid json");
    assert_eq!(value["type"], "log");
    assert_eq!(value["level"], "error");
    assert_eq!(value["layer"], "bridge");
}

#[test]
fn it_should_filter_out_info_when_log_level_is_warn() {
    // level=warn 設定下では Info / Debug は出ず、Warn のみが出る。
    // events.yaml Missing は warn なので 1 件出るが、Loaded info は出ないこと
    // を別 fixture（events.yaml が valid な driver）で確認する。
    let app = tempfile::Builder::new()
        .prefix("midori-cli-log-filter-")
        .tempdir()
        .expect("tempdir");
    // valid な events.yaml を配置 → Loaded info 経路
    let plugin_dir = app.path().join("plugins").join("driver-midi");
    std::fs::create_dir_all(&plugin_dir).expect("mkdir plugin");
    let valid_events = "schema_version: 1\n\
         events:\n  \
           noteOn:\n    \
             fields:\n      \
               channel: { type: uint8, range: [1, 16] }\n";
    std::fs::write(plugin_dir.join("events.yaml"), valid_events).expect("write events.yaml");
    let profile = write_tmp_profile("midi");

    let bin = env!("CARGO_BIN_EXE_midori");
    let output = Command::new(bin)
        .args([
            "--log-level",
            "warn",
            "--log-format",
            "json",
            "--app-data-dir",
            app.path().to_str().expect("utf-8"),
            "run",
            profile.path().to_str().expect("utf-8"),
        ])
        .output()
        .expect("spawn");
    assert!(output.status.success(), "valid setup should succeed");
    let stdout = String::from_utf8(output.stdout).expect("utf-8");
    assert!(
        !stdout.contains("\"level\":\"info\""),
        "info should be filtered out by --log-level warn: {stdout}"
    );
}
