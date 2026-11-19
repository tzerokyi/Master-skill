# Rustification Handoff — Master-skill

> **Owner handoff / TODO backlog. Updated 2026-11-19.**
> **Status: PARTIAL. NOT 100% native Rust.**
>
> Scope: `https://github.com/tzerokyi/Master-skill`, `main` at `38883dd4220ca8b5c5993dd2073ca0429030e906` when this inventory was taken (merged PR #1).
> This document records *unfinished work*, not a claim of passing tests. User explicitly deferred validation; do not claim a green build or full language migration without evidence.

## Goal and operating contract

Turn the behavioral **core** into reusable, publishable, native Rust with a stable typed library API, useful headless CLI (`--json`, nonzero failures, dry-run/force where applicable), and a thin optional MCP adapter seam. Keep the existing native egui desktop by connecting it to the same Rust core. No mandatory Python/Node/npm runtime, webview, JS bridge, background service, or external LLM for local operations. Windows portability from the outset; no compiled executables, Cargo `target/`, or caches in git.

Keep source documents, skill prompts, personas, and license notices intact. Source content is data, **not code to translate**. Preserve behavioral parity, source provenance, crisis-first boundaries, independent reviews and "no fabricated citations." The canonical Rust code lives in `crates/master-skill-core/`; don't fork parallel implementations in the GUI or CLI.

The owner allowed merging unverified source to `main` and **postponed verification**. That does NOT waive safe-by-default behavior, reviews, or an accurate statement of missing functionality.

## Current inventory — checked against main

- **136 `.py` files** remain (including tests and copied/plugin tool implementations); **4 `.js` / `.mjs` files** remain. Counts are file inventory, *not* counts of independently required ports.
- Existing Rust: `crates/master-skill-core/src/*.rs` plus `desktop/src/*.rs` and their tests.
- `desktop/src/cli.rs` still runs `bin/cli.mjs`, `scripts/test-fidelity.py`, and `npm test` via subprocesses.
- `package.json`, `bin/cli.mjs`, `scripts/run-gates.py`, `hooks/session_start.py`, `site/build.py`, `plugin/scripts/*.py`, `plugin/skills/create-master/tools/*.py`, and their workflow packaging still provide legacy functionality.
- `crates/master-skill-core/Cargo.toml` is an isolated crate with `serde`, `serde_json`, `unicode-normalization`, `sha2`; the repository root is not yet the canonical Cargo workspace. The native core has no checked-in standalone lockfile at inventory time.

### Implemented in source (not certified equivalent, not all verified)

- `catalog.rs`: catalog/alias/path checks, list/inspect/doctor, installed-file drift checks.
- `routing.rs`: crisis-first routing; mode, persona, situation, topic and default fallbacks.
- `search.rs`: offline Markdown sections, basic normalization and citations; **not complete OpenCC equivalence**.
- `install.rs`: native install/update/uninstall with managed SHA-256 files, dry-run/force, staging/rollback and generated-persona preservation constraints.
- `builder.rs`: offline reviewed persona writer; source citation-contract derivation, two reviewer hash/PASS gates, shared safety clause and atomic writes. **Not a FoJin collector or full Python builder port.**
- `main.rs` / `lib.rs`: native CLI and typed reusable APIs.
- Rust fixture tests under `crates/master-skill-core/tests/`; self-hosted `.github/workflows/native-rust-core.yml` exists. **No completed successful build/test evidence is recorded here.**

## Pending tasks (check only when implemented AND independently verified)

### P0 — Make the new Rust foundation compile cleanly (first task on resumption)

- [ ] Run `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, `cargo test --all-targets --features repository-fixtures`, and a linked release binary on the owner's **trusted** `[self-hosted, linux]` GitHub runner. Use the existing persistent cache and 12 build jobs; do not request local manual Cargo testing as the default.
- [ ] Repair actual compiler/test/rustfmt diagnostics on the exact new main SHA. The Rust code was committed without a completed Rust-runner check; assume **unverified**, not correct-by-default. A green `Validate & Test` (Python legacy suite) does not prove the Rust crate builds.
- [ ] Check source-vs-legacy JSON golden fixtures for `list`, `inspect`, `recommend`, `search`, `cite`, `doctor` and all mutation command error codes. Add deterministic equivalence tests and injection/path-escape fixtures.
- [ ] Generate/review a crate lockfile, pin minimum supported Rust version, audit dependencies/licenses and establish reliable `cargo package` / `cargo publish --dry-run` gates. Validate **standalone crate** without the surrounding checkout; keep data files out of MIT crate packaging.
- [ ] Check CI isolation: owner's self-hosted runner should never execute untrusted fork PR code. Don't treat the dependency-review failure caused by disabled GitHub Dependency Graph as a Rust compiler failure.

### P1 — Finish and harden native catalog, router, offline retrieval, installer

- [ ] Compare `bin/cli.mjs` **every command and JSON field** with native CLI: `install`, `update`, `uninstall`, `list`, `inspect`, `recommend`, `doctor`, `--all`, `--force`, `--dry-run`; normalize public result schemas, option positioning, aliases, shell exit codes and Windows semantics.
- [ ] Prove install security under symlinks, stale/partial install records, hand-edited files, concurrent installs, denied permissions, power loss, failed rename/rollback, and legacy `.master-skill-install.json` format/version compatibility.
- [ ] Preserve **generated** `create-master/masters/` and registered aliases/links on update and uninstall. Match legacy preservation and `--force` policy; do not delete a generated persona silently.
- [ ] Bring `scripts/query.py`, `scripts/cite.py`, `scripts/_skill_io.py` and `scripts/_masterpaths.py` parity into `search.rs`/shared parser, including **OpenCC t2s**, Unicode/diacritics, `著/着`, Chinese n-gram ranking, YAML frontmatter folding and CBETA citation extraction. Handle the copies in `plugin/scripts/` from the *same Rust logic*.
- [ ] Compare routing match-order, ASCII word boundaries/inflections, tie-break behavior, fallback row order, Romanized expansions and crisis exclusions against `routing.json`/the old JS.
- [ ] Move `doctor` checks for missing runtime sources, broken registration links, generator dependencies and stale installs to native code.

### P2 — Port the **entire** FoJin and persona creation pipeline

Original canonical implementation paths: `tools/fojin_bridge.py`, `tools/sutra_collector.py`, `tools/master_builder.py`, `tools/skill_writer.py`, `tools/cross_reference.py`, `tools/rag_query.py`, `tools/verify_sources.py`, `tools/version_manager.py`, `tools/check_deps.py`. Copied generator files also exist under `plugin/skills/create-master/tools/`. **Don't maintain duplicated Python/Rust business logic long-term.**

- [ ] Typed FoJin REST client adapter: URL allowlist, TLS, streaming response-size caps, deadlines, bounded retries/backoff, no automatic credential logging, structured failure and offline fixture mode. Network only on explicit request. Maintain current provenance and third-party untrusted-data boundaries.
- [ ] Port entity/source/text collection, search scope, exact external source identifiers, text excerpts and source completeness checks; retain safeguards against fabricated or unverifiable CBETA/BDRC/etc. identifiers.
- [ ] Port full `master_builder` prompt assembly (all analysis/voice/doctrine stages, untrusted-data fencing, marker stripping, prompt template loading, sanitization and review input identity).
- [ ] Match `skill_writer` exactly: YAML quoting, persona slug/pinyin behavior, metadata, `teaching.md`/`voice.md`/`SKILL.md` generation, shared safety block, provenance, citation contract, version history, reviewer hashes and forbidden unreviewed changes. The current `builder.rs` is **only an offline writer slice**, not the whole builder.
- [ ] Implement local query/RAG, cross-reference, update/versioning and reference checks as testable pure core modules; expose separate CLI commands and structured methods. External model completion should remain an **optional explicitly configured provider adapter**, not an implicit mandatory API.
- [ ] Port/replace plugin copies using one Rust executable + shared core; don't ship Python in newly generated runtime bundles.

### P3 — Move **all** structural validations and fidelity grading off Python

- [ ] Port `scripts/validate.py`, `validate-citation-contract.py`, `validate-citation-references.py`, `validate-citation-templates.py`, `validate-cross-critique.py`, `validate-curriculum-sources.py`, `validate-fidelity.py`, `validate-fixture-terms.py`, `validate-lore-triggers-content.py`, `validate-persona-fidelity.py`, `validate-promptfoo-configs.py`, `validate-quote-attribution.py`, `validate-recommend-triggers.py`, `validate-routing.py`, `validate-section-references.py`, `validate-self-audit-sources.py`, `verify_citations.py` and the underlying helper modules.
- [ ] Port `scripts/test-fidelity.py`, `persona_prompt.py`, `select-fidelity-smoke.py`, `smoke-eval-sdk.py`, `_evaluation_identity.py`, `_fixture_identity.py`, `_citation_evidence.py`, `_skill_context.py`, `reaudit-report.py`, `regrade-report.py`, `verify-adjudication.py` and the grading/provider helpers. Compare **offline dry-run, frozen report regrade, paid/online graded run** as distinct contracts.
- [ ] Preserve actual evidence vs claims: existence/format of a citation is **not** proof of a verbatim quote or doctrinal truth. Verify citation identifiers, original excerpts, source attribution, crisis response, tool-call counterfeits, review decisions and fixture identities. No fabricated PASS, no live API calls during offline tests.
- [ ] Translate `scripts/run-gates.py` plus `check-gate-liveness.py`, `check-audit-ignores.py`, `check-directory-readiness.py`, `check-eval-sdk-surface.py`, `check-manifest-versions.py`, `check-pe-subsystem.py`, `check-release-fidelity.py` and other verification scripts into Rust test bins/integration gates.
- [ ] Reuse old `tests/`, `scripts/tests/`, `eval/reports/` as immutable behavior fixtures until parity is proven, then replace the Python test runner with native `cargo test`/CLI offline checks.
- [ ] Ensure optional API/provider evaluation runs only when explicitly enabled and secrets are present, and that missing tests do NOT appear green.

### P4 — Wire the existing **native Rust desktop** to the same native core

- [ ] In `desktop/Cargo.toml` depend on `master-skill-core` (path now, published crate later) and establish a workspace only if it does not break existing release/configuration behavior.
- [ ] Replace `desktop/src/cli.rs` interpreter spawning (`NODE`, Python, npm), `desktop/src/command.rs` shell bridge and `desktop/src/app.rs` command handlers with **direct Rust core calls**. Preserve async/cancellation/state/error semantics rather than blocking the GUI thread.
- [ ] Replace `run_fidelity_dry_run[_for]` and `run_full_validation` with native evaluation/validation APIs after P3. Maintain desktop trace/baseline contracts and tests.
- [ ] Keep eframe/egui native frontend, platform-gated filesystem/process adapters, Windows-friendly paths, consistent `--help`/`--json` headless CLI. Frontend is **not** the reusable business core.
- [ ] Verify existing Linux/Windows/macOS desktop build/release workflow separately from a core-only Cargo green check. Never add binary assets to Git.

### P5 — Remove legacy runtime, hooks, packaging, distribution

- [ ] Replace `bin/cli.mjs` and `tests/cli.test.mjs` with native CLI + equivalent Rust integration tests. Keep compatibility wrappers only if **explicitly** required for transition; do not treat Node wrappers as 100% native.
- [ ] Replace `hooks/session_start.py` and the shell/batch hook wrappers; make `.claude-plugin/`, `.cursor-plugin/`, `hooks/`, `.codex/`, `.opencode/` and `gemini-extension.json` invoke the native executable without Python/Node. Verify Windows command-line quoting and noninteractive behavior.
- [ ] Convert `scripts/build-plugin-bundle.py` and generator plugin packaging to reproducible Rust tooling. Check manifests, paths, checksums, permission boundaries and install/uninstall idempotence.
- [ ] Replace npm/pip validation and publishing CI (`package.json`, `requirements*.txt`, `.github/workflows/validate-and-test.yml`, `npm-publish.yml`, `clawhub-publish.yml`, `persona-fidelity.yml`, `security-scan.yml`, `release-desktop.yml`, etc.) with trusted Cargo/native pipeline **only after** equivalent workflows exist.
- [ ] The **website** is a separate optional scope: `site/build.py`, `site/assets/site.js`, `site/assets/theme.js`. If the request is *entire repository has no Python/JS*, port these too. If scope is *all executable application/skill core native Rust*, document a static-site exception explicitly instead of falsely calling the whole repo 100% Rust.
- [ ] Retire Python/Node source **only after** equivalent Rust functionality, fixtures and installer distribution are verified. Keep historical source/license data; don't mass-delete skill documents, corpus files or evidence reports.

### P6 — crates.io readiness / portability / delivery

- [ ] Stabilize public Rust types, errors, command schemas and versioning. Split `master-skill-core` (pure domain) from thin native `master-skill` CLI and optional FoJin/provider adapters if justified by dependency boundaries.
- [ ] Public examples, rustdoc, changelog, LICENSE and SPDX/data-corpus licensing review. MIT code **must not embed** CC BY-NC-SA persona materials or third-party excerpts as if MIT.
- [ ] Deterministic CLI smoke on trusted self-hosted runner with warm Cargo cache and 12 cores: help, list, doctor, inspect, recommend/crisis, search/cite, install/dry-run/force, build-teacher safety, validators, evaluation dry-run. Exact commit SHA/run URL/checks/exit codes required.
- [ ] Windows compile + actual runtime path tests; separate core, native GUI, API/provider integrations. `cargo publish --dry-run` and package contents/license audit before actual publish. No release/publish until owner requests it.
- [ ] Update `README.md`, `README_EN.md`, `docs/architecture*.md`, contributor docs and all entrypoints to describe the actual native runtime and explicitly retire Node/Python setup instructions only once true.

## Next-worker entry point

1. Read this handoff, `crates/master-skill-core/README.md`, root `LICENSE` and owner `native-rust-delivery`/`rustify` rules in `tzerokyi/SKILLS`. Treat **Rustryx as design reference, not a target to edit**. This repository has an existing native desktop; a new frontend is not requested.
2. **Start with P0**: take a trustworthy diagnostic snapshot of `main`, resolve any Cargo errors, format and fix tests; then work through P1–P6, committing cohesive Rust modules and parity fixtures as you go. This handoff itself is not proof of compile success.
3. After each batch, mark a checkbox only when source is in `main` **and** its acceptance gate has evidence; include precise SHA, CI run URL and remaining gaps. Avoid saying "100%" based on files ending in `.rs`.
4. Final definition of **core 100% native Rust**: every core feature, generator, validator, evaluator, CLI and desktop invocation runs through Rust APIs/CLI without executing Node/Python; optional external providers remain explicit; cross-platform/native binary builds and parity checks have passed. If static site stays JS, say **core 100%, repo not 100%**.
