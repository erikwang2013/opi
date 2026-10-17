#!/bin/sh
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# deb preinst / rpm %pre：**停掉正在运行的 opi-candidates**（2026-10-17 用户裁决
# 「安装的时候中断正在运行的应用」的 Linux 等价物 —— Windows MSI 那边由
# scripts/postprocess-windows-msi.sh 的 taskkill 承担）。
#
# 为什么需要：dpkg/rpm 升级只替换磁盘文件，**已经开着的旧进程仍跑旧二进制** ——
# 2026-10-17 实测用户装完新 .deb 后设置窗左下角还是旧版本号，来源就是它。
#
# 同时把「当时在跑哪些实例（含参数）」记到 /run/opi-install.state，
# postinstall 按它复活（升级完把服务拉回来）。**用 /run 不用 /tmp**：/run 归 root、
# 世界不可写 ⇒ 不存在预置符号链接的攻击面。
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

STATE=/run/opi-install.state

# 记录 cmdline（arg0 是完整路径，postinstall 靠它拉起新二进制 —— 磁盘上的文件
# 换过之后，同一路径就是新版本）。
: > "$STATE"
for pid in $(opi_pids); do
    # /proc/<pid>/cmdline 是 NUL 分隔；转成单行空格分隔（参数都是简单 flag，够用）
    tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null >> "$STATE" || true
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

exit 0
