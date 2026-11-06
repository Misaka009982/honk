#!/usr/bin/env bash
# 只准备 SDK 主机打包器，核心和 LuCI 包共用。
set -euo pipefail
sdk=${1:?缺少 SDK 目录}
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
source "$repo/install/openwrt/sdk.sh"
archive=$(mktemp)
trap 'rm -f "$archive"' EXIT
curl -fsSL --retry 3 -o "$archive" "$OPENWRT_SDK_URL"
echo "$OPENWRT_SDK_SHA256  $archive" | sha256sum -c -
sdk_name=${OPENWRT_SDK_URL##*/}
sdk_name=${sdk_name%.tar.zst}
mkdir -p "$sdk"
tar --zstd -xf "$archive" --strip-components=1 -C "$sdk" "$sdk_name/staging_dir/host"
