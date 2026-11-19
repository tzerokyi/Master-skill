use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Component, Path};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::{Error, Repository, Result};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SkillKind { Persona, TeachingMode, Generator }

#[derive(Debug, Clone, Deserialize)]
pub struct Catalog {
    pub version: u32,
    pub skills: Vec<CatalogSkill>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CatalogSkill {
    pub name: String,
    pub kind: SkillKind,
    pub source: String,
    pub install_dir: String,
    pub aliases: Vec<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub requires: Option<String>,
    #[serde(default)]
    pub bundle_paths: Option<Vec<String>>,
}

pub(crate) fn safe_name(value: &str) -> bool {
    !value.is_empty() &&
        value.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn safe_relative(value: &str, allow_dot: bool) -> bool {
    if allow_dot && value == "." { return true; }
    if value.is_empty() || value == "." || value.contains('\\') || value.contains(':') {
        return false;
    }
    Path::new(value).components().all(|component| matches!(component, Component::Normal(_)))
}

impl Catalog {
    pub(crate) fn validate(&self, root: &Path) -> Result<()> {
        if self.version != 1 || self.skills.is_empty() {
            return Err(Error::Invalid("catalog version must be 1 with nonempty skills".into()));
        }
        let mut names = HashSet::new();
        let mut sources = HashSet::new();
        let mut installs = HashSet::new();
        let mut alias_owners: HashMap<&str, &str> = HashMap::new();
        for skill in &self.skills {
            if !safe_name(&skill.name) || !safe_name(&skill.install_dir)
                || !safe_relative(&skill.source, true) {
                return Err(Error::Invalid(format!("invalid catalog path/name: {}", skill.name)));
            }
            if !names.insert(skill.name.as_str())
                || !sources.insert(skill.source.as_str())
                || !installs.insert(skill.install_dir.as_str()) {
                return Err(Error::Invalid(format!("duplicate catalog entry: {}", skill.name)));
            }
            let source = fs::canonicalize(root.join(&skill.source))?;
            if !source.starts_with(root) || !source.is_dir() {
                return Err(Error::Invalid(format!("skill source escapes repository: {}", skill.source)));
            }
            if let Some(ref required) = skill.requires {
                if skill.kind != SkillKind::TeachingMode || required != "all-personas" {
                    return Err(Error::Invalid(format!("bad requires on {}", skill.name)));
                }
            }
            if skill.kind == SkillKind::Generator {
                let bundles = skill.bundle_paths.as_ref().ok_or_else(|| {
                    Error::Invalid("generator missing bundle_paths".into())
                })?;
                if bundles.is_empty() { return Err(Error::Invalid("empty bundle_paths".into())); }
                let mut seen = HashSet::new();
                for bundle in bundles {
                    if !safe_relative(bundle, false) || !seen.insert(bundle) {
                        return Err(Error::Invalid(format!("unsafe/duplicate bundle path: {bundle}")));
                    }
                    let target = fs::canonicalize(source.join(bundle))?;
                    if !target.starts_with(&source) {
                        return Err(Error::Invalid(format!("bundle escapes source: {bundle}")));
                    }
                }
            } else if skill.bundle_paths.is_some() {
                return Err(Error::Invalid("non-generator has bundle_paths".into()));
            }
            for alias in &skill.aliases {
                if !safe_name(alias) {
                    return Err(Error::Invalid(format!("unsafe alias: {alias}")));
                }
                if let Some(owner) = alias_owners.insert(alias, &skill.name) {
                    return Err(Error::Invalid(format!("duplicate alias: {alias} ({owner})")));
                }
            }
        }
        for (alias, owner) in alias_owners {
            if let Some(other) = self.skills.iter().find(|skill| skill.name == alias) {
                if other.name != owner {
                    return Err(Error::Invalid(format!("alias shadows canonical name: {alias}")));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SkillSummary {
    pub name: String,
    pub kind: SkillKind,
    pub install_dir: String,
    pub description: String,
    pub display_name: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MasterSummary {
    pub name: String,
    pub slug: String,
    pub description: String,
    pub display_name: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Inventory {
    pub count: usize,
    pub skill_count: usize,
    pub category_counts: BTreeMap<String, usize>,
    pub skills: Vec<SkillSummary>,
    pub masters: Vec<MasterSummary>,
}

fn value_str<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}
fn keywords(meta: &Value) -> Vec<String> {
    meta.pointer("/search_scope/keywords").and_then(Value::as_array)
        .into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect()
}
fn frontmatter_value(path: &Path, key: &str) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    if !content.starts_with("---\n") && !content.starts_with("---\r\n") { return None; }
    for line in content.lines().skip(1) {
        if line.trim() == "---" { break; }
        if line.starts_with(' ') || line.starts_with('-') { continue; }
        if let Some((name, raw)) = line.split_once(':') {
            if name == key {
                let text = raw.trim().trim_matches('"').trim_matches('\'');
                return (!text.is_empty()).then(|| text.to_owned());
            }
        }
    }
    None
}

pub(crate) fn list(repo: &Repository) -> Result<Inventory> {
    let mut counts = BTreeMap::from([
        ("persona".into(), 0usize),
        ("teaching-mode".into(), 0usize),
        ("generator".into(), 0usize),
    ]);
    let mut skills = Vec::new();
    let mut masters = Vec::new();
    for skill in &repo.catalog.skills {
        let meta = repo.meta(skill)?;
        let dir = repo.source(skill)?;
        let description = frontmatter_value(&dir.join("SKILL.md"), "description")
            .unwrap_or_default();
        let display_name = value_str(&meta, "name").map(str::to_owned);
        let summary = skill.summary.clone()
            .or_else(|| display_name.clone())
            .unwrap_or_else(|| description.clone());
        let kind = match skill.kind {
            SkillKind::Persona => "persona",
            SkillKind::TeachingMode => "teaching-mode",
            SkillKind::Generator => "generator",
        };
        *counts.get_mut(kind).expect("all kinds initialized") += 1;
        if skill.kind != SkillKind::Generator {
            masters.push(MasterSummary {
                name: skill.name.clone(),
                slug: skill.name.strip_prefix("master-").unwrap_or(&skill.name).to_owned(),
                description: description.clone(),
                display_name: display_name.clone(),
                summary: summary.clone(),
            });
        }
        skills.push(SkillSummary {
            name: skill.name.clone(),
            kind: skill.kind, install_dir: skill.install_dir.clone(),
            description, display_name, summary,
        });
    }
    // Legacy list includes every prebuilt directory, not just personas.
    masters.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Inventory { count: masters.len(), skill_count: skills.len(),
        category_counts: counts, skills, masters })
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Inspect {
    pub name: String,
    pub display_name: Option<String>,
    pub slug: String,
    pub version: Option<String>,
    pub tradition: Option<String>,
    pub school: Option<String>,
    pub era: Option<String>,
    pub installed: bool,
    pub live_grounding: bool,
    pub citation_format: Option<String>,
    pub sources: Vec<String>,
    pub search_keywords: Vec<String>,
}

pub(crate) fn inspect(repo: &Repository, name: &str) -> Result<Inspect> {
    let skill = repo.resolve(name)?;
    let dir = repo.source(skill)?;
    let meta = repo.meta(skill)?;
    let content = fs::read_to_string(dir.join("SKILL.md")).unwrap_or_default();
    let sources = meta.get("sources").and_then(Value::as_array)
        .into_iter().flatten().map(|source| {
            [value_str(source, "id"), value_str(source, "title")]
                .into_iter().flatten().collect::<Vec<_>>().join(" — ")
        }).collect();
    Ok(Inspect {
        name: skill.name.clone(),
        display_name: value_str(&meta, "name").map(str::to_owned),
        slug: value_str(&meta, "slug").unwrap_or_else(|| {
            skill.name.strip_prefix("master-").unwrap_or(&skill.name)
        }).to_owned(),
        version: frontmatter_value(&dir.join("SKILL.md"), "version")
            .or_else(|| value_str(&meta, "version").map(str::to_owned)),
        tradition: value_str(&meta, "tradition").map(str::to_owned),
        school: value_str(&meta, "school").map(str::to_owned),
        era: value_str(&meta, "era").map(str::to_owned),
        installed: repo.install_root.join(&skill.install_dir).is_dir(),
        live_grounding: content.contains("FoJin 实时检索") || content.contains("FoJin live"),
        citation_format: frontmatter_value(&dir.join("SKILL.md"), "citation_format"),
        sources,
        search_keywords: keywords(&meta),
    })
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Problem { pub code: String, pub name: String, pub message: String }

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DoctorReport {
    pub package_version: String,
    pub prebuilt_path: String,
    pub skills_path: String,
    pub available_skills: usize,
    pub installed_known_skills: usize,
    pub catalog_skills: usize,
    pub installed_catalog_skills: usize,
    pub other_installed_skill_dirs: usize,
    pub status: String,
    pub problems: Vec<Problem>,
    pub warnings: Vec<Problem>,
}

pub(crate) fn doctor(repo: &Repository) -> Result<DoctorReport> {
    let mut problems = Vec::new();
    let mut installed_names = HashSet::new();
    if repo.install_root.is_dir() {
        for entry in fs::read_dir(&repo.install_root)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                installed_names.insert(entry.file_name().to_string_lossy().into_owned());
            }
        }
    }
    for skill in &repo.catalog.skills {
        if !repo.source(skill)?.join("SKILL.md").is_file() {
            problems.push(Problem { code: "missing-skill-md".into(),
                name: skill.name.clone(), message: format!("{} is missing SKILL.md", skill.name) });
        }
    }
    let known: HashSet<&str> = repo.catalog.skills.iter().map(|s| s.install_dir.as_str()).collect();
    let installed_catalog_skills = installed_names.iter()
        .filter(|name| known.contains(name.as_str())).count();
    let installed_known_skills = repo.catalog.skills.iter()
        .filter(|skill| skill.kind != SkillKind::Generator
            && installed_names.contains(&skill.install_dir)).count();
    let warnings = if installed_catalog_skills == 0 {
        vec![Problem { code: "nothing-installed".into(), name: String::new(),
            message: "No skills installed; use native CLI for discovery, or legacy installer until native install parity ships".into() }]
    } else { Vec::new() };
    let inv = list(repo)?;
    Ok(DoctorReport {
        package_version: env!("CARGO_PKG_VERSION").into(),
        prebuilt_path: repo.root.join("prebuilt").display().to_string(),
        skills_path: repo.install_root.display().to_string(),
        available_skills: inv.count,
        installed_known_skills,
        catalog_skills: repo.catalog.skills.len(),
        installed_catalog_skills,
        other_installed_skill_dirs: installed_names.iter()
            .filter(|name| !known.contains(name.as_str())).count(),
        status: if problems.is_empty() { "ok" } else { "problems" }.into(),
        problems, warnings,
    })
}

pub(crate) fn persona_keywords(meta: &Value) -> Vec<String> { keywords(meta) }
pub(crate) fn meta_str<'a>(meta: &'a Value, key: &str) -> Option<&'a str> {
    value_str(meta, key)
}
