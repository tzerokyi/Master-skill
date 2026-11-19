//! Offline source/references retrieval. No RAG service or interpreter.
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use serde::Serialize;
use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};
use crate::{Error, Repository, Result};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SearchHit {
    pub section: String,
    pub preview: String,
    pub file: String,
}
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CitationHit {
    pub section: String,
    pub citation: String,
    pub file: String,
}

fn sections(content: &str) -> Vec<(String, String)> {
    let mut result: Vec<(String, String)> = Vec::new();
    for line in content.lines() {
        if let Some(header) = line.strip_prefix("## ") {
            result.push((header.trim().to_owned(), String::new()));
        } else if let Some((_, body)) = result.last_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }
    result
}

fn documents(root: &Path, folder: &str) -> Result<Vec<(String, String, String)>> {
    let dir = root.join(folder);
    if !dir.exists() { return Ok(Vec::new()); }
    let dir = fs::canonicalize(&dir)?;
    if !dir.starts_with(root) || !dir.is_dir() {
        return Err(Error::Invalid(format!("unsafe {folder}/ directory")));
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() { continue; }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "INDEX.md" || !name.ends_with(".md") { continue; }
        let path = fs::canonicalize(entry.path())?;
        if !path.starts_with(root) {
            return Err(Error::Invalid(format!("source file escapes root: {name}")));
        }
        paths.push((name, path));
    }
    paths.sort_by(|a, b| a.0.cmp(&b.0));
    paths.into_iter()
        .map(|(name, path)| Ok((folder.to_string(), name, fs::read_to_string(path)?)))
        .collect()
}

fn fold(input: &str) -> String {
    input.to_lowercase().nfkd().filter(|c| !is_combining_mark(*c))
        .map(|c| if c == '著' { '着' } else { c })
        .collect()
}
fn keywords(query: &str) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    let mut token = String::new();
    let mut is_han: Option<bool> = None;
    fn emit(result: &mut BTreeSet<String>, token: &str, han: bool) {
        if token.is_empty() { return; }
        if han {
            let chars: Vec<char> = token.chars().collect();
            if chars.len() == 1 { result.insert(token.to_owned()); }
            for width in 2..=chars.len().min(4) {
                for window in chars.windows(width) {
                    result.insert(window.iter().collect());
                }
            }
        } else if token.chars().count() > 1 {
            result.insert(token.to_owned());
        }
    }
    for c in fold(query).chars() {
        let han = matches!(c, '\u{3400}'..='\u{9fff}' | '\u{f900}'..='\u{faff}');
        let latin = c.is_ascii_alphanumeric();
        if !han && !latin {
            if let Some(prev) = is_han { emit(&mut result, &token, prev); }
            token.clear();
            is_han = None;
            continue;
        }
        if is_han != Some(han) {
            if let Some(prev) = is_han { emit(&mut result, &token, prev); }
            token.clear();
            is_han = Some(han);
        }
        token.push(c);
    }
    if let Some(prev) = is_han { emit(&mut result, &token, prev); }
    result
}

pub(crate) fn search(repo: &Repository, name: &str, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
    let terms = keywords(query);
    if terms.is_empty() || limit == 0 { return Ok(Vec::new()); }
    let root = repo.source(repo.resolve(name)?)?;
    let mut matches: Vec<(usize, usize, usize, SearchHit)> = Vec::new();
    let mut order = 0;
    for folder in ["sources", "references"] {
        for (folder, file, content) in documents(&root, folder)? {
            for (title, body) in sections(&content) {
                let haystack = fold(&format!("{title}\n{body}"));
                let matched: Vec<&String> = terms.iter().filter(|term| haystack.contains(term.as_str())).collect();
                if matched.is_empty() { continue; }
                let preview: String = body.trim().chars().take(200).collect();
                matches.push((
                    matched.iter().map(|term| term.chars().count()).max().unwrap_or(0),
                    matched.len(), order,
                    SearchHit { section: title, preview, file: format!("{folder}/{file}") },
                ));
                order += 1;
            }
        }
    }
    matches.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1))
        .then_with(|| a.2.cmp(&b.2)));
    Ok(matches.into_iter().take(limit.min(100)).map(|(_, _, _, hit)| hit).collect())
}

pub(crate) fn citations(repo: &Repository, name: &str, text: &str) -> Result<Vec<CitationHit>> {
    if text.trim().is_empty() {
        return Err(Error::Invalid("citation search text cannot be empty".into()));
    }
    let root = repo.source(repo.resolve(name)?)?;
    let dir = root.join("sources");
    if !dir.is_dir() {
        return Err(Error::Invalid(format!("no sources directory for {name}")));
    }
    let mut result = Vec::new();
    for (_, filename, content) in documents(&root, "sources")? {
        for (title, body) in sections(&content) {
            if !format!("{title}\n{body}").contains(text) { continue; }
            for line in body.lines() {
                if line.trim().contains("【《") {
                    result.push(CitationHit {
                        section: title.clone(),
                        citation: line.trim().into(),
                        file: filename.clone(),
                    });
                }
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chinese_ngrams_and_diacritics() {
        let terms = keywords("ānāpānasati 无念");
        assert!(terms.contains("anapanasati"));
        assert!(terms.contains("无念"));
    }
    #[test]
    fn headings_are_split() {
        let s = sections("# intro\n## A\nA body\n## B\nB body");
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].0, "A");
        assert!(s[1].1.contains("B body"));
    }
}
