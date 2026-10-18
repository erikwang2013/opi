#!/bin/sh
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# deb postinst / rpm %post：安装/升级成功后
#   1) 按 preinstall 记录的会话环境把 fcitx5 拉起来（**换 .so 的唯一办法**是进程重启，
#      见 preinstall.sh 头注）；
#   2) 把 preinstall 停掉的 opi-candidates 按原参数拉起。
# 顺序：先 fcitx5 再状态栏 —— 状态栏经 D-Bus 找 fcitx5，反过来起会白起。
#
# 以**桌面用户**身份拉起：包脚本是 root，GUI 不能以 root 跑 ⇒ 用 `runuser -u`。
# 桌面用户取 `SUDO_USER`（`sudo apt install` / `sudo dpkg -i` 的标准来源）。
# **没有 SUDO_USER**（商店/软件中心类 GUI 安装常如此）⇒ 静默跳过并说明：
#   · 设置窗本来就是用户手动开的，开一次就是新版本；
#   · 状态栏有会话级 autostart（/etc/xdg/autostart），下次登录自然回来；
#   · fcitx5 由会话 autostart（org.fcitx.Fcitx5.desktop）拉起，下次登录取新 .so。
# 已知边界（如实记）：如果升级前在跑的是**状态栏**，本次尽力拉起时环境变量
# （DISPLAY/XAUTHORITY）来自 root 脚本的继承值 —— 拉起失败只是进程直接退出
# （无窗口、无副作用），下次登录 autostart 仍会带回；真机验证待做。
#
# fcitx5 的失败一律**不阻断安装**（本脚本恒 exit 0）：拿不到状态/环境、找不到
# 二进制、runuser 失败、重启后看不到进程 —— 都只记一行日志，下次登录自然恢复。
set -u

# 测试钩子：见 preinstall.sh 同名注释（只有非 root 才认；生产恒为 /proc 与 /run）。
if [ "$(id -u)" -eq 0 ]; then
    PROC_ROOT=/proc
    STATE_DIR=/run
else
    PROC_ROOT="${OPI_PROC_ROOT:-/proc}"
    STATE_DIR="${OPI_STATE_DIR:-/run}"
fi
STATE="$STATE_DIR/opi-install.state"
FSTATE="$STATE_DIR/opi-install.fcitx5"

# 按 uid 找活着的 fcitx5（僵尸不算：本机实测有 14 个 [fcitx5] <defunct>）。
# 只用于「重启后到底有没有起来」的尽力而为判断 —— -d 会先 fork，退出码 0 只代表
# 启动命令发出去了；同 uid 另有实例时会掩盖失败，那种情况本来就少见。
opi_fcitx5_alive() {
    for d in "$PROC_ROOT"/[0-9]*; do
        [ -r "$d/comm" ] || continue
        [ "$(cat "$d/comm" 2>/dev/null)" = "fcitx5" ] || continue
        st="$(awk '/^State:/{print substr($2, 1, 1); exit}' "$d/status" 2>/dev/null)"
        case "$st" in
            ""|Z*) continue ;;
        esac
        [ "$(awk '/^Uid:/{print $2; exit}' "$d/status" 2>/dev/null)" = "$1" ] && return 0
    done
    return 1
}

# 每条 fcitx5 记录调用一次（记录已解析进 f_* 全局变量，结束时清空）。
# **不用 eval、不拼 shell**：值只作为 `env KEY=VALUE` 的单个参数传给 runuser。
opi_restart_fcitx5() {
    if [ -z "$f_user" ]; then
        :    # 空记录 / 文件尾
    elif [ "$f_user" != "$user" ] || [ -z "$f_uid" ] || [ -z "$f_home" ] \
         || [ -z "$f_dbus" ] || { [ -z "$f_display" ] && [ -z "$f_wayland" ]; }; then
        echo "opi: fcitx5 状态不完整或与当前安装者不一致 —— 跳过这条自动重启；下次登录自然恢复"
    else
        bin="$(command -v fcitx5 2>/dev/null || true)"
        if [ -z "$bin" ]; then
            echo "opi: 找不到 fcitx5 —— 跳过自动重启；下次登录自然恢复"
        else
            set -- env "HOME=$f_home" "USER=$f_user" "LOGNAME=$f_user" \
                PATH=/usr/local/bin:/usr/bin:/bin
            [ -n "$f_display" ] && set -- "$@" "DISPLAY=$f_display"
            [ -n "$f_wayland" ] && set -- "$@" "WAYLAND_DISPLAY=$f_wayland"
            [ -n "$f_runtime" ] && set -- "$@" "XDG_RUNTIME_DIR=$f_runtime"
            [ -n "$f_cfg" ] && set -- "$@" "XDG_CONFIG_HOME=$f_cfg"
            [ -n "$f_data" ] && set -- "$@" "XDG_DATA_HOME=$f_data"
            [ -n "$f_lang" ] && set -- "$@" "LANG=$f_lang"
            [ -n "$f_lcall" ] && set -- "$@" "LC_ALL=$f_lcall"
            set -- "$@" "DBUS_SESSION_BUS_ADDRESS=$f_dbus"
            [ -n "$f_xauth" ] && set -- "$@" "XAUTHORITY=$f_xauth"

            # --replace：preinstall 已经停过，但万一它又被人/autostart 拉起，
            # 这里也要保证最终跑的是**新** .so，而不是「已经有一个所以不启动」。
            if runuser -u "$f_user" -- "$@" "$bin" -d --replace >/dev/null 2>&1; then
                i=0
                while [ "$i" -lt 20 ] && ! opi_fcitx5_alive "$f_uid"; do
                    sleep 0.1
                    i=$((i + 1))
                done
                if opi_fcitx5_alive "$f_uid"; then
                    echo "opi: 已按原会话环境重启 fcitx5（用户 $f_user，原 pid $f_pid）"
                else
                    echo "opi: fcitx5 重启后没看到存活进程 —— 下次登录自然恢复"
                fi
            else
                echo "opi: fcitx5 自动重启失败 —— 下次登录自然恢复"
            fi
        fi
    fi

    f_pid=""; f_user=""; f_uid=""; f_home=""
    f_display=""; f_wayland=""; f_dbus=""; f_xauth=""
    f_runtime=""; f_cfg=""; f_data=""; f_lang=""; f_lcall=""
}

if [ ! -s "$STATE" ] && [ ! -s "$FSTATE" ]; then
    rm -f "$STATE" "$FSTATE"
    exit 0
fi

user="${SUDO_USER:-}"
if [ -z "$user" ] || [ "$user" = root ]; then
    rm -f "$STATE" "$FSTATE"
    echo "opi: 升级前有实例/输入法在运行，但确定不了桌面用户（无 SUDO_USER）——跳过自动拉起；下次登录自然恢复"
    exit 0
fi

# ---- 1) fcitx5（块格式：空行 = 一条记录结束）----
if [ -s "$FSTATE" ] && [ "${OPI_RESTART_FCITX5:-0}" = "1" ]; then
    f_pid=""; f_user=""; f_uid=""; f_home=""
    f_display=""; f_wayland=""; f_dbus=""; f_xauth=""
    f_runtime=""; f_cfg=""; f_data=""; f_lang=""; f_lcall=""
    while IFS= read -r line || [ -n "$line" ]; do
        case "$line" in
            "")                         opi_restart_fcitx5 ;;
            pid=*)                      f_pid="${line#pid=}" ;;
            user=*)                     f_user="${line#user=}" ;;
            uid=*)                      f_uid="${line#uid=}" ;;
            HOME=*)                     f_home="${line#HOME=}" ;;
            DISPLAY=*)                  f_display="${line#DISPLAY=}" ;;
            WAYLAND_DISPLAY=*)          f_wayland="${line#WAYLAND_DISPLAY=}" ;;
            DBUS_SESSION_BUS_ADDRESS=*) f_dbus="${line#DBUS_SESSION_BUS_ADDRESS=}" ;;
            XAUTHORITY=*)               f_xauth="${line#XAUTHORITY=}" ;;
            XDG_RUNTIME_DIR=*)          f_runtime="${line#XDG_RUNTIME_DIR=}" ;;
            XDG_CONFIG_HOME=*)          f_cfg="${line#XDG_CONFIG_HOME=}" ;;
            XDG_DATA_HOME=*)            f_data="${line#XDG_DATA_HOME=}" ;;
            LANG=*)                     f_lang="${line#LANG=}" ;;
            LC_ALL=*)                   f_lcall="${line#LC_ALL=}" ;;
        esac
    done < "$FSTATE"
    # 写侧总会补空行；这里再收一次尾，防手写/截断的文件丢最后一条
    opi_restart_fcitx5
    rm -f "$FSTATE"
fi

# ---- 2) opi-candidates（原有逻辑）----
if [ -s "$STATE" ]; then
    while IFS= read -r line; do
        # 拆成 arg0 + 参数（参数都是无空格的 flag；带空格的参数不支持，见上）
        # shellcheck disable=SC2086
        set -- $line
        [ $# -ge 1 ] || continue
        bin="$1"
        [ -x "$bin" ] || continue
        shift
        runuser -u "$user" -- "$bin" "$@" >/dev/null 2>&1 &
    done < "$STATE"
fi

rm -f "$STATE"
exit 0
