#!/system/bin/sh
# Mice Sing-box KSU Module - Intelligent Installer
# 重构版：云端配置同步 + 二进制归一化 + 软链化

WORKSPACE="/data/adb/sing-box-workspace"
CONFIG_URL="https://miceworld.top/sing-box-config-templates/mobile/config.template.json"
ENV_URL="https://miceworld.top/sing-box-config-templates/mobile/.env.example"

ui_print "============================================"
ui_print "   Mice Sing-box KSU Module Installer      "
ui_print "============================================"

# ============================================
# Step 1: 优雅停止现有服务
# ============================================
ui_print ""
ui_print ">>> Step 1: 正在停止服务..."

# 停止 sing-box 主进程（精准匹配避免误杀）
pkill -15 -f "$WORKSPACE/bin/sing-box" >/dev/null 2>&1 || true
# 停止 sbc 脚本进程
pkill -15 -f "sbc" >/dev/null 2>&1 || true

ui_print "    ✅ 服务已停止"

# ============================================
# Step 2: 创建 Workspace 目录结构
# ============================================
ui_print ""
ui_print ">>> Step 2: 正在创建工作空间..."
mkdir -p "$WORKSPACE/bin" "$WORKSPACE/etc" "$WORKSPACE/var/lib" "$WORKSPACE/var/run" "$WORKSPACE/var/log"
ui_print "    ✅ 目录结构创建完成"

# ============================================
# Step 3: 二进制归一化（移动而非复制）
# ============================================
ui_print ""
ui_print ">>> Step 3: 正在归集二进制文件..."

# 检查 MODPATH/bin 是否有文件
if [ -d "$MODPATH/bin" ] && [ "$(ls -A $MODPATH/bin 2>/dev/null)" ]; then
    # 移动所有文件到 Workspace（移动后原目录应为空或被删除）
    for file in $MODPATH/bin/*; do
        if [ -f "$file" ]; then
            filename=$(basename "$file")
            mv "$file" "$WORKSPACE/bin/" && chmod 755 "$WORKSPACE/bin/$filename"
            ui_print "    📦 $filename -> Workspace"
        fi
    done
    # 删除空目录
    rmdir $MODPATH/bin 2>/dev/null || rm -rf $MODPATH/bin
else
    ui_print "    ℹ️  MODPATH/bin 为空，跳过移动"
fi

# 确保 Workspace 二进制有正确权限
chmod -R 755 "$WORKSPACE/bin/" 2>/dev/null || true
ui_print "    ✅ 二进制归一化完成"

# ============================================
# Step 4: 系统级软链化
# ============================================
ui_print ""
ui_print ">>> Step 4: 正在建立系统软链接..."

# 确保目录存在
mkdir -p "$MODPATH/system/bin"

# 创建软链接（源路径 -> 目标路径）
ln -sf "$WORKSPACE/bin/sbc" "$MODPATH/system/bin/sbc" && ui_print "    🔗 sbc -> Workspace"
ln -sf "$WORKSPACE/bin/envsubst" "$MODPATH/system/bin/envsubst" && ui_print "    🔗 envsubst -> Workspace"
ln -sf "$WORKSPACE/bin/sing-box" "$MODPATH/system/bin/sing-box" && ui_print "    🔗 sing-box -> Workspace"

ui_print "    ✅ 软链接建立完成"

# ============================================
# Step 5: 云端下载配置文件
# ============================================
ui_print ""
ui_print ">>> Step 5: 正在同步云端配置..."

TIMESTAMP=$(date +%s)
DOWNLOAD_URL="${CONFIG_URL}?t=${TIMESTAMP}"

# 使用 curl -k 忽略证书问题，设置超时 10 秒
if curl -kfsSL --connect-timeout 10 --max-time 30 "$DOWNLOAD_URL" -o "$WORKSPACE/config.template.json" 2>/dev/null; then
    # 简单校验
    if grep -q "inbounds" "$WORKSPACE/config.template.json" 2>/dev/null; then
        chmod 644 "$WORKSPACE/config.template.json"
        ui_print "    ✅ 配置同步成功"
    else
        ui_print "    ⚠️  下载的配置无效，将保留现有配置"
        rm -f "$WORKSPACE/config.template.json"
    fi
else
    ui_print "    ⚠️  网络连接失败或下载超时"
    ui_print "    💡 提示: 请确保网络通畅后运行 'sbc update' 手动同步"
fi

# ============================================
# Step 6: 初始化 .env 凭证（从云端拉取模板）
# ============================================
ui_print ""
ui_print ">>> Step 6: 正在初始化环境变量..."

if [ ! -f "$WORKSPACE/.env" ]; then
    # 优先从云端下载 .env 模板
    if curl -kfsSL --connect-timeout 10 --max-time 30 "${ENV_URL}?t=${TIMESTAMP}" -o "$WORKSPACE/.env" 2>/dev/null; then
        chmod 600 "$WORKSPACE/.env"
        ui_print "    ✅ .env 模板已从云端拉取"
    else
        # 下载失败则生成最小化模板
        cat > "$WORKSPACE/.env" << 'ENVEOF'
# Mice Sing-box 环境变量配置
# 请编辑并填入以下变量：
SUB_URL_1=""
ENVEOF
        chmod 600 "$WORKSPACE/.env"
        ui_print "    ⚠️  云端拉取失败，已生成最小化模板"
        ui_print "    💡 提示: 请联网后运行 'sbc update' 更新完整配置"
    fi
    ui_print ""
    ui_print "📌 首次安装必读:"
    ui_print "   1. 请编辑: $WORKSPACE/.env"
    ui_print "   2. 填入 SUB_URL_1 等变量"
    ui_print "   3. 保存后执行: sbc restart"
else
    ui_print "    ℹ️  .env 已存在，跳过初始化"
fi

# ============================================
# 完成
# ============================================
ui_print ""
ui_print "============================================"
ui_print "   ✅ 安装完成！请重启手机以激活模块    "
ui_print "============================================"

# 检查软链接是否就绪
if [ -L "$MODPATH/system/bin/sing-box" ]; then
    ui_print ""
    ui_print "💡 如需立即使用，请重启系统"
fi
