#!/usr/bin/env bash
# 完整性检查必须拒绝缺包、空包，尤其不能只发布 APK 或只发布 IPK。
set -euo pipefail
ci_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
source "$ci_dir/../../install/openwrt/targets.sh"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
for arch in "${OPENWRT_ARCHES[@]}"; do
  for format in apk ipk; do
    printf 'fixture\n' > "$work/honk-v1.2.3-openwrt-25.12-$arch.$format"
  done
done
bash "$ci_dir/check-openwrt-packages.sh" "$work" v1.2.3
missing="$work/honk-v1.2.3-openwrt-25.12-aarch64_cortex-a53.ipk"
rm "$missing"
if bash "$ci_dir/check-openwrt-packages.sh" "$work" v1.2.3; then
  echo '缺少一个 IPK 时应当失败' >&2
  exit 1
fi
: > "$missing"
if bash "$ci_dir/check-openwrt-packages.sh" "$work" v1.2.3; then
  echo '空 IPK 应当失败' >&2
  exit 1
fi
printf 'fixture\n' > "$missing"
bash "$ci_dir/check-openwrt-packages.sh" "$work" v1.2.3
echo 'OpenWrt 产物完整性用例通过'
