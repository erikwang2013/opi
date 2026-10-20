#!/bin/sh
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# deb prerm / rpm %preun：**卸载前停掉运行中的实例**（与 Windows MSI 的无条件
# taskkill 同一条语义：卸载时旧进程占着文件走「删除但进程仍活」那条路，
# 用户看到的是「卸了还在跑」）。
set -u

# 升级时**不停**：deb 的 prerm upgrade 跑在 preinst 之前，这里杀了进程，preinst
# 就记录不到 cmdline，postinst 便无从拉起 —— 状态栏会消失到下次登录。停 + 记
# 统一由 preinst 做（rpm 的 %pre 同样在新文件落盘前跑）。卸载仍走下面的停止逻辑。
case "${1:-}" in
    upgrade|1) exit 0 ;;
esac

# 测试钩子（scripts/packaging/test-restart-fcitx5.sh）：指向假 /proc。**只有非 root
# 才认**：生产里维护脚本恒为 root（dpkg/rpm），此时固定 /proc。
if [ "$(id -u)" -eq 0 ]; then
    PROC_ROOT=/proc
else
    PROC_ROOT="${OPI_PROC_ROOT:-/proc}"
fi

# 找当前实例（comm == opi）与 ≤1.17.x 的旧进程（opi-candidates）的 pid —— 卸载时
# 两种都得停，不然旧进程会一直跑到下次登录。**不用 pgrep/pkill**：procps 在最小
# 系统上不保证存在（Debian 容器实测没有）；/proc 扫描零依赖，rpm/deb 通吃。
opi_pids() {
    for d in "$PROC_ROOT"/[0-9]*; do
        [ -r "$d/comm" ] || continue
        case "$(cat "$d/comm" 2>/dev/null)" in
            opi|opi-candidates) echo "${d##*/}" ;;
        esac
    done
}

if [ -n "$(opi_pids)" ]; then
    for pid in $(opi_pids); do kill -TERM "$pid" 2>/dev/null || true; done
    i=0
    while [ -n "$(opi_pids)" ] && [ "$i" -lt 30 ]; do
        sleep 0.1
        i=$((i + 1))
    done
    for pid in $(opi_pids); do kill -KILL "$pid" 2>/dev/null || true; done
    echo "opi: 已停止运行中的 opi（卸载中）"
fi

exit 0
