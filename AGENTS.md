# wiina

Read [the product contract](docs/product-spec.md), [status](docs/status.md), and
[implementation plan](docs/implementation-plan.md) before changing behavior.
Baseline: official IINA v1.5.0, never silently upgrade it.

Use small Rust modules, explicit ownership and typed intent. Comments explain why,
upstream semantics, or unsafe invariants; avoid narrating obvious statements.
No existing application code was present at project acquisition.

Build on Windows 11 x64 with Rust 1.99.0 (edition 2024) and MSVC build tools:
`cargo build --workspace --locked`, `cargo run -p wiina --locked -- <media>`.
Verify `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked`. Native tests require the pinned DLL.
Never report an unrun GUI/HDR test as passed. Keep implementation and verification separate.
Do not package native binaries until licensing and corresponding-source obligations are closed.
