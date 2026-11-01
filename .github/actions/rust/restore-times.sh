#!/usr/bin/env bash
set -euo pipefail
# Cargo compares source mtimes. A fresh checkout is newer than restored artifacts.
# Normalize only after the separate workspace cache proves an exact compiler/config/source key.
# Misses and dependency-only fallback restores retain fresh mtimes and must rebuild.
if [ "${CI_RUST_CACHE_HIT:-}" != true ]; then
  exit 0
fi
git ls-files -z -- core/src core/prices core/tests core/build.rs core/Cargo.toml core/Cargo.lock |
  TZ=UTC xargs -0 touch -t 197001010000.00 --
