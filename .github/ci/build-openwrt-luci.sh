#!/usr/bin/env bash
# 独立打包 LuCI，复用固定上游页面与翻译，不编译 honk 核心。
# 用法：bash .github/ci/build-openwrt-luci.sh <dist-dir> <prefix> <sdk-dir>
set -euo pipefail
dist=$(cd -- "${1:?缺少产物目录}" && pwd -P)
prefix=${2:?缺少产物前缀}
sdk=$(cd -- "${3:?缺少 SDK 目录}" && pwd -P)
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
module="$repo/install/openwrt/luci"
source "$module/sources.sh"
[[ $prefix =~ ^[A-Za-z0-9][A-Za-z0-9._-]*$ ]] || { echo '产物前缀无效' >&2; exit 1; }
apk="$sdk/staging_dir/host/bin/apk"
[[ -x "$apk" ]] || { echo 'SDK 中缺少 APK v3 打包器' >&2; exit 1; }
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

fetch_revision() {
  local url=$1 revision=$2 destination=$3
  git init --quiet "$destination"
  git -C "$destination" fetch --depth 1 "$url" "$revision"
  git -C "$destination" checkout --detach FETCH_HEAD
  [[ $(git -C "$destination" rev-parse HEAD) == "$revision" ]]
}
fetch_revision https://github.com/daeuniverse/openwrt-packages.git "$DAE_LUCI_REVISION" "$work/upstream"
fetch_revision https://github.com/openwrt/luci.git "$LUCI_TOOL_REVISION" "$work/tools"
make -C "$work/tools/modules/luci-base/src" po2lmo

# 先保存上游原文与适配脚本；发布源码能够重现页面修改和翻译转换。
source_root="$work/honk-luci-source"
mkdir -p "$source_root/upstream" "$source_root/packaging"
cp -R "$work/upstream/luci-app-honk" "$work/upstream/luci-lib-daeuniverse" "$source_root/upstream/"
cp -R "$module" "$source_root/packaging/"
cp "$repo/.github/ci/build-openwrt-luci.sh" "$repo/.github/ci/prepare-openwrt-sdk.sh" \
  "$repo/.github/ci/check-openwrt-luci.sh" "$repo/install/openwrt/sdk.sh" "$source_root/packaging/"
cp "$repo/LICENSE" "$source_root/LICENSE"
printf '上游页面提交：%s\nLuCI 翻译工具提交：%s\n适配提交：%s\n' \
  "$DAE_LUCI_REVISION" "$LUCI_TOOL_REVISION" "$(git -C "$repo" rev-parse HEAD)" > "$source_root/REVISIONS"
export SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-$(git -C "$repo" show -s --format=%ct HEAD)}
tar --sort=name --mtime="@$SOURCE_DATE_EPOCH" --owner=0 --group=0 --numeric-owner \
  -czf "$dist/honk-luci-source-$prefix.tar.gz" -C "$work" honk-luci-source

python3 "$module/adapt.py" "$work/upstream"
node --check "$work/upstream/luci-lib-daeuniverse/htdocs/luci-static/resources/daeuniverse/service.js"
node --check "$work/upstream/luci-app-honk/htdocs/luci-static/resources/view/honk/main.js"
node --check "$work/upstream/luci-app-honk/htdocs/luci-static/resources/honk/nativeapi.js"
for package in "${LUCI_PACKAGES[@]}"; do
  root="$work/$package/root"
  control="$work/$package/control"
  pack="$work/$package/ipk"
  mkdir -p "$root" "$control" "$pack"
  case "$package" in
    luci-lib-daeuniverse)
      depends='luci-base rpcd-mod-file jshn'
      description='honk 与 dae 的 LuCI 服务管理共享页面'
      mkdir -p "$root/www" "$root/usr/libexec"
      cp -R "$work/upstream/$package/htdocs/." "$root/www/"
      install -m 0755 "$module/daeuniverse-pkg" "$root/usr/libexec/daeuniverse-pkg"
      ;;
    luci-app-honk)
      depends='honk luci-lib-daeuniverse'
      description='honk 的 LuCI 服务管理、配置编辑与日志页面'
      mkdir -p "$root/www"
      cp -R "$work/upstream/$package/htdocs/." "$root/www/"
      cp -R "$work/upstream/$package/root/." "$root/"
      ;;
    luci-i18n-daeuniverse-zh-cn)
      depends='luci-lib-daeuniverse'
      description='daeuniverse LuCI 简体中文翻译'
      mkdir -p "$root/usr/lib/lua/luci/i18n" "$root/etc/uci-defaults"
      "$work/tools/modules/luci-base/src/po2lmo" \
        "$work/upstream/luci-lib-daeuniverse/po/zh_Hans/daeuniverse.po" \
        "$root/usr/lib/lua/luci/i18n/daeuniverse.zh-cn.lmo"
      [[ -s "$root/usr/lib/lua/luci/i18n/daeuniverse.zh-cn.lmo" ]]
      printf "uci set 'luci.languages.zh_cn=简体中文 (Simplified Chinese)'\nuci commit luci\n" \
        > "$root/etc/uci-defaults/luci-i18n-daeuniverse-zh-cn"
      ;;
  esac
  mkdir -p "$root/usr/share/licenses/$package"
  cp "$repo/LICENSE" "$root/usr/share/licenses/$package/COPYING"
  cp "$module/README.md" "$root/usr/share/licenses/$package/NOTICE.md"
  cat > "$control/control" <<EOF
Package: $package
Version: $LUCI_PACKAGE_VERSION-1
Architecture: all
Maintainer: Misaka009982
Section: luci
Priority: optional
License: GPL-3.0-or-later
Depends: ${depends// /, }
Source: https://github.com/daeuniverse/openwrt-packages/tree/$DAE_LUCI_REVISION
Description: $description
Installed-Size: $(du -sb "$root" | cut -f1)
EOF
  install -m 0755 "$module/postinst" "$control/postinst"
  printf '2.0\n' > "$pack/debian-binary"
  tar --sort=name --mtime="@$SOURCE_DATE_EPOCH" --owner=0 --group=0 --numeric-owner \
    -czf "$pack/control.tar.gz" -C "$control" .
  tar --sort=name --mtime="@$SOURCE_DATE_EPOCH" --owner=0 --group=0 --numeric-owner \
    -czf "$pack/data.tar.gz" -C "$root" .
  tar --sort=name --mtime="@$SOURCE_DATE_EPOCH" --owner=0 --group=0 --numeric-owner \
    -czf "$dist/$package-$prefix-openwrt-25.12-all.ipk" -C "$pack" ./debian-binary ./control.tar.gz ./data.tar.gz

  metadata="$root/lib/apk/packages"
  mkdir -p "$metadata"
  (cd "$root" && find . -type f -printf '/%P\n' | sort) > "$work/$package/files"
  cp "$work/$package/files" "$metadata/$package.list"
  fakeroot sh -ec 'chown -R 0:0 "$1"; shift; exec "$@"' sh "$root" "$apk" mkpkg \
    --info "name:$package" --info "version:$LUCI_PACKAGE_VERSION-r1" --info 'arch:noarch' \
    --info "description:$description" --info 'license:GPL-3.0-or-later' --info "origin:$package" \
    --info 'url:https://github.com/Misaka009982/honk' --info 'maintainer:Misaka009982' \
    --info "depends:$depends" --script "post-install:$control/postinst" --script "post-upgrade:$control/postinst" \
    --files "$root" --output "$dist/$package-$prefix-openwrt-25.12-noarch.apk"
  "$apk" --allow-untrusted verify "$dist/$package-$prefix-openwrt-25.12-noarch.apk"
  printf '已生成 LuCI 包：%s（IPK all / APK noarch）\n' "$package"
done
bash "$repo/.github/ci/check-openwrt-luci.sh" "$dist" "$prefix"
