#![cfg(feature = "repository-fixtures")]

use master_skill_core::{InstallOptions, Repository};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

struct Guard(PathBuf);
impl Drop for Guard {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}
fn setup() -> (Repository, Guard) {
    let ns = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let tmp = std::env::temp_dir().join(format!("master-skill-test-{}-{ns}", std::process::id()));
    let repo = Repository::open(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .unwrap().with_install_root(&tmp);
    (repo, Guard(tmp))
}
fn options(dry_run: bool, force: bool) -> InstallOptions {
    InstallOptions { dry_run, force }
}

#[test]
fn install_dry_run_and_roundtrip() {
    let (repo, guard) = setup();
    let names = vec!["huineng".into()];
    let planned = repo.install(&names, options(true, false)).unwrap();
    assert!(planned.dry_run);
    assert!(!guard.0.exists());
    let result = repo.install(&names, options(false, false)).unwrap();
    assert_eq!(result.actions.len(), 1);
    let target = guard.0.join("master-huineng").join("SKILL.md");
    assert!(target.is_file());
    assert!(repo.inspect("huineng").unwrap().installed);
    let report = repo.doctor().unwrap();
    assert_eq!(report.status, "ok");
    let uninstalled = repo.uninstall(&names, options(false, false)).unwrap();
    assert_eq!(uninstalled.actions[0].action, "uninstalled");
    assert!(!target.exists());
}

#[test]
fn local_user_edit_blocks_replacement_or_deletion_without_force() {
    let (repo, guard) = setup();
    let names = vec!["huineng".into()];
    repo.install(&names, options(false, false)).unwrap();
    let file = guard.0.join("master-huineng").join("SKILL.md");
    fs::write(&file, "custom user content").unwrap();
    assert!(repo.install(&names, options(false, false)).is_err());
    assert!(repo.uninstall(&names, options(false, false)).is_err());
    assert!(repo.doctor().unwrap().problems.iter()
        .any(|problem| problem.code == "installed-outdated"));
    assert_eq!(fs::read_to_string(&file).unwrap(), "custom user content");
    repo.install(&names, options(false, true)).unwrap();
    assert_ne!(fs::read_to_string(&file).unwrap(), "custom user content");
}

#[test]
fn teacher_generator_carries_user_personas_across_updates() {
    let (repo, guard) = setup();
    let names = vec!["create-master".into()];
    repo.install(&names, options(false, false)).unwrap();
    let data = guard.0.join("create-master").join("masters").join("my-persona");
    fs::create_dir_all(&data).unwrap();
    fs::write(data.join("SKILL.md"), "important generated content").unwrap();
    repo.install(&names, options(false, false)).unwrap();
    assert!(repo.uninstall(&names, options(false, false)).is_err());
    assert_eq!(fs::read_to_string(data.join("SKILL.md")).unwrap(), "important generated content");
}
