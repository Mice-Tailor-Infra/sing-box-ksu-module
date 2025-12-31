# Architecture & Engineering Standards

## 1. Filesystem Philosophy (文件系统哲学)

- **Single Source of Truth**: 二进制文件（`sing-box`, `sbc`, `envsubst`）必须且只能物理存在于 `/data/adb/sing-box-workspace/bin/`。
- **Symlink Strategy**: 模块目录 (`$MODPATH`) 不应包含任何物理二进制文件。所有对外暴露的可执行命令（如 `/system/bin/sbc`）必须通过**软链接**指向 Workspace。
- **Ephemeral Installer**: 安装包 (ZIP) 仅作为传输介质。安装脚本 (`customize.sh`) 负责将资产搬运至 Workspace，随后必须清理 `$MODPATH` 中的冗余文件，保持模块目录的极致轻量。

## 2. Configuration Strategy (配置策略)

- **Cloud-Native**: 模块不携带默认配置。安装时必须通过 `curl` 从 CDN (`miceworld.top`) 实时拉取最新模板。
- **Atomic Updates**: 配置文件更新必须使用 `.tmp` 下载 + `mv` 覆盖的原子操作，防止读取残缺文件。

## 3. Build & Release (构建与发布)

- **Lean Artifacts**: 发布包 (`.zip`) 严禁包含 `CHANGELOG.md`, `LICENSE`, `.git` 等非运行时文件。只包含核心脚本和二进制。
- **Dynamic Metadata**: `module.prop` 中的版本信息必须在 CI 构建时动态注入，源码中保持占位符。

## 4. Execution Safety (执行安全)

- **Process Management**: 在覆盖二进制前，必须使用 `pkill -15` 优雅终止正在运行的进程，防止 `Text file busy` 错误。
- **Error Handling**: 网络请求必须包含重试逻辑或明确的错误提示，严禁静默失败。

## 5. AI-Assisted Development (AI 协作规范)

### 5.1 Critical Domain Knowledge (关键领域知识)

| 知识项 | 说明 | 来源 |
|--------|------|------|
| `versionCode` 类型限制 | 必须是 int32，不能使用 `date +%Y%m%d%H` 等超长数字，否则安卓模块安装时崩溃 | 教训: 2026-01-01 r21 重构 |
| `module.prop` 自动注入 | `version` 和 `versionCode` 由 CI 从 CHANGELOG.md 动态提取，源码中用占位符 `${DISPLAY_VER}` / `${V_CODE}` | 现有架构 |
| 强制推送安全 | 使用 `--force-with-lease` 而非 `--force`，避免覆盖他人的远程提交 | Git 最佳实践 |
| Commit 合并 | 用 `commit --amend` 将改动合并到上一个 commit，保持历史整洁 | 教训: 2026-01-01 r21 重构 |

### 5.2 Review Points for AI-Generated Code (AI 代码审查要点)

当 AI 生成代码后，开发者必须重点检查：

1. **网络请求**: 是否使用 `-k` 忽略证书？是否设置 `--connect-timeout` 和 `--max-timeout` 超时？下载失败时是否继续安装而非 `exit 1`？
2. **软链接方向**: 是否为 `ln -sf $WORKSPACE/bin/xxx $MODPATH/system/bin/xxx`（源路径在前，目标路径在后）？
3. **构建产物**: ZIP 命令是否排除了 `.git`、`*.md`、`LICENSE` 等非运行时文件？
4. **权限设置**: 二进制是否设置了正确的 `chmod 755` 权限？敏感文件如 `.env` 应为 `chmod 600`？
5. **错误处理**: 所有可能失败的命令（如 `curl`、`mv`）是否有兜底逻辑或明确提示？

### 5.3 Learning Process (学习记录)

| 日期 | 版本 | 学到的内容 | 产出 |
|------|------|-----------|------|
| 2026-01-01 | r21 重构 | Android module.prop 的 versionCode 是 int32 类型，有长度限制；模块版本号由 CI 自动从 CHANGELOG.md 提取注入；Git 强制推送应使用 `--force-with-lease` | ARCHITECTURE_SKILLS.md 本节内容 |

### 5.4 Effective Prompt Patterns (高效提示词模式)

```
# 推荐格式（结构化指令）
任务目标：XXX
约束条件：
1. XXX
2. XXX
审查要点：
- XXX
- XXX

# 不推荐
帮我改一下xxx
```

**关键原则**: 给 AI 越多的上下文和约束条件，产出质量越高。每次重构后应更新本文档，将新学到的经验传承给未来的 AI 协作。
