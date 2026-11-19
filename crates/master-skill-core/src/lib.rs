//! Native, headless Master-skill operations.
//!
//! All methods work on a caller-supplied checkout. No runtime Node, Python,
//! desktop window, network service, or global process state is required.
//! Persona data is NOT bundled in this MIT-licensed crate: the caller supplies
//! a Master-skill checkout, whose content has separate licenses.
mod builder;
mod catalog;
mod install;
mod routing;
mod search;

pub use builder::{derive_citation_contract, review_inputs, Approval, CitationContract, Review, ReviewInputs, TeacherOptions, TeacherResult, TeacherSpec};
pub use catalog::{Catalog, CatalogSkill, DoctorReport, Inspect, Inventory, SkillKind};
pub use install::{InstallAction, InstallOptions, InstallReport};
pub use routing::{MasterMatch, Recommendation};
pub use search::{CitationHit, SearchHit};

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// Stable library error type (does not leak CLI printing or process exits).
#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Json(serde_json::Error),
    Invalid(String),
    NotFound(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Json(e) => write!(f, "JSON error: {e}"),
            Self::Invalid(message) => write!(f, "invalid Master-skill data: {message}"),
            Self::NotFound(name) => write!(f, "unknown skill: {name}"),
        }
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self { Self::Io(value) }
}
impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self { Self::Json(value) }
}

pub type Result<T> = std::result::Result<T, Error>;

/// An explicit repository context. Safe to use from a GUI, CLI, test or MCP
/// adapter; every operation returns data rather than writing stdout.
#[derive(Debug)]
pub struct Repository {
    root: PathBuf,
    install_root: PathBuf,
    catalog: Catalog,
    routing: routing::Routing,
}

impl Repository {
    /// Open and validate catalog/routing from a checkout.
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = fs::canonicalize(root.as_ref())?;
        let catalog: Catalog = read_json(&root.join("skill-catalog.json"))?;
        catalog.validate(&root)?;
        let routing: routing::Routing = read_json(&root.join("routing.json"))?;
        routing.validate(&catalog)?;
        let home = std::env::var_os("HOME")
            .filter(|s| !s.is_empty())
            .or_else(|| std::env::var_os("USERPROFILE").filter(|s| !s.is_empty()))
            .map(PathBuf::from)
            .unwrap_or_else(|| root.clone());
        Ok(Self {
            root, catalog, routing,
            install_root: home.join(".claude").join("skills"),
        })
    }

    /// Override the installed-skills directory without touching the real HOME.
    pub fn with_install_root(mut self, install_root: impl Into<PathBuf>) -> Self {
        self.install_root = install_root.into();
        self
    }
    pub fn root(&self) -> &Path { &self.root }
    pub fn install_root(&self) -> &Path { &self.install_root }
    pub fn catalog(&self) -> &Catalog { &self.catalog }

    pub fn list(&self) -> Result<Inventory> { catalog::list(self) }
    pub fn inspect(&self, name: &str) -> Result<Inspect> { catalog::inspect(self, name) }
    pub fn doctor(&self) -> Result<DoctorReport> { catalog::doctor(self) }
    pub fn recommend(&self, query: &str) -> Result<Recommendation> {
        routing::recommend(self, query)
    }
    pub fn search(&self, name: &str, query: &str, limit: usize) -> Result<Vec<SearchHit>> {
        search::search(self, name, query, limit)
    }
    pub fn citations(&self, name: &str, text: &str) -> Result<Vec<CitationHit>> {
        search::citations(self, name, text)
    }

    /// Canonical catalog name for a supported alias; reject path components.
    pub fn resolve(&self, name: &str) -> Result<&CatalogSkill> {
        if !catalog::safe_name(name) {
            return Err(Error::Invalid(format!("unsafe skill name: {name}")));
        }
        self.catalog.skills.iter()
            .find(|skill| skill.name == name || skill.aliases.iter().any(|alias| alias == name))
            .ok_or_else(|| Error::NotFound(name.to_owned()))
    }

    pub(crate) fn source(&self, skill: &CatalogSkill) -> Result<PathBuf> {
        // Verified at construction, checked again to detect post-open changes.
        let path = fs::canonicalize(self.root.join(&skill.source))?;
        if !path.starts_with(&self.root) || !path.is_dir() {
            return Err(Error::Invalid(format!("unsafe skill source: {}", skill.source)));
        }
        Ok(path)
    }

    pub(crate) fn meta(&self, skill: &CatalogSkill) -> Result<serde_json::Value> {
        let path = self.source(skill)?.join("meta.json");
        if path.is_file() { read_json(&path) } else { Ok(serde_json::json!({})) }
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

#[cfg(all(test, feature = "repository-fixtures"))]
mod tests {
    use super::*;
    fn checkout() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn catalog_has_21_entries() {
        let repo = Repository::open(checkout()).unwrap();
        let inv = repo.list().unwrap();
        assert_eq!(inv.skill_count, 21);
        assert_eq!(inv.count, 20);
        assert_eq!(repo.resolve("huineng").unwrap().name, "master-huineng");
        assert!(repo.resolve("../huineng").is_err());
    }
    #[test]
    fn full_and_short_inspection_are_equal() {
        let repo = Repository::open(checkout()).unwrap();
        assert_eq!(repo.inspect("huineng").unwrap(), repo.inspect("master-huineng").unwrap());
    }
    #[test]
    fn no_crisis_recommends_a_persona() {
        let repo = Repository::open(checkout()).unwrap();
        let result = repo.recommend("I want to die").unwrap();
        assert_eq!(result.kind, "crisis");
        assert!(result.masters.is_empty());
        assert!(result.command.is_none());
    }
    #[test]
    fn routing_mode_precedes_persona() {
        let repo = Repository::open(checkout()).unwrap();
        let result = repo.recommend("禅宗从哪开始学 入门").unwrap();
        assert_eq!(result.mode.as_deref(), Some("master-curriculum"));
    }
    #[test]
    fn missing_skill_is_an_error() {
        let repo = Repository::open(checkout()).unwrap();
        assert!(matches!(repo.inspect("nonexistent"), Err(Error::NotFound(_))));
    }
}
