#!/usr/bin/env bash
set -euo pipefail

# Usage: check-release-artifacts.sh <prefix>, where <prefix> is `debug` or the formal tag (e.g. v0.1.0-rc.1).
prefix=${1:?usage: check-release-artifacts.sh <debug|tag>}

check() {
  local artifact=$1
  if [[ ! -f "$artifact" || ! -s "$artifact" ]]; then
    printf '::error::Missing or empty release artifact: %s. Re-run all jobs to rebuild the complete set.\n' "$artifact" >&2
    exit 1
  fi
}

# A53 使用完整 ARM64 musl 核心；不再要求其他架构或精简变体。
check "dist/honk-core-$prefix-aarch64-unknown-linux-musl-full.tar.gz"
