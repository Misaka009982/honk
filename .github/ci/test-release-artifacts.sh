#!/usr/bin/env bash
set -euo pipefail

ci_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cd "$work"

create_set() {
  local prefix=$1
  mkdir dist
  printf 'artifact\n' > "dist/honk-core-$prefix-aarch64-unknown-linux-musl-full.tar.gz"
}

expect_rejection() {
  local prefix=$1 status=0
  bash "$ci_dir/check-release-artifacts.sh" "$prefix" > failure.log 2>&1 || status=$?
  if [[ "$status" != 1 ]]; then
    cat failure.log >&2
    printf 'Expected incomplete %s artifacts to be rejected, got exit %s\n' "$prefix" "$status" >&2
    exit 1
  fi
}

# 分支、Debug 和正式标签均只要求 A53 对应的核心归档。
for prefix in main debug v0.1.0-rc.1; do
  create_set "$prefix"
  bash "$ci_dir/check-release-artifacts.sh" "$prefix"

  artifact=dist/honk-core-$prefix-aarch64-unknown-linux-musl-full.tar.gz
  mv "$artifact" saved.tar.gz
  expect_rejection "$prefix"
  mv saved.tar.gz "$artifact"
  bash "$ci_dir/check-release-artifacts.sh" "$prefix"

  : > "$artifact"
  expect_rejection "$prefix"
  rm "$artifact"
  mkdir "$artifact"
  expect_rejection "$prefix"
  rm -r dist
  expect_rejection "$prefix"
done

# A complete set under the other prefix does not satisfy the check.
create_set debug
expect_rejection v0.1.0-rc.1

printf 'Release artifact preflight regression passed\n'
