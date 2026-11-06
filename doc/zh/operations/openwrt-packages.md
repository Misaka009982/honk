# OpenWrt 25.12 安装包

本分支在原有 Release 构建之外增加 OpenWrt 安装包，复用完整 `musl` / `mimalloc` 二进制，包含 Clash API、原生 API 和两套 doona UI。包内不包含 Geo 数据，也不依赖 `v2ray-geoip`、`v2ray-geosite`。

官方 OpenWrt 25.12 默认使用 **APK v3**；保留 OPKG 的定制固件使用 **IPK**。两种格式分别构建，不能互相改名替代。请使用设备当前的包管理器，不要为安装 honk 切换包管理器。

## 一键安装

在 25.12 设备上以 root 执行：

```sh
curl -fL https://raw.githubusercontent.com/Misaka009982/honk/main/scripts/install.sh -o /tmp/honk-install.sh && sh /tmp/honk-install.sh
```

没有 curl 可用 `wget -O /tmp/honk-install.sh https://raw.githubusercontent.com/Misaka009982/honk/main/scripts/install.sh && sh /tmp/honk-install.sh`。命令先完整下载，再执行脚本；上面的 `main` 地址需合并本分支后使用，安装包需先在本仓库发布。本脚本参考 [kenzok8/openwrt-honk](https://github.com/kenzok8/openwrt-honk/blob/main/scripts/install.sh) 的平台检测与版本选择流程。

默认仓库为 `Misaka009982/honk`，在最近 30 个 Release 中优先选择包含本机包与 `SHA256SUMS` 的正式版，没有时选择匹配的 SemVer 预发布。不会自动选择 Debug，也不会安装其他架构或切换固件包管理器。脚本先核对下载校验和与包内名称/架构，再调用包管理器，保留完整安装日志，最后确认目标版本和核心服务文件。

| 环境变量 | 用途 |
| --- | --- |
| `HONK_VERSION=v0.1.0-beta.5` | 指定已发布的标签，需含本分支安装包；`debug` / `debug.*` 也可显式指定。 |
| `HONK_REPO=owner/repo` | 使用另一份采用相同产物命名的仓库。 |
| `HONK_PM=apk` / `opkg` | 同时安装了两种工具时显式选择固件的包管理器，默认优先 APK。 |
| `GH_PROXY=https://mirror.example/` | 可选 Release 附件镜像前缀，镜像失败回退 GitHub；默认直连，API 元数据仍直连。 |

例如 `HONK_VERSION=debug sh /tmp/honk-install.sh`。脚本不安装 LuCI 插件或外部 Geo 数据包，面板使用核心内嵌 doona。首次安装后仍需配置并启用服务；升级保留原配置、重启已启用的服务。不是 25.12、内核低于 6.12、未知架构、找不到 Release 产物或校验失败时退出。

## 选择文件

| 设备包架构 | 二进制 | 文件后缀 |
| --- | --- | --- |
| `x86_64` | x86_64 musl | `.apk` / `.ipk` |
| `aarch64_generic` | ARMv8 musl | `.apk` / `.ipk` |
| `aarch64_cortex-a53` | ARMv8 musl | `.apk` / `.ipk` |
| `aarch64_cortex-a72` | ARMv8 musl | `.apk` / `.ipk` |
| `aarch64_cortex-a76` | ARMv8 musl | `.apk` / `.ipk` |

APK 固件用 `apk --print-arch` 查看架构；OPKG 固件用 `opkg print-architecture`。选择与固件包架构完全匹配的文件，ARM64 各标签共用基线 ARMv8 二进制，不使用 `all` 绕过架构检查。本分支不新增 32 位 ARM/MIPS 编译。

文件名为 `honk-<tag>-openwrt-25.12-<arch>.apk` 或 `.ipk`，例如 `honk-v0.1.0-beta.5-openwrt-25.12-x86_64.apk`。这只是命名示例：既有 beta.5 发布不会自动增加本分支产物，需构建本分支。包管理器中的包名为 `honk`，与 OpenWrt 既有 honk 包一致。

## 安装与启用

将对应安装包和发布页的 `SHA256SUMS` 放在同一目录，先核对校验和。例如：

```sh
awk '$2 == "honk-v0.1.0-beta.5-openwrt-25.12-x86_64.apk"' SHA256SUMS | sha256sum -c -
```

APK v3 本地包未附加软件源签名；核对下载来源及 SHA-256 后安装：

```sh
apk update
apk add --allow-untrusted ./honk-v0.1.0-beta.5-openwrt-25.12-x86_64.apk
```

OPKG 固件使用：

```sh
opkg update
opkg install ./honk-v0.1.0-beta.5-openwrt-25.12-x86_64.ipk
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

`release.yml` 的独立 `openwrt` job 在原有二进制构建后运行，正式、Debug、手动构建均生成五种架构标签的两种格式，共十个包。手动构建从 Actions 的 `openwrt-25.12-packages` artifact 下载，并使用其中的 `OPENWRT-SHA256SUMS` 校验；正式及 Debug 发布将包加入附件、`SHA256SUMS` 和构建来源证明。手动与 Debug 包的版本使用工作区版本加 `_alpha<run_number>` / `~alpha.<run_number>`，避免把开发包伪装为正式版本。

打包模块为 [build-openwrt-packages.sh](../../../.github/ci/build-openwrt-packages.sh) 和 [install/openwrt](../../../install/openwrt/targets.sh)。APK 使用 SHA-256 固定的官方 OpenWrt 25.12.0 SDK 主机打包器；只提取主机工具，不重新编译 honk。IPK 采用 OPKG 的 control/data gzip-tar 格式；两个格式共用服务、配置和依赖。没有借用 nfpm 的 Alpine APK 输出。

本仓库的新安装包已取消 Geo 包依赖；仅在继续使用外部 `daeuniverse/openwrt-packages` 源码配方时才需另行应用 [Geo 配方补丁](../../../install/openwrt-geodata-bootstrap.patch)。

本会话未执行构建、包管理器安装或运行测试。Linux CI 需执行完整打包与元数据检查；OpenWrt 设备需验收安装、启停、升级保留配置、所需 Geo 首次下载及远程规则自动刷新。验收时观察 `logread -e honk`，至少覆盖两轮刷新；失败时先将 `assets.rules.interval` 设为 `0` 并显式重载，必要时停止服务并恢复上一版本及配置。
