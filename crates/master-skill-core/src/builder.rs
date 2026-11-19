//! Reviewed, deterministic persona creation. External LLM/FoJin clients
//! can supply a TeacherSpec; Rust guards source and approval integrity.
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use crate::{Error, Repository, Result};

const DISCLAIMER: &str = "本内容依据历史佛教文献生成，仅供参考学习。如需正式修行指导，请亲近善知识。所有回答均须附 persona 已声明且可核验的来源。";
const SAFETY_BEGIN: &str = "<!-- safety-clause:begin — 共享安全条款，逐字同步自 prompts/safety_clause.md，由 scripts/validate.py 核对；勿在此单独改写 -->";
const SAFETY_END: &str = "<!-- safety-clause:end -->";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CitationContract {
    pub version: u32,
    pub claim_policy: String,
    pub required_for: Vec<String>,
    pub allowed_source_types: Vec<String>,
    pub minimum_claim_coverage: f64,
    pub live_retrieval_allowed: bool,
}
pub fn derive_citation_contract(sources: &[Value]) -> Result<CitationContract> {
    if sources.is_empty() { return Err(Error::Invalid("sources cannot be empty".into())); }
    let mut types = BTreeSet::new();
    for (i, source) in sources.iter().enumerate() {
        let kind = source.get("type").and_then(Value::as_str).unwrap_or("").trim();
        if kind.is_empty() {
            return Err(Error::Invalid(format!("sources[{i}].type must be nonempty")));
        }
        types.insert(kind.to_owned());
    }
    Ok(CitationContract {
        version: 1,
        claim_policy: "declared_sources_only".into(),
        required_for: vec!["doctrinal_claim".into(), "practice_guidance".into(), "text_interpretation".into()],
        allowed_source_types: types.into_iter().collect(),
        minimum_claim_coverage: 0.9,
        live_retrieval_allowed: true,
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Approval {
    pub verdict: String,
    pub input_sha256: String,
    pub reviewer: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Review {
    pub doctrine: Approval,
    pub voice: Approval,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TeacherSpec {
    pub name: String,
    #[serde(default)]
    pub slug: Option<String>,
    pub tradition: String,
    pub school: String,
    pub era: String,
    pub languages: Vec<String>,
    pub teaching_content: String,
    pub voice_content: String,
    pub sources: Vec<Value>,
    #[serde(default)]
    pub citation_contract: Option<CitationContract>,
    #[serde(default)]
    pub fojin_entity_id: Option<String>,
    #[serde(default)]
    pub review: Option<Review>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewInputs {
    pub doctrine: Value,
    pub voice: Value,
    pub doctrine_sha256: String,
    pub voice_sha256: String,
    pub citation_contract: CitationContract,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct TeacherOptions { pub dry_run: bool, pub force: bool }

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeacherResult {
    pub slug: String,
    pub directory: String,
    pub dry_run: bool,
    pub review_inputs: ReviewInputs,
}

fn sha256(data: &[u8]) -> String { format!("{:x}", Sha256::digest(data)) }
fn digest_json(value: &Value) -> Result<String> { Ok(sha256(&serde_json::to_vec(value)?)) }

fn sanitize(input: &str) -> String {
    input.chars().filter(|c| !matches!(*c,
        '\u{0000}'..='\u{0008}' | '\u{000b}'..='\u{000c}' |
        '\u{000e}'..='\u{001f}' | '\u{007f}'..='\u{009f}' |
        '\u{200b}'..='\u{200f}' | '\u{2028}'..='\u{202e}' |
        '\u{2066}'..='\u{2069}' | '\u{feff}')).collect()
}
fn valid_slug(slug: &str) -> bool {
    !slug.is_empty() && slug.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
fn slug(spec: &TeacherSpec) -> Result<String> {
    if let Some(slug) = &spec.slug {
        if !valid_slug(slug) { return Err(Error::Invalid("slug must contain only a-z, 0-9 and -".into())); }
        return Ok(slug.clone());
    }
    let value: String = spec.name.to_lowercase().chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == ' ')
        .map(|c| if c == ' ' { '-' } else { c })
        .collect();
    let value = value.trim_matches('-');
    if valid_slug(value) { Ok(value.into()) }
    else { Ok(format!("unnamed-{}", &sha256(spec.name.as_bytes())[..10])) }
}
fn today() -> String {
    let days = SystemTime::now().duration_since(UNIX_EPOCH)
        .map(|d| (d.as_secs() / 86400) as i64).unwrap_or(0);
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    if m <= 2 { y += 1; }
    format!("{y:04}-{m:02}-{d:02}")
}

pub fn review_inputs(spec: &TeacherSpec) -> Result<ReviewInputs> {
    if spec.name.trim().is_empty() || spec.tradition.trim().is_empty() ||
        spec.school.trim().is_empty() || spec.era.trim().is_empty() ||
        spec.languages.is_empty() || spec.teaching_content.trim().is_empty() ||
        spec.voice_content.trim().is_empty() {
        return Err(Error::Invalid("required teacher spec fields must be nonempty".into()));
    }
    let contract = derive_citation_contract(&spec.sources)?;
    if spec.citation_contract.as_ref().is_some_and(|value| value != &contract) {
        return Err(Error::Invalid("citation_contract differs from derived source types".into()));
    }
    let doctrine = json!({
        "teaching_content": spec.teaching_content,
        "sources": spec.sources,
        "citation_contract": contract,
    });
    let voice = json!({
        "voice_content": spec.voice_content,
        "teaching_content": spec.teaching_content,
    });
    Ok(ReviewInputs {
        doctrine_sha256: digest_json(&doctrine)?,
        voice_sha256: digest_json(&voice)?,
        doctrine, voice, citation_contract: contract,
    })
}

fn assert_approved(spec: &TeacherSpec, inputs: &ReviewInputs) -> Result<()> {
    let review = spec.review.as_ref()
        .ok_or_else(|| Error::Invalid("doctrine and voice review approvals required".into()))?;
    for (stage, actual, hash) in [
        ("doctrine", &review.doctrine, &inputs.doctrine_sha256),
        ("voice", &review.voice, &inputs.voice_sha256),
    ] {
        if actual.verdict != "PASS" || actual.input_sha256 != *hash ||
            actual.reviewer.trim().is_empty() {
            return Err(Error::Invalid(format!("{stage} review is missing or stale")));
        }
    }
    if review.doctrine.reviewer.trim() == review.voice.reviewer.trim() {
        return Err(Error::Invalid("independent reviewers required".into()));
    }
    for value in [&spec.name, &spec.tradition, &spec.school,
        &spec.teaching_content, &spec.voice_content] {
        if sanitize(value) != *value {
            return Err(Error::Invalid("reviewed text contains unsafe control characters".into()));
        }
    }
    Ok(())
}

fn skill_markdown(spec: &TeacherSpec, slug: &str, safety: &str) -> Result<String> {
    // JSON-quoted strings also parse as YAML scalars, and cannot inject keys.
    let fm_name = serde_json::to_string(&format!("master-{slug}"))?;
    let fm_description = serde_json::to_string(
        &format!("依据{}（{}{}）的教学风格与教义体系",
            spec.name, spec.tradition, spec.school))?;
    Ok(format!(r#"---
name: {fm_name}
description: {fm_description}
user-invocable: true
---

# {name}

{DISCLAIMER}

## 决策树：先分流

- 危机信号 → 先执行安全条款 CRISIS FIRST，不进入角色讲法
- 求高下、印证或证果 → 执行 NO RANKING BY THE AI / NO CERTIFICATION
- 其他问题 → 运行规则

<HARD-GATE>

{SAFETY_BEGIN}
{safety}
{SAFETY_END}

**QUOTATION MARKS HOLD ORIGINAL TEXT ONLY — 引号只装原文.**
引用必须逐字复核并附出处；不可伪造原文、来源或出处。

</HARD-GATE>

---

## PART A — 教义体系

{teaching}

## PART B — 说法风格

{voice}

## 运行规则

1. 先执行 HARD-GATE 和 voice.md 硬规则
2. 依据 teaching.md 和 sources/ 检索内容，禁止编造出处
3. 教义断言、修行指导、文本解释必须引用 meta.json.sources[] 中可验证来源
4. live 数据只有来源类型及标识属于允许的 citation_contract 时才可引用
5. 用户要求白话时可用白话，但不可省略出处
6. 遇到超出范围的问题坦诚说明，不得假装本角色已验证

"#, name = spec.name, teaching = spec.teaching_content, voice = spec.voice_content))
}

impl Repository {
    pub fn prepare_teacher(&self, spec: &TeacherSpec) -> Result<ReviewInputs> {
        review_inputs(spec)
    }

    /// Atomically write a reviewed persona; dry_run prepares review hashes only.
    pub fn build_teacher(&self, spec: &TeacherSpec, output_root: impl AsRef<Path>,
        options: TeacherOptions) -> Result<TeacherResult> {
        let inputs = review_inputs(spec)?;
        let slug = slug(spec)?;
        let dest = output_root.as_ref().join(format!("master-{slug}"));
        let report = TeacherResult {
            slug: slug.clone(), directory: dest.display().to_string(),
            dry_run: options.dry_run, review_inputs: inputs,
        };
        if options.dry_run { return Ok(report); }
        assert_approved(spec, &report.review_inputs)?;
        let clause = fs::read_to_string(self.root.join("prompts/safety_clause.md"))?;
        if clause.trim().is_empty() { return Err(Error::Invalid("empty safety clause".into())); }
        let skill = skill_markdown(spec, &slug, clause.trim())?;
        let date = today();
        let meta = json!({
            "name": spec.name, "slug": slug, "tradition": spec.tradition,
            "school": spec.school, "era": spec.era, "languages": spec.languages,
            "fojin_entity_id": spec.fojin_entity_id,
            "sources": spec.sources,
            "citation_contract": report.review_inputs.citation_contract,
            "version": "1.0.0", "created_at": date, "updated_at": date,
            "disclaimer": DISCLAIMER,
        });
        fs::create_dir_all(output_root.as_ref())?;
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos()).unwrap_or(0);
        let temp = output_root.as_ref().join(format!(".master-{slug}-stage-{}-{nonce}",
            std::process::id()));
        let backup = output_root.as_ref().join(format!(".master-{slug}-backup-{}-{nonce}",
            std::process::id()));
        fs::create_dir(&temp)?;
        let prepared = (|| -> Result<()> {
            fs::create_dir(temp.join("versions"))?;
            fs::write(temp.join("teaching.md"), &spec.teaching_content)?;
            fs::write(temp.join("voice.md"), &spec.voice_content)?;
            fs::write(temp.join("SKILL.md"), &skill)?;
            fs::write(temp.join("meta.json"), serde_json::to_vec_pretty(&meta)?)?;
            fs::write(temp.join("doctrine-review-input.json"),
                serde_json::to_vec_pretty(&report.review_inputs.doctrine)?)?;
            let mut record = serde_json::to_value(spec.review.as_ref()
                .expect("review already approved"))?;
            record["skill_sha256"] = json!(sha256(skill.as_bytes()));
            fs::write(temp.join("review-record.json"), serde_json::to_vec_pretty(&record)?)?;
            Ok(())
        })();
        if let Err(err) = prepared {
            let _ = fs::remove_dir_all(&temp);
            return Err(err);
        }
        let existing = fs::symlink_metadata(&dest).is_ok();
        if existing && !options.force {
            let _ = fs::remove_dir_all(&temp);
            return Err(Error::Invalid("persona already exists; force is required".into()));
        }
        if existing { fs::rename(&dest, &backup)?; }
        if let Err(err) = fs::rename(&temp, &dest) {
            if existing {
                if let Err(rollback) = fs::rename(&backup, &dest) {
                    return Err(Error::Invalid(format!("write failed: {err}; rollback failed: {rollback}; backup at {}", backup.display())));
                }
            }
            let _ = fs::remove_dir_all(&temp);
            return Err(err.into());
        }
        if existing { fs::remove_dir_all(&backup)?; }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_empty_manifest() { assert!(derive_citation_contract(&[]).is_err()); }
    #[test]
    fn slug_rejects_traversal() {
        let spec = TeacherSpec {
            name: "Test".into(), slug: Some("../evil".into()),
            tradition: "Test".into(), school: "Test".into(), era: "Test".into(),
            languages: vec!["en".into()], teaching_content: "A".into(),
            voice_content: "B".into(), sources: vec![json!({"type":"cbeta"})],
            citation_contract: None, fojin_entity_id: None, review: None,
        };
        assert!(slug(&spec).is_err());
    }
}
