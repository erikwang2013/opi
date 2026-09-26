#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# OPI fcitx5 胶水的验证 harness —— **手工跑，不进 CI**（CI 上没有 fcitx5 的
# 头/库，也没有 dbus 会话）。它编译并运行三样东西：
#   opi_json_check    狭 JSON 解析器 vs 真 serde_json 输出（逐字节）
#   opi_panel_driver  直接构造 OpiEngine + 真 fcitx::InputContext，验面板推进
#   opi_e2e.py        真 fcitx5 守护进程 + 私有 dbus 会话，验用户实际收到的信号
#
# 用法：
#   FCITX5_HEADERS=/tmp/fcitx5-hdr/root/usr/include/Fcitx5 \
#     crates/fcitx5-opi/cpp/run-harness.sh
#
# FCITX5_HEADERS 必填：填**包含 Core/ Utils/ Config/ 三个子目录**的那一层。
#   /usr/include/Fcitx5（装了 fcitx5-dev 的机器）
#   /tmp/fcitx5-hdr/root/usr/include/Fcitx5（deb 解包，见下）
# 没装 fcitx5-dev 时这样拿（不需要 root）：
#   cd /tmp && rm -rf fcitx5-hdr && mkdir -p fcitx5-hdr && cd fcitx5-hdr
#   apt-get download libfcitx5core-dev libfcitx5utils-dev libfcitx5config-dev
#   apt-get download libfcitx5core7 libfcitx5config6 libfcitx5utils2
#   for f in *.deb; do dpkg-deb -x "$f" root/; done
#
# FCITX5_LIBS 选填：库目录。缺省从 FCITX5_HEADERS 推（deb 解包布局），推不出
# 就用 /usr/lib/x86_64-linux-gnu。
#
# FCITX5_SYSTEM_ADDONS 选填：系统 addon 目录（缺省
# /usr/lib/x86_64-linux-gnu/fcitx5）。FCITX_ADDON_DIRS 是替换语义，必须把它带
# 上，否则系统输入法全部失效。
#
# OPI_HARNESS_WORK 选填：工作目录。缺省 mktemp 一个（要复查产物就指个固定路径）。
#
# 环境是隔离开的：独立的 XDG_CONFIG_HOME / XDG_DATA_HOME / dbus 会话，
# **不会碰用户真正的 fcitx5 配置，也不会连到用户正在跑的实例上**。

set -euo pipefail

here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd "$here/../../.." && pwd)

: "${FCITX5_HEADERS:?未设置。填包含 Core/ Utils/ Config/ 的目录，例如 /tmp/fcitx5-hdr/root/usr/include/Fcitx5 —— 获取办法见本脚本头部注释}"

if [[ -z "${FCITX5_LIBS:-}" ]]; then
    # 按候选目录**实地探一下有没有 libFcitx5Core.so**，别靠数 ../ 的层数猜
    # （deb 解包是 <root>/usr/include/Fcitx5 + <root>/usr/lib/x86_64-linux-gnu）。
    FCITX5_LIBS=""
    for cand in "$FCITX5_HEADERS/../../lib/x86_64-linux-gnu" \
                "$FCITX5_HEADERS/../../lib64" "$FCITX5_HEADERS/../../lib" \
                /usr/lib/x86_64-linux-gnu /usr/lib64 /usr/lib; do
        if [[ -e "$cand/libFcitx5Core.so" ]]; then
            FCITX5_LIBS=$(cd "$cand" && pwd)
            break
        fi
    done
    : "${FCITX5_LIBS:?推不出库目录（找不到 libFcitx5Core.so），请显式设 FCITX5_LIBS}"
fi

work=${OPI_HARNESS_WORK:-$(mktemp -d /tmp/opi-harness.XXXXXX)}
addons="$work/addons"
rustlib="$repo/target/release"
INC=(-I"$FCITX5_HEADERS/Core" -I"$FCITX5_HEADERS/Utils" -I"$FCITX5_HEADERS/Config")
LNK=(-L"$rustlib" -lfcitx5_opi -L"$FCITX5_LIBS" -lFcitx5Core -lFcitx5Utils -lFcitx5Config)

echo "== 头: $FCITX5_HEADERS"
echo "== 库: $FCITX5_LIBS"
echo "== 工作目录: $work"
mkdir -p "$addons" "$work/data/fcitx5/addon" "$work/data/fcitx5/inputmethod" \
         "$work/data/opi" "$work/cfg/fcitx5"

echo "== 1/6 构建 Rust cdylib"
cargo build --release -p fcitx5_opi --manifest-path "$repo/Cargo.toml"

echo "== 2/6 编译并链接胶水 .so（-Wall -Wextra 零警告才算过）"
g++ -std=c++17 -Wall -Wextra -shared -fPIC \
    -o "$addons/libfcitx5_opi_glue.so" "$here/opi_fcitx5.cpp" \
    "${INC[@]}" "${LNK[@]}" -Wl,-rpath,'$ORIGIN'
cp "$rustlib/libfcitx5_opi.so" "$addons/"
# addon 名 libfcitx5_opi_glue 找不到 libfcitx5_opi 就整条链路失效，先查清楚。
# 注意 grep -c 在「零命中」时退出码是 1，而零命中正是这里要的结果 ——
# 配合 pipefail 会把「一切正常」当成失败，故吞掉退出码自己判。
undef=$(LD_LIBRARY_PATH="$rustlib:$FCITX5_LIBS" ldd -r "$addons/libfcitx5_opi_glue.so" \
        | grep -c 'undefined\|not found' || true)
echo "   ldd -r 未定义符号数: $undef（0 才正常）"
nm -D --defined-only "$addons/libfcitx5_opi_glue.so" | grep fcitx_addon_factory \
    || { echo "   !! 导出符号里没有 fcitx_addon_factory_instance，addon 加载不进来"; exit 1; }

echo "== 3/6 编译两支 C++ 检查程序"
g++ -std=c++17 -Wall -Wextra -o "$work/opi_panel_driver" \
    "$here/opi_panel_driver.cpp" "${INC[@]}" "${LNK[@]}"
g++ -std=c++17 -Wall -Wextra -o "$work/opi_json_check" \
    "$here/opi_json_check.cpp" "${INC[@]}" "${LNK[@]}"

echo "== 4/6 布置 XDG 数据（addon/inputmethod conf + 词库）"
cp "$repo/crates/fcitx5-opi/data/addon/opi_fcitx5.conf" "$work/data/fcitx5/addon/"
cp "$repo/crates/fcitx5-opi/data/inputmethod/opi.conf" "$work/data/fcitx5/inputmethod/"
cp "$repo/data/generated/luna.opid" "$work/data/opi/luna.opid"
# 输入法组里必须**同时**有 keyboard-us 和 opi：opi 不在组里时 SetCurrentIM("opi")
# 静默无效（不报错、不切换、不发 CurrentIM），看着就像插件坏了。
cat > "$work/cfg/fcitx5/profile" <<'PROFILE'
[Groups/0]
Name=opi-harness
Default Layout=us
DefaultIM=opi

[Groups/0/Items/0]
Name=keyboard-us
Layout=

[Groups/0/Items/1]
Name=opi
Layout=

[GroupOrder]
0=opi-harness
PROFILE

echo "== 5/6 C++ 检查（不需要 dbus/守护进程）"
export LD_LIBRARY_PATH="$rustlib:$FCITX5_LIBS"
XDG_DATA_HOME="$work/data" "$work/opi_panel_driver"
echo
"$work/opi_json_check"

echo
echo "== 6/6 端到端（私有 dbus 会话 + 真 fcitx5 守护进程）"
dbus-run-session -- env HARNESS_WORK="$work" E2E_PY="$here/opi_e2e.py" \
    LD_LIBRARY_PATH="$rustlib:$FCITX5_LIBS" bash -s <<'EOS'
set -u
# FCITX_ADDON_DIRS 是**替换**语义：系统目录必须手抄在后面，否则拼音等一起失效。
export FCITX_ADDON_DIRS="$HARNESS_WORK/addons:${FCITX5_SYSTEM_ADDONS:-/usr/lib/x86_64-linux-gnu/fcitx5}"
export XDG_DATA_HOME="$HARNESS_WORK/data"
export XDG_CONFIG_HOME="$HARNESS_WORK/cfg"
# testui 会把其它 addon 全禁掉（Override Enabled Addons: {testui}），
# 所以要额外 --enable 本 addon，否则它根本不加载、测出来全是空信号。
fcitx5 --ui=testui --enable opi_fcitx5 -d > "$HARNESS_WORK/fcitx5.log" 2>&1 &
pid=$!
sleep 5
for mode in basic page; do python3 "$E2E_PY" "$mode"; echo; done
# 能力位对照：0x12 无 ClientSideInputPanel → 面板推给 UI addon（客户端看不到）
python3 "$E2E_PY" caps 0x12; echo
python3 "$E2E_PY" caps 0x8000000012
kill $pid 2>/dev/null || true
wait $pid 2>/dev/null || true
EOS

echo
echo "== 守护进程加载记录（$work/fcitx5.log）"
grep -E 'Loaded addon opi_fcitx5|Unloading addon opi_fcitx5|Override Enabled Addons' \
    "$work/fcitx5.log" || echo "  （没找到 opi_fcitx5 的加载记录 —— 上面 e2e 的信号全是假的，别采信）"
echo
echo "== 完成。产物与日志留在 $work"
