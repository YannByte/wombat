#!/usr/bin/env bash
set -euo pipefail
: "${ImageVersion:?A hosted runner image version is required}"
# Keep source hashes out of the dependency key, but bind workspace reuse to the
# actual compiler, image and caller settings. Hash values without exposing them.
compiler_version=$("${RUSTC:-rustc}" -vV)
compiler_key=$({
  printf '%s\0' "$compiler_version"
  printf '%s\0' "$ImageVersion" "$(pwd -P)" "${HOME:-}" "${CARGO_HOME:-}" \
    "${CARGO_INCREMENTAL:-}" "${RUSTFLAGS:-}" \
    "${RUSTC:-}" "${RUSTC_WRAPPER:-}" "${RUSTC_WORKSPACE_WRAPPER:-}" \
    "${RUSTDOCFLAGS:-}" "${RUSTUP_TOOLCHAIN:-}" \
    "${CC:-}" "${CFLAGS:-}" "${CXX:-}" "${CXXFLAGS:-}" "${MACOSX_DEPLOYMENT_TARGET:-}"
  for compiler_setting in ${!CARGO_@}; do
    case "$compiler_setting" in
      (CARGO_BUILD_*|CARGO_PROFILE_*|CARGO_TARGET_*|CARGO_ENCODED_*) printf '%s\n' "$compiler_setting" ;;
    esac
  done | LC_ALL=C sort | while IFS= read -r compiler_setting; do
    printf '%s\0%s\0' "$compiler_setting" "${!compiler_setting}"
  done
} | git hash-object --stdin)
printf 'key=%s\n' "$compiler_key" >> "$GITHUB_OUTPUT"
