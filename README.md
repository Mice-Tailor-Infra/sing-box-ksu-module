# Sing-box for KernelSU (Optimized)

[![CI Build](https://github.com/cagedbird043/sing-box-ksu-module/actions/workflows/release.yml/badge.svg)](https://github.com/cagedbird043/sing-box-ksu-module/actions)

为 Android 设备深度定制的高性能 Sing-box 运行时环境。

## 🎖️ 核心特性

- **Unix-like Workspace**: 基于 `/data/adb/sing-box-workspace` 建立符合 FHS 规范的目录结构。
- **Hot Update**: 支持 `customize.sh` 热切换逻辑，刷入即生效，无需重启手机。
- **Ultra Performance**: 默认开启 **MTU 9000** 调优，配合 `exclude_package` 物理级绕过，保障网络稳定性。
- **AWK envsubst**: 集成轻量级渲染引擎，实现配置模板与本地凭证 (`.env`) 的彻底解耦。
- **SBC Command**: 全局 `sbc` 命令，像操作 Linux 服务器一样管理你的手机代理。

## 🏗️ 架构

- **二进制工厂**: [sing-box-auto-build-ci](https://github.com/cagedbird043/sing-box-auto-build-ci)
- **配置模板**: [sing-box-config-templates](https://github.com/cagedbird043/sing-box-config-templates)
- **CDN 加速**: [miceworld.top](https://miceworld.top)
