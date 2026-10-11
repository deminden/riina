#!/usr/bin/env bash
set -euo pipefail

# Build from a fixed recipe revision, then preserve every fetched source checkout
# used by the binary so the release source archive matches the native engine.
readonly BUILDER_REVISION='b2856ee6deed6957380b32465f359e67f67fafd8'
readonly OPENSSL_REVISION='d8bf6cdd4849925c30e4f1911c7acb49cb34b702'
readonly REPO_ROOT="$PWD"
readonly WORK_ROOT="${RUNNER_TEMP:-$PWD}/riina-native-build"
readonly BUILDER="$WORK_ROOT/mpv-winbuild-cmake"
readonly SOURCE_ARCHIVE_ROOT="$PWD/native-source"

print_build_failure() {
  local status="$1"
  local command="$2"
  trap - ERR
  local diagnostics="$REPO_ROOT/native-build-failure.log"
  {
    echo "Native engine build failed with exit code $status while running: $command"
    echo "Recent dependency build logs:"
    if [[ -d "$BUILDER/build_x86_64/packages" ]]; then
      find "$BUILDER/build_x86_64/packages" -type f \
        \( -name '*-configure-err.log' -o -name '*-configure-out.log' \
           -o -name '*-build-err.log' -o -name '*-build-out.log' \) -size +0c \
        -printf '%T@ %p\0' \
        | sort -z -nr | head -z -n 24 | cut -z -d' ' -f2- \
        | while IFS= read -r -d '' log; do
            echo "--- $log ---"
            tail -n 100 "$log"
          done
    fi
  } > "$diagnostics" 2>&1 || true
  cat "$diagnostics" >&2
  exit "$status"
}
trap 'print_build_failure "$?" "$BASH_COMMAND"' ERR

# ExternalProject applies dependency patches in multiple Ninja phases. Git am
# needs an identity each time it creates the patch commit, including during mpv.
run_ninja() {
  GIT_COMMITTER_NAME='Riina native build' \
    GIT_COMMITTER_EMAIL='native-build@riina.invalid' \
    ninja "$@"
}

rm -f "$REPO_ROOT/native-build-failure.log"
rm -rf "$WORK_ROOT" "$SOURCE_ARCHIVE_ROOT"
mkdir -p "$WORK_ROOT"
git clone --filter=blob:none https://github.com/shinchiro/mpv-winbuild-cmake.git "$BUILDER"
git -C "$BUILDER" checkout --detach "$BUILDER_REVISION"
git -C "$BUILDER" apply "$PWD/tools/mpv/remove-x264.patch"

cd "$BUILDER"
cmake \
  -DTARGET_ARCH=x86_64-w64-mingw32 \
  -DCOMPILER_TOOLCHAIN=clang \
  -DCMAKE_INSTALL_PREFIX="$BUILDER/clang_root" \
  -DMINGW_INSTALL_PREFIX="$BUILDER/build_x86_64/x86_64-w64-mingw32" \
  -DSINGLE_SOURCE_LOCATION="$BUILDER/src_packages" \
  -DRUSTUP_LOCATION="$BUILDER/clang_root/install_rustup" \
  -DENABLE_CCACHE=ON \
  -DCLANG_PACKAGES_LTO=ON \
  -G Ninja --fresh -B build_x86_64 -S .
run_ninja -C build_x86_64 download
run_ninja -C build_x86_64 update

# The mpv target does not bootstrap the selected cross-toolchain. The pinned
# builder documents these as explicit build targets; in particular, skipping
# llvm leaves the clang wrapper pointing at a nonexistent clang executable.
run_ninja -C build_x86_64 llvm
run_ninja -C build_x86_64 rustup
run_ninja -C build_x86_64 llvm-clang

# The GPLv3 package depends on OpenSSL 3's Apache-2.0 terms. Verify both the
# source revision pinned by the builder recipe and the license shipped with it.
openssl_source="$BUILDER/src_packages/openssl"
test "$(git -C "$openssl_source" rev-parse HEAD)" = "$OPENSSL_REVISION"
grep -q 'Apache License' "$openssl_source/LICENSE.txt"

# x264 is not part of the release engine or its corresponding source set.
rm -rf "$BUILDER/src_packages/x264"

if [[ -n "${RIINA_NATIVE_SOURCE_ROOT:-}" ]]; then
  # Rebuilds can restore the captured repositories after CMake's fetch phase,
  # then check every locked revision and file before compiling the engine.
  rm -rf "$BUILDER/src_packages"
  mkdir -p "$BUILDER/src_packages"
  cp -a "$RIINA_NATIVE_SOURCE_ROOT/." "$BUILDER/src_packages/"
  while IFS=$'\t' read -r record first second third; do
    case "$record" in
      source_git)
        test "$(git -C "$BUILDER/src_packages/$first" rev-parse HEAD)" = "$third"
        ;;
      source_sha256)
        test "$(sha256sum "$BUILDER/src_packages/$first" | cut -d' ' -f1)" = "$second"
        ;;
    esac
  done < "$RIINA_NATIVE_SOURCE_ROOT/../SOURCE-MANIFEST.tsv"
fi

# The manifest is made after upstream's update step and before the build. This
# records the precise source states that will be archived with the executable.
mkdir -p "$SOURCE_ARCHIVE_ROOT/buildscripts" "$SOURCE_ARCHIVE_ROOT/native-sources"
git -C "$BUILDER" archive "$BUILDER_REVISION" | tar -xf - -C "$SOURCE_ARCHIVE_ROOT/buildscripts/"
cp -a "$REPO_ROOT/tools/mpv/build-windows.sh" "$SOURCE_ARCHIVE_ROOT/"
cp -a "$REPO_ROOT/tools/mpv/remove-x264.patch" "$SOURCE_ARCHIVE_ROOT/"
{
  printf 'builder_revision\t%s\n' "$BUILDER_REVISION"
  printf 'x264_policy\tFFmpeg libx264 encoder disabled; built-in H.264 decoder retained\n'
  printf 'builder_patch_sha256\t%s\n' "$(sha256sum "$REPO_ROOT/tools/mpv/remove-x264.patch" | cut -d' ' -f1)"
  printf 'container_image\t%s\n' "${RIINA_BUILD_IMAGE:-ghcr.io/shinchiro/archlinux:latest}"
  while IFS= read -r -d '' gitdir; do
    repo="${gitdir%/.git}"
    rel="${repo#"$BUILDER/"}"
    printf 'source_git\t%s\t%s\t%s\n' "$rel" "$(git -C "$repo" remote get-url origin 2>/dev/null || true)" "$(git -C "$repo" rev-parse HEAD)"
  done < <(find "$BUILDER/src_packages" -mindepth 2 -type d -name .git -print0 | sort -z)
  while IFS= read -r -d '' source; do
    printf 'source_sha256\t%s\t%s\n' "${source#"$BUILDER/src_packages/"}" "$(sha256sum "$source" | cut -d' ' -f1)"
  done < <(find "$BUILDER/src_packages" -type f ! -path '*/.git/*' -print0 | sort -z)
} > "$SOURCE_ARCHIVE_ROOT/SOURCE-MANIFEST.tsv"
cp -a "$BUILDER/src_packages/." "$SOURCE_ARCHIVE_ROOT/native-sources/"

# Build mpv first; its separate packaging target only adds the Windows runtime
# files to a package directory and does not depend on or compile the engine.
run_ninja -C build_x86_64 mpv
run_ninja -C build_x86_64 mpv-packaging
runtime_dir="$(find "$BUILDER/build_x86_64" -mindepth 1 -maxdepth 1 -type d -name 'mpv-x86_64-*' ! -name '*debug*' -print -quit)"
test -n "$runtime_dir"
test -s "$runtime_dir/libmpv-2.dll"
mkdir -p "$GITHUB_WORKSPACE/native-runtime"
cp -a "$runtime_dir/." "$GITHUB_WORKSPACE/native-runtime/"
cp "$SOURCE_ARCHIVE_ROOT/SOURCE-MANIFEST.tsv" "$GITHUB_WORKSPACE/native-runtime/"
mkdir -p "$GITHUB_WORKSPACE/native-runtime/THIRD_PARTY_LICENSES"
while IFS= read -r -d '' license_file; do
  relative="${license_file#"$BUILDER/src_packages/"}"
  destination="$GITHUB_WORKSPACE/native-runtime/THIRD_PARTY_LICENSES/$relative"
  mkdir -p "$(dirname "$destination")"
  cp "$license_file" "$destination"
done < <(find "$BUILDER/src_packages" -type f \
  \( -iname 'LICENSE*' -o -iname 'LICENCE*' -o -iname 'COPYING*' -o -iname 'NOTICE*' \) \
  ! -path '*/.git/*' -print0 | sort -z)
{
  printf 'path\tsha256\n'
  while IFS= read -r -d '' runtime_file; do
    printf '%s\t%s\n' "${runtime_file#"$GITHUB_WORKSPACE/native-runtime/"}" "$(sha256sum "$runtime_file" | cut -d' ' -f1)"
  done < <(find "$GITHUB_WORKSPACE/native-runtime" -type f ! -name 'RUNTIME-MANIFEST.tsv' -print0 | sort -z)
} > "$GITHUB_WORKSPACE/native-runtime/RUNTIME-MANIFEST.tsv"

tar --zstd -cf "$GITHUB_WORKSPACE/riina-native-source.tar.zst" -C "$(dirname "$SOURCE_ARCHIVE_ROOT")" "$(basename "$SOURCE_ARCHIVE_ROOT")"
sha256sum "$GITHUB_WORKSPACE/riina-native-source.tar.zst" > "$GITHUB_WORKSPACE/riina-native-source.tar.zst.sha256"
