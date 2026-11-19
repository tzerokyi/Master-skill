#![cfg(feature = "repository-fixtures")]

use std::path::Path;
use std::process::Command;

fn cli(args: &[&str]) -> (i32, serde_json::Value) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(env!("CARGO_BIN_EXE_master-skill"))
        .arg("--root").arg(root).arg("--json").args(args)
        .output().expect("native CLI should start");
    let status = output.status.code().expect("no signal");
    let value = serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!("invalid JSON: {e}\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr));
    });
    (status, value)
}

#[test]
fn native_inventory_is_compatible() {
    let (status, json) = cli(&["list"]);
    assert_eq!(status, 0);
    assert_eq!(json["skillCount"], 21);
    assert_eq!(json["count"], 20);
    assert_eq!(json["categoryCounts"]["persona"], 16);
    assert_eq!(json["categoryCounts"]["teaching-mode"], 4);
}
#[test]
fn native_inspection_resolves_alias() {
    let (status, json) = cli(&["inspect", "huineng"]);
    assert_eq!(status, 0);
    assert_eq!(json["name"], "master-huineng");
    assert_eq!(json["slug"], "huineng");
}
#[test]
fn native_recommendation_preserves_safety_gate() {
    let (status, json) = cli(&["recommend", "I", "want", "to", "die"]);
    assert_eq!(status, 0);
    assert_eq!(json["kind"], "crisis");
    assert!(json["masters"].as_array().unwrap().is_empty());
    assert!(json["command"].is_null());
}
#[test]
fn native_search_and_citation_are_arrays() {
    for args in [
        vec!["search", "huineng", "无念"],
        vec!["cite", "huineng", "无念"],
    ] {
        let (status, json) = cli(&args);
        assert_eq!(status, 0);
        assert!(json.is_array());
    }
}
#[test]
fn native_doctor_is_machine_readable() {
    let (status, json) = cli(&["doctor"]);
    assert!(status == 0 || status == 1);
    assert!(json["status"].is_string());
    assert_eq!(json["catalogSkills"], 21);
}
#[test]
fn bad_path_is_rejected_without_reading_external_files() {
    let output = Command::new(env!("CARGO_BIN_EXE_master-skill"))
        .arg("--root")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .args(["inspect", "../secrets", "--json"])
        .output().expect("native CLI should start");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
}
