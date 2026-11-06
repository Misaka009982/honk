#!/bin/sh
# 参考 kenzok8/openwrt-honk 的安装流程，适配本分支的 APK v3 / IPK 与 Geo 自动下载。
set -eu

fail() {
	printf '错误：%s\n' "$*" >&2
	exit 1
}

fetch() {
	if command -v curl >/dev/null 2>&1; then
		curl -fL --retry 3 --connect-timeout 10 --max-time 180 \
			--proto '=https' --proto-redir '=https' -o "$2" "$1"
	else
		wget -O "$2" "$1"
	fi
}

download_release() {
	if [ -n "${GH_PROXY:-}" ]; then
		case "$GH_PROXY" in https://*) ;; *) fail 'GH_PROXY 必须是 HTTPS 地址' ;; esac
		if fetch "${GH_PROXY%/}/$1" "$2"; then
			return 0
		fi
		printf '镜像下载失败，改用 GitHub 直连。\n' >&2
	fi
	fetch "$1" "$2"
}

release_field() {
	# 只读取固定字段，不执行固件配置中的 shell 内容。
	awk -F= -v field="$2" '$1 == field { gsub(/[\047\042\r]/, "", $2); print $2; exit }' "$1"
}

detect_platform() {
	[ -r /etc/openwrt_release ] || fail '仅支持 OpenWrt / ImmortalWrt 25.12 固件'
	firmware=$(release_field /etc/openwrt_release DISTRIB_RELEASE)
	case "$firmware" in 25.12|25.12.*) ;; *) fail "当前固件为 $firmware；本脚本仅安装 25.12 的包" ;; esac
	kernel=$(uname -r)
	major=${kernel%%.*}
	minor=${kernel#*.}
	minor=${minor%%.*}
	case "$major:$minor" in *[!0-9:]*|:*|*:) fail "无法识别内核版本 $kernel" ;; esac
	if [ "$major" -lt 6 ] || { [ "$major" -eq 6 ] && [ "$minor" -lt 12 ]; }; then
		fail "honk 需要 Linux 6.12+，当前为 $kernel"
	fi
	case "${HONK_PM:-auto}" in
		auto)
			if command -v apk >/dev/null 2>&1; then
				pm=apk
			elif command -v opkg >/dev/null 2>&1; then
				pm=opkg
			else
				fail '未找到 apk 或 opkg'
			fi ;;
		apk|opkg) pm=$HONK_PM; command -v "$pm" >/dev/null 2>&1 || fail "未找到 $pm" ;;
		*) fail 'HONK_PM 只能为 auto、apk 或 opkg' ;;
	esac
	if [ "$pm" = apk ]; then
		arch=$(apk --print-arch)
		ext=apk
	else
		arch=$(opkg print-architecture | awk '$1 == "arch" && $2 != "all" && $2 != "noarch" && $3 + 0 > priority { priority=$3; result=$2 } END { print result }')
		ext=ipk
	fi
	case "$arch" in
		x86_64|aarch64_generic|aarch64_cortex-a53|aarch64_cortex-a72|aarch64_cortex-a76) ;;
		*) fail "尚无 $arch 的安装包，不会强制安装其他架构" ;;
	esac
	printf '检测到固件 %s · 内核 %s · 架构 %s · %s\n' "$firmware" "$kernel" "$arch" "$pm"
}

package_name() {
	case "$1" in
		debug|debug.*) printf 'honk-debug-openwrt-25.12-%s.%s\n' "$arch" "$ext" ;;
		*) printf 'honk-%s-openwrt-25.12-%s.%s\n' "$1" "$arch" "$ext" ;;
	esac
}

has_assets() {
	asset_names=$(jsonfilter -i "$1" -e "$2.assets[*].name") || return 1
	printf '%s\n' "$asset_names" | grep -qxF "$3" || return 1
	printf '%s\n' "$asset_names" | grep -qxF SHA256SUMS
}

resolve_release() {
	if [ -n "${HONK_VERSION:-}" ]; then
		tag=$HONK_VERSION
		case "$tag" in ''|*[!A-Za-z0-9._-]*) fail 'HONK_VERSION 含无效字符' ;; esac
		case "$tag" in v*|debug|debug.*) ;; *) fail 'HONK_VERSION 必须是 v 开头的标签或 debug 标签' ;; esac
		fetch "https://api.github.com/repos/$repo/releases/tags/$tag" "$work/release.json" || fail "无法读取版本 $tag"
		package=$(package_name "$tag")
		has_assets "$work/release.json" '@' "$package" || fail "版本 $tag 缺少 $package 或 SHA256SUMS，请先发布本分支的完整构建"
		return 0
	fi
	fetch "https://api.github.com/repos/$repo/releases?per_page=30" "$work/releases.json" || fail '无法读取发布列表，可用 HONK_VERSION 指定版本'
	# 优先正式版；没有匹配包的正式版时允许 beta/rc，Debug 必须显式指定。
	for prerelease in false true; do
		index=0
		while [ "$index" -lt 30 ]; do
			scope="@[$index]"
			index=$((index + 1))
			tag=$(jsonfilter -i "$work/releases.json" -e "$scope.tag_name") || continue
			printf '%s\n' "$tag" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+(-(alpha|beta|rc)\.[0-9]+)?$' || continue
			[ "$(jsonfilter -i "$work/releases.json" -e "$scope.draft")" = false ] || continue
			[ "$(jsonfilter -i "$work/releases.json" -e "$scope.prerelease")" = "$prerelease" ] || continue
			package=$(package_name "$tag")
			if has_assets "$work/releases.json" "$scope" "$package"; then
				return 0
			fi
		done
	done
	fail '最近 30 个发布没有适配的安装包和校验和；请先发布本分支，或用 HONK_VERSION 指定已有版本'
}

verify_checksum() {
	# 精确匹配一个条目，拒绝缺失、重复及格式无效的校验行。
	checksum=$(awk -v file="$2" '$2 == file { hash=$1; count++ } END { if (count != 1 || length(hash) != 64 || hash ~ /[^0-9a-fA-F]/) exit 1; print hash }' "$1") || fail 'SHA256SUMS 中缺少唯一有效的安装包校验值'
	printf '%s  %s\n' "$checksum" "$3" | sha256sum -c - || fail '安装包 SHA-256 不匹配，已停止安装'
}

read_metadata() {
	if [ "$pm" = apk ]; then
		apk --allow-untrusted verify "$work/$package" || fail 'APK 校验失败'
		apk --allow-untrusted adbdump "$work/$package" > "$work/metadata"
		package_id=$(awk '$1 == "name:" { print $2; exit }' "$work/metadata")
		package_arch=$(awk '$1 == "arch:" { print $2; exit }' "$work/metadata")
		package_version=$(awk '$1 == "version:" { print $2; exit }' "$work/metadata")
	else
		tar -xzf "$work/$package" -C "$work" ./control.tar.gz
		tar -xOzf "$work/control.tar.gz" ./control > "$work/metadata"
		package_id=$(awk '$1 == "Package:" { print $2; exit }' "$work/metadata")
		package_arch=$(awk '$1 == "Architecture:" { print $2; exit }' "$work/metadata")
		package_version=$(awk '$1 == "Version:" { print $2; exit }' "$work/metadata")
	fi
	[ "$package_id" = honk ] && [ "$package_arch" = "$arch" ] && [ -n "$package_version" ] || fail '安装包的名称、架构或版本元数据无效'
}

ensure_btf() {
	if [ -r /sys/kernel/btf/vmlinux ] || [ -r /usr/lib/debug/boot/vmlinux ]; then
		return 0
	fi
	printf '内核没有提供 BTF，尝试从当前固件软件源安装 vmlinux-btf。\n'
	if [ "$pm" = apk ]; then
		apk add vmlinux-btf || fail '软件源没有可用 BTF，请为当前内核提供匹配的 vmlinux-btf 后重试'
	else
		opkg install vmlinux-btf || fail '软件源没有可用 BTF，请为当前内核提供匹配的 vmlinux-btf 后重试'
	fi
	[ -r /sys/kernel/btf/vmlinux ] || [ -r /usr/lib/debug/boot/vmlinux ] || fail '安装后仍缺少 BTF，未安装 honk'
}

main() {
	[ "$#" -eq 0 ] || fail '通过 HONK_VERSION、HONK_REPO、HONK_PM 和 GH_PROXY 环境变量配置安装'
	[ "$(id -u)" = 0 ] || fail '请以 root 运行'
	for tool in jsonfilter sha256sum awk grep tar mktemp; do
		command -v "$tool" >/dev/null 2>&1 || fail "缺少命令 $tool"
	done
	command -v curl >/dev/null 2>&1 || command -v wget >/dev/null 2>&1 || fail '需要 curl 或 wget'
	repo=${HONK_REPO:-Misaka009982/honk}
	printf '%s\n' "$repo" | grep -Eq '^[A-Za-z0-9][A-Za-z0-9_.-]*/[A-Za-z0-9][A-Za-z0-9_.-]*$' || fail 'HONK_REPO 必须是 owner/repo'
	detect_platform
	umask 077
	work=$(mktemp -d /tmp/honk-install.XXXXXX)
	trap 'rm -rf "$work"' EXIT
	trap 'exit 130' INT
	trap 'exit 143' TERM HUP
	resolve_release
	printf '安装来源：%s · 版本：%s · 文件：%s\n' "$repo" "$tag" "$package"
	base="https://github.com/$repo/releases/download/$tag"
	download_release "$base/SHA256SUMS" "$work/SHA256SUMS" || fail '校验和下载失败'
	download_release "$base/$package" "$work/$package" || fail '安装包下载失败'
	verify_checksum "$work/SHA256SUMS" "$package" "$work/$package"
	read_metadata
	if [ "$pm" = apk ]; then
		apk update
		ensure_btf
		if apk add --allow-untrusted "$work/$package"; then :; else
			code=$?
			printf 'apk 安装失败（退出码 %s），请检查上方完整日志与固件软件源。\n' "$code" >&2
			exit "$code"
		fi
		installed=$(apk list --installed honk)
		printf '%s\n' "$installed" | awk -v expected="honk-$package_version" '$1 == expected { found=1 } END { exit !found }' || fail '目标版本未成功安装'
	else
		opkg update
		ensure_btf
		if opkg install "$work/$package"; then :; else
			code=$?
			printf 'opkg 安装失败（退出码 %s），请检查上方完整日志与固件软件源。\n' "$code" >&2
			exit "$code"
		fi
		installed=$(opkg status honk)
		printf '%s\n' "$installed" | awk -v expected="$package_version" '$1 == "Version:" && $2 == expected { version=1 } $1 == "Status:" && $NF == "installed" { installed=1 } END { exit !(version && installed) }' || fail '目标版本未成功安装'
	fi
	[ -x /usr/bin/honk-core ] && [ -x /etc/init.d/honk ] || fail '安装后缺少核心程序或 procd 服务'
	/usr/bin/honk-core --version
	printf '%s\n' '安装完成。配置：/etc/honk/config.dae；已有配置由包管理器保留。' \
		'首次安装请配置节点和接口，再执行 /etc/init.d/honk enable 和 /etc/init.d/honk start。' \
		'已启用的服务在升级后重启；查看日志：logread -e honk。'
}

# 便于离线用例检查版本选择和校验流程，正常执行始终进入 main。
if [ "${HONK_INSTALLER_LIBRARY:-0}" != 1 ]; then
	main "$@"
fi
