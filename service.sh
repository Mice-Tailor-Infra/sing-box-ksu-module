#!/system/bin/sh
# Mice System Tools - Runner (Systemd-like Auto-Restart)

WORKSPACE="/data/adb/sing-box-workspace"
BIN="$WORKSPACE/bin/sing-box"
CONFFILE="$WORKSPACE/etc/config.json"
TEMPLATE="$WORKSPACE/config.template.json"
LOG_FILE="$WORKSPACE/var/log/sing-box.log"

# 检查是否已在运行，防止重复启动
if pgrep -f "$BIN" > /dev/null; then
    exit 0
fi

RETRY_COUNT=0
MAX_RETRIES=4
RETRY_DELAY=15 # 稍微拉长一点，给 Android 系统网络准备时间

ui_log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $1" >> "$LOG_FILE"
}

ui_log "--- 启动守护进程 ---"

while [ $RETRY_COUNT -lt $MAX_RETRIES ]; do
    ui_log "开始第 $((RETRY_COUNT + 1)) 次尝试..."

    # 1. 渲染配置
    if [ -f "$WORKSPACE/.env" ]; then
        set -a
        . "$WORKSPACE/.env"
        set +a
    fi

    # 1. 渲染配置 (Strict Rust Mode)
    # 相信 Rust 核心：不再提供 envsubst 回退，因为新版 JSON 模板与旧版 shell 替换不兼容
    SBC_RS="$WORKSPACE/bin/sbc-rs"
    if [ -x "$SBC_RS" ] && [ -f "$TEMPLATE" ]; then
        ui_log "正在渲染配置..."
        "$SBC_RS" render --template "$TEMPLATE" --output "$CONFFILE" >> "$LOG_FILE" 2>&1
        if [ $? -ne 0 ]; then
             ui_log "❌ 配置渲染失败 (Rust Core Error)，跳过本次启动。"
             sleep $RETRY_DELAY
             RETRY_COUNT=$((RETRY_COUNT + 1))
             continue
        fi
    else
        ui_log "❌ 严重错误: 找不到核心组件 ($SBC_RS) 或 模板丢失。"
        sleep $RETRY_DELAY
        RETRY_COUNT=$((RETRY_COUNT + 1))
        continue
    fi

    # 2. 运行内核 (不使用 exec 以便捕获状态)
    "$BIN" run -D "$WORKSPACE/var/lib" -c "$CONFFILE" >> "$LOG_FILE" 2>&1
    
    # 获取退出状态码
    EXIT_CODE=$?

    # 3. 判定退出状态
    if [ $EXIT_CODE -eq 0 ]; then
        ui_log "Sing-box 正常停止 (Exit 0)。"
        break
    else
        RETRY_COUNT=$((RETRY_COUNT + 1))
        ui_log "Sing-box 异常退出 (Exit $EXIT_CODE)。"
        
        if [ $RETRY_COUNT -lt $MAX_RETRIES ]; then
            ui_log "$RETRY_DELAY 秒后重试 ($RETRY_COUNT/$MAX_RETRIES)..."
            sleep $RETRY_DELAY
        else
            ui_log "已达到最大重试次数 ($MAX_RETRIES)，放弃启动。"
        fi
    fi
done