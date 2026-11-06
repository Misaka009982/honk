# OpenWrt 25.12 packages

本分支当前只构建 OpenWrt 25.12 `aarch64_cortex-a53` 的完整 musl 核心，提供 APK v3 与 OPKG/IPK；同时提供架构无关的 LuCI 共享库、管理页及简体中文包。其他架构与变体待稳定后再扩展，不依赖系统 Geo 数据包。

安装、架构选择、默认配置、构建产物与未执行的验收项目统一见[OpenWrt 安装说明](../../zh/operations/openwrt-packages.md)。官方 25.12 使用 APK v3，OPKG 定制固件使用 IPK，请按固件已有包管理器选择。
