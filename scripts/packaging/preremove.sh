#!/bin/sh
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# deb prerm / rpm %preun：**卸载前停掉运行中的实例**（与 Windows MSI 的无条件
# taskkill 同一条语义：卸载时旧进程占着文件走「删除但进程仍活」那条路，
# 用户看到的是「卸了还在跑」）。
set -u

# 找所有 comm == opi-candidates 的 pid。**不用 pgrep/pkill**：procps 在最小
# 系统上不保证存在（Debian 容器实测没有）；/proc 扫描零依赖，rpm/deb 通吃。
opi_pids() {
    for d in /proc/[0-9]*; do
        [ -r "$d/comm" ] || continue
        if [ "$(cat "$d/comm" 2>/dev/null)" = "opi-candidates" ]; then
            echo "${d#/proc/}"
        fi
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
    echo "opi: 已停止运行中的 opi-candidates（卸载中）"
fi

exit 0
