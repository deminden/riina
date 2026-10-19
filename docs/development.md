# Development

Run commands from the repository root. Read [AGENTS.md](../AGENTS.md) before
changing behavior; it contains the working, verification and publishing rules.

## Windows setup

Requirements: Windows 11 x64, Rust 1.99.0 (edition 2024), MSVC C++ tools,
Windows SDK and 7-Zip. Versions are pinned in `rust-toolchain.toml`,
`Cargo.toml` and `Cargo.lock`.

```powershell
./scripts/fetch-reference.ps1
./scripts/fetch-mpv.ps1
cargo build --workspace --locked
cargo run -p riina --locked -- "C:\media\movie.mkv"
```

The reference fetch verifies IINA v1.5.0 against
[`reference/upstream-lock.json`](../reference/upstream-lock.json).
The mpv fetch supplies a pinned **development** DLL under `tools/mpv/`;
release packages use the separately identified release engine.

If this checkout uses workspace-local Rust/MSVC tools, use the development
wrapper. Pass an explicit argument array so PowerShell preserves `--`:

```powershell
./scripts/dev.ps1 -CargoArguments @('build', '--workspace', '--locked')
./scripts/dev.ps1 -CargoArguments @('run', '-p', 'riina', '--locked', '--', 'C:\media\movie.mkv')
```

Windows defaults to native D3D11 video. Add `--shared-gl` after `--` to inspect
the shared SDR renderer; `--native-video` explicitly selects the default route.
For macOS/Linux setup, see [port development](port-development.md).

## Repository map

| Path | Responsibility |
|---|---|
| `crates/core/` | Portable commands, state, preferences, catalogs and policies |
| `crates/mpv/` | libmpv FFI, worker, observations and rendering integration |
| `crates/app/src/` | Application orchestration and UI adapters |
| `crates/app/src/platform/` | OS services, native windows and presentation |
| `crates/app/ui/` | Slint components and design tokens |
| `scripts/` | Dependency setup, checks, packaging and capture tools |
| `tests/fixtures/` | Original synthetic media and provenance |
| `tests/visual/` | Capture manifests and presentation evidence |
| `reference/` | Pinned source locks and ignored local reference material |
| `tools/`, `target/` | Local dependencies and generated build output |

## Checks

[Verification](verification.md#running-checks) contains the commands, native
prerequisites, GUI procedure and the latest scoped results. Keep implementation
and verification states separate in the requirement inventories.

Quick Settings widget regressions in `crates/app/tests/quick_settings_input.rs`
run with the workspace tests. They exercise the generated player controls through
Slint's headless test backend, including pointer/key input and focus changes.
Debug builds retain the Slint lookup metadata those tests need. Release-profile
widget tests require `SLINT_EMIT_DEBUG_INFO=1`; normal distribution builds omit
that metadata. Native window/input and physical-display checks remain separate.

The read-only installed-font diagnostic remains available separately from the
player. It reports counts and discovery timing without listing private font paths:

```powershell
cargo run -p riina --bin font-catalog-probe --locked
```

## GUI sessions

Use an isolated profile and a single short fixture. Confirm the filename,
playback state and selected panel before capture. After each test, close that
Riina instance and confirm it exited before rebuilding or testing another build.
If necessary, terminate only the exact process started for that test.

For flashing, black patches, clipping or composition reports, capture the full
virtual desktop with `scripts/capture-windows-desktop.ps1`. Check its dimensions
and non-black probes, then inspect the full image. Compare before launch and after
exit for a stationary artifact. Window-only captures can supplement control
inspection. Keep unrelated private desktop content out of published evidence.

## Build storage

Keep incremental compilation enabled. Review `target/debug` during repeated
builds, but clean generated artifacts only above **50 GB**, after confirming no
build, check or application is using them. Prefer the specific stale package or
cache. Preserve logs, release evidence and reference files; do not routinely
erase shared Cargo downloads. Keep references outside `target/`.

## Release candidates

1. Choose the exact source revision and version; use the configured GitHub account.
2. Reuse a successful native artifact only if source revisions, patches, scripts,
   architecture and provenance match the candidate's build identity.
   If its CI artifact has expired, the release workflow's `native_release_tag`
   input can restore the engine from an existing published package. It verifies
   the archive checksum, tag/source identity, unchanged `tools/mpv` tree, runtime
   inventory and native notices. It does not reuse the previous application.
3. Run portable packaging tests, assemble the archive, and verify runtime contents,
   notices, hashes and source/build manifests.
4. Extract the archive and run the actual executable through
   `scripts/smoke-windows-package.ps1`. Keep native engine checks as well.
5. Record physical-display, audible-output and HDR checks separately. Publish only
   the verified candidate when a public milestone or explicit request authorizes it.

Use the release workflow's manual dispatch for a candidate. Set
`build_release_candidate=true` and select one native reuse source. Leave
`publish_release=false` when the archive needs a desktop GUI check first; publish
those exact verified assets afterward. `publish_release=true` publishes after the
same run's package checks pass. Creating the version tag does not start another
binary build.

### Retest a saved package without rebuilding

If compilation and native checks succeeded but the GUI environment failed, use
**Verify saved Windows package** with the original `candidate_run_id`. It requires
the original release workflow's build/lint/native gates to have passed, checks the
artifact's repository and source identity, downloads the saved ZIP, and verifies
its checksum, runtime, notices and exact corresponding source before running it.
Successful verification uploads the original ZIP and checksums plus a fresh
`RELEASE-VERIFICATION.json`; it does not compile or publish anything.

Hosted Windows runners may expose only the system OpenGL 1.1 implementation.
Slint's FemtoVG interface needs a newer OpenGL context even though mpv presents
video with D3D11. Both release workflows stage checksum-pinned Mesa 26.0.3 x64
`opengl32.dll` and `libgallium_wgl.dll` beside the disposable test executable and
select `llvmpipe` for that smoke step. The release ZIP is unchanged. This is a
software-interface test, not physical GPU, compositor or HDR certification.
Normal desktop verification uses the original extracted package and its driver.

The test-only driver comes from the
[Mesa Windows distribution](https://github.com/pal1000/mesa-dist-win/releases/tag/26.0.3).
Its archive hash is pinned in `scripts/prepare-windows-ci-graphics.py`; no system
driver installation or third-party deployment script is used.

The source and package license scopes are settled; see [licensing](licensing.md).
Use the immutable application tag, Cargo lockfile, native manifest and public
upstream sources for [source access](SOURCE-ACCESS.md). Do not duplicate native
or vendored Rust source trees in release assets.
