#!/bin/sh
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# deb postinst / rpm %post：安装/升级成功后
#   1) 调 restart-fcitx5.sh 热重启 fcitx5（D-Bus Controller1.Restart）；换 .so 只能
#      靠进程级重载 —— `fcitx5-remote -r` / ReloadAddonConfig 只重读配置（实测假绿）；
#   2) 把 preinstall 停掉的 opi-candidates 按原参数拉起。
# 顺序：先 fcitx5 再状态栏 —— 状态栏经 D-Bus 找 fcitx5，反过来起会白起。
#
# helper 为什么在 /usr/libexec：nfpm 的 scripts: 每一项都是**单文件嵌入**，同目录的
# restart-fcitx5.sh 不会跟着进包；它由 nfpm contents 显式安装（见 scripts/nfpm.yaml）。
# 源码树里直接跑本脚本（测试）时用同目录那份；只有非 root 才认环境变量覆盖，
# 生产里维护脚本恒为 root，路径固定。
#
# opi-candidates 以**桌面用户**身份拉起：包脚本是 root，GUI 不能以 root 跑 ⇒ 用
# `runuser -u`。桌面用户取 `SUDO_USER`（`sudo apt install` / `sudo dpkg -i` 的标准
# 来源）。**没有 SUDO_USER**（商店/软件中心类 GUI 安装常如此）⇒ 跳过拉起并说明：
#   · 设置窗本来就是用户手动开的，开一次就是新版本；
#   · 状态栏有会话级 autostart（/etc/xdg/autostart），下次登录自然回来。
#   拉起时显式传会话总线环境（XDG_RUNTIME_DIR / DBUS_SESSION_BUS_ADDRESS，按目标
#   用户 uid 推导）：装包脚本的环境里没有这两个变量，缺了状态栏就连不上 fcitx5
#   （opi-ctl 报 UnknownObject）。已知边界（如实记）：如果升级前在跑的是**状态栏**，
#   窗口环境（DISPLAY/XAUTHORITY）仍来自 root 脚本的继承值 —— 拉起失败只是进程
#   直接退出（无窗口、无副作用），下次登录 autostart 仍会带回。
#
# 本脚本恒 exit 0：热重启与拉起失败都只记日志，不阻断安装。
set -u

# 测试钩子：见 preinstall.sh 同名注释（只有非 root 才认；生产恒为 /proc 与 /run）。
if [ "$(id -u)" -eq 0 ]; then
    PROC_ROOT=/proc
    STATE_DIR=/run
    AUTOSTART_FILE=/etc/xdg/autostart/opi-statusbar.desktop
    RUN_ROOT=/run/user
else
    PROC_ROOT="${OPI_PROC_ROOT:-/proc}"
    STATE_DIR="${OPI_STATE_DIR:-/run}"
    AUTOSTART_FILE="${OPI_AUTOSTART_FILE:-/etc/xdg/autostart/opi-statusbar.desktop}"
    RUN_ROOT="${OPI_RUN_ROOT:-/run/user}"
fi
STATE="$STATE_DIR/opi-install.state"

# 升级判定：deb postinst = `configure <旧版本>`；rpm %post = `2`。首次安装分别是
# `configure`（无第二个参数）与 `1` —— 迁移兜底只对升级生效。
is_upgrade=0
if [ "${1:-}" = configure ] && [ -n "${2:-}" ]; then
    is_upgrade=1
elif [ "${1:-}" = 2 ]; then
    is_upgrade=1
fi

# 旧版 opt-in 路径（OPI_RESTART_FCITX5=1）可能留下块格式状态文件；本版不再读它，
# 清掉，免得下一次升级把它误当成新状态。
rm -f "$STATE_DIR/opi-install.fcitx5"

# ---- 1) fcitx5 热重启（见 restart-fcitx5.sh 头注释；失败只记日志）----
if [ "$(id -u)" -eq 0 ]; then
    HELPER=/usr/libexec/opi/restart-fcitx5.sh
else
    HELPER="${OPI_RESTART_FCITX5_HELPER:-$(dirname "$0")/restart-fcitx5.sh}"
fi
if [ -f "$HELPER" ]; then
    sh "$HELPER" || true
else
    echo "opi: 找不到 fcitx5 热重启脚本（$HELPER）—— 跳过；请手动重启 fcitx5 或重新登录"
fi

# ---- 2) opi-candidates（原有逻辑）----
user="${SUDO_USER:-}"
[ "$user" = root ] && user=""

if [ ! -s "$STATE" ]; then
    # 旧包 → 本版的过渡：旧版 prerm 在 preinst **之前**就杀了实例，preinst 记录不到，
    # state 为空。仅升级时按系统自启项把状态栏拉回来；首次安装不替用户启动。
    # 已知边界：rpm 的旧包 %preun 在新包 %post **之后**跑，仍会杀掉这里拉起的实例
    #（见 docs/install/linux.md §7），只能等下次登录恢复。
    if [ "$is_upgrade" -eq 1 ]; then
        if [ -z "$user" ]; then
            echo "opi: 从旧版升级：确定不了桌面用户（无 SUDO_USER）—— 跳过状态栏自动恢复；重新登录可恢复"
        else
            exec_line="$(sed -n 's/^Exec=//p' "$AUTOSTART_FILE" 2>/dev/null | head -n 1)"
            if [ -n "$exec_line" ]; then
                # 自启项由本包生成（无空格路径 + --statusbar）；带空格的路径不支持。
                # shellcheck disable=SC2086
                set -- $exec_line
                bin="$1"; shift
                if [ -x "$bin" ]; then
                    uid="$(id -u "$user")"
                    runuser -u "$user" -- env "XDG_RUNTIME_DIR=$RUN_ROOT/$uid" \
                        "DBUS_SESSION_BUS_ADDRESS=unix:path=$RUN_ROOT/$uid/bus" \
                        "$bin" "$@" >/dev/null 2>&1 &
                    echo "opi: 从旧版升级：已按状态栏自启项恢复（$user）"
                else
                    echo "opi: 从旧版升级：状态栏程序不存在（$bin）—— 跳过自动恢复"
                fi
            else
                echo "opi: 从旧版升级：找不到状态栏自启项（$AUTOSTART_FILE）—— 跳过自动恢复"
            fi
        fi
    fi
    rm -f "$STATE"
    exit 0
fi

if [ -z "$user" ]; then
    rm -f "$STATE"
    echo "opi: 升级前有实例在运行，但确定不了桌面用户（无 SUDO_USER）——跳过自动拉起；下次登录自然恢复"
    exit 0
fi

uid="$(id -u "$user")"
while IFS= read -r line; do
    # 拆成 arg0 + 参数（参数都是无空格的 flag；带空格的参数不支持，见 preinstall）
    # shellcheck disable=SC2086
    set -- $line
    [ $# -ge 1 ] || continue
    bin="$1"
    [ -x "$bin" ] || continue
    shift
    runuser -u "$user" -- env "XDG_RUNTIME_DIR=$RUN_ROOT/$uid" \
        "DBUS_SESSION_BUS_ADDRESS=unix:path=$RUN_ROOT/$uid/bus" \
        "$bin" "$@" >/dev/null 2>&1 &
done < "$STATE"

rm -f "$STATE"
exit 0
