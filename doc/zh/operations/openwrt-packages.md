# OpenWrt 25.12 安装包

本分支当前只构建 **`aarch64_cortex-a53`**，稳定后再扩展其他架构。核心使用完整 `musl` / `jemalloc` 二进制，包含 Clash API、原生 API 和两套 doona UI；另提供 LuCI 管理页、共享库及简体中文包。包内不包含 Geo 数据，也不依赖 `v2ray-geoip`、`v2ray-geosite`。

官方 OpenWrt 25.12 默认使用 **APK v3**；保留 OPKG 的定制固件使用 **IPK**。两种格式分别构建，不能互相改名替代。请使用设备当前的包管理器，不要为安装 honk 切换包管理器。

## 一键安装

在 25.12 设备上以 root 执行：

```sh
curl -fL https://raw.githubusercontent.com/Misaka009982/honk/main/scripts/install.sh -o /tmp/honk-install.sh && sh /tmp/honk-install.sh
```

没有 curl 可用 `wget -O /tmp/honk-install.sh https://raw.githubusercontent.com/Misaka009982/honk/main/scripts/install.sh && sh /tmp/honk-install.sh`。命令先完整下载，再执行脚本；安装包需先在本仓库发布。本脚本参考 [kenzok8/openwrt-honk](https://github.com/kenzok8/openwrt-honk/blob/main/scripts/install.sh) 的平台检测与版本选择流程。

默认仓库为 `Misaka009982/honk`，在最近 30 个 Release 中优先选择包含本机包与 `SHA256SUMS` 的正式版，没有时选择匹配的 SemVer 预发布。不会自动选择 Debug，也不会安装其他架构或切换固件包管理器。脚本先核对下载校验和与包内名称/架构，再调用包管理器，保留完整安装日志，最后确认目标版本和核心服务文件。

| 环境变量 | 用途 |
| --- | --- |
| `HONK_VERSION=v0.1.0-beta.5` | 指定已发布的标签，需含本分支安装包；`debug` / `debug.*` 也可显式指定。 |
| `HONK_REPO=owner/repo` | 使用另一份采用相同产物命名的仓库。 |
| `HONK_PM=apk` / `opkg` | 同时安装了两种工具时显式选择固件的包管理器，默认优先 APK。 |
| `HONK_LUCI=0` | 只安装核心；默认 `1`，同时安装 LuCI 共享库、管理页和简体中文包。 |
| `GH_PROXY=https://mirror.example/` | 可选 Release 附件镜像前缀，镜像失败回退 GitHub；默认直连，API 元数据仍直连。 |

例如 `HONK_VERSION=debug.build.42221567511 sh /tmp/honk-install.sh`。默认安装四个包，逐个校验后交给包管理器；老发布只有核心时可显式设置 `HONK_LUCI=0`。LuCI 位于“服务 → honk”，提供启停、开机启动、配置编辑和日志；doona 仍是核心的独立管理面板。升级使用本仓库安装脚本，LuCI 不调用 daeuniverse 软件源升级，以免覆盖定制核心。首次安装后仍需配置并启用服务；升级保留原配置、重启已启用的服务。不是 25.12、内核低于 6.12、未知架构、找不到 Release 产物或校验失败时退出。

## 选择文件

| 包 | 架构 | 格式 |
| --- | --- | --- |
| `honk` | `aarch64_cortex-a53`，ARMv8 musl | `.apk` / `.ipk` |
| `luci-lib-daeuniverse` | APK `noarch` / IPK `all` | `.apk` / `.ipk` |
| `luci-app-honk` | APK `noarch` / IPK `all` | `.apk` / `.ipk` |
| `luci-i18n-daeuniverse-zh-cn` | APK `noarch` / IPK `all` | `.apk` / `.ipk` |

APK 固件用 `apk --print-arch` 查看架构；OPKG 固件用 `opkg print-architecture`。核心必须匹配固件包架构；LuCI 是 JavaScript、Shell 和翻译数据，架构无关，可以与 A53 核心组合。当前不构建 x86_64、其他 ARM64 标签或精简 API/stock 变体。

核心文件名为 `honk-<prefix>-openwrt-25.12-aarch64_cortex-a53.apk` / `.ipk`，LuCI 为 `<包名>-<prefix>-openwrt-25.12-noarch.apk` 或 `-all.ipk`。`prefix` 对应分支名、正式版本标签或 `debug`；以下以 Debug 附件命名举例，安装前需完成发布。

## 安装与启用

将对应的四个安装包和发布页的 `SHA256SUMS` 放在同一目录，先逐个核对校验和，例如核心 IPK：

```sh
awk '$2 == "honk-debug-openwrt-25.12-aarch64_cortex-a53.ipk"' SHA256SUMS | sha256sum -c -
```

APK v3 本地包未附加软件源签名；核对下载来源及 SHA-256 后安装：

```sh
apk update
apk add --allow-untrusted ./honk-debug-openwrt-25.12-aarch64_cortex-a53.apk \
    ./luci-lib-daeuniverse-debug-openwrt-25.12-noarch.apk \
    ./luci-app-honk-debug-openwrt-25.12-noarch.apk \
    ./luci-i18n-daeuniverse-zh-cn-debug-openwrt-25.12-noarch.apk
```

OPKG 固件使用：

```sh
opkg update
opkg install ./honk-debug-openwrt-25.12-aarch64_cortex-a53.ipk \
    ./luci-lib-daeuniverse-debug-openwrt-25.12-all.ipk \
    ./luci-app-honk-debug-openwrt-25.12-all.ipk \
    ./luci-i18n-daeuniverse-zh-cn-debug-openwrt-25.12-all.ipk
```

包管理器从当前固件软件源解决 `ca-bundle`、`kmod-veth`、TC/BPF、nftables/NFQUEUE 和 conntrack 依赖。BTF 优先使用内核自带的 `/sys/kernel/btf/vmlinux`，因此不强制依赖外部 BTF 包；若内核未提供，一键脚本会尝试从当前固件软件源安装 `vmlinux-btf`，仍缺失则停止。手动安装者需自行准备 `/usr/lib/debug/boot/vmlinux`。不从第三方下载其他内核的 BTF，也不修改软件源。

内核模块和外部 BTF 必须与当前固件内核匹配，不要使用强制忽略依赖安装。软件源没有对应组件时，应先修复固件的软件源或内核功能。

安装不自动启动服务。编辑 `/etc/honk/config.dae`，配置节点、路由、接口和需要的 API 后，再执行：

```sh
/etc/init.d/honk enable
/etc/init.d/honk start
logread -e honk
```

默认配置只直连本机流量，不设置固定 LAN 接口，也不开放管理监听器。网关流量需要配置 `global.lan_interface`；`/etc/config/honk` 的 `config_file` 可改用另一入口。启用原生 UI 时按[原生 API 配置](../reference/experimental.md#native_api)设置认证与监听地址。`dae` 与 honk 不能同时使用 `dae0`/`daens`，服务启动会检查已运行的 dae。

默认设置 `assets.geodata.route: direct`。首次需要 Geo 规则时，缺失的 GeoIP 下载 `geoip-lite.dat` 并保存为 `/etc/honk/data/geoip.dat`，Geosite 下载完整 `geosite.dat`；已有文件继续使用。无 Geo 引用且未启用原生管理预下载时不下载无用数据。详情见[首次 Geo 下载](./modular-additions.md#首次-geo-下载)。

远程 `.dae` 规则默认每 24 小时刷新。修改 `assets.rules.interval` 后执行 `/etc/init.d/honk reload` 接受配置；设为 `0` 关闭定时刷新。下载失败保留有效缓存，变化且通过校验后自动生效。

升级保留用户配置和开机设置，已启用的服务安装后重启。数据与规则缓存位于 `/etc/honk/`，加入 sysupgrade 保留清单。卸载会停止服务并移除开机链接，不主动删除运行时数据。

## 构建与同步上游

构建工作流仅接受 `workflow_dispatch` 手动触发，push、PR、标签、Release 发布和定时任务均不会自动启动构建。进入 [Actions → Release](https://github.com/Misaka009982/honk/actions/workflows/release.yml)，点击 **Run workflow** 并选择 `main`；完成后在该次运行的 artifacts 下载 `openwrt-25.12-packages`。`CI`、`Docker`、`Weekly` 与 `Reload benchmark` 同样由维护者在 Actions 中手动选择运行。

`CI` 的 `changes` 任务和 Release 的脚本预检使用临时 `v1.2.3` 文件模拟缺包、空包，日志中的“测试夹具，预期拒绝”表示自测场景。实际安装包由 Release 的 `OpenWrt 25.12 A53 APK / IPK + LuCI` 任务生成并上传；选择 `main` 构建时，A53 的 OPKG 包位于上述 artifact 内，文件名为 `honk-main-openwrt-25.12-aarch64_cortex-a53.ipk`。该 artifact 保留 1 天，请及时下载；真实缺包仍会报错并阻止上传。

需要把已成功的 `main` 构建发布到 Releases 时，进入 [Actions → Publish existing build](https://github.com/Misaka009982/honk/actions/workflows/publish-existing.yml)，选择 `main` 并填写原 Release 的运行编号 `run_id`（链接末尾的数字，例如 `42221567511`）。该入口仅提取未过期的 ARM64 完整核心和 OpenWrt 产物，发布为 `debug.build.<run_id>`，标签指向原构建提交；不重新编译核心。旧产物没有 LuCI 时，单独补建固定版本的 LuCI 包及翻译转换工具；新产物已有 LuCI 时整套校验复用。OpenWrt 核心附件使用 `honk-debug-openwrt-25.12-aarch64_cortex-a53.apk` / `.ipk`，包内版本与内容不变；一键安装时显式指定 `HONK_VERSION=debug.build.<run_id>`。若 artifacts 已过期，需先重新手动运行 Release。

需要为已有标签构建发布物时，由维护者手动指定该标签，例如 `gh workflow run release.yml --ref <tag> --repo Misaka009982/honk`。正式 `v*` 标签仍执行版本与变更记录检查并创建草稿，`debug.*` 标签仍更新 Debug 发布；分支上的手动构建只提供 artifacts，发布使用上述独立入口。

`release.yml` 只编译一份 `aarch64-unknown-linux-musl` 完整核心，停用其他架构、gnu、stock 和精简 API 变体及 DEB/RPM/Arch 打包。`openwrt` job 生成 A53 核心的 APK/IPK 和三个 LuCI 组件的双格式包，共八个安装包，并附带 LuCI 配套源码。手动构建从 `openwrt-25.12-packages` artifact 下载并用 `OPENWRT-SHA256SUMS` 校验；标签发布附带 `SHA256SUMS` 与构建来源证明，复用已有构建入口则记录原构建链接。手动与 Debug 核心包版本仍使用工作区版本加 `_alpha<run_number>` / `~alpha.<run_number>`。

打包模块为 [build-openwrt-packages.sh](../../../.github/ci/build-openwrt-packages.sh) 和 [install/openwrt](../../../install/openwrt/targets.sh)。APK 使用 SHA-256 固定的官方 OpenWrt 25.12.0 SDK 主机打包器；只提取主机工具，不重新编译 honk。IPK 采用 OPKG 的 control/data gzip-tar 格式；两个格式共用服务、配置和依赖。没有借用 nfpm 的 Alpine APK 输出。

LuCI 使用独立的 [build-openwrt-luci.sh](../../../.github/ci/build-openwrt-luci.sh) 与 [适配模块](../../../install/openwrt/luci/README.md)。页面和中文 PO 文件固定来自 daeuniverse/openwrt-packages；只在工作流中编译主机 `po2lmo` 转换翻译，不再次编译 honk。远程 include 保留在配置中交由核心处理，LuCI 只展开本地文件。

本仓库的新安装包已取消 Geo 包依赖；仅在继续使用外部 `daeuniverse/openwrt-packages` 源码配方时才需另行应用 [Geo 配方补丁](../../../install/openwrt-geodata-bootstrap.patch)。

本会话未执行构建、包管理器安装或运行测试。Linux CI 需执行完整打包与元数据检查；OpenWrt 设备需验收安装、启停、升级保留配置、所需 Geo 首次下载及远程规则自动刷新。验收时观察 `logread -e honk`，至少覆盖两轮刷新；失败时先将 `assets.rules.interval` 设为 `0` 并显式重载，必要时停止服务并恢复上一版本及配置。
