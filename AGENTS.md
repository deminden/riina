# riina

Read [the product contract](docs/product-spec.md), [status](docs/status.md), and
[implementation plan](docs/implementation-plan.md) before changing behavior.
Baseline: official IINA v1.5.0, never silently upgrade it.

Use small Rust modules, explicit ownership and typed intent. Comments explain why,
upstream semantics, or unsafe invariants; avoid narrating obvious statements.
No existing application code was present at project acquisition.
Original riina contributions are MIT. Develop independently from observable
behavior and documented platform APIs; do not translate IINA implementation code
or copy its protected assets. Read docs/licensing.md before changing license terms.

Build on Windows 11 x64 with Rust 1.99.0 (edition 2024) and MSVC build tools:
`cargo build --workspace --locked`, `cargo run -p riina --locked -- <media>`.
Verify `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked`. Native tests require the pinned DLL.
Never report an unrun GUI/HDR test as passed. Keep implementation and verification separate.
Do not package native binaries until licensing and corresponding-source obligations are closed.

Current product direction: riina is a Rust IINA reimplementation. Windows 11 x64
is the current target; macOS and Linux ports are near-term work, not supported. Pixel-perfect
interface parity and full behavioral/feature parity with IINA v1.5.0 are primary
acceptance goals. Preserve its elegant interface; do not substitute a redesign.
The requested extension is opt-in volume boost up to 200%; the default volume
range remains 0–100%. Default behavior and appearance target exact IINA parity. No Vim
controls are requested. Historical wiina evidence remains under its original name.
