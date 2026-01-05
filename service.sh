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

# 核心启动：由 Rust 接管一切 (渲染 + 进程守护 + 信号转发 + 重启循环 + 日志轮转)
export WORKSPACE="$WORKSPACE"
export LOG_FILE="$LOG_FILE"

exec "$WORKSPACE/bin/sbc-rs" run \
    --config "$CONFFILE" \
    --template "$TEMPLATE" \
    -D "$WORKSPACE/var/lib/sing-box" \
    >> "$LOG_FILE" 2>&1