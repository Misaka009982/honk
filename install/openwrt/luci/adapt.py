"""对固定上游 LuCI 源码做集中适配；源码变化时拒绝静默漏改。"""
import json
import sys
from pathlib import Path


def replace_once(path, old, new):
    text = path.read_text(encoding="utf-8")
    if text.count(old) != 1:
        raise ValueError(f"上游源码与适配点不匹配：{path}")
    path.write_text(text.replace(old, new), encoding="utf-8")


def adapt(root):
    service = root / "luci-lib-daeuniverse/htdocs/luci-static/resources/daeuniverse/service.js"
    replace_once(
        service,
        "function expandPattern(root, pattern) {\n\tconst path =",
        "function expandPattern(root, pattern) {\n"
        "\t// 远程规则由核心刷新，本地编辑器只修改配置中的 URL。\n"
        "\tif (/^https?:\\/\\//i.test(pattern))\n"
        "\t\treturn Promise.resolve([]);\n"
        "\tconst path =",
    )
    replace_once(
        service,
        "\t\t\t' ', this.checkButton, this.packageStatus",
        "\t\t\t' ', E('span', { 'class': 'cbi-value-description' }, '通过本仓库的一键安装脚本升级')",
    )
    replace_once(
        service,
        "\t\t\tthis.packagePolling = true;\n"
        "\t\t\tthis.checkButton.disabled = true;\n"
        "\t\t\tpoll.add(() => this.pollPackageJob(), 2);",
        "\t\t\t// 本分支不从固件源升级核心，也不启动包管理器轮询。",
    )
    acl_path = root / "luci-app-honk/root/usr/share/rpcd/acl.d/luci-app-honk.json"
    acl = json.loads(acl_path.read_text(encoding="utf-8"))
    permissions = acl["luci-app-honk"]
    for command in ["/usr/libexec/daeuniverse-pkg check *", "/usr/libexec/daeuniverse-pkg status"]:
        del permissions["read"]["file"][command]
    del permissions["write"]["file"]["/usr/libexec/daeuniverse-pkg upgrade *"]
    acl_path.write_text(json.dumps(acl, ensure_ascii=False, indent="\t") + "\n", encoding="utf-8")


if __name__ == "__main__":
    adapt(Path(sys.argv[1]))
