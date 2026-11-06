Fixed

- 为成功的 main Release 构建增加手动复用发布入口，保留原核心提交与校验，避免分支构建成功后没有可安装的 Release 附件。

- 稳定前将 Release 核心构建收窄为 A53 所需的 ARM64 musl full，提供 A53 APK/IPK 与三个 LuCI 组件的双格式包；一键安装默认包含 LuCI 管理页、共享库和简体中文。
