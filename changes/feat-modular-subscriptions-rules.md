Added

- 兼容 Sub-Store 的 AnyTLS、Clash/Mihomo 与裸节点数组，保留实际连接参数并补充脱敏订阅失败原因。
- 文件配置支持远程 `.dae` 路由规则和有效缓存，默认每 24 小时刷新并在变化通过校验后自动生效；`assets.rules.interval: 0` 关闭定时刷新。
- 启动时按需下载 GeoIP lite 和完整 Geosite，提供外部 OpenWrt 配方去依赖补丁。
- 构建增加 OpenWrt 25.12 APK v3 与 OPKG/IPK 安装包，包含 procd 服务、配置保留及发布校验和。
- 提供自动识别包管理器、架构与可用发布的一键安装脚本，校验 SHA-256 后安装，并在 README 说明本分支改动与安装入口。
