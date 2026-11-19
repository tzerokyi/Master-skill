# master-skill-core

Independent native Rust library and CLI over a Master-skill checkout.

## Implemented in phase 1

- 21-skill inventory, validated catalog aliases and paths
- Typed inspect and source metadata
- Deterministic routing (crisis first, then modes, persona scoring, situational and topic fallbacks)
- Offline sources/references section search and citation extraction
- Read-only doctor and CLI JSON output
- Native transactional install/update/uninstall, dry-run and local-edit guards
- Native reviewed offline teacher writer, source contract and independent approval hashes

This crate executes **no Node, Python, browser or network process**. It can be
consumed from other Rust crates, native desktop frontends or a future opt-in
MCP adapter. Core APIs return data, not CLI text or exit codes.

## Usage

From the repository root, run:

    cargo test --manifest-path crates/master-skill-core/Cargo.toml
    cargo test --manifest-path crates/master-skill-core/Cargo.toml --features repository-fixtures
    cargo run --manifest-path crates/master-skill-core/Cargo.toml -- --help
    cargo run --manifest-path crates/master-skill-core/Cargo.toml -- --root . list --json
    cargo run --manifest-path crates/master-skill-core/Cargo.toml -- --root . inspect huineng --json
    cargo run --manifest-path crates/master-skill-core/Cargo.toml -- --root . recommend "禅宗从哪开始学" --json
    cargo run --manifest-path crates/master-skill-core/Cargo.toml -- --root . search huineng "无念" --json
    cargo run --manifest-path crates/master-skill-core/Cargo.toml -- --root . cite huineng "无念" --json
    cargo run --manifest-path crates/master-skill-core/Cargo.toml -- --root . doctor --json

The CLI accepts a checkout via --root, MASTER_SKILL_ROOT, or an ancestor
directory containing skill-catalog.json and routing.json. With installed
crates.io packages, provide a separately licensed checkout as data.

Library usage:

    use master_skill_core::{Repository, Result};
    fn example() -> Result<()> {
        let repo = Repository::open("/path/to/Master-skill")?;
        let inventory = repo.list()?;
        let master = repo.inspect("huineng")?;
        let recommendation = repo.recommend("禅宗从哪开始学")?;
        let hits = repo.search("huineng", "无念", 20)?;
        let citations = repo.citations("huineng", "无念")?;
        let health = repo.doctor()?;
        println!("{} {} {} {} {} {}", inventory.count, master.name,
            recommendation.kind, hits.len(), citations.len(), health.status);
        Ok(())
    }

The CLI prints JSON/human results to stdout; diagnostics to stderr. Exit
status 0 is success, 1 is doctor/validation problems, and 2 is a command
error. The crisis route returns no persona and must not be converted into
a doctrinal recommendation. Local integrators should present appropriate
country-specific help information.

## Migration gaps

This is a working scope **proposal awaiting Cargo verification**, not full
runtime parity. The existing Node, Python, and desktop code is preserved.
Still required for complete Rustification:

- FoJin HTTP collector/LLM orchestration and Python cross-reference tools
- all audit/fidelity/quote validation and release gates in native Rust
- native desktop replacing the currently Node/Python-backed CliClient
- full OpenCC t2s equivalence in offline query search (current native search
  has NFKD/diacritics and 著/着 folding, but not full Chinese t2s conversion)
- lockfile, compatibility audit and verified crates.io publication

## License boundary

Rust code is MIT according to the upstream LICENSE code scope. Persona
material under prebuilt/ is CC BY-NC-SA 4.0, prompts under prompts/ are
CC BY 4.0, and third-party excerpts retain source rights. The crate does
not embed, relicense or redistribute any of that material.


Additional native commands:

    cargo run --manifest-path crates/master-skill-core/Cargo.toml -- --root . install huineng --dry-run --json
    cargo run --manifest-path crates/master-skill-core/Cargo.toml -- --root . install --all --dry-run --json
    cargo run --manifest-path crates/master-skill-core/Cargo.toml -- --root . update --all --dry-run --json
    cargo run --manifest-path crates/master-skill-core/Cargo.toml -- --root . uninstall huineng --dry-run --json

Mutation APIs: Repository::install(&[String], InstallOptions),
Repository::update_all(InstallOptions), Repository::uninstall(&[String],
InstallOptions). Installations use staging directories and rename rollback,
sha256-managed file manifests, and an explicit force flag to replace user edits.
Set Repository::with_install_root(path) to test without touching HOME.
The generator's masters/ data is preserved on update, but symbolic-link-based
user data needs manual backup/review; the native implementation refuses to
follow such links.


Offline teacher writer: supply a JSON TeacherSpec, then run
review-teacher --spec FILE --json to obtain doctrine/voice review hashes.
Two independent PASS approvals with matching input_sha256 values are required
before build-teacher can write a persona. A dry run only calculates inputs.

    cargo run --manifest-path crates/master-skill-core/Cargo.toml -- --root . review-teacher --spec teacher.json --json
    cargo run --manifest-path crates/master-skill-core/Cargo.toml -- --root . build-teacher --spec teacher.json --output ./masters --dry-run --json

Live FoJin retrieval and LLM analysis remain unported. This writer does not
automatically hallucinate teaching content or waive independent reviews.
