#!/usr/bin/env bash
set -euo pipefail
# rust-cache prunes the unhashed MSVC executable from deps. Cargo requires it
# even when every source fingerprint matches; retain it in the same keyed cache.
case "${1:-}" in
  save|restore) ;;
  *) echo 'Expected save or restore' >&2; exit 1 ;;
esac
if [ "${RUNNER_OS:-}" != Windows ]; then exit 0; fi
artifact=core/target/release/deps/wombat_core.exe
retained=core/.ci-rust-outputs/release/wombat_core.exe
if [ "$1" = save ]; then
  if [ ! -f "$artifact" ]; then exit 0; fi
  mkdir -p -- "$(dirname "$retained")"
  cp -p -- "$artifact" "$retained"
elif [ "${CI_RUST_CACHE_HIT:-}" = true ] && [ -f "$retained" ]; then
  mkdir -p -- "$(dirname "$artifact")"
  cp -p -- "$retained" "$artifact"
fi
