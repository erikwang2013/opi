#!/bin/sh
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# deb postinst / rpm %post：安装/升级成功后**把 preinstall 停掉的实例按原参数拉起**
# （2026-10-17 用户裁决「安装成功后启动新服务」的 Linux 等价物）。
#
# 以**桌面用户**身份拉起：包脚本是 root，GUI 不能以 root 跑 ⇒ 用 `runuser -u`。
# 桌面用户取 `SUDO_USER`（`sudo apt install` / `sudo dpkg -i` 的标准来源）。
# **没有 SUDO_USER**（商店/软件中心类 GUI 安装常如此）⇒ 静默跳过并说明：
#   · 设置窗本来就是用户手动开的，开一次就是新版本；
#   · 状态栏有会话级 autostart（/etc/xdg/autostart），下次登录自然回来。
# 已知边界（如实记）：如果升级前在跑的是**状态栏**，本次尽力拉起时环境变量
# （DISPLAY/XAUTHORITY）来自 root 脚本的继承值 —— 拉起失败只是进程直接退出
# （无窗口、无副作用），下次登录 autostart 仍会带回；真机验证待做。
set -u

STATE=/run/opi-install.state
[ -s "$STATE" ] || { rm -f "$STATE"; exit 0; }

user="${SUDO_USER:-}"
if [ -z "$user" ] || [ "$user" = root ]; then
    rm -f "$STATE"
    echo "opi: 升级前有实例在运行，但确定不了桌面用户（无 SUDO_USER）——跳过自动拉起"
    exit 0
fi

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

rm -f "$STATE"
exit 0
