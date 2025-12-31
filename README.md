# Sing-box for KernelSU

[![CI Build](https://github.com/cagedbird043/sing-box-ksu-module/actions/workflows/release.yml/badge.svg)](https://github.com/cagedbird043/sing-box-ksu-module/actions)

为 Android 设备深度定制的高性能 Sing-box 运行时环境，支持 KernelSU 模块化部署。

## 🎖️ 核心特性

| 特性 | 说明 |
|------|------|
| **云端配置同步** | 安装时自动从 CDN 拉取最新配置模板，始终保持最新状态 |
| **二进制归一化** | 所有二进制集中存放在 `/data/adb/sing-box-workspace/bin/`，模块目录极致轻量 |
| **系统级软链** | 通过 `$MODPATH/system/bin/` 建立软链接，`sbc` 命令全局可用 |
| **热更新** | `sbc update` 指令支持 OTA 增量更新，无需重启手机 |
| **凭证隔离** | `.env` 文件与配置模板分离，保护隐私不外泄 |

## 🏗️ 架构设计

```
sing-box-ksu-module/
├── bin/                    # 临时传输介质，安装后移动到 Workspace
├── system/bin/            # 软链接目录（安装时创建）
├── customize.sh           # 安装逻辑：进程停止 → 二进制归一 → 软链创建 → 云端同步
├── service.sh             # 守护进程：开机自启 + 崩溃重试
├── module.prop            # 模块元信息（CI 自动注入版本）
└── README.md              # 本文档
```

### 运行时目录结构

```
/data/adb/sing-box-workspace/
├── bin/                   # 唯一二进制存储（sing-box, sbc, envsubst）
├── etc/                   # 配置目录
│   └── config.template.json  # 云端同步的配置文件
├── var/
│   ├── lib/               # 运行时数据
│   ├── run/               # PID 文件
│   └── log/               # 日志文件
└── .env                   # 凭证文件（本地生成，不打包）
```

## 🚀 快速开始

### 1. 安装模块

1. 在 Magisk/KernelSU 中刷入 ZIP 包
2. **重启手机**（首次安装必须重启以激活软链接）
3. 重启后即可使用 `sbc` 命令

### 2. 配置订阅

```bash
# 编辑凭证文件
sbc edit

# 或手动编辑
vi /data/adb/sing-box-workspace/.env

# 示例配置
SUB_URL_1="https://your-subscription-url.com"
SUB_TYPE="auto"
```

### 3. 管理服务

| 命令 | 说明 |
|------|------|
| `sbc start` | 启动 sing-box 服务 |
| `sbc stop` | 停止服务（优雅终止） |
| `sbc restart` | 重启服务 |
| `sbc status` | 查看运行状态 |
| `sbc update` | 从云端拉取最新配置 |
| `sbc logs` | 查看实时日志 |
| `sbc edit` | 编辑 .env 凭证 |

## 📦 模块结构

### 发布包内容（ZIP）

```
sing-box-ksu-module-v1.12.14-r21.zip
├── bin/               # sing-box 二进制（来自 CI 构建）
├── system/            # 空目录（用于存放软链接）
├── customize.sh       # 安装脚本
├── service.sh         # 守护进程脚本
├── module.prop        # 模块元信息
└── README.md          # 本文档
```

**注意**：配置文件和 .env 模板不打包，由安装时从云端拉取。

### 不包含的文件

- `config.template.json` - 云端管理
- `.env.example` - 云端管理
- `CHANGELOG.md` - 仅在仓库中
- `LICENSE` - 仅在仓库中

## 🔧 高级配置

### 环境变量

编辑 `$WORKSPACE/.env`：

```bash
# 订阅配置
SUB_URL_1="https://example.com/sub"
SUB_URL_2=""                    # 支持多订阅，留空则跳过

# 解析类型 (auto, json, base64, sip008)
SUB_TYPE="auto"

# 自定义变量（可在配置模板中使用）
CUSTOM_VAR_1=""
```

### 日志查看

```bash
# 实时日志
sbc logs

# 查看最后 100 行
tail -n 100 /data/adb/sing-box-workspace/var/log/sing-box.log
```

## 🏭 组件依赖

| 组件 | 仓库 | 用途 |
|------|------|------|
| 核心二进制 | [sing-box-auto-build-ci](https://github.com/cagedbird043/sing-box-auto-build-ci) | 自动构建 android-arm64 二进制 |
| 配置模板 | [sing-box-config-templates](https://github.com/cagedbird043/sing-box-config-templates) | 移动端分流规则模板 |
| CDN 加速 | [miceworld.top](https://miceworld.top) | 全球加速分发 |

## 📄 许可证

本项目基于 [MIT License](LICENSE) 开源。

## 🙏 致谢

- [Sing-box](https://github.com/SagerNet/sing-box) - 强大的代理客户端
- [KernelSU](https://github.com/KernelSU/KernelSU) - Android root 解决方案
