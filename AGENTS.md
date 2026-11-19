# Master-skill repository agent handoff

For **any Rustification / core / CLI / API / desktop / packaging** work in this repository:

1. Read [RUSTIFICATION_HANDOFF.md](RUSTIFICATION_HANDOFF.md) before changing code. It owns the outstanding migration backlog and the definition of “100% native Rust”.
2. Reuse `crates/master-skill-core/` as the single domain implementation; don't create a second Python/Node-backed adapter and call it native.
3. Honor original safety, citation provenance, independent review, license and user-owned generated-data preservation contracts. No false passing-test claims.
4. Keep desktop UI native Rust; do not add webviews, HTML/CSS/JS app layers. Run standard CI on the verified trusted self-hosted runner when verification resumes; never commit binaries or `target/`.
5. Reflect completed **verified** migration tasks by updating the checkbox in the handoff. Rust source committed but not verified is not automatically done.

This is a repository-specific pointer, not permission to rewrite the separate Rustryx reference repository.
