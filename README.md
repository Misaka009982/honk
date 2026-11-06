<div align="center">

# honk

**An experimental transparent-proxy engine for Linux, written in Rust.**

**[Documentation](https://daeuniverse.org/honk/)** / **[Web UI demo](https://demo.daeuniverse.org/)** / **[Releases](https://github.com/Misaka009982/honk/releases)**

English / [简体中文](README.zh.md)

[Capabilities](#capabilities) / [Install](#install) / [Web interface](#web-interface) / [Report problems](#report-problems)

</div>

> **Early alpha, not recommended for production.** Expect breaking changes, incomplete features, and limited real-world validation.

honk follows the design of dae's eBPF datapath and configuration syntax, and of sing-box's outbound groups and Clash API. It is an independent implementation, not a port of either project.

## 本项目修改

本仓库基于上游 **`v0.1.0-beta.5`（`88d470c6`）**，扩展功能尽量集中在独立模块，方便同步上游。下载本项目产物请使用 [Misaka009982/honk Releases](https://github.com/Misaka009982/honk/releases)，上游既有发布不包含这些修改。

- **Sub-Store 兼容**：支持 URI/Base64、Clash/Mihomo 和裸节点数组；AnyTLS 的密码、SNI、ALPN、证书校验及 UDP 参数进入原生拨号模型，同时提供脱敏失败原因。
- **远程 `.dae` 规则**：HTTP(S) `include`、有效缓存和自动热重载。`assets.rules.interval` 默认 `24h`，`0` 关闭定时刷新；失败保留当前规则。
- **Geo 首次下载**：按需下载 **GeoIP lite** 与完整 **Geosite**，本地仍保存为 `geoip.dat` / `geosite.dat`；自定义来源优先。
- **OpenWrt 25.12**：当前只构建 `aarch64_cortex-a53` 的完整核心 APK/IPK，另附 LuCI 管理页、共享库和简体中文包；保留 procd 服务和用户配置，不依赖 `v2ray-*` 数据包。

模块接入位置与限制见[修改说明](doc/zh/operations/modular-additions.md)。本地仅完成静态核查，编译、回归测试和设备联网验收待 Linux/OpenWrt 环境执行。

**构建由维护者手动触发。** `CI`、`Release`、`Docker`、`Weekly` 和 `Reload benchmark` 仅保留 `workflow_dispatch`，提交代码、创建 PR、推送标签、发布 Release 或到达定时时间均不会自动启动构建。在 [Actions → Release](https://github.com/Misaka009982/honk/actions/workflows/release.yml) 点击 **Run workflow**、选择 `main`，即可构建并下载包含 APK/IPK 的 artifacts；手动运行 `CI` 可执行格式检查和测试。

**把已有构建发布到 Releases：** 在 [Actions → Publish existing build](https://github.com/Misaka009982/honk/actions/workflows/publish-existing.yml) 选择 `main`，填写成功的 Release 运行编号 `run_id`。该入口复用未过期核心产物，发布为 `debug.build.<run_id>`；旧构建缺少 LuCI 时会单独补建页面与翻译包，不重新编译核心。安装时指定 `HONK_VERSION=debug.build.<run_id>`。选择 `main` 运行 Release 本身只生成 artifacts，不会发布。

## OpenWrt 一键安装

适用 OpenWrt / ImmortalWrt **25.12、Linux 6.12+**；当前只发布 **`aarch64_cortex-a53`**，稳定后再扩展其他架构。在路由器上以 root 执行，先完整下载脚本，下载成功后安装：

```sh
curl -fL https://raw.githubusercontent.com/Misaka009982/honk/main/scripts/install.sh -o /tmp/honk-install.sh && sh /tmp/honk-install.sh
```

没有 `curl` 时：

```sh
wget -O /tmp/honk-install.sh https://raw.githubusercontent.com/Misaka009982/honk/main/scripts/install.sh && sh /tmp/honk-install.sh
```

脚本默认安装核心及三个 LuCI 组件，进入“服务 → honk”管理；`HONK_LUCI=0` 可只安装核心。自动识别 APK/OPKG 和包架构，优先选择带完整安装包的正式版本，无正式版本时选择 beta/rc 等预发布，核对 SHA-256 后安装；Debug 需显式指定 `HONK_VERSION=debug` 或 `HONK_VERSION=debug.build.<run_id>`。首次安装不启动服务；配置 `/etc/honk/config.dae` 后执行 `/etc/init.d/honk enable` 与 `/etc/init.d/honk start`，也可通过 LuCI 操作。升级保留配置，已启用服务会重启。

安装包需先在本仓库发布；只有源码或 Actions artifacts 时，脚本会明确退出。脚本参考 [kenzok8/openwrt-honk](https://github.com/kenzok8/openwrt-honk/blob/main/scripts/install.sh) 的安装流程。更多参数、手动安装和构建说明见 [OpenWrt 安装文档](doc/zh/operations/openwrt-packages.md)。

## Capabilities

- Transparent TCP/UDP for LAN-forwarded and host-originated traffic through TC eBPF, `dae0`/`daens`, and compiled routing.
- Outbounds: SOCKS5, Shadowsocks/2022, Trojan, AnyTLS, Hysteria2, TUIC, Juicity, VMess, VLESS, `direct`, and `block`. Trojan/VMess/VLESS support H2 XHTTP (`packet-up`, `stream-up`, `stream-one`); VMess is TCP-only. See [protocol limits](doc/en/reference/nodes.md).
- Groups: Selector (the default), URLTest, LoadBalance, Fallback, and Score. Score uses business observations and bounded validation; see [group policies](doc/en/reference/groups.md#score-policy).
- DNS: UDP, TCP, DoT, DoH, DoQ, and DoH3 upstreams, optionally through a node or group, with routing and caching.
- Configuration and control: dae syntax, subscriptions, reload, a Clash-compatible REST/WebSocket API, and the `honk-tool` CLI toolbox.

There is no FakeIP engine, full mihomo parity, or Windows/macOS datapath.

## Install

当前本分支只发布 A53 使用的 ARM64 musl `full` 核心和 OpenWrt/LuCI 安装包，从 [Releases](https://github.com/Misaka009982/honk/releases) 下载。下表保留上游各变体的能力说明；其他架构、gnu、stock、精简 API 和 DEB/RPM/Arch 产物暂不构建。

| Build | Native API | Clash API | Embedded doona and doona lite |
| --- | --- | --- | --- |
| `full` | Yes | Yes | Yes |
| `native-api` | Yes | No | No |
| `clash-api` | No | Yes | No |

当前归档使用静态 musl 和 jemalloc，包含全部 API 与两套内嵌 UI。标签发布附带校验和及构建来源证明；复用已有构建的发布入口附带校验和与原构建链接。

Packages for Debian, Ubuntu, Fedora, openSUSE and Arch Linux are covered in the [installation guide](https://daeuniverse.org/honk/installation/). 本分支的 OpenWrt APK/IPK 与一键安装见[安装文档](doc/en/operations/openwrt-packages.md)。

For source builds, follow [Build from source](doc/en/how-to-start.md#build-from-source). Release binaries embed eBPF; plain `cargo build --release` does not enable the real datapath.

Follow the [startup guide](doc/en/how-to-start.md) for kernel checks, installation, and connectivity verification. Initial deployment changes network hooks, namespaces, and sysctls; keep an out-of-band management path available.

## Web interface

honk works with two kinds of web UI:

- [doona](https://github.com/Zakkaus/doona) uses the native API, which adds observations, diagnostics and configuration editing. doona lite is a lighter preview edition. Try the [demo](https://demo.daeuniverse.org/) or the [lite demo](https://demo.daeuniverse.org/lite/).
- Clash dashboards such as [zashboard](https://github.com/Zephyruso/zashboard) use the Clash-compatible API.

![doona Activity page with traffic charts and status cards](https://daeuniverse.org/doona/screenshots/en/activity-light.webp)

![doona lite Nodes page](https://daeuniverse.org/doona/images/lite-nodes-light.webp)

For doona, enable `experimental.native_api` and set `ui: 'embedded'` in a `full` build, or `ui: 'embedded-lite'` for doona lite; the [doona quick start](https://daeuniverse.org/doona/quick-start) walks through it. A `native-api` build can serve a separately [installed doona](https://daeuniverse.org/doona/install) from its directory instead. For a Clash dashboard, set `experimental.clash_api`; honk downloads zashboard on first start. The [experimental settings](doc/en/reference/experimental.md) list every field of both.

## Documentation

The [documentation index](doc/README.md) links configuration guides, native and Clash API references, subsystem designs, and operations guides in both languages.

## Versions and compatibility

- Releases follow [SemVer](https://semver.org/) from `v0.1.0`; `-alpha.N`, `-beta.N`, and `-rc.N` tags are prereleases. Before `1.0`, minor releases may change configuration or behaviour; read the release notes before upgrading.
- Transparent proxying requires root, Linux 6.12+, the required kernel options, and bpffs. See the startup guide linked above.
- Configuration uses [dae syntax](doc/en/configuration.md). See [NFQUEUE operation](doc/en/design/nfqueue.md) and [VLESS migration](doc/en/reference/nodes.md#migration-from-vless_mode) before changing an existing deployment.
- The native API uses `/api/v1`; before `1.0` its contract may change in a minor release. `GET /api/v1/version` reports the contract identity.

## Development

The workspace contains configuration, shared eBPF types, NFQUEUE, outbound, core, and tooling crates; `crates/honk-ebpf` builds separately. See the [Justfile](Justfile) and [contribution policy](CONTRIBUTING.md).

Use [mock mode](doc/en/how-to-start.md#unprivileged-development) for unprivileged userspace development. `--mock-ebpf` does not intercept traffic or test the datapath.

滚动 [Debug 预发布](https://github.com/Misaka009982/honk/releases/tag/debug)在本仓库发布 `debug.*` 构建后更新。使用 release profile；保留期限与发布流程见[发布规则](.agents/rules/release.md)。

## Report problems

本分支问题请提交至 [Misaka009982/honk issues](https://github.com/Misaka009982/honk/issues)。Report web interface problems in [doona issues](https://github.com/Zakkaus/doona/issues).

For vulnerabilities, follow [SECURITY.md](.github/SECURITY.md).

## Acknowledgments

- [dae](https://github.com/daeuniverse/dae) / daed-rs: eBPF transparent proxy lineage
- [sing-box](https://github.com/SagerNet/sing-box): outbound group and Clash API patterns
- [daeuniverse/outbound](https://github.com/daeuniverse/outbound): protocol reference
- [juicity-rs](https://github.com/juicity/juicity-rs) by Markson Pigeonzilla Plus: Juicity protocol reference, wire-format alignment, and live interop testing
- [aya-rs](https://github.com/aya-rs/aya): Rust eBPF

## License

[GPL-3.0-only](LICENSE).

```text
SPDX-License-Identifier: GPL-3.0-only
Copyright (c) 2025, glassyiris <honk@catmint.cc> and honk contributors
```

The development-only [dae parser oracle](tools/dae-parse/README.md) links dae (AGPL-3.0-only). It is never shipped or linked into `honk-core`.

## TODO

- [ ] Evaluate AF_XDP and XDP paths for further performance gains
- [ ] Add a honk-specific REST API beyond Clash compatibility
- [ ] Add inbound support
- [ ] Explore macOS and Windows support; the datapath is Linux eBPF, so these platforms need another way to capture traffic
