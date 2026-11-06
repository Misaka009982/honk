# LuCI 独立打包模块

构建入口为 `.github/ci/build-openwrt-luci.sh`，固定来源见 `sources.sh`。页面和翻译来自 [daeuniverse/openwrt-packages](https://github.com/daeuniverse/openwrt-packages)，原项目声明 GPL-3.0-or-later；本目录适配代码采用相同许可证。翻译转换使用固定 OpenWrt LuCI 提交的 `po2lmo` 主机工具（Apache-2.0），工具不随安装包分发。完整 GPL 文本位于仓库根目录 `LICENSE`，安装包包含该文本和本说明。

提供 `luci-lib-daeuniverse`、`luci-app-honk`、`luci-i18n-daeuniverse-zh-cn`，分别生成 `all.ipk` 和 `noarch.apk`。主核心仍按 CPU 架构打包。

集中适配点：

- `adapt.py` 跳过 HTTP(S) include 的本地展开，防止将远程 URL 当成本机文件；URL 仍在配置编辑器中修改，由核心下载和定时刷新。
- 包升级按钮与轮询停用，RPC ACL 移除升级操作；显示已装版本的 `daeuniverse-pkg` 同时支持 APK/OPKG。更新请使用本仓库一键安装脚本，避免固件源覆盖定制核心。
- `postinst` 刷新 LuCI 菜单/RPC 缓存并登记已安装的中文翻译，不启动 honk。

发布物同时包含 `honk-luci-source-<prefix>.tar.gz`，保存上游页面原文、版本记录和适配脚本。更新上游固定提交时，需核对 `adapt.py` 的唯一匹配和 LuCI API；匹配漂移会终止打包，不会跳过适配。

本地仅做静态核查；包构建、LuCI 页面、权限、服务启停和中文显示由维护者在手动 CI/OpenWrt 设备上验证。
