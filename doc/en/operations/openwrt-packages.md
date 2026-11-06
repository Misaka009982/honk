# OpenWrt 25.12 packages

本分支当前只构建 OpenWrt 25.12 `aarch64_cortex-a53` 的完整 musl 核心，提供 APK v3 与 OPKG/IPK；同时提供架构无关的 LuCI 共享库、管理页及简体中文包。其他架构与变体待稳定后再扩展，不依赖系统 Geo 数据包。

安装、架构选择、默认配置、构建产物与未执行的验收项目统一见[OpenWrt 安装说明](../../zh/operations/openwrt-packages.md)。官方 25.12 使用 APK v3，OPKG 定制固件使用 IPK，请按固件已有包管理器选择。

[复用已有构建](../../zh/operations/openwrt-packages.md#复用已有构建)前需由维护者创建指向原运行提交的 `debug.build.<run_id>` 标签，避免内置 `GITHUB_TOKEN` 创建历史标签时返回 403。完整安装包在发布前保存为保留 7 天的 artifact，发布失败时仍可下载。
