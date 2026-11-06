#!/usr/bin/env bash
# 旧构建和新构建都必须保留核心字节，并拒绝损坏包后重新计算校验和。
set -euo pipefail
ci_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
source "$ci_dir/../../install/openwrt/luci/sources.sh"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
input="$work/input"
mkdir "$input"
core=honk-core-main-aarch64-unknown-linux-musl-full.tar.gz
printf 'core-bytes\n' > "$input/$core"
printf 'unselected\n' > "$input/honk-core-main-x86_64-unknown-linux-musl-full.tar.gz"
for format in apk ipk; do
  printf 'a53-%s\n' "$format" > "$input/honk-main-openwrt-25.12-aarch64_cortex-a53.$format"
done
(cd "$input" && sha256sum -- *.apk *.ipk > OPENWRT-SHA256SUMS)
bash "$ci_dir/prepare-existing-release.sh" "$input" "$work/old"
cmp "$input/$core" "$work/old/$core"
[[ ! -e "$work/old/honk-core-main-x86_64-unknown-linux-musl-full.tar.gz" ]]
[[ ! -e "$work/old/honk-luci-source-debug.tar.gz" ]]
for format in apk ipk; do
  cmp "$input/honk-main-openwrt-25.12-aarch64_cortex-a53.$format" \
    "$work/old/honk-debug-openwrt-25.12-aarch64_cortex-a53.$format"
done

for package in "${LUCI_PACKAGES[@]}"; do
  for target in all.ipk noarch.apk; do
    printf '%s %s\n' "$package" "$target" > "$input/$package-main-openwrt-25.12-$target"
  done
done
printf 'luci-source\n' > "$input/honk-luci-source-main.tar.gz"
(cd "$input" && sha256sum -- *.apk *.ipk honk-luci-source-main.tar.gz > OPENWRT-SHA256SUMS)
bash "$ci_dir/prepare-existing-release.sh" "$input" "$work/current"
for package in "${LUCI_PACKAGES[@]}"; do
  for target in all.ipk noarch.apk; do
    cmp "$input/$package-main-openwrt-25.12-$target" "$work/current/$package-debug-openwrt-25.12-$target"
  done
done
cmp "$input/honk-luci-source-main.tar.gz" "$work/current/honk-luci-source-debug.tar.gz"

printf '[负向用例] 修改临时 A53 IPK，必须在发布准备时拒绝。\n'
printf 'corrupted\n' > "$input/honk-main-openwrt-25.12-aarch64_cortex-a53.ipk"
status=0
bash "$ci_dir/prepare-existing-release.sh" "$input" "$work/rejected" || status=$?
[[ $status == 1 && ! -e "$work/rejected" ]] || { echo '损坏核心不能产生发布目录' >&2; exit 1; }
printf '已有构建的发布准备离线用例通过\n'
