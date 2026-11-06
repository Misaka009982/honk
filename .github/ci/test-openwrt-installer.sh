#!/usr/bin/env bash
# 仅使用本地发布元数据与文件，验证安装器不会选错版本或接受损坏下载。
set -euo pipefail
ci_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
fixtures="$work/fixtures"
mkdir -p "$fixtures" "$work/bin"
cat > "$work/bin/jsonfilter" <<'PY'
#!/usr/bin/env python3
import json
import re
import sys

args = sys.argv[1:]
with open(args[args.index("-i") + 1], encoding="utf-8") as stream:
    value = json.load(stream)
expression = args[args.index("-e") + 1]
match = re.fullmatch(r"@(\[(\d+)\])?\.(tag_name|draft|prerelease|assets\[\*\]\.name)", expression)
if not match:
    sys.exit(1)
if match[2] is not None:
    index = int(match[2])
    if index >= len(value):
        sys.exit(1)
    value = value[index]
field = match[3]
if field == "assets[*].name":
    for asset in value.get("assets", []):
        print(asset["name"])
else:
    selected = value[field]
    print(str(selected).lower() if isinstance(selected, bool) else selected)
PY
chmod +x "$work/bin/jsonfilter"
export PATH="$work/bin:$PATH"
HONK_INSTALLER_LIBRARY=1 source "$ci_dir/../../scripts/install.sh"
repo=example/honk
arch=x86_64
ext=apk
HONK_LUCI=0
unset HONK_VERSION

cat > "$fixtures/releases.json" <<'JSON'
[
  {"tag_name":"debug", "draft":false, "prerelease":true, "assets":[]},
  {"tag_name":"v3.0.0", "draft":false, "prerelease":false, "assets":[]},
  {"tag_name":"v2.0.0-beta.1", "draft":false, "prerelease":true, "assets":[{"name":"honk-v2.0.0-beta.1-openwrt-25.12-x86_64.apk"},{"name":"SHA256SUMS"}]},
  {"tag_name":"v1.0.0", "draft":false, "prerelease":false, "assets":[{"name":"honk-v1.0.0-openwrt-25.12-x86_64.apk"},{"name":"SHA256SUMS"}]}
]
JSON
fetch() { cp "$fixtures/releases.json" "$2"; }
resolve_release
[[ $tag == v1.0.0 ]] || { echo '应优先正式版且跳过没有安装包的发布' >&2; exit 1; }
python3 - "$fixtures/releases.json" <<'PY'
import json
import sys
path = sys.argv[1]
with open(path, encoding="utf-8") as stream:
    data = json.load(stream)
with open(path, "w", encoding="utf-8") as stream:
    json.dump(data[:3], stream)
PY
resolve_release
[[ $tag == v2.0.0-beta.1 ]] || { echo '只有预发布时应能安装 beta' >&2; exit 1; }
arch=aarch64_cortex-a53
if (trap - EXIT; resolve_release); then
  echo '缺少本机架构时必须拒绝' >&2
  exit 1
fi
arch=x86_64
ext=ipk
[[ $(package_name debug.2026.1) == honk-debug-openwrt-25.12-x86_64.ipk ]]

# A53 核心必须搭配 all 架构的三份 LuCI IPK，缺一份就拒绝整套安装。
arch=aarch64_cortex-a53
HONK_LUCI=1
tag=debug.build.42221567511
package=$(package_name "$tag")
cat > "$fixtures/bundle.json" <<'JSON'
{"assets":[
  {"name":"honk-debug-openwrt-25.12-aarch64_cortex-a53.ipk"},
  {"name":"luci-lib-daeuniverse-debug-openwrt-25.12-all.ipk"},
  {"name":"luci-app-honk-debug-openwrt-25.12-all.ipk"},
  {"name":"luci-i18n-daeuniverse-zh-cn-debug-openwrt-25.12-all.ipk"},
  {"name":"SHA256SUMS"}
]}
JSON
has_assets "$fixtures/bundle.json" '@' "$package"
for component in luci-lib-daeuniverse luci-app-honk luci-i18n-daeuniverse-zh-cn; do
  python3 - "$fixtures/bundle.json" "$fixtures/incomplete.json" "$component" <<'PY'
import json
import sys
with open(sys.argv[1], encoding="utf-8") as stream:
    data = json.load(stream)
data["assets"] = [asset for asset in data["assets"] if not asset["name"].startswith(sys.argv[3] + "-")]
with open(sys.argv[2], "w", encoding="utf-8") as stream:
    json.dump(data, stream)
PY
  if has_assets "$fixtures/incomplete.json" '@' "$package"; then
    echo "缺少 $component 时必须拒绝整套安装" >&2
    exit 1
  fi
done
HONK_LUCI=0
has_assets "$fixtures/incomplete.json" '@' "$package"
ext=apk
[[ $(luci_package_names "$tag") == $'luci-lib-daeuniverse-debug-openwrt-25.12-noarch.apk\nluci-app-honk-debug-openwrt-25.12-noarch.apk\nluci-i18n-daeuniverse-zh-cn-debug-openwrt-25.12-noarch.apk' ]]
ext=ipk

# 包名和架构从实际 control 文件读取；all 仅适用于 LuCI，不能冒充 A53 核心。
mkdir "$work/control-fixture"
printf 'Package: luci-app-honk\nVersion: 2026.11.6-1\nArchitecture: all\n' > "$work/control-fixture/control"
tar -czf "$work/control.tar.gz" -C "$work/control-fixture" ./control
tar -czf "$work/luci.ipk" -C "$work" ./control.tar.gz
pm=opkg
read_metadata "$work/luci.ipk" luci-app-honk all
if (trap - EXIT; read_metadata "$work/luci.ipk" honk "$arch"); then
  echo 'LuCI 包不能代替核心' >&2
  exit 1
fi

printf 'package-content\n' > "$work/package.ipk"
hash=$(sha256sum "$work/package.ipk")
printf '%s  package.ipk\n' "${hash%% *}" > "$work/SHA256SUMS"
verify_checksum "$work/SHA256SUMS" package.ipk "$work/package.ipk"
printf 'corrupted\n' > "$work/package.ipk"
if (trap - EXIT; verify_checksum "$work/SHA256SUMS" package.ipk "$work/package.ipk"); then
  echo '损坏下载不能继续安装' >&2
  exit 1
fi
printf '%s  package.ipk\n' "${hash%% *}" >> "$work/SHA256SUMS"
if (trap - EXIT; verify_checksum "$work/SHA256SUMS" package.ipk "$work/package.ipk"); then
  echo '重复校验条目必须拒绝' >&2
  exit 1
fi
printf 'OpenWrt 安装器离线用例通过\n'
