//! Transactional local skill installation. No subprocesses, symlink traversal or
//! network calls. Mutating operations require a caller-supplied installation root.
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::catalog::{CatalogSkill, SkillKind};
use crate::{Error, Repository, Result};

const RECORD: &str = ".master-skill-install.json";

#[derive(Clone, Copy, Debug, Default)]
pub struct InstallOptions {
    pub dry_run: bool,
    pub force: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallAction {
    pub name: String,
    pub destination: String,
    pub action: String,
    pub reason: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct InstallReport {
    pub dry_run: bool,
    pub actions: Vec<InstallAction>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    schema: u32,
    name: String,
    version: String,
    files: BTreeMap<String, String>,
}

fn safe_relative(rel: &str) -> bool {
    !rel.is_empty() && !rel.contains('\\') &&
        Path::new(rel).components().all(|c| matches!(c, Component::Normal(_)))
}

fn path_to_record(rel: &Path) -> Result<String> {
    let mut parts = Vec::new();
    for component in rel.components() {
        match component {
            Component::Normal(value) => parts.push(value.to_str().ok_or_else(|| {
                Error::Invalid("non-UTF8 installation path".into())
            })?),
            _ => return Err(Error::Invalid("non-relative installation path".into())),
        }
    }
    Ok(parts.join("/"))
}

/// Collect only normal files under the selected source roots. Never follow
/// symbolic links, even when they point within the checkout.
fn visit_files(base: &Path, rel: &Path, files: &mut Vec<PathBuf>, generator: bool) -> Result<()> {
    let path = base.join(rel);
    let stat = fs::symlink_metadata(&path)?;
    if stat.file_type().is_symlink() { return Ok(()); }
    if stat.is_dir() {
        if rel.file_name().is_some_and(|n| n == "__pycache__") ||
            (generator && rel.components().next().is_some_and(|c| c.as_os_str() == "masters")) {
            return Ok(());
        }
        let mut children = fs::read_dir(&path)?.collect::<std::result::Result<Vec<_>, _>>()?;
        children.sort_by_key(|entry| entry.file_name());
        for child in children {
            visit_files(base, &rel.join(child.file_name()), files, generator)?;
        }
    } else if stat.is_file() && rel.extension().is_none_or(|e| e != "pyc") &&
        rel.file_name().is_none_or(|e| e != RECORD) {
        files.push(rel.to_path_buf());
    }
    Ok(())
}

pub(crate) fn source_files(repo: &Repository, skill: &CatalogSkill) -> Result<Vec<PathBuf>> {
    let source = repo.source(skill)?;
    let mut files = Vec::new();
    if skill.kind == SkillKind::Generator {
        for bundle in skill.bundle_paths.as_ref().ok_or_else(|| {
            Error::Invalid("generator without bundle_paths".into())
        })? {
            if bundle != "masters" {
                visit_files(&source, Path::new(bundle), &mut files, true)?;
            }
        }
    } else {
        visit_files(&source, Path::new(""), &mut files, false)?;
    }
    files.sort();
    files.dedup();
    Ok(files)
}

pub(crate) fn sha256_regular(path: &Path) -> Result<String> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(Error::Invalid(format!("not a regular file: {}", path.display())));
    }
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 { break; }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn actual_files(root: &Path) -> Result<BTreeSet<String>> {
    fn walk(root: &Path, rel: &Path, out: &mut BTreeSet<String>) -> Result<()> {
        let dir = root.join(rel);
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let relative = rel.join(entry.file_name());
            let name = path_to_record(&relative)?;
            if name == RECORD { continue; }
            let ft = entry.file_type()?;
            if ft.is_dir() {
                if entry.file_name() == "__pycache__" { continue; }
                walk(root, &relative, out)?;
            } else if relative.extension().is_none_or(|x| x != "pyc") {
                // Symlinks count as modifications, never silently discarded.
                out.insert(name);
            }
        }
        Ok(())
    }
    let mut out = BTreeSet::new();
    if root.is_dir() { walk(root, Path::new(""), &mut out)?; }
    Ok(out)
}

/// Compare installed paths to the recorded hashes, catching user edits,
/// replaced links, unexpected extra files and missing records.
pub fn local_modifications(dest: &Path, expected_name: &str) -> Result<Vec<String>> {
    if fs::symlink_metadata(dest).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound) {
        return Ok(Vec::new());
    }
    let stat = fs::symlink_metadata(dest)?;
    if !stat.is_dir() || stat.file_type().is_symlink() {
        return Ok(vec!["linked or non-directory installation".into()]);
    }
    let record = dest.join(RECORD);
    let bytes = match fs::read(&record) {
        Ok(bytes) if fs::symlink_metadata(&record)?.is_file() => bytes,
        _ => return Ok(vec!["missing or unreadable installation record".into()]),
    };
    let manifest: Manifest = match serde_json::from_slice(&bytes) {
        Ok(value) => value,
        Err(_) => return Ok(vec!["invalid installation record".into()]),
    };
    if manifest.schema != 1 || manifest.name != expected_name {
        return Ok(vec!["invalid installation record".into()]);
    }
    let mut actual = actual_files(dest)?;
    // User-generated personas are not managed package files. Preserve them
    // without requiring --force on every generator update.
    if expected_name == "create-master" {
        actual.retain(|name| !name.starts_with("masters/"));
    }
    let mut changed = Vec::new();
    for (name, expected) in manifest.files {
        if !safe_relative(&name) {
            return Ok(vec!["invalid installation record path".into()]);
        }
        if !actual.remove(&name) || sha256_regular(&dest.join(&name)).ok().as_deref() != Some(&expected) {
            changed.push(name);
        }
    }
    changed.extend(actual);
    Ok(changed)
}

fn unique_temp(root: &Path, skill: &str, suffix: &str) -> PathBuf {
    let ns = SystemTime::now().duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos()).unwrap_or_default();
    root.join(format!(".{skill}-{suffix}-{}-{ns}", std::process::id()))
}

fn copy_owned_files(source: &Path, dest: &Path, rels: &[PathBuf]) -> Result<BTreeMap<String, String>> {
    let mut hashes = BTreeMap::new();
    for rel in rels {
        let origin = source.join(rel);
        let dst = dest.join(rel);
        if let Some(parent) = dst.parent() { fs::create_dir_all(parent)?; }
        fs::copy(&origin, &dst)?;
        // Hash copy, not source, so record can be verified independently.
        hashes.insert(path_to_record(rel)?, sha256_regular(&dst)?);
    }
    Ok(hashes)
}

fn copy_user_data(source: &Path, dest: &Path) -> Result<()> {
    // Generated content is owned by the user; an external symlink is not
    // interpreted or recursively traversed during a privileged update.
    let stat = fs::symlink_metadata(source)?;
    if !stat.is_dir() || stat.file_type().is_symlink() {
        return Err(Error::Invalid("generator masters/ is a symlink; back it up manually".into()));
    }
    fn reject_links(root: &Path) -> Result<()> {
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let ft = entry.file_type()?;
            if ft.is_symlink() {
                return Err(Error::Invalid(format!(
                    "generated user data contains a link; back it up manually: {}",
                    entry.path().display()
                )));
            }
            if ft.is_dir() { reject_links(&entry.path())?; }
        }
        Ok(())
    }
    reject_links(source)?;
    fs::create_dir_all(dest)?;
    let mut files = Vec::new();
    visit_files(source, Path::new(""), &mut files, false)?;
    for rel in files {
        let dst = dest.join(&rel);
        if let Some(parent) = dst.parent() { fs::create_dir_all(parent)?; }
        fs::copy(source.join(&rel), dst)?;
    }
    Ok(())
}

impl Repository {
    fn planned_skills(&self, names: &[String]) -> Result<Vec<(String, Option<String>)>> {
        let requested: Vec<&CatalogSkill> = if names.iter().any(|n| n == "--all") {
            self.catalog.skills.iter().collect()
        } else {
            names.iter().map(|name| self.resolve(name))
                .collect::<Result<Vec<_>>>()?
        };
        if requested.is_empty() {
            return Err(Error::Invalid("no skills requested".into()));
        }
        let explicit: HashSet<&str> = requested.iter().map(|s| s.name.as_str()).collect();
        let mut out: Vec<(String, Option<String>)> =
            requested.iter().map(|s| (s.name.clone(), None)).collect();
        let mut seen: HashSet<String> = out.iter().map(|item| item.0.clone()).collect();
        for skill in requested {
            if skill.requires.as_deref() != Some("all-personas") { continue; }
            for candidate in &self.catalog.skills {
                if candidate.kind != SkillKind::Persona ||
                    explicit.contains(candidate.name.as_str()) ||
                    self.install_root.join(&candidate.install_dir).join("SKILL.md").is_file() {
                    continue;
                }
                if seen.insert(candidate.name.clone()) {
                    out.push((candidate.name.clone(), Some(skill.name.clone())));
                }
            }
        }
        Ok(out)
    }

    /// Transactional replacement, with complete preflight: refuse user edits
    /// on any destination before making any changes unless force is explicit.
    pub fn install(&self, names: &[String], options: InstallOptions) -> Result<InstallReport> {
        let plan = self.planned_skills(names)?;
        let mut actions = Vec::new();
        for (name, required_by) in &plan {
            let skill = self.resolve(name)?;
            let dest = self.install_root.join(&skill.install_dir);
            let changes = local_modifications(&dest, &skill.name)?;
            if !changes.is_empty() && !options.force && !options.dry_run {
                return Err(Error::Invalid(format!(
                    "{} has local edits ({}); pass force=true or back up your data",
                    skill.name, changes.into_iter().take(3).collect::<Vec<_>>().join(", ")
                )));
            }
            actions.push(InstallAction {
                name: name.clone(), destination: dest.display().to_string(),
                action: if options.dry_run { "would-install" } else { "installed" }.into(),
                reason: required_by.clone(),
            });
        }
        if options.dry_run { return Ok(InstallReport { dry_run: true, actions }); }
        fs::create_dir_all(&self.install_root)?;
        for (name, _) in plan {
            let skill = self.resolve(&name)?;
            let dest = self.install_root.join(&skill.install_dir);
            let stage = unique_temp(&self.install_root, &skill.install_dir, "stage");
            let backup = unique_temp(&self.install_root, &skill.install_dir, "backup");
            fs::create_dir(&stage)?;
            let prepared = (|| -> Result<()> {
                let source = self.source(skill)?;
                let files = source_files(self, skill)?;
                let hashes = copy_owned_files(&source, &stage, &files)?;
                if skill.kind == SkillKind::Generator {
                    let user_data = dest.join("masters");
                    if fs::symlink_metadata(&user_data).is_ok() {
                        copy_user_data(&user_data, &stage.join("masters"))?;
                    } else {
                        fs::create_dir_all(stage.join("masters"))?;
                    }
                }
                let record = Manifest { schema: 1, name: name.clone(),
                    version: env!("CARGO_PKG_VERSION").into(), files: hashes };
                fs::write(stage.join(RECORD), serde_json::to_vec_pretty(&record)?)?;
                Ok(())
            })();
            if let Err(err) = prepared {
                let _ = fs::remove_dir_all(&stage);
                return Err(err);
            }
            let existed = fs::symlink_metadata(&dest).is_ok();
            if existed { fs::rename(&dest, &backup)?; }
            if let Err(err) = fs::rename(&stage, &dest) {
                if existed {
                    if let Err(rollback) = fs::rename(&backup, &dest) {
                        return Err(Error::Invalid(format!(
                            "install failed: {err}; rollback failed: {rollback}; saved at {}",
                            backup.display()
                        )));
                    }
                }
                let _ = fs::remove_dir_all(stage);
                return Err(Error::Io(err));
            }
            if existed { fs::remove_dir_all(backup)?; }
        }
        Ok(InstallReport { dry_run: false, actions })
    }

    /// Reinstall every catalog entry; same overwrite protections as install.
    pub fn update_all(&self, options: InstallOptions) -> Result<InstallReport> {
        self.install(&["--all".into()], options)
    }

    /// Uninstall is destructive; user edits and generator personas require
    /// an explicit force. In dry-run it never touches the filesystem.
    pub fn uninstall(&self, names: &[String], options: InstallOptions) -> Result<InstallReport> {
        if names.is_empty() { return Err(Error::Invalid("no skills requested".into())); }
        let mut actions = Vec::new();
        for name in names {
            let skill = self.resolve(name)?;
            let dest = self.install_root.join(&skill.install_dir);
            if fs::symlink_metadata(&dest).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound) {
                return Err(Error::NotFound(format!("not installed: {}", skill.name)));
            }
            let changes = local_modifications(&dest, &skill.name)?;
            let generated = if skill.kind == SkillKind::Generator {
                let user_data = dest.join("masters");
                fs::symlink_metadata(&user_data).is_ok() &&
                    (fs::symlink_metadata(&user_data)?.file_type().is_symlink() ||
                     fs::read_dir(&user_data)?.next().is_some())
            } else { false };
            if (!changes.is_empty() || generated) && !options.force {
                return Err(Error::Invalid(format!(
                    "{} contains local edits/generated data; uninstall requires force=true",
                    skill.name
                )));
            }
            actions.push(InstallAction {
                name: skill.name.clone(), destination: dest.display().to_string(),
                action: if options.dry_run { "would-uninstall" } else { "uninstalled" }.into(),
                reason: None,
            });
        }
        if !options.dry_run {
            for action in &actions {
                let dest = Path::new(&action.destination);
                let stat = fs::symlink_metadata(dest)?;
                if stat.is_dir() && !stat.file_type().is_symlink() {
                    fs::remove_dir_all(dest)?;
                } else { fs::remove_file(dest)?; }
            }
        }
        Ok(InstallReport { dry_run: options.dry_run, actions })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn valid_relative_paths() {
        assert!(safe_relative("references/teaching.md"));
        assert!(!safe_relative("../oops"));
        assert!(!safe_relative("C:\\oops"));
    }
}
