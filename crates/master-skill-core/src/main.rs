//! Small native CLI adapter over master_skill_core::Repository.
//! No shell subprocess, interpreter, browser, listener or desktop framework.
use std::env;
use std::path::PathBuf;
use master_skill_core::Repository;
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

OPTIONS:
  --root PATH                  Checkout root (or MASTER_SKILL_ROOT)
  --json                       Print structured JSON on stdout
  --limit N                    Max search results (default 20, max 100)
  --master NAME                Named alternative to positional name for search/cite
  --q QUERY, --text TEXT       Named alternative to positional query for search/cite
  --brief                      Show search titles/paths only (human output)
  --help, -h                   Show help
  --version, -V                Show crate version

Read-only phase 1: install/update/uninstall, Python generator, and the existing
Node desktop bridge have NOT yet been ported. See README.md for migration status.
"#;

#[derive(Default)]
struct Options {
    root: Option<PathBuf>,
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
    let repo = Repository::open(root)?;
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
