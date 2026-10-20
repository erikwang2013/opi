#!/bin/sh
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# deb preinst / rpm %pre：**停掉正在运行的 opi**
# （2026-10-17 用户裁决「安装的时候中断正在运行的应用」的 Linux 等价物 —— Windows MSI
# 那边由 scripts/postprocess-windows-msi.sh 的 taskkill 承担）。
#
# 为什么需要：dpkg/rpm 升级只替换磁盘文件，**已经开着的旧进程仍跑旧代码** ——
# 2026-10-17 实测用户装完新 .deb 后设置窗左下角还是旧版本号，来源就是它。
#
# fcitx5 **不在这里动**。插件 .so 换新由 postinstall 调 restart-fcitx5.sh 做 D-Bus
# `Controller1.Restart` 热重启（同一总线上卸载/重载 addon）；安装前杀 fcitx5 曾造成
# 用户「整个键盘消失」，不再采用。`fcitx5-remote -r` 只重读配置、不换 .so（实测假绿）。
#
# 状态文件（postinstall 读完即删）：
#   /run/opi-install.state  每行 = 一个 opi 实例的完整命令行（复活用）
#
# 用 /run 不用 /tmp：/run 归 root、世界不可写 ⇒ 不存在预置符号链接的攻击面。
set -u

# 测试钩子（scripts/packaging/test-restart-fcitx5.sh）：指向假 /proc 与临时状态目录。
# **只有非 root 才认**：维护脚本在生产里永远是 root（dpkg/rpm），此时固定 /proc 与
# /run —— 不给「能控制环境变量的人」重定向 root 扫描/写入点的机会。
if [ "$(id -u)" -eq 0 ]; then
    PROC_ROOT=/proc
    STATE_DIR=/run
else
    PROC_ROOT="${OPI_PROC_ROOT:-/proc}"
    STATE_DIR="${OPI_STATE_DIR:-/run}"
fi
STATE="$STATE_DIR/opi-install.state"

# 找当前实例（comm == opi）的 pid。**不用 pgrep/pkill**：procps 在最小
# 系统上不保证存在（Debian 容器实测没有）；/proc 扫描零依赖，rpm/deb 通吃。
opi_pids() {
    for d in "$PROC_ROOT"/[0-9]*; do
        [ -r "$d/comm" ] || continue
        if [ "$(cat "$d/comm" 2>/dev/null)" = "opi" ]; then
            echo "${d##*/}"
        fi
    done
}

# ≤1.17.x 的桌面进程名。只用于升级时**停掉**旧进程，**不记录**：旧路径随新包解包
# 被删，记了 postinstall 也拉不起来（它按 `[ -x "$bin" ]` 跳过）。state 留空正好
# 走 postinstall 的「从旧版升级按自启项恢复」那条路，新落点由自启项决定。
legacy_pids() {
    for d in "$PROC_ROOT"/[0-9]*; do
        [ -r "$d/comm" ] || continue
        if [ "$(cat "$d/comm" 2>/dev/null)" = "opi-candidates" ]; then
            echo "${d##*/}"
        fi
    done
}

# 记录 cmdline（arg0 是完整路径，postinstall 靠它拉起新二进制 —— 磁盘上的文件
# 换过之后，同一路径就是新版本）。
: > "$STATE"
current_pids="$(opi_pids)"
for pid in $current_pids; do
    # /proc/<pid>/cmdline 是 NUL 分隔；转成单行空格分隔（参数都是简单 flag，够用）
    tr '\0' ' ' < "$PROC_ROOT/$pid/cmdline" 2>/dev/null >> "$STATE" || true
    printf '\n' >> "$STATE"
done

old_pids="$(legacy_pids)"
if [ -n "$current_pids" ] || [ -n "$old_pids" ]; then
    for pid in $current_pids $old_pids; do kill -TERM "$pid" 2>/dev/null || true; done
    # 优雅退出最多等 3 秒，然后强杀（与 Windows 的 taskkill /f 同一档语义）
    i=0
    while { [ -n "$(opi_pids)" ] || [ -n "$(legacy_pids)" ]; } && [ "$i" -lt 30 ]; do
        sleep 0.1
        i=$((i + 1))
    done
    for pid in $(opi_pids) $(legacy_pids); do kill -KILL "$pid" 2>/dev/null || true; done
    [ -n "$current_pids" ] && echo "opi: 已停止运行中的 opi（升级完成后按记录自动拉起）"
    [ -n "$old_pids" ] && echo "opi: 已停止旧版 opi-candidates（升级后按自启项恢复）"
fi

exit 0
