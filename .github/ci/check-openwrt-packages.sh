#!/usr/bin/env bash
# 发布前要求每个支持的架构同时具有 APK v3 与 IPK，拒绝部分产物。
set -euo pipefail

dist=${1:?缺少产物目录}
prefix=${2:?缺少产物前缀}
# 负向用例保留完整诊断和退出码，只避免把临时夹具标为发布错误。
case ${3:-} in
  '') diagnostic_prefix='::error::' ;;
  --test-fixture) diagnostic_prefix='[测试夹具，预期拒绝] ' ;;
  *) printf '未知参数：%s\n' "$3" >&2; exit 2 ;;
esac
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
source "$repo/install/openwrt/targets.sh"
for arch in "${OPENWRT_ARCHES[@]}"; do
  for format in apk ipk; do
    package="$dist/honk-$prefix-openwrt-25.12-$arch.$format"
    if [[ ! -f "$package" || ! -s "$package" || -L "$package" ]]; then
      printf '%sOpenWrt 安装包缺失或为空：%s\n' "$diagnostic_prefix" "$package" >&2
      exit 1
    fi
  done
done
