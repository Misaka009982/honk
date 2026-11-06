#!/usr/bin/env bash
# 发布前要求每个支持的架构同时具有 APK v3 与 IPK，拒绝部分产物。
set -euo pipefail

dist=${1:?缺少产物目录}
prefix=${2:?缺少产物前缀}
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
source "$repo/install/openwrt/targets.sh"
for arch in "${OPENWRT_ARCHES[@]}"; do
  for format in apk ipk; do
    package="$dist/honk-$prefix-openwrt-25.12-$arch.$format"
    if [[ ! -f "$package" || ! -s "$package" || -L "$package" ]]; then
      printf '::error::OpenWrt 安装包缺失或为空：%s\n' "$package" >&2
      exit 1
    fi
  done
done
