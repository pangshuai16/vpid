#!/bin/bash
# run.sh - 自动配置 WSL2 X11 显示并启动 vpid
# 用法: ./run.sh [debug|release]

set -e

# 检测 WSL2 环境
if ! grep -qi microsoft /proc/version; then
    echo "未检测到 WSL2 环境，使用本机 DISPLAY=$DISPLAY"
    exec ./target/$1/vpid
fi

# WSL2: 自动从 resolv.conf 获取宿主机 IP 并设置 DISPLAY
HOST_IP=$(grep nameserver /etc/resolv.conf | awk '{print $2}' | head -1)
if [ -z "$HOST_IP" ]; then
    echo "错误: 无法获取 WSL2 宿主机 IP"
    exit 1
fi

export DISPLAY="$HOST_IP:0"
echo "WSL2 检测到，DISPLAY=$DISPLAY"

# X11 认证（如果 VcXsrv 配置了访问控制）
if [ -f ~/.Xauthority ]; then
    export XAUTHORITY=~/.Xauthority
fi

# Qt 软件渲染后备（GPU 加速失败时使用）
export QT_QUICK_BACKEND=${QT_QUICK_BACKEND:-software}
export QSG_RHI_BACKEND=${QSG_RHI_BACKEND:-null}
export LIBGL_ALWAYS_SOFTWARE=1

# 验证 X server 连通性
if ! command -v xdpyinfo >/dev/null 2>&1; then
    echo "提示: 安装 xdpyinfo 可测试 X 连接: sudo dnf install xorg-x11-utils"
else
    if ! xdpyinfo >/dev/null 2>&1; then
        echo "警告: 无法连接 X server ($DISPLAY)，请确认 VcXsrv 已在 Windows 端启动"
        exit 1
    fi
fi

echo "启动 vpid ($1)..."
exec ./target/$1/vpid
