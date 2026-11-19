//! Deterministic routing port of bin/cli.mjs and routing.json.
use std::collections::{BTreeMap, HashSet};
use serde::{Deserialize, Serialize};
use crate::catalog::{meta_str, persona_keywords, SkillKind};
use crate::{Error, Repository, Result};

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct Routing {
    pub version: u32,
    #[serde(default)]
    pub weights: Weights,
    #[serde(default)]
    pub min_keyword_length: usize,
    #[serde(default)]
    pub crisis: Crisis,
    #[serde(default)]
    pub romanized: BTreeMap<String, String>,
    #[serde(default)]
    pub mode_rules: Vec<ModeRule>,
    #[serde(default)]
    pub situations: Vec<RouteRow>,
    #[serde(default)]
    pub topic_pairings: Vec<RouteRow>,
    #[serde(default)]
    pub default_pairing: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct Weights {
    #[serde(default)]
    pub keyword_hit: usize,
}
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct Crisis {
    #[serde(default)]
    pub keywords: Vec<String>,
}
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ModeRule {
    pub mode: String,
    pub order: usize,
    pub keywords: Vec<String>,
    pub note: Option<String>,
}
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RouteRow {
    pub id: String,
    pub keywords: Vec<String>,
    pub masters: Vec<String>,
    pub note: Option<String>,
    #[serde(default)]
    pub referral: bool,
}

impl Routing {
    pub(crate) fn validate(&self, catalog: &crate::Catalog) -> Result<()> {
        if self.version != 1 { return Err(Error::Invalid("routing version must be 1".into())); }
        let personas: HashSet<&str> = catalog.skills.iter()
            .filter(|s| s.kind == SkillKind::Persona).map(|s| s.name.as_str()).collect();
        let modes: HashSet<&str> = catalog.skills.iter()
            .filter(|s| s.kind == SkillKind::TeachingMode).map(|s| s.name.as_str()).collect();
        for mode in &self.mode_rules {
            if !modes.contains(mode.mode.as_str()) {
                return Err(Error::Invalid(format!("unknown mode: {}", mode.mode)));
            }
        }
        for name in self.default_pairing.iter()
            .chain(self.situations.iter().flat_map(|row| row.masters.iter()))
            .chain(self.topic_pairings.iter().flat_map(|row| row.masters.iter())) {
            if !personas.contains(name.as_str()) {
                return Err(Error::Invalid(format!("unknown routed persona: {name}")));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MasterMatch {
    pub name: String,
    pub command: String,
    pub tradition: String,
    pub score: usize,
    pub matched: Vec<String>,
}
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DoctrinalFallback {
    pub name: String,
    pub command: String,
    pub why: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    pub query: String,
    pub resolved_by: String,
    pub kind: String,
    pub mode: Option<String>,
    pub command: Option<String>,
    pub matched: Vec<String>,
    pub note: Option<String>,
    pub masters: Vec<MasterMatch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referral: Option<bool>,
    #[serde(rename = "ifDoctrinal", skip_serializing_if = "Option::is_none")]
    pub if_doctrinal: Option<DoctrinalFallback>,
}

impl Recommendation {
    fn new(query: &str, resolved_by: &str, kind: &str) -> Self {
        Self {
            query: query.to_owned(),
            resolved_by: resolved_by.to_owned(),
            kind: kind.to_owned(), mode: None, command: None, matched: Vec::new(),
            note: None, masters: Vec::new(), referral: None, if_doctrinal: None,
        }
    }
}

#[derive(Debug)]
struct Candidate {
    name: String,
    tradition: String,
    score: usize,
    matched: Vec<String>,
}

fn hit(text: &str, word: &str) -> bool {
    let word = word.to_lowercase();
    if word.is_empty() { return false; }
    if !word.is_ascii() { return text.contains(&word); }
    // Node's ASCII rules: no embedded word matches, simple suffixes accepted.
    // Byte offsets are safe here: ASCII matches always begin/end on UTF-8 boundaries.
    for (start, _) in text.match_indices(&word) {
        let end = start + word.len();
        if start > 0 && text.as_bytes()[start - 1].is_ascii_alphanumeric() { continue; }
        for suffix in ["", "s", "es", "d", "ed", "ing"] {
            let after = end + suffix.len();
            if text.get(end..after) == Some(suffix)
                && (after == text.len() ||
                    !text.as_bytes()[after].is_ascii_alphanumeric()) {
                return true;
            }
        }
    }
    false
}

fn matching(text: &str, keywords: &[String]) -> Vec<String> {
    keywords.iter().filter(|word| hit(text, word)).cloned().collect()
}
fn contains_han(text: &str) -> bool {
    text.chars().any(|c| matches!(c,
        '\u{3400}'..='\u{9fff}' | '\u{f900}'..='\u{faff}' | '\u{20000}'..='\u{2a6df}'))
}

fn personas(repo: &Repository, text: &str) -> Result<Vec<Candidate>> {
    let weight = if repo.routing.weights.keyword_hit == 0 { 3 } else { repo.routing.weights.keyword_hit };
    let minimum = if repo.routing.min_keyword_length == 0 { 2 } else { repo.routing.min_keyword_length };
    let mut scored = Vec::new();
    for skill in &repo.catalog.skills {
        if skill.kind != SkillKind::Persona { continue; }
        let meta = repo.meta(skill)?;
        let keywords = persona_keywords(&meta);
        let eligible: Vec<String> = keywords.into_iter()
            .filter(|keyword| keyword.chars().count() >= minimum).collect();
        let matched = matching(text, &eligible);
        if !matched.is_empty() {
            scored.push(Candidate {
                name: skill.name.clone(),
                tradition: meta_str(&meta, "tradition").unwrap_or("(unspecified)").into(),
                score: matched.len() * weight,
                matched,
            });
        }
    }
    scored.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.name.cmp(&b.name)));
    Ok(scored)
}
fn to_match(c: Candidate) -> MasterMatch {
    MasterMatch { command: format!("/{}", c.name), name: c.name,
        tradition: c.tradition, score: c.score, matched: c.matched }
}
fn expand(repo: &Repository, names: &[String]) -> Result<Vec<MasterMatch>> {
    let mut out = Vec::new();
    for name in names {
        let meta = repo.meta(repo.resolve(name)?)?;
        out.push(MasterMatch {
            name: name.to_owned(), command: format!("/{name}"),
            tradition: meta_str(&meta, "tradition").unwrap_or("(unspecified)").to_owned(),
            score: 0, matched: Vec::new(),
        });
    }
    Ok(out)
}
fn row<'a>(rows: &'a [RouteRow], text: &str) -> Option<(&'a RouteRow, Vec<String>)> {
    let mut found: Vec<(&RouteRow, Vec<String>)> = rows.iter()
        .filter_map(|row| {
            let hits = matching(text, &row.keywords);
            (!hits.is_empty()).then_some((row, hits))
        }).collect();
    found.sort_by(|(a, ah), (b, bh)| {
        bh.len().cmp(&ah.len())
            .then_with(|| bh.iter().map(|s| s.chars().count()).max().cmp(
                &ah.iter().map(|s| s.chars().count()).max()))
            .then_with(|| a.id.cmp(&b.id))
    });
    found.into_iter().next()
}

pub(crate) fn recommend(repo: &Repository, query: &str) -> Result<Recommendation> {
    if query.trim().is_empty() {
        return Err(Error::Invalid("query cannot be empty".into()));
    }
    let original = query.to_lowercase();
    let expanded: Vec<&str> = repo.routing.romanized.iter()
        .filter(|(term, _)| hit(&original, term))
        .map(|(_, target)| target.as_str()).collect();
    let q = if expanded.is_empty() { original }
        else { format!("{} {}", original, expanded.join(" ")) };
    let crisis = matching(&q, &repo.routing.crisis.keywords);
    if !crisis.is_empty() {
        let mut result = Recommendation::new(query, "crisis", "crisis");
        result.matched = crisis;
        result.note = Some("危机表述：不推荐祖师，请立即寻求专业帮助".into());
        result.if_doctrinal = Some(DoctrinalFallback {
            name: "master-buddhaghosa".into(),
            command: "/master-buddhaghosa".into(),
            why: "上座部律藏注释：戒律与伦理".into(),
        });
        return Ok(result);
    }
    let mut rules = repo.routing.mode_rules.iter().collect::<Vec<_>>();
    rules.sort_by_key(|rule| rule.order);
    for rule in rules {
        let matched = matching(&q, &rule.keywords);
        if matched.is_empty() { continue; }
        let mut result = Recommendation::new(query, "mode_rules", "teaching-mode");
        result.mode = Some(rule.mode.clone());
        result.command = Some(format!("/{}", rule.mode));
        result.matched = matched;
        result.note = rule.note.clone();
        return Ok(result);
    }
    let mut scored = personas(repo, &q)?;
    if !scored.is_empty() {
        let mut result = Recommendation::new(query, "persona_keywords", "persona");
        let mut chosen = Vec::new();
        let mut seen = HashSet::new();
        while !scored.is_empty() && chosen.len() < 3 {
            let at = scored.iter()
                .position(|c| !seen.contains(&c.tradition)).unwrap_or(0);
            let candidate = scored.remove(at);
            seen.insert(candidate.tradition.clone());
            chosen.push(to_match(candidate));
        }
        result.masters = chosen;
        return Ok(result);
    }
    for (rows, by) in [
        (repo.routing.situations.as_slice(), "situations"),
        (repo.routing.topic_pairings.as_slice(), "topic_pairings"),
    ] {
        if let Some((route, matched)) = row(rows, &q) {
            let mut result = Recommendation::new(query, by, "persona");
            result.matched = matched;
            result.note = route.note.clone();
            if route.referral { result.referral = Some(true); }
            result.masters = expand(repo, &route.masters)?;
            return Ok(result);
        }
    }
    if !contains_han(&q) {
        let mut result = Recommendation::new(query, "unmatched_non_chinese", "none");
        result.note = Some("Keyword matching is Chinese-based; no route for this query".into());
        return Ok(result);
    }
    let mut result = Recommendation::new(query, "default_pairing", "persona");
    result.note = Some("无关键词命中，回退到默认配对".into());
    result.masters = expand(repo, &repo.routing.default_pairing)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ascii_keywords_do_not_match_inside_words() {
        assert!(!hit("zenith", "zen"));
        assert!(!hit("self-harmony", "self-harm"));
        assert!(hit("I practice zen", "zen"));
        assert!(hit("curriculums", "curriculum"));
    }
    #[test]
    fn han_keywords_are_substrings() {
        assert!(hit("我要学习禅宗", "禅宗"));
    }
}
