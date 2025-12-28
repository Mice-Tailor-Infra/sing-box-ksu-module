#!/system/bin/sh
# Mice System Tools - Workspace Builder

WORKSPACE="/data/adb/sing-box-workspace"
SBC_PATH="/system/bin/sbc"

ui_print "--------------------------------------"
ui_print "    Mice Sing-box Workspace Builder   "
ui_print "--------------------------------------"

# 0. 热更新前置：如果有旧版服务在运行，先停止
if [ -x "$SBC_PATH" ]; then
    ui_print "- 正在执行热停机..."
    $SBC_PATH stop >/dev/null 2>&1
fi

# 1. 创建符合 FHS 规范的目录架构
ui_print "- 正在初始化工作空间目录..."
mkdir -p $WORKSPACE/bin
mkdir -p $WORKSPACE/etc
mkdir -p $WORKSPACE/var/lib
mkdir -p $WORKSPACE/var/run
mkdir -p $WORKSPACE/var/log

ui_print "- 正在自动部署组件..."

# 2. 部署核心组件
cp -f $MODPATH/bin/sing-box $WORKSPACE/bin/
chmod 755 $WORKSPACE/bin/sing-box

ui_print "- 部署 envsubst 渲染引擎..."
cp -f $MODPATH/system/bin/envsubst $WORKSPACE/bin/
chmod 755 $WORKSPACE/bin/envsubst

ui_print "- 部署 sbc 控制脚本..."
cp -f $MODPATH/system/bin/sbc $WORKSPACE/bin/
chmod 755 $WORKSPACE/bin/sbc

cp -f $MODPATH/etc/config.template.json $WORKSPACE/etc/
chmod 644 $WORKSPACE/etc/config.template.json

# 3. 智能凭证初始化
if [ ! -f "$WORKSPACE/.env" ]; then
    ui_print "- 正在初始化凭证文件 .env ..."
    cp -f $MODPATH/.env.example $WORKSPACE/.env
    chmod 600 $WORKSPACE/.env
    ui_print "   [OK] 已为您自动创建 $WORKSPACE/.env"
else
    ui_print "- 发现已存在的 .env 凭证，保留用户原始配置。"
fi

# 另外保留一份 example 备查
cp -f $MODPATH/.env.example $WORKSPACE/.env.example

# 4. 安全审计与指引
ui_print " "
ui_print "📌 后续操作指引:"
ui_print "   请使用 MT 管理器编辑: $WORKSPACE/.env"
ui_print "   填入 SUB_URL_1 等变量后，执行 su -c sbc restart"
# 5. 热更新后置：重新拉起服务
# 只有在 sbc 已经部署成功的情况下才执行
if [ -x "$WORKSPACE/bin/sbc" ]; then
    ui_print "- 正在热启动 sing-box 服务..."
    # 异步执行，不阻塞安装进程
    sh $MODPATH/service.sh >/dev/null 2>&1 &
    ui_print "   [OK] 服务已重载，无需重启手机。"
fi

ui_print "--------------------------------------"