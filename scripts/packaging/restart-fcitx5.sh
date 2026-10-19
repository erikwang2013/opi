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
#   ⚠️ 2026-10-20 Deepin 实测：Restart 也可能**退出后不再回来**（DDE 经
#   app-DDE-org.fcitx.Fcitx5@*.service 起 fcitx5，没有 fcitx5.service）。所以重启后
#   总线无所有者时，按抓取到的原会话环境直接以目标用户拉起 `fcitx5 -d`（见下）。
#
# 只认**标准会话总线** `$RUN_ROOT/<uid>/bus`（生产即 /run/user/<uid>/bus）。进程
# environ 里可能是私有/测试总线（如 dbus-run-session 的 /tmp/dbus-*）；在那里调
# Restart 既换不掉桌面上的 .so，又可能把测试实例退出 —— 一律忽略。
#
# 用户发现：SUDO_USER → PKEXEC_UID（pkexec / 软件中心）→ DOAS_USER →
# 扫 /run/user/*/bus。只在该 uid 确有**活着的** fcitx5 进程、**且标准总线已有**
# `org.fcitx.Fcitx5` 所有者时才调用；否则静默跳过 —— 绝不在会话总线上盲调
# （无所有者时 D-Bus 激活文件会把缺会话环境的 fcitx5 拉起来，形成双实例）。
# 预检用 GetNameOwner 取唯一名（:1.N），Restart 直接打在该唯一名上：预检与调用
# 之间 fcitx5 恰好退出时只会得到调用失败，不会触发 D-Bus 激活。
#
# 成功判据是 GetNameOwner 出现**不同的唯一名**（同一总线上的唯一名不会复用），
# 最多等 12s（慢词典加载）。超时后：
#   · 旧所有者仍在 → Restart 没生效：只记日志，不强杀；
#   · 无所有者且标准总线上没有活着的 fcitx5 进程 → 以目标用户身份 + 原会话环境
#     （DISPLAY / WAYLAND_DISPLAY / XAUTHORITY / XDG_SESSION_TYPE / …）拉起
#     `fcitx5 -d`。**不通过 D-Bus 激活**：那会绕过会话环境并形成双实例。
#     ⚠️ 环境必须在调 Restart **之前**抓取：Restart 成功后旧 pid 就没了，
#     /proc/<旧 pid>/environ 不可读 —— 等到 fallback 再读只会拿到空显示环境；
#   · 无所有者但仍有活进程 → 视为正在启动，绝不重复拉起。
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
# 环境变量的人」重定向 root 扫描点的机会。等待次数同理（只影响耗时，不影响安全）。
if [ "$(id -u)" -eq 0 ]; then
    PROC_ROOT=/proc
    RUN_ROOT=/run/user
    WAIT_TICKS=120          # 12s：慢词典加载
    LAUNCH_WAIT_TICKS=80    # 8s
else
    PROC_ROOT="${OPI_PROC_ROOT:-/proc}"
    RUN_ROOT="${OPI_RUN_ROOT:-/run/user}"
    WAIT_TICKS="${OPI_WAIT_TICKS:-120}"
    LAUNCH_WAIT_TICKS="${OPI_WAIT_TICKS:-80}"
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

# 该 uid 的标准会话总线地址；没有这个 socket 时返回 1（私有/测试总线不算）。
opi_session_bus_addr() { # $1=uid
    [ -e "$RUN_ROOT/$1/bus" ] || return 1
    printf 'unix:path=%s\n' "$RUN_ROOT/$1/bus"
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

# 会话环境必须在 Restart **之前**抓取：Restart 成功后旧 pid 就没了，
# /proc/<旧 pid>/environ 不可读。结果放全局 CAP_ENV（每行 KEY=VALUE，只收非空值）、
# CAP_DISPLAY_SET、CAP_EXE。XDG_RUNTIME_DIR / DBUS_SESSION_BUS_ADDRESS 不抓 ——
# 它们由 opi_restart_for_uid 按标准总线选定，照抄旧值可能抄到私有总线。
CAP_ENV=""
CAP_DISPLAY_SET=0
CAP_EXE=""
opi_capture_env() { # $1=pid
    CAP_ENV=""
    CAP_DISPLAY_SET=0
    for key in DISPLAY WAYLAND_DISPLAY XAUTHORITY XDG_SESSION_TYPE \
        XDG_CURRENT_DESKTOP XDG_CONFIG_HOME XDG_DATA_HOME HOME USER LOGNAME LANG; do
        value="$(opi_env_of_pid "$1" "$key")"
        if [ -n "$value" ]; then
            CAP_ENV="$CAP_ENV$key=$value
"
            case "$key" in
                DISPLAY|WAYLAND_DISPLAY) CAP_DISPLAY_SET=1 ;;
            esac
        fi
    done
    CAP_EXE="$(readlink "$PROC_ROOT/$1/exe" 2>/dev/null || true)"
}

# 以目标用户身份拉起 fcitx5，环境用 Restart 前抓到的 CAP_ENV。
opi_launch_fcitx5() { # $1=user $2=runtime $3=addr
    user=$1; runtime=$2; addr=$3
    bin="$(command -v fcitx5 2>/dev/null || true)"
    [ -n "$bin" ] || bin="$CAP_EXE"
    [ -n "$bin" ] || bin=/usr/bin/fcitx5
    if [ ! -x "$bin" ]; then
        log "找不到 fcitx5 可执行文件 —— 无法自动拉起（用户 $user）；请重新登录恢复输入法"
        return 0
    fi

    set -- env "XDG_RUNTIME_DIR=$runtime" "DBUS_SESSION_BUS_ADDRESS=$addr"
    old_ifs=$IFS
    IFS='
'
    for kv in $CAP_ENV; do
        [ -n "$kv" ] && set -- "$@" "$kv"
    done
    IFS=$old_ifs
    set -- "$@" "$bin" -d

    # 只在本进程不具备会话环境时提醒；仍拉起 —— 宁可让输入法回来，也不留下空会话。
    if [ "$CAP_DISPLAY_SET" -eq 0 ]; then
        log "注意：没抓到 DISPLAY/WAYLAND_DISPLAY（用户 $user）—— 仍按现有环境拉起"
    fi

    if [ "$(id -u)" -eq 0 ]; then
        runuser -u "$user" -- "$@" >/dev/null 2>&1
    else
        "$@" >/dev/null 2>&1
    fi
}

# 重启后总线无所有者时，确认「没有所有者、也没有活着的本总线实例」再拉起。
opi_relaunch_after_restart() { # $1=uid $2=user $3=runtime $4=addr
    uid=$1; user=$2; runtime=$3; addr=$4

    # 再查一次关闭竞态：等待期间 fcitx5 可能刚好接管了总线。
    if opi_bus_fcitx5_owner "$user" "$runtime" "$addr" >/dev/null; then
        log "fcitx5 总线已有所有者 —— 不再拉起（用户 $user）"
        return 0
    fi
    for p in $(opi_live_fcitx5_pids "$uid"); do
        paddr="$(opi_env_of_pid "$p" DBUS_SESSION_BUS_ADDRESS)"
        # 读不到 environ（root 无 CAP_SYS_PTRACE）时按「可能在本总线」保守处理。
        if [ -z "$paddr" ] || [ "$paddr" = "$addr" ]; then
            log "fcitx5 进程（pid $p）仍在启动/运行 —— 不重复拉起（用户 $user）"
            return 0
        fi
    done

    log "fcitx5 重启后没有回来（用户 $user）—— 以原会话环境重新拉起"
    opi_launch_fcitx5 "$user" "$runtime" "$addr"

    i=0
    while [ "$i" -lt "$LAUNCH_WAIT_TICKS" ]; do
        if opi_bus_fcitx5_owner "$user" "$runtime" "$addr" >/dev/null; then
            log "已重新拉起 fcitx5（用户 $user）"
            return 0
        fi
        sleep 0.1
        i=$((i + 1))
    done
    log "fcitx5 未能重新拉起（用户 $user）—— 请重新登录恢复输入法"
}

opi_restart_on_bus() { # $1=uid $2=user $3=runtime $4=addr $5=pid
    uid=$1; user=$2; runtime=$3; addr=$4; pid=$5

    # 先抓环境：Restart 成功后旧 pid 消失，这里再读就晚了。
    opi_capture_env "$pid"

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
    fi || log "fcitx5 Restart 调用失败（用户 $user）—— 检查实例是否还在"

    # 成功判据：出现**不同的**唯一名（Restart 是自重启，唯一名不会复用）。
    # 调用的返回码不作为判据 —— 进程退出时调用本身可能报错。
    i=0
    while [ "$i" -lt "$WAIT_TICKS" ]; do
        now="$(opi_bus_fcitx5_owner "$user" "$runtime" "$addr" || true)"
        if [ -n "$now" ] && [ "$now" != "$owner" ]; then
            log "已热重启 fcitx5（用户 $user）"
            return 0
        fi
        sleep 0.1
        i=$((i + 1))
    done

    now="$(opi_bus_fcitx5_owner "$user" "$runtime" "$addr" || true)"
    if [ -n "$now" ]; then
        log "fcitx5 热重启未生效（用户 $user）：旧实例仍在运行 —— 未强杀；请手动重启或重新登录"
        return 0
    fi
    opi_relaunch_after_restart "$uid" "$user" "$runtime" "$addr"
}

# 一个 uid：只重启标准会话总线上的 fcitx5（私有/测试总线忽略）
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

    addr="$(opi_session_bus_addr "$uid")" || {
        log "fcitx5（用户 $user）不在标准会话总线上（$RUN_ROOT/$uid/bus 不存在）—— 跳过；私有/测试总线不重启"
        return 0
    }

    # 环境/pid 取标准总线上的那个实例；environ 读不到时退回第一个活进程。
    pick=""
    for pid in $pids; do
        if [ "$(opi_env_of_pid "$pid" DBUS_SESSION_BUS_ADDRESS)" = "$addr" ]; then
            pick=$pid
            break
        fi
    done
    [ -n "$pick" ] || pick="$(printf '%s\n' "$pids" | head -n 1)"
    runtime="$(opi_env_of_pid "$pick" XDG_RUNTIME_DIR)"
    [ -n "$runtime" ] || runtime="$RUN_ROOT/$uid"

    log "正在热重启 fcitx5（用户 $user，pid $pick）"
    opi_restart_on_bus "$uid" "$user" "$runtime" "$addr" "$pick"
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
