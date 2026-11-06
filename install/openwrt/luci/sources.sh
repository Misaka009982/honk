#!/usr/bin/env bash
# 固定上游源码，防止构建时跟随 main 改变页面、权限或翻译。
DAE_LUCI_REVISION=e48b222f239e2572244c8cf495fbce79b0da7528
LUCI_TOOL_REVISION=e969939b49167516e219d465df8030d98d443d67
LUCI_PACKAGE_VERSION=2026.11.6
LUCI_PACKAGES=(luci-lib-daeuniverse luci-app-honk luci-i18n-daeuniverse-zh-cn)
