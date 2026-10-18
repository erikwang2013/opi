#!/bin/sh
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# deb preinst / rpm %pre：**停掉正在运行的 opi-candidates 与（属于安装者的）fcitx5**
# （2026-10-17 用户裁决「安装的时候中断正在运行的应用」的 Linux 等价物 —— Windows MSI
# 那边由 scripts/postprocess-windows-msi.sh 的 taskkill 承担）。
#
# 为什么需要：dpkg/rpm 升级只替换磁盘文件，**已经开着的旧进程仍跑旧代码** ——
# 2026-10-17 实测用户装完新 .deb 后设置窗左下角还是旧版本号，来源就是它；
# 2026-10-18 用户报障「覆盖安装后还要重启才生效」则是 fcitx5：插件 .so 被 fcitx5
# 进程加载，换包后它仍跑旧的映射，**必须整个进程重启**。`fcitx5-remote -r` 只让框架
# 重读配置、不换 .so（实测是假绿）—— 别拿它当替代。
#
# 两个状态文件（postinstall 读完即删）：
#   /run/opi-install.state    每行 = 一个 opi-candidates 实例的完整命令行（复活用）
#   /run/opi-install.fcitx5   fcitx5 专用，**块格式**（每个实例要带自己的会话环境）：
#       一条记录 = 若干 key=value 行 + 一个空行；键序固定：
#         pid=<pid>
#         user=<SUDO_USER>
#         uid=<该用户的 uid>
#         HOME=... / DISPLAY=... / WAYLAND_DISPLAY=... / DBUS_SESSION_BUS_ADDRESS=... /
#         XAUTHORITY=... / XDG_RUNTIME_DIR=... / XDG_CONFIG_HOME=... /
#         XDG_DATA_HOME=... / LANG=... / LC_ALL=...
#       （环境行只从 /proc/<pid>/environ 取上面这条**白名单**、只写非空项）
#     **一条记录一个实例**：同一用户同时有多个 fcitx5 是实测存在的（本机就有会话总线
#     `/run/user/1000/bus` 与私有总线 `/tmp/dbus-*` 两个），所以不能套用
#     opi-install.state 那种「一行一条命令」的格式 —— key=value 行绝不能被当命令执行。
#
# 用 /run 不用 /tmp：/run 归 root、世界不可写 ⇒ 不存在预置符号链接的攻击面；
# fcitx5 状态里含用户会话路径（HOME/XAUTHORITY），创建时 umask 077。
#
# ⚠️ 只动 **安装者自己的** fcitx5（uid == SUDO_USER）。拿不到 SUDO_USER（软件中心 /
# pkexec 类安装）、拿不到显示环境或会话总线时：**不停也不重启**，只记一行日志，交给
# 下次登录的会话 autostart 恢复。绝不在无显示环境 / CI 容器里盲目重启。
#
# 停止用 kill -TERM（不是 fcitx5-remote -e）：进程级、不依赖会话总线可达；fcitx5
# 5.1.12 的 /proc/<pid>/status SigCgt 实测含 SIGTERM（bit 14）⇒ 走正常退出路径。
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
FSTATE="$STATE_DIR/opi-install.fcitx5"

# 找所有 comm == opi-candidates 的 pid。**不用 pgrep/pkill**：procps 在最小
# 系统上不保证存在（Debian 容器实测没有）；/proc 扫描零依赖，rpm/deb 通吃。
opi_pids() {
    for d in "$PROC_ROOT"/[0-9]*; do
        [ -r "$d/comm" ] || continue
        if [ "$(cat "$d/comm" 2>/dev/null)" = "opi-candidates" ]; then
            echo "${d##*/}"
        fi
    done
}

# /proc/<pid>/environ 是 NUL 分隔的。取白名单变量的值；只取第一条、去掉行尾 CR。
# 值里真出现换行时只保留第一段 —— 状态文件是行格式，宁可丢半截也别让它破坏格式。
opi_env_value() {
    tr '\0' '\n' < "$PROC_ROOT/$1/environ" 2>/dev/null \
        | sed -n "s/^$2=//p" | head -n 1 | tr -d '\r'
}

# 活着的进程（僵尸不算：comm 仍是 fcitx5，但地址空间都没了，不能算会话里有输入法）
opi_proc_alive() {
    st="$(awk '/^State:/{print substr($2, 1, 1); exit}' "$PROC_ROOT/$1/status" 2>/dev/null)"
    case "$st" in
        ""|Z*) return 1 ;;
    esac
    return 0
}

opi_proc_uid() {
    awk '/^Uid:/{print $2; exit}' "$PROC_ROOT/$1/status" 2>/dev/null
}

# comm 必须是 fcitx5（fcitx5-remote / fcitx5-helper 不会命中）
opi_fcitx5_pids() {
    for d in "$PROC_ROOT"/[0-9]*; do
        [ -r "$d/comm" ] || continue
        [ "$(cat "$d/comm" 2>/dev/null)" = "fcitx5" ] || continue
        pid="${d##*/}"
        opi_proc_alive "$pid" && echo "$pid"
    done
}

# ---- opi-candidates（原有逻辑）----

# 记录 cmdline（arg0 是完整路径，postinstall 靠它拉起新二进制 —— 磁盘上的文件
# 换过之后，同一路径就是新版本）。
: > "$STATE"
for pid in $(opi_pids); do
    # /proc/<pid>/cmdline 是 NUL 分隔；转成单行空格分隔（参数都是简单 flag，够用）
    tr '\0' ' ' < "$PROC_ROOT/$pid/cmdline" 2>/dev/null >> "$STATE" || true
    printf '\n' >> "$STATE"
done

if [ -s "$STATE" ]; then
    for pid in $(opi_pids); do kill -TERM "$pid" 2>/dev/null || true; done
    # 优雅退出最多等 3 秒，然后强杀（与 Windows 的 taskkill /f 同一档语义）
    i=0
    while [ -n "$(opi_pids)" ] && [ "$i" -lt 30 ]; do
        sleep 0.1
        i=$((i + 1))
    done
    for pid in $(opi_pids); do kill -KILL "$pid" 2>/dev/null || true; done
    echo "opi: 已停止运行中的 opi-candidates（升级完成后按记录自动拉起）"
fi

# ---- fcitx5（2026-10-18，2026-10-19 默认关闭）----
#
# 用户报障：结束并重启 fcitx5 后 Linux 整个键盘没了。安装脚本只应处理**应用进程**
# （opi-candidates）；fcitx5 是用户桌面的输入框架，动它风险远大于收益。从此默认
# 不再停 fcitx5：新 .so 在用户下次登录/手动重启 fcitx5 后生效。
# 仍保留旧的记录/重启实现，供显式 opt-in：OPI_RESTART_FCITX5=1。打包脚本不设该变量。
if [ "${OPI_RESTART_FCITX5:-0}" = "1" ]; then

# 上一次安装若没走完（postinst 没跑到），状态会留在 /run；先清掉，避免拿陈旧记录
# 去重启别人的会话。
rm -f "$FSTATE"

user="${SUDO_USER:-}"
uid=""
if [ -n "$user" ] && [ "$user" != root ]; then
    uid="$(id -u "$user" 2>/dev/null || true)"
    case "$uid" in
        ""|*[!0-9]*) uid="" ;;
    esac
fi

# 有 fcitx5 但确定不了用户：只记日志（不猜、不碰别人的会话，也不先停掉 —— 停了没人拉）
if [ -z "$uid" ] && [ -n "$(opi_fcitx5_pids)" ]; then
    echo "opi: 检测到 fcitx5，但确定不了安装者用户（无 SUDO_USER）——跳过自动重启；下次登录自然恢复"
fi

if [ -n "$uid" ]; then
    stopped=0
    for pid in $(opi_fcitx5_pids); do
        [ "$(opi_proc_uid "$pid")" = "$uid" ] || continue

        v_home="$(opi_env_value "$pid" HOME)"
        v_display="$(opi_env_value "$pid" DISPLAY)"
        v_wayland="$(opi_env_value "$pid" WAYLAND_DISPLAY)"
        v_dbus="$(opi_env_value "$pid" DBUS_SESSION_BUS_ADDRESS)"

        # 没有 HOME / 显示环境 / 会话总线 ⇒ 不是能自动恢复的桌面会话（CI 容器、纯 TTY、
        # 或总线已死的残骸）。不停：停了之后没法保证还能把它拉起来。
        if [ -z "$v_home" ] || [ -z "$v_dbus" ] \
           || { [ -z "$v_display" ] && [ -z "$v_wayland" ]; }; then
            echo "opi: fcitx5（pid $pid）缺少 HOME/显示/会话总线环境 —— 不自动重启；下次登录自然恢复"
            continue
        fi

        # 记录该实例的会话环境（白名单）。umask 077：里面有 HOME/XAUTHORITY 路径。
        (
            umask 077
            {
                printf 'pid=%s\n' "$pid"
                printf 'user=%s\n' "$user"
                printf 'uid=%s\n' "$uid"
                for key in HOME DISPLAY WAYLAND_DISPLAY DBUS_SESSION_BUS_ADDRESS \
                           XAUTHORITY XDG_RUNTIME_DIR XDG_CONFIG_HOME XDG_DATA_HOME \
                           LANG LC_ALL; do
                    val="$(opi_env_value "$pid" "$key")"
                    [ -n "$val" ] && printf '%s=%s\n' "$key" "$val"
                done
                printf '\n'
            } >> "$FSTATE"
        )

        kill -TERM "$pid" 2>/dev/null || true
        # 优雅退出最多等 3 秒，然后强杀（与上面的 opi-candidates 同一档语义）
        i=0
        while opi_proc_alive "$pid" && [ "$i" -lt 30 ]; do
            sleep 0.1
            i=$((i + 1))
        done
        opi_proc_alive "$pid" && kill -KILL "$pid" 2>/dev/null || true
        stopped=$((stopped + 1))
    done

    if [ "$stopped" -gt 0 ]; then
        echo "opi: 已停止 $stopped 个 fcitx5 实例（安装完成后按原会话环境重启；正在输入会被打断）"
    fi
fi

fi  # OPI_RESTART_FCITX5

exit 0
