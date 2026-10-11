# riina

Read [the product contract](docs/product-spec.md), [status](docs/status.md), and
[implementation plan](docs/implementation-plan.md) before changing behavior.
Baseline: official IINA v1.5.0, never silently upgrade it.

When in doubt, follow the pinned IINA v1.5.0 architectural decisions: inspect its
ownership, lifecycle, and subsystem boundaries, and mirror them wherever the target
platform permits. Use a documented platform equivalent when it does not. This
guides independent implementations; do not translate or copy IINA source code.

Use small Rust modules, explicit ownership and typed intent. Comments explain why,
upstream semantics, or unsafe invariants; avoid narrating obvious statements.
No existing application code was present at project acquisition.
Original riina contributions are MIT. Develop independently from observable
behavior and documented platform APIs; do not translate IINA implementation code
or copy its protected assets. Read docs/licensing.md before changing license terms.

Keep incremental development and commits local. Publish public GitHub updates
only at substantial verified milestones or when the user explicitly requests it.
Use semantic versioning: advance the patch digit for noticeable milestones and
the minor digit for major milestones. The current development version is 0.2.15.

Build on Windows 11 x64 with Rust 1.99.0 (edition 2024) and MSVC build tools:
`cargo build --workspace --locked`, `cargo run -p riina --locked -- <media>`.
Verify `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked`. Native tests require the pinned DLL.
Never report an unrun GUI/HDR test as passed. Keep implementation and verification separate.
After staging asset changes, run `python scripts/check-assets.py --staged` before
publishing. Provenance hashes must match Git's canonical bytes, not local CRLF files.
The repository's license choice is settled: original Riina source is MIT and
the combined application package is GPLv3. Before publishing a native binary,
verify the exact package, notices, corresponding source, and playback checks.
Never spend development time waiting on a long-running local build, test suite, or CI
job. Launch long checks without blocking, then immediately continue with the next
independent implementation, interface-parity, audit, or release task while they run.
Keep a short list of the checks in flight and the code they cover. Check a result only
when it can change the next action, when dependent work is ready, or before reporting
verification; do not poll repeatedly or let a running check pause development. If a
check fails, inspect the failure, fix it, and resume independent work while any rerun
is in flight. Do not start duplicate builds or CI runs just to get another status update.
Monitor free space on C: during large or repeated builds. When space is running low,
remove stale project-local Cargo build artifacts with `cargo clean` (or a specific
project target directory), after confirming no active check is using that directory.
For large Windows builds with a growing incremental cache, set `CARGO_INCREMENTAL=0`
for the check or build so generated compiler state does not consume the drive.
Do not routinely erase the shared Cargo registry or Git download caches: they are
costly to repopulate and are not the project's generated build output.

Current product direction: riina is a Rust IINA reimplementation. Windows 11 x64
remains the current supported target. The user has made full macOS-version parity
an active goal now, alongside continued portable architecture work; do not claim
macOS support until the full app, playback, rendering and lifecycle are built and
verified there. Pixel-perfect interface parity and full behavioral/feature parity
with IINA v1.5.0 are acceptance goals. Preserve its elegant interface; do not
substitute a redesign.
The requested extension is opt-in volume boost up to 200%; the default volume
range remains 0–100%. Default behavior and appearance target exact IINA parity. No Vim
controls are requested. Historical wiina evidence remains under its original name.

Current user priority: implement the fully missing IINA behaviors first, then
make the interface visually indistinguishable from the pinned macOS version step
by step. Use actual captured Windows and pinned macOS reference surfaces for the
visual phase. Keep the complete interface scope; do not treat a few corrected
controls or source dimensions as proof of full pixel parity.

Project maturity: Riina is an established, functioning media-player application
beyond its initial feasibility milestones, with real Windows playback, extensive
player/settings workflows and regression coverage. Full IINA behavior and visual
parity remain active acceptance goals; describe current evidence and gaps precisely.

Develop macOS/Linux application ports alongside Windows work now. Shared feature
changes must preserve portable boundaries. Require full application builds and
playback/lifecycle evidence before declaring either port supported; library-only
CI is insufficient.
