#!/usr/bin/env bash
# 使用已构建的完整 musl 产物，生成 OpenWrt 25.12 的 APK v3 和 IPK。
# 用法：bash .github/ci/build-openwrt-packages.sh <dist-dir> <prefix> <version> <sdk-dir>
set -euo pipefail

dist=$(cd -- "${1:?缺少产物目录}" && pwd -P)
prefix=${2:?缺少产物前缀}
version=${3:?缺少版本号}
sdk=$(cd -- "${4:?缺少 OpenWrt 25.12 SDK 目录}" && pwd -P)
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
source "$repo/install/openwrt/targets.sh"
apk="$sdk/staging_dir/host/bin/apk"
[[ -x "$apk" ]] || { echo 'SDK 中缺少 APK v3 打包器' >&2; exit 1; }
[[ $prefix =~ ^[A-Za-z0-9][A-Za-z0-9._-]*$ ]] || { echo '产物前缀无效' >&2; exit 1; }
version=${version#v}
if [[ ! $version =~ ^([0-9]+\.[0-9]+\.[0-9]+)(-(alpha|beta|rc)\.([0-9]+))?$ ]]; then
  echo '版本号须为 MAJOR.MINOR.PATCH 或带 -alpha.N/-beta.N/-rc.N' >&2
  exit 1
fi
apk_version=${BASH_REMATCH[1]}
[[ -z ${BASH_REMATCH[2]} ]] || apk_version+="_${BASH_REMATCH[3]}${BASH_REMATCH[4]}"
apk_version+=-r1
ipk_version=${version//-/\~}-1
export SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-$(git -C "$repo" show -s --format=%ct HEAD)}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# 运行时需要内核功能和 CA 根；完整静态二进制不依赖系统 C++ 或 Geo 包。
depends='ca-bundle kmod-veth kmod-sched-core kmod-sched-bpf kmod-nft-core kmod-nft-queue kmod-nfnetlink-queue kmod-nf-conntrack'
for arch in x86_64 aarch64; do
  name="honk-core-$prefix-$arch-unknown-linux-musl-full"
  tar -xzf "$dist/$name.tar.gz" -C "$work"
  binary="$work/$name/honk-core"
  [[ -s "$binary" ]] || { echo "缺少二进制：$binary" >&2; exit 1; }
  # 安装包不能将动态链接的非 OpenWrt 二进制误标为静态通用包。
  program_headers=$(readelf -l "$binary")
  dynamic_entries=$(readelf -d "$binary")
  if [[ $program_headers == *INTERP* || $dynamic_entries == *'(NEEDED)'* ]]; then
    echo "二进制存在动态解释器或动态库依赖：$binary" >&2
    exit 1
  fi
  root="$work/$arch/root"
  mkdir -p "$root/usr/bin" "$root/usr/share/licenses/honk" "$root/usr/share/doc/honk" \
    "$root/etc/init.d" "$root/etc/config" "$root/etc/honk" "$root/lib/upgrade/keep.d"
  install -m 0755 "$binary" "$root/usr/bin/honk-core"
  install -m 0644 "$work/$name/LICENSE" "$root/usr/share/licenses/honk/LICENSE"
  cp -R "$work/$name/doona" "$work/$name/doona-lite" "$root/usr/share/doc/honk/"
  install -m 0755 "$repo/install/openwrt/honk.init" "$root/etc/init.d/honk"
  install -m 0600 "$repo/install/openwrt/honk.config" "$root/etc/config/honk"
  install -m 0600 "$repo/install/openwrt/config.dae" "$root/etc/honk/config.dae"
  printf '%s\n' /etc/config/honk /etc/honk/ > "$root/lib/upgrade/keep.d/honk"
  chmod 0700 "$root/etc/honk"

  for package_arch in "${OPENWRT_ARCHES[@]}"; do
    [[ $(openwrt_binary_arch "$package_arch") == "$arch" ]] || continue
    output="$dist/honk-$prefix-openwrt-25.12-$package_arch"
    control="$work/$package_arch/control"
    mkdir -p "$control"
    cat > "$control/control" <<EOF
Package: honk
Version: $ipk_version
Architecture: $package_arch
Maintainer: daeuniverse <dae@v2raya.org>
Section: net
Priority: optional
License: GPL-3.0-only
Depends: ${depends// /, }
Source: https://github.com/daeuniverse/honk
Description: honk transparent proxy with embedded doona UI
Installed-Size: $(du -sb "$root" | cut -f1)
EOF
    printf '%s\n' /etc/config/honk /etc/honk/config.dae > "$control/conffiles"
    install -m 0755 "$repo/install/openwrt/postinst" "$control/postinst"
    install -m 0755 "$repo/install/openwrt/prerm" "$control/prerm"
    pack="$work/$package_arch/ipk"
    mkdir -p "$pack"
    printf '2.0\n' > "$pack/debian-binary"
    tar --sort=name --mtime="@$SOURCE_DATE_EPOCH" --owner=0 --group=0 --numeric-owner \
      -czf "$pack/control.tar.gz" -C "$control" .
    tar --sort=name --mtime="@$SOURCE_DATE_EPOCH" --owner=0 --group=0 --numeric-owner \
      -czf "$pack/data.tar.gz" -C "$root" .
    tar --sort=name --mtime="@$SOURCE_DATE_EPOCH" --owner=0 --group=0 --numeric-owner \
      -czf "$output.ipk" -C "$pack" ./debian-binary ./control.tar.gz ./data.tar.gz

    apk_root="$work/$package_arch/apk"
    mkdir -p "$apk_root"
    cp -a "$root/." "$apk_root/"
    metadata="$apk_root/lib/apk/packages"
    mkdir -p "$metadata"
    cp "$control/conffiles" "$metadata/honk.conffiles"
    while IFS= read -r file; do
      checksum=$(sha256sum "$apk_root$file")
      printf '%s %s\n' "$file" "${checksum%% *}"
    done < "$control/conffiles" > "$metadata/honk.conffiles_static"
    (cd "$apk_root" && find . \( -type f -o -type l \) -printf '/%P\n' | sort) > "$work/$package_arch/files"
    cp "$work/$package_arch/files" "$metadata/honk.list"
    # APK 的卸载钩子没有 OPKG 的 upgrade 参数；升级只停进程，不禁用开机链接。
    pre_upgrade="$work/$package_arch/pre-upgrade"
    printf '#!/bin/sh\n[ -z "${IPKG_INSTROOT:-}" ] || exit 0\n[ "${IPKG_NO_SCRIPT:-0}" != 1 ] || exit 0\n/etc/init.d/honk stop\n' > "$pre_upgrade"
    fakeroot sh -ec 'chown -R 0:0 "$1"; shift; exec "$@"' sh "$apk_root" "$apk" mkpkg \
      --info 'name:honk' --info "version:$apk_version" --info "arch:$package_arch" \
      --info 'description:honk 透明代理，含完整原生 API 与 doona UI' \
      --info 'license:GPL-3.0-only' --info 'origin:honk' \
      --info 'url:https://github.com/daeuniverse/honk' \
      --info 'maintainer:daeuniverse <dae@v2raya.org>' --info "depends:$depends" \
      --script "post-install:$control/postinst" --script "pre-upgrade:$pre_upgrade" \
      --script "post-upgrade:$control/postinst" --script "pre-deinstall:$control/prerm" \
      --files "$apk_root" --output "$output.apk"
    "$apk" --allow-untrusted verify "$output.apk"
    [[ -s "$output.apk" && -s "$output.ipk" ]]
    printf '已生成 %s.{apk,ipk}\n' "$output"
  done
done
