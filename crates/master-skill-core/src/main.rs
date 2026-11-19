//! Small native CLI adapter over master_skill_core::Repository.
//! No shell subprocess, interpreter, browser, listener or desktop framework.
use std::env;
use std::path::PathBuf;
use master_skill_core::{InstallOptions, Repository, TeacherOptions, TeacherSpec};
use serde::Serialize;

const HELP: &str = r#"master-skill (native Rust core)

USAGE:
  master-skill [--root PATH] [--json] <COMMAND> [arguments]

COMMANDS:
  list                         List all 21 installable skills
  inspect <name>               Inspect persona metadata and declared sources
  recommend <query...>         Route a question to a persona or teaching mode
  search <name> <query...>     Search local sources/ and references/
  cite <name> <text...>        Find declared citation lines in local sources/
  doctor                       Check checkout and installed skill directories
  validate                     Validate catalog, routing, and skill paths
  install <names...> | --all    Install selected skills transactionally
  update --all                 Reinstall catalog with change protection
  uninstall <names...>         Remove selected installed skills
  review-teacher --spec FILE   Prepare canonical review hashes for persona spec
  build-teacher --spec FILE    Build approved persona into --output DIR

OPTIONS:
  --root PATH                  Checkout root (or MASTER_SKILL_ROOT)
  --install-root PATH          Alternative target directory (default ~/.claude/skills)
  --spec FILE                  JSON persona generation specification
  --output DIR                 Destination directory for generated personas
  --dry-run                    Preview writes without changing disk
  --force                      Permit overwriting locally modified installs
  --json                       Print structured JSON on stdout
  --limit N                    Max search results (default 20, max 100)
  --master NAME                Named alternative to positional name for search/cite
  --q QUERY, --text TEXT       Named alternative to positional query for search/cite
  --brief                      Show search titles/paths only (human output)
  --help, -h                   Show help
  --version, -V                Show crate version

Native reviewed offline generator is available; FoJin retrieval/LLM and desktop
Node/Python bridge remain unported.
See README.md for migration status.
"#;

#[derive(Default)]
struct Options {
    root: Option<PathBuf>,
    install_root: Option<PathBuf>,
    spec: Option<PathBuf>,
    output: Option<PathBuf>,
    dry_run: bool,
    force: bool,
    json: bool,
    brief: bool,
    limit: usize,
    master: Option<String>,
    query: Option<String>,
    args: Vec<String>,
}

fn parse() -> std::result::Result<Options, String> {
    let mut opt = Options { limit: 20, ..Options::default() };
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => opt.root = Some(PathBuf::from(
                args.next().ok_or("--root requires a path")?)),
            "--json" => opt.json = true,
            "--spec" => opt.spec = Some(PathBuf::from(args.next().ok_or("--spec needs JSON path")?)),
            "--output" => opt.output = Some(PathBuf::from(args.next().ok_or("--output needs a path")?)),
            "--dry-run" => opt.dry_run = true,
            "--force" => opt.force = true,
            "--install-root" => opt.install_root = Some(PathBuf::from(
                args.next().ok_or("--install-root requires a path")?)),
            "--brief" => opt.brief = true,
            "--limit" => {
                opt.limit = args.next().ok_or("--limit requires a number")?
                    .parse().map_err(|_| "--limit must be an integer")?;
                if opt.limit == 0 || opt.limit > 100 {
                    return Err("--limit must be in 1..=100".into());
                }
            }
            "--master" => opt.master = Some(args.next().ok_or("--master requires a name")?),
            "--q" | "--text" => opt.query = Some(args.next().ok_or("--q/--text requires text")?),
            "--help" | "-h" => opt.args.push("--help".into()),
            "--version" | "-V" => opt.args.push("--version".into()),
            "--all" => opt.args.push("--all".into()),
            _ if arg.starts_with('-') => return Err(format!("unknown option: {arg}")),
            _ => opt.args.push(arg),
        }
    }
    Ok(opt)
}

fn discover(explicit: Option<PathBuf>) -> std::result::Result<PathBuf, String> {
    if let Some(path) = explicit.or_else(|| env::var_os("MASTER_SKILL_ROOT").map(PathBuf::from)) {
        return Ok(path);
    }
    let cwd = env::current_dir().map_err(|e| format!("cannot read current directory: {e}"))?;
    for dir in cwd.ancestors() {
        if dir.join("skill-catalog.json").is_file() && dir.join("routing.json").is_file() {
            return Ok(dir.to_path_buf());
        }
    }
    Err("checkout not found: pass --root PATH or set MASTER_SKILL_ROOT".into())
}

fn print_json<T: Serialize>(value: &T) -> std::result::Result<(), serde_json::Error> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
fn name_and_query<'a>(opt: &'a Options, rest: &'a [String]) -> std::result::Result<(&'a str, String), String> {
    let (name, positional_query) = if let Some(name) = opt.master.as_deref() {
        (name, rest)
    } else {
        let (name, rest) = rest.split_first()
            .ok_or("missing skill name; use <name> or --master NAME")?;
        (name.as_str(), rest)
    };
    let query = opt.query.clone().unwrap_or_else(|| positional_query.join(" "));
    if query.trim().is_empty() { return Err("missing query or text".into()); }
    Ok((name, query))
}

fn execute(opt: Options) -> std::result::Result<i32, Box<dyn std::error::Error>> {
    if opt.args.first().is_none_or(|s| s == "--help") {
        println!("{HELP}");
        return Ok(0);
    }
    if opt.args.first().is_some_and(|s| s == "--version") {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return Ok(0);
    }
    let root = discover(opt.root.clone())?;
    let mut repo = Repository::open(root)?;
    if let Some(install_root) = &opt.install_root {
        repo = repo.with_install_root(install_root);
    }
    let command = opt.args.first().map(String::as_str).unwrap_or("");
    let args = &opt.args[1..];
    let status = match command {
        "list" => {
            let data = repo.list()?;
            if opt.json { print_json(&data)?; }
            else {
                println!("Installable skills ({}):", data.skill_count);
                for skill in data.skills { println!("  {:<24} {}", skill.name, skill.summary); }
            }
            0
        }
        "inspect" => {
            let name = args.first().ok_or("inspect requires <name>")?;
            let data = repo.inspect(name)?;
            if opt.json { print_json(&data)?; }
            else {
                println!("{} ({})", data.display_name.as_deref().unwrap_or(&data.name), data.name);
                println!("tradition: {} | school: {} | era: {}",
                    data.tradition.as_deref().unwrap_or("—"),
                    data.school.as_deref().unwrap_or("—"),
                    data.era.as_deref().unwrap_or("—"));
                println!("installed: {} | sources: {}", data.installed, data.sources.len());
                for source in data.sources { println!("  - {source}"); }
            }
            0
        }
        "recommend" => {
            if args.is_empty() { return Err("recommend requires <query...>".into()); }
            let data = repo.recommend(&args.join(" "))?;
            if opt.json { print_json(&data)?; }
            else {
                println!("{} ({})", data.resolved_by, data.kind);
                if let Some(note) = data.note { println!("{note}"); }
                if let Some(command) = data.command { println!("{command}"); }
                for master in data.masters { println!("{} [{}]", master.command, master.matched.join(", ")); }
            }
            0
        }
        "search" => {
            let (name, query) = name_and_query(&opt, args)?;
            let hits = repo.search(name, &query, opt.limit)?;
            if opt.json { print_json(&hits)?; }
            else {
                for hit in hits {
                    println!("## {} — {}", hit.section, hit.file);
                    if !opt.brief { println!("{}", hit.preview); }
                }
            }
            0
        }
        "cite" => {
            let (name, query) = name_and_query(&opt, args)?;
            let hits = repo.citations(name, &query)?;
            if opt.json { print_json(&hits)?; }
            else { for hit in hits { println!("[{}] {}", hit.section, hit.citation); } }
            0
        }
        "install" | "update" | "uninstall" => {
            let options = InstallOptions { dry_run: opt.dry_run, force: opt.force };
            let requested: Vec<String> = args.to_vec();
            let data = match command {
                "install" => repo.install(&requested, options)?,
                "update" => {
                    if requested.len() != 1 || requested[0] != "--all" {
                        return Err("update accepts only --all".into());
                    }
                    repo.update_all(options)?
                }
                _ => repo.uninstall(&requested, options)?,
            };
            if opt.json { print_json(&data)?; }
            else {
                for action in data.actions {
                    println!("{} {} -> {}", action.action, action.name, action.destination);
                }
            }
            0
        }
        "review-teacher" | "build-teacher" => {
            let spec_path = opt.spec.as_deref().ok_or("--spec FILE is required")?;
            let spec: TeacherSpec = serde_json::from_slice(&std::fs::read(spec_path)?)?;
            if command == "review-teacher" {
                let inputs = repo.prepare_teacher(&spec)?;
                if opt.json { print_json(&inputs)?; }
                else {
                    println!("doctrine: {}", inputs.doctrine_sha256);
                    println!("voice: {}", inputs.voice_sha256);
                }
            } else {
                let output = opt.output.as_deref().ok_or("--output DIR is required")?;
                let data = repo.build_teacher(&spec, output, TeacherOptions {
                    dry_run: opt.dry_run, force: opt.force,
                })?;
                if opt.json { print_json(&data)?; }
                else { println!("{} -> {}", data.slug, data.directory); }
            }
            0
        }
        "doctor" => {
            let data = repo.doctor()?;
            let healthy = data.status == "ok";
            if opt.json { print_json(&data)?; }
            else {
                println!("status: {}", data.status);
                println!("installed: {}/{}", data.installed_catalog_skills, data.catalog_skills);
                for problem in data.problems { eprintln!("problem: {}", problem.message); }
                for warning in data.warnings { eprintln!("warning: {}", warning.message); }
            }
            if healthy { 0 } else { 1 }
        }
        "validate" => {
            let data = repo.doctor()?;
            if opt.json { print_json(&data)?; }
            else { println!("catalog/routing parsed; status: {}", data.status); }
            if data.status == "ok" { 0 } else { 1 }
        }
        _ => return Err(format!("unknown command: {command} (see --help)").into()),
    };
    Ok(status)
}
fn main() {
    let outcome: std::result::Result<i32, Box<dyn std::error::Error>> =
        parse().map_err(Into::into).and_then(execute);
    match outcome {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("master-skill: {err}");
            std::process::exit(2);
        }
    }
}

#[cfg(all(test, feature = "repository-fixtures"))]
mod tests {
    use super::*;
    #[test]
    fn find_checkout_above_crate() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(path.ancestors().any(|parent| parent.join("skill-catalog.json").is_file()));
    }
}
