# OpenWrt 25.12 packages

本分支同时生成 OpenWrt 25.12 的 APK v3 和 OPKG/IPK 安装包，覆盖 `x86_64`、`aarch64_generic`、`aarch64_cortex-a53`、`aarch64_cortex-a72`、`aarch64_cortex-a76`。包使用完整 musl 构建，包含两套 API 与嵌入 UI，不依赖系统 Geo 数据包。

安装、架构选择、默认配置、构建产物与未执行的验收项目统一见[OpenWrt 安装说明](../../zh/operations/openwrt-packages.md)。官方 25.12 使用 APK v3，OPKG 定制固件使用 IPK，请按固件已有包管理器选择。
