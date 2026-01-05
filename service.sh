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

rotate_log() {
    # 限制日志大小为 1MB
    MAX_SIZE=1048576
    if [ -f "$LOG_FILE" ]; then
        SIZE=$(stat -c%s "$LOG_FILE")
        if [ "$SIZE" -gt "$MAX_SIZE" ]; then
            mv "$LOG_FILE" "${LOG_FILE}.old"
            ui_log "🔄 日志已轮转（超过 1MB）。"
        fi
    fi
}

rotate_log
ui_log "--- 启动守护进程 (Rust Supervisor Mode) ---"

while [ $RETRY_COUNT -lt $MAX_RETRIES ]; do
    # 0. 检查手动停止标志 (Stop Flag)
    if [ -f "$WORKSPACE/STOP" ]; then
        ui_log "🛑 检测到停止标志 (STOP Flag)，终止守护循环。"
        break
    fi

    SBC_RS="$WORKSPACE/bin/sbc-rs"
    
    if [ ! -x "$SBC_RS" ]; then
         ui_log "❌ 严重错误: sbc-rs 二进制丢失！"
         exit 1
    fi

    # 核心启动：由 Rust 接管一切 (渲染 + 进程守护 + 信号转发)
    # service.sh 退化为简单的无限重启器
    export WORKSPACE="$WORKSPACE"
    "$SBC_RS" run \
        --config "$CONFFILE" \
        --template "$TEMPLATE" \
        -D "$WORKSPACE/var/lib/sing-box" \
        >> "$LOG_FILE" 2>&1
        
    EXIT_CODE=$?
    
    if [ $EXIT_CODE -eq 0 ]; then
        ui_log "Sing-box (Rust) 正常停止。"
        break
    else
        ui_log "Sing-box (Rust) 异常退出 (Code $EXIT_CODE)。"
        RETRY_COUNT=$((RETRY_COUNT + 1))
        sleep $RETRY_DELAY
    fi
done