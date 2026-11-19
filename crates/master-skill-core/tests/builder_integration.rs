#![cfg(feature = "repository-fixtures")]

use master_skill_core::{Approval, Repository, Review, TeacherOptions, TeacherSpec};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn sample() -> TeacherSpec {
    TeacherSpec {
        name: "Offline Test".into(),
        slug: Some("offline-test".into()),
        tradition: "test-only".into(),
        school: "deterministic".into(),
        era: "fixture".into(),
        languages: vec!["en".into()],
        teaching_content: "Test teaching with declared sources.".into(),
        voice_content: "Concise test voice.".into(),
        sources: vec![json!({
            "type": "compiled_teaching",
            "id": "OfflineSmoke:Deterministic",
            "title": "Deterministic offline fixture"
        })],
        citation_contract: None,
        fojin_entity_id: None,
        review: None,
    }
}

#[test]
fn offline_builder_requires_two_current_independent_approvals() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let repo = Repository::open(root).unwrap();
    let date = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let out = std::env::temp_dir().join(format!("master-rust-builder-{}-{date}", std::process::id()));
    let mut spec = sample();
    let hashes = repo.prepare_teacher(&spec).unwrap();
    assert_eq!(hashes.citation_contract.allowed_source_types, vec!["compiled_teaching"]);
    let preview = repo.build_teacher(&spec, &out, TeacherOptions { dry_run: true, force: false }).unwrap();
    assert!(preview.dry_run);
    assert!(!out.exists());
    assert!(repo.build_teacher(&spec, &out, TeacherOptions::default()).is_err());
    spec.review = Some(Review {
        doctrine: Approval {
            verdict: "PASS".into(), input_sha256: hashes.doctrine_sha256.clone(),
            reviewer: "independent-doctrine-reviewer".into(),
        },
        voice: Approval {
            verdict: "PASS".into(), input_sha256: hashes.voice_sha256,
            reviewer: "independent-voice-reviewer".into(),
        }
    });
    let built = repo.build_teacher(&spec, &out, TeacherOptions::default()).unwrap();
    assert!(!built.dry_run);
    let skill = out.join("master-offline-test").join("SKILL.md");
    assert!(skill.is_file());
    let data = fs::read_to_string(&skill).unwrap();
    assert!(data.contains("<HARD-GATE>"));
    assert!(data.contains("PART A"));
    assert!(out.join("master-offline-test").join("review-record.json").exists());
    assert!(repo.build_teacher(&spec, &out, TeacherOptions::default()).is_err());
    fs::remove_dir_all(out).unwrap();
}

#[test]
fn changing_approved_content_invalidates_review() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let repo = Repository::open(root).unwrap();
    let mut spec = sample();
    let hashes = repo.prepare_teacher(&spec).unwrap();
    spec.review = Some(Review {
        doctrine: Approval { verdict: "PASS".into(), input_sha256: hashes.doctrine_sha256,
            reviewer: "reviewer-a".into() },
        voice: Approval { verdict: "PASS".into(), input_sha256: hashes.voice_sha256,
            reviewer: "reviewer-b".into() },
    });
    spec.teaching_content.push_str(" changed after approval");
    let out = std::env::temp_dir().join("not-created-because-review-stale");
    assert!(repo.build_teacher(&spec, out, TeacherOptions::default()).is_err());
}
