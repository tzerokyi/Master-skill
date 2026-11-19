# master-skill-core

Independent native Rust library and CLI over a Master-skill checkout.

## Implemented in phase 1

- 21-skill inventory, validated catalog aliases and paths
- Typed inspect and source metadata
- Deterministic routing (crisis first, then modes, persona scoring, situational and topic fallbacks)
- Offline sources/references section search and citation extraction
- Read-only doctor and CLI JSON output

This crate executes **no Node, Python, browser or network process**. It can be
consumed from other Rust crates, native desktop frontends or a future opt-in
MCP adapter. Core APIs return data, not CLI text or exit codes.

## Usage

From the repository root, run:

    cargo test --manifest-path crates/master-skill-core/Cargo.toml
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

- transactional install/update/uninstall, including SHA-256 managed-file
  records, user edits, generated persona preservation and force semantics
- FoJin HTTP collector/generator, Python builder and cross-reference tools
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
