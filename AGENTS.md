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

Use the configured GitHub account identity for project commits, releases, and
other published actions. Project responsibility belongs to the publishing
account; do not present any one person as the sole developer. Do not name or
attribute project work to coding agents, assistants, automation tools, or models
in repository files, commit messages, trailers, release notes, or contributor
lists. Let the GitHub contributor record reflect the people who contribute as
the project evolves. Do not rewrite published history unless the user explicitly
requests it.

## Contributors

The README links to GitHub's contributor graph, which is the maintained record
of project contributors. Keep contributor documentation inclusive and account-
based; never describe a single person as the sole developer or name automated
coding tools as contributors.

Keep incremental development and commits local. Publish public GitHub updates
only at substantial verified milestones or when the user explicitly requests it.
Use semantic versioning: advance the patch digit for noticeable milestones and
the minor digit for major milestones. The current development version is 0.3.6.

Build on Windows 11 x64 with Rust 1.99.0 (edition 2024) and MSVC build tools:
`cargo build --workspace --locked`, `cargo run -p riina --locked -- <media>`.
Verify `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked`. Native tests require the pinned DLL.
Never report an unrun GUI/HDR test as passed. Keep implementation and verification separate.
After each GUI feature test, close the Riina instance used and confirm no Riina
window or process remains before rebuilding or testing another version. This avoids
stale builds remaining open and slowing later development. If normal shutdown fails,
end only the exact Riina process started for that test.
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
keep Cargo incremental compilation enabled. Measure the project-local
`target\debug` directory and clean its generated build artifacts only when it grows
above 50 GB, after confirming no active check or application is using that directory.
Prefer cleaning the specific oversized project cache or package outputs; preserve
logs, release evidence, and other non-generated project artifacts.
Do not routinely erase the shared Cargo registry or Git download caches: they are
costly to repopulate and are not the project's generated build output.
Periodically review the project-local `target` directory and clean stale Cargo
build artifacts when they are no longer useful and the size threshold is exceeded.
For release workflows, reuse a successful native-engine artifact from an earlier
run when the workflow supports it, to avoid repeating multi-hour native builds.
Verify the artifact is successful and matches the exact native source revisions,
patches, build scripts, target architecture, and package provenance for the
candidate; never reuse an artifact with a mismatched or unverifiable build
identity. Do not duplicate full native or vendored Rust source trees in release
assets. Keep source access clear through the immutable Riina release tag,
Cargo.lock, pinned native repository/revision manifest, public upstream sources,
and build instructions alongside the binary.

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
