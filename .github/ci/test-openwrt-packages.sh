#!/usr/bin/env bash
# 完整性检查必须拒绝缺包、空包，尤其不能只发布 APK 或只发布 IPK。
set -euo pipefail
ci_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
source "$ci_dir/../../install/openwrt/targets.sh"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

expect_rejection() {
  local scenario=$1 status=0
  printf '[负向用例] %s；仅操作临时夹具。\n' "$scenario"
  bash "$ci_dir/check-openwrt-packages.sh" "$work" v1.2.3 --test-fixture || status=$?
  if [[ $status != 1 ]]; then
    printf '::error::%s 应返回 1，实际返回 %s\n' "$scenario" "$status" >&2
    exit 1
  fi
  printf '[通过] %s已按预期拒绝。\n' "$scenario"
}

printf 'OpenWrt 产物完整性自测：v1.2.3 为测试版本，所有文件均为临时夹具。\n'
for arch in "${OPENWRT_ARCHES[@]}"; do
  for format in apk ipk; do
    printf 'fixture\n' > "$work/honk-v1.2.3-openwrt-25.12-$arch.$format"
  done
done
bash "$ci_dir/check-openwrt-packages.sh" "$work" v1.2.3
missing="$work/honk-v1.2.3-openwrt-25.12-aarch64_cortex-a53.ipk"
rm "$missing"
expect_rejection '模拟缺少 A53 IPK'
: > "$missing"
expect_rejection '模拟空 A53 IPK'
printf 'fixture\n' > "$missing"
bash "$ci_dir/check-openwrt-packages.sh" "$work" v1.2.3
echo '[通过] 已恢复 A53 IPK 夹具，全部架构的 APK/IPK 完整性检查通过。'
echo 'OpenWrt 产物完整性用例通过'
