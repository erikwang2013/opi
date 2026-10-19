#!/bin/sh
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# 安装/升级后热重启 fcitx5：D-Bus `org.fcitx.Fcitx.Controller1.Restart`。
#
# 为什么是它（2026-10-19 本机私有 dbus + 真 fcitx5 实测）：
#   · Restart 让 fcitx5 卸载并重新加载全部 addon（日志 `Unloading addon …` →
#     `Loaded addon …`），磁盘上的新 .so 因此生效；会话环境（总线/显示）保留。
#   · `fcitx5-remote -r` 与 `ReloadAddonConfig` 只重读配置、**不换 .so**（实测假绿）。
#   · `fcitx5 -d --replace` 是先杀再起，曾造成用户「整个键盘消失」—— 不再使用。
#   ⚠️ 实测 Restart 是**自重启**：旧 pid 会消失、新 pid 接管同一个总线名（不是字面
#   同进程）。这不是 `-d --replace` 的杀-起竞态；重启由 fcitx5 自己完成。
#
# 用户发现：SUDO_USER → PKEXEC_UID（pkexec / 软件中心）→ DOAS_USER →
# 扫 /run/user/*/bus。只在该 uid 确有**活着的** fcitx5 进程、**且该总线已有**
# `org.fcitx.Fcitx5` 所有者时才调用；否则静默跳过 —— 绝不在会话总线上盲调
# （无所有者时 D-Bus 激活文件会把缺会话环境的 fcitx5 拉起来，形成双实例）。
# 预检用 GetNameOwner 取唯一名（:1.N），Restart 直接打在该唯一名上：预检与调用
# 之间 fcitx5 恰好退出时只会得到调用失败，不会触发 D-Bus 激活。
#
# 恒 exit 0：安装脚本不能因热重启失败而回滚；失败只记一行日志。
# 应急关闭：OPI_NO_RESTART_FCITX5=1。
#
# 单独用法：
#   scripts/packaging/restart-fcitx5.sh          # 普通用户：重启自己的 fcitx5
#   sudo scripts/packaging/restart-fcitx5.sh     # root：按 SUDO_USER / 已登录用户发现
set -u

# 测试钩子（scripts/packaging/test-restart-fcitx5.sh）：指向假 /proc 与假 /run/user。
# **只有非 root 才认**：生产里维护脚本恒为 root，固定真实路径 —— 不给「能控制
# 环境变量的人」重定向 root 扫描点的机会。
if [ "$(id -u)" -eq 0 ]; then
    PROC_ROOT=/proc
    RUN_ROOT=/run/user
else
    PROC_ROOT="${OPI_PROC_ROOT:-/proc}"
    RUN_ROOT="${OPI_RUN_ROOT:-/run/user}"
fi

log() { echo "opi: $*"; }

opi_uid_of_user() {
    id -u "$1" 2>/dev/null || true
}

# 活着的 fcitx5（comm 精确匹配；僵尸不算：本机实测有 14 个 [fcitx5] <defunct>）
opi_live_fcitx5_pids() { # $1=uid
    for d in "$PROC_ROOT"/[0-9]*; do
        [ -r "$d/comm" ] || continue
        [ "$(cat "$d/comm" 2>/dev/null)" = "fcitx5" ] || continue
        st="$(awk '/^State:/{print substr($2, 1, 1); exit}' "$d/status" 2>/dev/null)"
        case "$st" in
            ""|Z*) continue ;;
        esac
        [ "$(awk '/^Uid:/{print $2; exit}' "$d/status" 2>/dev/null)" = "$1" ] \
            && echo "${d##*/}"
    done
}

opi_env_of_pid() { # $1=pid $2=key
    # 用 cat 读而不是 `<` 重定向：读不了时（root 在无 CAP_SYS_PTRACE 的容器里）
    # 重定向错误由 shell 直接打到 stderr，2>/dev/null 盖不住，会污染安装日志。
    cat "$PROC_ROOT/$1/environ" 2>/dev/null \
        | tr '\0' '\n' | sed -n "s/^$2=//p" | head -n 1 | tr -d '\r'
}

opi_any_live_fcitx5() {
    for d in "$PROC_ROOT"/[0-9]*; do
        [ -r "$d/comm" ] || continue
        [ "$(cat "$d/comm" 2>/dev/null)" = "fcitx5" ] || continue
        st="$(awk '/^State:/{print substr($2, 1, 1); exit}' "$d/status" 2>/dev/null)"
        case "$st" in
            ""|Z*) continue ;;
        esac
        return 0
    done
    return 1
}

# 以目标用户身份跑一条 D-Bus 命令（root 时 runuser；同 uid 时直接跑）
opi_as_user() { # $1=user $2=runtime $3=addr, rest = command
    user=$1; runtime=$2; addr=$3; shift 3
    if [ "$(id -u)" -eq 0 ]; then
        runuser -u "$user" -- env "XDG_RUNTIME_DIR=$runtime" \
            "DBUS_SESSION_BUS_ADDRESS=$addr" "$@"
    else
        env "XDG_RUNTIME_DIR=$runtime" "DBUS_SESSION_BUS_ADDRESS=$addr" "$@"
    fi
}

# 该总线上 fcitx5 的唯一名（:1.N）；没有所有者时返回 1。GetNameOwner 是纯查询，
# 不会激活服务。对唯一名调用后续方法同样不触发激活 —— 这是预检与 Restart 之间
# fcitx5 恰好退出时的安全边界。
opi_bus_fcitx5_owner() { # $1=user $2=runtime $3=addr
    if [ "$DBUS_TOOL" = busctl ]; then
        out="$(opi_as_user "$1" "$2" "$3" busctl --user call \
            org.freedesktop.DBus /org/freedesktop/DBus \
            org.freedesktop.DBus GetNameOwner s org.fcitx.Fcitx5 2>/dev/null)"
        owner="$(printf '%s\n' "$out" | sed -n 's/^s "\(.*\)"$/\1/p')"
    else
        out="$(opi_as_user "$1" "$2" "$3" gdbus call --session \
            --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
            --method org.freedesktop.DBus.GetNameOwner org.fcitx.Fcitx5 2>/dev/null)"
        owner="$(printf '%s\n' "$out" | sed -n "s/^('\(.*\)',)$/\1/p")"
    fi
    [ -n "$owner" ] || return 1
    printf '%s\n' "$owner"
}

opi_restart_on_bus() { # $1=user $2=runtime $3=addr
    user=$1; runtime=$2; addr=$3

    # 调 Restart 前先取唯一名。没有 owner 时 D-Bus 会按激活文件拉起一个新实例
    # （缺会话环境、双输入法）—— 那正是要避免的形态。
    owner="$(opi_bus_fcitx5_owner "$user" "$runtime" "$addr")" || {
        log "无法在会话总线上确认 org.fcitx.Fcitx5 所有者（用户 $user）—— 跳过重启，避免激活新实例"
        return 0
    }

    if [ "$DBUS_TOOL" = busctl ]; then
        opi_as_user "$user" "$runtime" "$addr" busctl --user call \
            "$owner" /controller \
            org.fcitx.Fcitx.Controller1 Restart >/dev/null 2>&1
    else
        opi_as_user "$user" "$runtime" "$addr" gdbus call --session \
            --dest "$owner" --object-path /controller \
            --method org.fcitx.Fcitx.Controller1.Restart >/dev/null 2>&1
    fi || {
        log "fcitx5 热重启失败（用户 $user）—— 请手动重启 fcitx5 或重新登录，新插件才会生效"
        return 0
    }

    # Restart 是自重启：旧 pid 会消失。等总线名重新出现（最多 3 秒），
    # 用 GetNameOwner 而不是调 fcitx5 自己的方法 —— 后者可能把服务激活起来。
    i=0
    while [ "$i" -lt 30 ]; do
        if opi_bus_fcitx5_owner "$user" "$runtime" "$addr" >/dev/null; then
            log "已热重启 fcitx5（用户 $user）"
            return 0
        fi
        sleep 0.1
        i=$((i + 1))
    done
    log "fcitx5 热重启后没有回来（用户 $user）—— 请重新登录恢复输入法"
    return 0
}

# 一个 uid：只重启该 uid 活着的 fcitx5 所用的总线（按地址去重）
opi_restart_for_uid() { # $1=uid
    uid=$1
    pids="$(opi_live_fcitx5_pids "$uid")"
    if [ -z "$pids" ]; then
        return 0
    fi
    user="$(id -nu "$uid" 2>/dev/null || true)"
    if [ -z "$user" ]; then
        log "检测到 fcitx5（uid $uid），但查不到对应用户 —— 跳过热重启"
        return 0
    fi

    if [ -z "$DBUS_TOOL" ]; then
        log "找不到 busctl/gdbus，无法热重启 fcitx5 —— 请手动重启 fcitx5 或重新登录"
        return 0
    fi

    seen=""
    for pid in $pids; do
        addr="$(opi_env_of_pid "$pid" DBUS_SESSION_BUS_ADDRESS)"
        if [ -z "$addr" ]; then
            # environ 里读不到总线时，退回标准会话总线（系统未导出该变量时）
            if [ -e "$RUN_ROOT/$uid/bus" ]; then
                addr="unix:path=$RUN_ROOT/$uid/bus"
            else
                log "fcitx5（pid $pid，用户 $user）没有会话总线 —— 跳过这个实例"
                continue
            fi
        fi
        runtime="$(opi_env_of_pid "$pid" XDG_RUNTIME_DIR)"
        [ -n "$runtime" ] || runtime="$RUN_ROOT/$uid"

        # 同一总线只重启一次（同一用户可能有多个 fcitx5 实例，也可能共享总线）
        case "
$seen" in
            *"
$addr"*) continue ;;
        esac
        seen="$seen$addr
"

        log "正在热重启 fcitx5（用户 $user，pid $pid）"
        opi_restart_on_bus "$user" "$runtime" "$addr"
    done
}

# ---- 发现用户 ----
if [ "${OPI_NO_RESTART_FCITX5:-0}" = "1" ]; then
    log "已按 OPI_NO_RESTART_FCITX5=1 跳过 fcitx5 热重启"
    exit 0
fi

DBUS_TOOL=""
if command -v busctl >/dev/null 2>&1; then
    DBUS_TOOL=busctl
elif command -v gdbus >/dev/null 2>&1; then
    DBUS_TOOL=gdbus
fi

if [ "$(id -u)" -ne 0 ]; then
    # 普通用户直接跑：目标就是自己
    opi_restart_for_uid "$(id -u)"
    exit 0
fi

CANDIDATES=""
add_candidate() {
    case "${1:-}" in
        ''|0|*[!0-9]*) return 0 ;;
    esac
    case " $CANDIDATES " in
        *" $1 "*) ;;
        *) CANDIDATES="$CANDIDATES $1" ;;
    esac
}

[ -n "${SUDO_USER:-}" ] && [ "$SUDO_USER" != root ] \
    && add_candidate "$(opi_uid_of_user "$SUDO_USER")"
[ -n "${PKEXEC_UID:-}" ] && add_candidate "$PKEXEC_UID"
[ -n "${DOAS_USER:-}" ] && [ "$DOAS_USER" != root ] \
    && add_candidate "$(opi_uid_of_user "$DOAS_USER")"

if [ -n "$CANDIDATES" ]; then
    for uid in $CANDIDATES; do
        opi_restart_for_uid "$uid"
    done
    exit 0
fi

# 没有 SUDO_USER 等信息（软件中心 / pkexec 类安装）：从已登录会话总线反推 uid，
# 再逐个确认该 uid 真有活着的 fcitx5。没有会话总线就没有可重启的对象。
found_bus=0
for bus in "$RUN_ROOT"/[0-9]*/bus; do
    [ -e "$bus" ] || continue
    uid="${bus%/bus}"
    uid="${uid##*/}"
    case "$uid" in
        ''|0|*[!0-9]*) continue ;;
    esac
    found_bus=1
    opi_restart_for_uid "$uid"
done

if [ "$found_bus" -eq 0 ] && opi_any_live_fcitx5; then
    log "检测到 fcitx5，但确定不了桌面用户 —— 跳过热重启；请手动重启 fcitx5 或重新登录"
fi

exit 0
