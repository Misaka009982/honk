# beta.5 模块接入与使用说明

基线为 `v0.1.0-beta.5`，提交 `88d470c662e0ee7a3de3e67afc6129f67e66c1bf`。本分支增加订阅格式适配、远程规则缓存和首次 Geo 下载，不改变现有出站协议实现。

## Sub-Store 订阅

继续使用现有 `subscription` 配置，支持 URI 列表及其 Base64 包装、Clash/Mihomo `proxies` 和 Sub-Store 裸 JSON 节点数组。推荐让 Sub-Store 输出 Clash Meta/Mihomo 格式以保留完整节点参数。

AnyTLS 适配会生成原生 `AnyTlsConfig` 并交给已有 AnyTLS 出站处理器：

- 将转换器附加的 `security=none&type=tcp` 解释为 AnyTLS 必需的 TLS。保留密码、SNI 和证书验证设置；默认不跳过证书验证。
- URI 的 `alpn`、`udp`、`anytls-network` 与空闲会话参数进入实际拨号模型，再重新派生节点 ID。会话时长接受秒数及 `s`、`m`、`h` 等原有订阅时长写法。
- Clash/Mihomo 的 `network: tcp` 与 `udp: true` 分别作为承载传输和 UDP 能力。显式 `anytls-network: tcp` 仍限制 UDP。
- 原生单节点分享链接解析保持严格；上述兼容只应用于订阅导入。明确相互矛盾的 TLS/REALITY、传输或 UDP 声明仍拒绝该条目，其余有效节点继续导入。

`fetch_failed` 保留为兼容错误码，但 provider `last_error` 和刷新 operation 增加脱敏 `details.stage`、`http_status`、`diagnostic_code`、`entry_index`（按原因提供），并显示具体的固定原因。HTTP/DNS/TLS 下载失败与节点正文解析失败可以据此区分；错误详情不包含 URL、正文或密码。

## 远程规则

仅支持文件模式 `--store file`，通过原有 `include` 声明：

```dae
include {
    'https://rules.example.com/block.dae'
    'local-rules.dae'
}

assets {
    rules {
        interval: 24h
    }
}

routing {
    fallback: direct
}
```

远程正文示例：

```dae
routing {
    domain(suffix: ads.example.com) -> block
    dip(192.0.2.0/24) -> direct
}
```

远程文件只允许 `routing` 块，不接受嵌套 `include`、节点、订阅、DNS、全局或管理设置。其他规则格式，例如 Clash `payload`、纯域名列表、sing-box `.srs`，需要先转换成该格式。

启动和显式配置重载（SIGHUP / 原生 API reload）会直连刷新，使用配置中的 bootstrap DNS 和 bypass mark。每个 URL 的重定向链共用 30 秒预算，最多 5 次；拒绝 HTTPS 降级、凭据 URL 和不符合现有订阅重定向策略的跳转。

与 Mihomo 的定时更新使用效果类似，`assets.rules.interval` 默认为 `24h`（86400 秒），支持秒数及 `s`、`m`、`h`，`0` 只关闭定时刷新，启动和手动重载仍刷新。间隔从上次刷新尝试完成后计算，错过的周期不补跑；规则内容未变化时不写缓存、不激活新配置。内容变化并通过校验后自动热重载，沿用现有连接保留机制。所有远程 URL 共用该间隔，使用 honk 的 `.dae` 语法，不接受 Mihomo 的 `rule-providers` YAML/MRS。

定时刷新与手动配置操作在同一所有者内串行处理，不会重叠下载。发现本地配置存在尚未重载的修改时，本轮定时刷新跳过；先显式重载才能接受本地修改。修改间隔也通过正常重载生效，无需重启进程。

缓存位于入口配置同级的 `.honk-remote-rules/<URL 派生 UUID>.dae`，文件名不含查询参数。下载或候选校验失败时，重新校验并使用旧缓存；首次没有可用缓存则启动/重载失败。远程与本地配置共同受 32 个来源、8 MiB 总量限制，重复 URL 同样拒绝。入口自身的规则先合并，再按 `include` 声明顺序合并，沿用已有规则优先级。

离线解析、验证和源编辑只读取缓存，不联网。因此首次添加远程 URL 时，应编辑磁盘上的入口配置后重启或显式重载；尚无缓存的 URL 不能通过离线 full 验证。缓存来源在原生 API 中只读，修改应发生在远程源或本地 `include` 声明处。数据库配置模式不提供此下载接入。

下载和校验通过后逐文件持久化；磁盘写入错误可能留下部分已更新缓存，不能保证跨文件回滚。失败重载保留正在运行的配置。缓存目录不得使用符号链接。

## 首次 Geo 下载

此模块在启用 `native-api` 的构建中可用（完整发行构建与原生 API 构建均包含）。在启动路由前补齐缺失文件，已有本地或系统 Geo 文件继续使用。

| 资产 | 内置远程文件 | 本地文件 |
| --- | --- | --- |
| GeoIP | `geoip-lite.dat` | `global.data_dir/geoip.dat` |
| Geosite | 完整 `geosite.dat` | `global.data_dir/geosite.dat` |

默认源来自 MetaCubeX `meta-rules-dat` 的 `release` 分支，依次尝试 raw.githubusercontent.com 与 fastly.jsdelivr.net。手动和自动更新共用相同默认源，GeoIP 不会在后续默认更新时切回完整版。配置文件与 API 已保存的自定义 URL 保持原有优先级；旧版已安装的完整 GeoIP 文件不会被首次下载主动替换。

首次下载发生在运行时路由可用之前，必须显式允许直连：

```dae
assets {
    geodata {
        route: direct
    }
}
```

有状态数据库时读取内置或已存储 URL；没有状态数据库时需要显式设置 `assets.geodata.geoip` 和 `geosite`。启用原生 API、`config_write` 与自动更新时可预下载未引用的缺失资产；否则仅补齐当前流量/DNS 规则要求的数据。非必需预下载失败只记录警告；必需资产无法下载、缺失规则所需分类、配置要求 routing/group 出口而本地又无数据时，启动报错。不会绕过指定出口自行改为直连。

下载沿用既有 HTTP、TLS 和大小限制，默认验证 `.sha256sum`；仅 404 表示服务端没有校验文件。验证关闭时遵循已存储设置。文件解析及当前所需分类检查通过后，以不覆盖已有文件的方式写入实际 data_dir。Lite 不保证包含所有完整 GeoIP 分类；需要额外分类时使用包含它们的自定义 URL。

OpenWrt 的 `v2ray-geoip` / `v2ray-geosite` 依赖位于独立仓库 `daeuniverse/openwrt-packages`。本仓库提供 [配方补丁](../../../install/openwrt-geodata-bootstrap.patch)，去掉强制依赖和旧符号链接，并在默认配置中设置 `assets.geodata.route: direct`。需让配方构建本分支代码后，在该打包仓库应用补丁；仅删除依赖而继续构建未修改的 beta.5 不具备首次下载能力。已有安装升级时保留的用户配置需自行补上直连设置。

## 同步上游时的接入点

模块使用既有解析器、HTTP 下载器、出站注册器与配置协调器，没有新增第三方依赖。

| 模块 | 独立文件 | 现有文件接入点 |
| --- | --- | --- |
| Sub-Store 格式适配 | `crates/honk-core/src/subscription/compat.rs`、`compat/anytls.rs` | `subscription.rs` 的 URI、结构化列表和 Clash 条目入口 |
| 订阅失败详情 | `subscription/failure.rs` | 下载错误类型、supervisor 的失败观察、`native_api/providers.rs` 的错误投影 |
| 远程规则语义 | `crates/honk-config/src/parser/remote.rs` | `parser/mod.rs` 的 include 展开/正文边界，以及 `parser/sources.rs` 的加载参数 |
| 远程规则下载与缓存 | `crates/honk-core/src/remote_rules.rs`、`remote_rules/cache.rs` | `lib.rs` 启动与非 native reload、`native_api/config/coordinator.rs` reload；`native_api/config.rs` 只读来源判定 |
| 远程规则定时刷新 | `remote_rules/schedule.rs`、`native_api/config/coordinator/remote_rules.rs` | `assets.rs` 与 `parser/assets.rs` 的 `rules.interval`；协调器和 SIGHUP 循环各一个定时分支 |
| Geo 首次下载 | `native_api/geodata/bootstrap.rs` | `lib.rs` 状态库准备之后、`native_api/geodata.rs` 公开原有直连下载函数 |
| Geo 默认文件 | `native_api/geodata/sources.rs` 的 `DEFAULT_URLS` | GeoIP 两个默认 URL 改为 `geoip-lite.dat` |
| OpenWrt 配方 | `install/openwrt-geodata-bootstrap.patch` | 外部打包仓库的 `honk/Makefile` 与 `honk/files/config.dae` |
| OpenWrt 双格式安装包 | `.github/ci/build-openwrt-packages.sh`、`check-openwrt-packages.sh`、`install/openwrt/` | `release.yml` 的 `openwrt` job、校验和与附件；`release-assets.sh` 下载表。用法见 [25.12 安装说明](./openwrt-packages.md) |

同步时复制独立模块及相应测试，再合并表中接入点。由于接入了现有来源权威和刷新生命周期，无法保证跨版本只复制模块就生效；若上游调整这些接口，需要核对入口签名及所有者约束。

## 验证交接

本会话按 Windows 环境约束只执行静态核查，不编译、运行测试或安装依赖。已检查差异空白、UTF-8 编码、文档文件链接、统一补丁格式、调用方与条件编译路径；本机未发现 Rust/rustfmt 工具链，未执行编译器类型检查或格式化检查。以下回归用例已编写但尚未运行：

- `subscription::compat`：URI、Base64、Clash、裸 JSON、ALPN、UDP、凭据、节点 ID 与有效兄弟条目；本地 TLS/AnyTLS 实际拨号和数据回显。
- `subscription::failure`、`native_api::providers::tests`：HTTP 状态、原始条目位置、错误脱敏和保留已加载节点。
- `remote_include`、`remote_rules::tests`：合并顺序、冷缓存拒绝、断网回退、非法远程正文与符号链接拒绝；内容相同不改缓存，本地待重载编辑拒绝自动应用。
- `remote_rules::schedule`、`native_api::config::tests::remote_rules`：间隔变更/关闭、首次延迟、不补跑、定时变化进入实际路由、无效更新保留规则、手动刷新串行与停机取消。
- `native_api::geodata::bootstrap`：lite 下载与校验、本地 `geoip.dat` 命名、已有文件不被覆盖、校验和不符不落盘、损坏数据与缺分类拒绝；已有来源设置测试覆盖配置 URL 与已保存设置的优先级。

本分支新增远程规则定时刷新与 OpenWrt 双格式安装器的回归用例已编写，尚未执行。安装器用例只使用本地元数据与文件，覆盖正式版优先、beta 回退、未知架构、坏校验和及重复校验条目；打包完整性用例覆盖缺包与空包。Windows 环境没有可用 Rust/rustfmt/Bash 工具链，静态核查不等同于编译、格式检查或 shell 解析检查。

在已配好项目工具链的 Linux 构建环境，按仓库现有流程执行格式检查、`cargo test -p honk-config` 与 `cargo test -p honk-core --features native-api`，并补做真实订阅节点 TCP/UDP 和 OpenWrt 首次启动/断网重启验收。未获得现场失败订阅正文，不能把当前兼容项断言为所有现场失败的共同原因。
