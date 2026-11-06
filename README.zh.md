<div align="center">

# honk

**用 Rust 编写的实验性 Linux 透明代理引擎。**

**[文档](https://daeuniverse.org/zh-CN/honk/)** / **[网页界面示范](https://demo.daeuniverse.org/)** / **[下载](https://github.com/Misaka009982/honk/releases)**

[English](README.md) / 简体中文

[功能概览](#功能概览) / [安装](#安装) / [网页界面](#网页界面) / [报告问题](#报告问题)

</div>

> **早期 alpha，不建议用于生产。** 可能发生破坏性变更，部分功能尚未完成，真实环境验证仍然有限。

honk 的 eBPF 数据面和配置语法参考 dae 的设计，出站组与 Clash API 参考 sing-box 的设计。它是独立实现，并非任一项目的移植。

## 本项目修改

本仓库基于上游 **`v0.1.0-beta.5`（`88d470c6`）**，扩展功能尽量集中在独立模块，方便同步上游。下载本项目产物请使用 [Misaka009982/honk Releases](https://github.com/Misaka009982/honk/releases)，上游既有发布不包含这些修改。

- **Sub-Store 兼容**：支持 URI/Base64、Clash/Mihomo 和裸节点数组；AnyTLS 的密码、SNI、ALPN、证书校验及 UDP 参数进入原生拨号模型，同时提供脱敏失败原因。
- **远程 `.dae` 规则**：HTTP(S) `include`、有效缓存和自动热重载。`assets.rules.interval` 默认 `24h`，`0` 关闭定时刷新；失败保留当前规则。
- **Geo 首次下载**：按需下载 **GeoIP lite** 与完整 **Geosite**，本地仍保存为 `geoip.dat` / `geosite.dat`；自定义来源优先。
- **OpenWrt 25.12**：构建 APK v3 与 OPKG/IPK，包含 procd 服务和配置保留，不依赖 `v2ray-*` 数据包；提供一键安装脚本。

模块接入位置与限制见[修改说明](doc/zh/operations/modular-additions.md)。本地仅完成静态核查，编译、回归测试和设备联网验收待 Linux/OpenWrt 环境执行。

**构建由维护者手动触发。** `CI`、`Release`、`Docker`、`Weekly` 和 `Reload benchmark` 仅保留 `workflow_dispatch`，提交代码、创建 PR、推送标签、发布 Release 或到达定时时间均不会自动启动构建。在 [Actions → Release](https://github.com/Misaka009982/honk/actions/workflows/release.yml) 点击 **Run workflow**、选择 `main`，即可构建并下载包含 APK/IPK 的 artifacts；手动运行 `CI` 可执行格式检查和测试。

## OpenWrt 一键安装

适用 OpenWrt / ImmortalWrt **25.12、Linux 6.12+**，支持 x86_64 与文档列出的 ARM64 架构。在路由器上以 root 执行，先完整下载脚本，下载成功后安装：

```sh
curl -fL https://raw.githubusercontent.com/Misaka009982/honk/main/scripts/install.sh -o /tmp/honk-install.sh && sh /tmp/honk-install.sh
```

没有 `curl` 时：

```sh
wget -O /tmp/honk-install.sh https://raw.githubusercontent.com/Misaka009982/honk/main/scripts/install.sh && sh /tmp/honk-install.sh
```

脚本自动识别 APK/OPKG 和包架构，优先选择带匹配安装包的正式版本，无正式版本时选择 beta/rc 等预发布，核对 SHA-256 后安装；Debug 需显式指定 `HONK_VERSION=debug`。可以执行 `HONK_VERSION=v0.1.0-beta.5 sh /tmp/honk-install.sh` 指定已经发布的版本。首次安装不启动服务；配置 `/etc/honk/config.dae` 后执行 `/etc/init.d/honk enable` 与 `/etc/init.d/honk start`。升级由包管理器保留配置，已启用服务会重启。

上面的 `main` 地址和 Release 安装包需在本分支合并并发布后可用；只有源码尚未发布安装包时，脚本会明确退出。脚本参考 [kenzok8/openwrt-honk](https://github.com/kenzok8/openwrt-honk/blob/main/scripts/install.sh) 的安装流程。更多参数、手动安装和构建说明见 [OpenWrt 安装文档](doc/zh/operations/openwrt-packages.md)。

## 功能概览

- 透明 TCP/UDP：通过 TC eBPF、`dae0`/`daens` 和编译路由处理 LAN 转发及本机发起的流量。
- 出站：SOCKS5、Shadowsocks/2022、Trojan、AnyTLS、Hysteria2、TUIC、Juicity、VMess、VLESS、`direct` 和 `block`。Trojan/VMess/VLESS 支持 H2 XHTTP（`packet-up`、`stream-up`、`stream-one`）；VMess 仅支持 TCP。详见[协议限制](doc/zh/reference/nodes.md)。
- 组策略：Selector（默认）、URLTest、LoadBalance、Fallback 和 Score。Score 使用业务观测与有界验证，详见[组策略参考](doc/zh/reference/groups.md#score-策略)。
- DNS：支持 UDP、TCP、DoT、DoH、DoQ、DoH3 上游，可经节点或组出站，并提供路由和缓存。
- 配置与控制：dae 语法、订阅、重载、Clash 兼容 REST/WebSocket API，以及 `honk-tool` CLI 工具箱。

不提供 FakeIP 引擎、完整 mihomo 兼容性或 Windows/macOS 数据面。

## 安装

从 [Releases](https://github.com/Misaka009982/honk/releases) 下载适合目标平台的归档。发布物支持 x86_64 与 aarch64 Linux，一般用途建议选择 `full`。

| 构建 | 原生 API | Clash API | 内嵌 doona 与 doona lite |
| --- | --- | --- | --- |
| `full` | 是 | 是 | 是 |
| `native-api` | 是 | 否 | 否 |
| `clash-api` | 否 | 是 | 否 |

各构建均提供静态链接的 musl 版本，`full` 另提供链接 glibc 的 `gnu` 版本。名称含 `stock` 的归档使用系统分配器而非 jemalloc。每个版本的 Downloads 一节列出校验和及构建来源证明。

Debian、Ubuntu、Fedora、openSUSE 和 Arch Linux 的软件包安装见[安装指南](https://daeuniverse.org/zh-CN/honk/installation/)。本分支的 OpenWrt APK/IPK 与一键安装见[安装文档](doc/zh/operations/openwrt-packages.md)。

自行编译请按[从源码构建](doc/zh/how-to-start.md#从源码构建)准备。发布二进制内嵌 eBPF；仅执行 `cargo build --release` 不会启用真实数据面。

按[启动指南](doc/zh/how-to-start.md)检查内核、安装程序并验证连通性。首次部署会修改网络 hook、命名空间和 sysctl，须保留带外管理通道。

## 网页界面

honk 可配合以下两类网页界面使用：

- [doona](https://github.com/Zakkaus/doona)：基于原生 API，提供运行观测、诊断与配置编辑功能。doona lite 为其轻量预览版。在线演示见[标准版](https://demo.daeuniverse.org/)与 [lite 版](https://demo.daeuniverse.org/lite/)。
- Clash 面板：基于 Clash 兼容 API，例如 [zashboard](https://github.com/Zephyruso/zashboard)。

![doona 活动页的流量图表与状态卡片](https://daeuniverse.org/doona/screenshots/zh-CN/activity-light.webp)

![doona lite 节点页](https://daeuniverse.org/doona/images/lite-nodes-light.webp)

使用 `full` 构建时，在 `experimental.native_api` 中将 `ui` 设为 `'embedded'` 即可启用 doona，设为 `'embedded-lite'` 则启用 doona lite。`native-api` 构建可将 `ui` 设为另行[安装的 doona](https://daeuniverse.org/zh-CN/doona/install) 所在目录。配置步骤见 [doona 快速入门](https://daeuniverse.org/zh-CN/doona/quick-start)。启用 Clash 面板需配置 `experimental.clash_api`，honk 首次启动时自动下载 zashboard。各设置项的说明见 [experimental 设置](doc/zh/reference/experimental.md)。

## 文档

[文档索引](doc/README.md)汇总中英文配置指南、原生与 Clash API 参考、子系统设计及运维指南。

## 版本与兼容性

- 自 `v0.1.0` 起，发布遵循 [SemVer](https://semver.org/lang/zh-CN/)；带 `-alpha.N`、`-beta.N` 或 `-rc.N` 后缀的标签为预发布。`1.0` 之前，次版本可能改变配置或行为，升级前应阅读发布说明。
- 透明代理需要 root 权限、Linux 6.12+、所需内核选项和 bpffs。详见上方链接的启动指南。
- 配置使用 [dae 语法](doc/zh/configuration.md)。修改已有部署前，请查阅 [NFQUEUE 运行说明](doc/zh/design/nfqueue.md)和 [VLESS 迁移指南](doc/zh/reference/nodes.md#从-vless_mode-迁移)。
- 原生 API 使用 `/api/v1`，其契约在 `1.0` 之前可能随次版本变化。`GET /api/v1/version` 返回契约标识。

## 开发

工作区包含配置、共享 eBPF 类型、NFQUEUE、出站、核心和工具 crate；`crates/honk-ebpf` 单独构建。详见 [Justfile](Justfile) 与[贡献政策](CONTRIBUTING.md)。

无 root 的用户态开发可使用 [mock 模式](doc/zh/how-to-start.md#无-root-开发)。`--mock-ebpf` 不会拦截流量，不能代替数据面测试。

滚动 [Debug 预发布](https://github.com/Misaka009982/honk/releases/tag/debug)在本仓库发布 `debug.*` 构建后更新。使用 release profile；保留期限与发布流程见[发布规则](.agents/rules/release.md)。

## 报告问题

本分支的代理、路由、DNS 和安装问题请提交至 [Misaka009982/honk issues](https://github.com/Misaka009982/honk/issues)；网页界面问题请提交至 [doona issues](https://github.com/Zakkaus/doona/issues)。

安全漏洞请按 [SECURITY.md](.github/SECURITY.md) 报告。

## 致谢

- [dae](https://github.com/daeuniverse/dae) / daed-rs：eBPF 透明代理谱系
- [sing-box](https://github.com/SagerNet/sing-box)：出站组与 Clash API 模式
- [daeuniverse/outbound](https://github.com/daeuniverse/outbound)：协议参考
- [juicity-rs](https://github.com/juicity/juicity-rs)（Markson Pigeonzilla Plus）：Juicity 协议参考、wire 格式对齐及真实互通测试
- [aya-rs](https://github.com/aya-rs/aya)：Rust eBPF

## 许可证

[GPL-3.0-only](LICENSE)。

```text
SPDX-License-Identifier: GPL-3.0-only
Copyright (c) 2025, glassyiris <honk@catmint.cc> and honk contributors
```

仅用于开发的 [dae 解析器 oracle](tools/dae-parse/README.md)链接了 dae（AGPL-3.0-only）。它不随发布物分发，也不链接进 `honk-core`。

## TODO

- [ ] 评估 AF_XDP 与 XDP 路径以进一步提升性能
- [ ] 在 Clash 兼容 API 之外添加 honk 专用 REST API
- [ ] 添加入站支持
- [ ] 探索 macOS 与 Windows 支持；数据面基于 Linux eBPF，这两个平台需要另一种接管流量的方式
