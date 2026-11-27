# Source access for binary releases

The GitHub-generated source archive for a Riina binary release is the exact
application source at that release's tag. The Windows package's
`SOURCE-COMMIT.txt` records the full Git commit and must match that tag.

Rust dependencies are pinned by `Cargo.lock`; Cargo retrieves the exact crate
archives from crates.io and verifies each recorded checksum. They are not copied
into an additional vendor directory in the release.

Native engine dependencies are listed in the package's `SOURCE-MANIFEST.tsv`.
It records each public source repository and immutable revision, plus SHA-256
hashes for the upstream files, the build script and the patches. The
release-tagged `tools/mpv/build-windows.sh` applies the Matroska field-order
patch to the indexed mpv source before compiling. To rebuild the engine, use
the recorded build image digest and source revisions with that script from
the tagged Riina source checkout.

The release page links to the exact tag source next to the binary download.
`SOURCE-MANIFEST.tsv`, the lockfile and the checked-in build recipe identify the
complete source inputs without duplicating their files in the binary or a second
release archive.
The binary package continues to include all required third-party license and
attribution notices.
