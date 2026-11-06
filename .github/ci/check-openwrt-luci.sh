#!/usr/bin/env bash
# LuCI 的共享库、管理页和中文翻译必须成套提供，两种格式缺一不可。
set -euo pipefail
dist=${1:?缺少产物目录}
prefix=${2:?缺少产物前缀}
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
source "$repo/install/openwrt/luci/sources.sh"
for package in "${LUCI_PACKAGES[@]}"; do
  for target in all.ipk noarch.apk; do
    file="$dist/$package-$prefix-openwrt-25.12-$target"
    if [[ ! -f "$file" || ! -s "$file" || -L "$file" ]]; then
      printf '::error::LuCI 安装包缺失或为空：%s\n' "$file" >&2
      exit 1
    fi
  done
done
source_archive="$dist/honk-luci-source-$prefix.tar.gz"
[[ -f "$source_archive" && -s "$source_archive" && ! -L "$source_archive" ]] || {
  printf '::error::缺少 LuCI 配套源码：%s\n' "$source_archive" >&2
  exit 1
}
