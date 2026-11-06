#!/usr/bin/env bash
# 复用 main 构建内容；OpenWrt 附件使用既有 Debug 安装器的文件名。
set -euo pipefail
input=$(cd -- "${1:?缺少构建产物目录}" && pwd -P)
output=${2:?缺少发布目录}
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
source "$repo/install/openwrt/targets.sh"
HONK_INSTALLER_LIBRARY=1 source "$repo/scripts/install.sh"

bash "$repo/.github/ci/check-openwrt-packages.sh" "$input" main
# 每个安装包都必须有唯一校验项，不能只验证校验文件中碰巧存在的子集。
for arch in "${OPENWRT_ARCHES[@]}"; do
  for format in apk ipk; do
    name="honk-main-openwrt-25.12-$arch.$format"
    verify_checksum "$input/OPENWRT-SHA256SUMS" "$name" "$input/$name"
  done
done

mkdir -- "$output"
cp -- "$input/honk-core-main-aarch64-unknown-linux-musl-full.tar.gz" "$output/"
for arch in "${OPENWRT_ARCHES[@]}"; do
  for format in apk ipk; do
    cp -- "$input/honk-main-openwrt-25.12-$arch.$format" \
      "$output/honk-debug-openwrt-25.12-$arch.$format"
  done
done
bash "$repo/.github/ci/check-openwrt-packages.sh" "$output" debug

# 旧构建没有 LuCI；新构建一旦包含其中任一组件，就必须整套复用并校验。
if compgen -G "$input/luci-*-main-openwrt-25.12-*" >/dev/null || [[ -e "$input/honk-luci-source-main.tar.gz" ]]; then
  source "$repo/install/openwrt/luci/sources.sh"
  bash "$repo/.github/ci/check-openwrt-luci.sh" "$input" main
  for package in "${LUCI_PACKAGES[@]}"; do
    for target in all.ipk noarch.apk; do
      name="$package-main-openwrt-25.12-$target"
      verify_checksum "$input/OPENWRT-SHA256SUMS" "$name" "$input/$name"
      cp -- "$input/$name" "$output/$package-debug-openwrt-25.12-$target"
    done
  done
  name=honk-luci-source-main.tar.gz
  verify_checksum "$input/OPENWRT-SHA256SUMS" "$name" "$input/$name"
  cp -- "$input/$name" "$output/honk-luci-source-debug.tar.gz"
fi
