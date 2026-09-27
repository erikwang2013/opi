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
# OPI_LUNA_OPID 选填：词库源。缺省 android/app/src/main/assets/luna.opid（入库
# 副本，与 CMakeLists 的缺省源同一份）—— 本地重编的那份 data/generated/luna.opid
# **没入库**，指向它会让本脚本在全新 clone 上直接断在 cp。
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

echo "== 2/6 编译并链接胶水 .so（-Wall -Wextra -Werror：警告即失败）"
g++ -std=c++17 -Wall -Wextra -Werror -shared -fPIC \
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
g++ -std=c++17 -Wall -Wextra -Werror -o "$work/opi_panel_driver" \
    "$here/opi_panel_driver.cpp" "${INC[@]}" "${LNK[@]}"
g++ -std=c++17 -Wall -Wextra -Werror -o "$work/opi_json_check" \
    "$here/opi_json_check.cpp" "${INC[@]}" "${LNK[@]}"

echo "== 4/6 布置 XDG 数据（addon/inputmethod conf + 词库）"
cp "$repo/crates/fcitx5-opi/data/addon/opi_fcitx5.conf" "$work/data/fcitx5/addon/"
cp "$repo/crates/fcitx5-opi/data/inputmethod/opi.conf" "$work/data/fcitx5/inputmethod/"
# 词库源取**入库副本**（android 部署那份），与 cpp/CMakeLists.txt 的缺省源同一份。
# 原来这里指的是 data/generated/luna.opid —— 那是 gitignore 的本地重编产物，
# 全新 clone 上**不存在**，而它正是这条 cp 要的那份：于是这条「唯一端到端通路」
# 在干净树上第一步就断在这里（cp 退出码 1）。要用刚重编的那份就显式覆盖：
#   OPI_LUNA_OPID=data/generated/luna.opid crates/fcitx5-opi/cpp/run-harness.sh
: "${OPI_LUNA_OPID:=$repo/android/app/src/main/assets/luna.opid}"
[ -f "$OPI_LUNA_OPID" ] || { echo "!! 找不到词库源：$OPI_LUNA_OPID" >&2; exit 1; }
cp "$OPI_LUNA_OPID" "$work/data/opi/luna.opid"
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

[Groups/0/Items/2]
Name=pinyin
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
# 每个变体一个**独立**的 dbus-run-session + 独立守护进程。两个理由：
#
# 1) **不加 `-d`**。`-d` 会 daemonize：`$!` 是那个立刻退出的父进程，`kill $!`
#    打空（实测返回 1），真正在跑的守护进程是另一个 pid 且**留下来**。于是
#    「起来 → 跑 e2e → 收掉」这条链的后半段是假的，下一次运行可能落在**旧
#    守护进程**上出结果 —— 改完代码却测出「和上一版毫无区别」的假绿，就是
#    这么来的。前台起，`$!` 才是真 pid；收完再确认一次它真的没了。
# 2) 每个变体独立会话。同一 dbus 会话里起第二个 fcitx5，它会拿不到 dbus 名、
#    把已加载的 addon 全部卸掉、一个字都不服务（日志：`Unable to request dbus
#    name. Is there another fcitx already running?`），而 e2e **照样连上并出
#    结果** —— 出结果是**第一个**实例。共用会话时四个变体还会互相污染状态
#    （当前 IM、残留缓冲），日志也混在一个文件里分不清归属。
#
# 这里**不做** `pkill -f 'fcitx5 --ui=testui'`：每个会话的 bus 地址都不同，
# 别的会话里的残留守护进程根本收不到我们的调用，清它没有收益；而这条命令会
# 误杀同一台机器上别人正在跑的 harness。
e2e_rc=0
vi=0
# punct = 标点模块探针；`punct keyboard-us` 是它的**对照组**（同一个脚本换 IM，
# 见 opi_e2e.py 的 punct 模式说明）。两组必须成对跑，否则「测不出来」没有参照。
variants=(basic page passthrough "caps 0x12" "caps 0x8000000012" punct "punct keyboard-us" punctopi modes fullwidth)
for variant in "${variants[@]}"; do
    vi=$((vi + 1))
    echo
    echo "--- e2e 变体 $vi/${#variants[@]}: opi_e2e.py $variant"
    dbus-run-session -- env HARNESS_WORK="$work" E2E_PY="$here/opi_e2e.py" \
        VARIANT="$variant" LOG="$work/fcitx5-$vi.log" \
        LD_LIBRARY_PATH="$rustlib:$FCITX5_LIBS" bash -s <<'EOS' || e2e_rc=1
set -u
# FCITX_ADDON_DIRS 是**替换**语义：系统目录必须手抄在后面，否则拼音等一起失效。
export FCITX_ADDON_DIRS="$HARNESS_WORK/addons:${FCITX5_SYSTEM_ADDONS:-/usr/lib/x86_64-linux-gnu/fcitx5}"
export XDG_DATA_HOME="$HARNESS_WORK/data"
export XDG_CONFIG_HOME="$HARNESS_WORK/cfg"
# testui 会把其它 addon 全禁掉（Override Enabled Addons: {testui}），
# 所以要额外 --enable 本 addon，否则它根本不加载、测出来全是空信号。
#
# punctuation 也要一并 --enable：它是**用户真实配置里开着**的全局模块
# （/usr/share/fcitx5/addon/punctuation.conf Category=Module + 用户 conf 里
# Enabled=True），而 testui 的 Override 把它一起禁掉了。少这一个 --enable，
# harness 里的标点行为就与用户桌面不一致 —— 实测该 addon 是否加载，取决于
# 这一行（不加则日志里没有 `Loaded addon punctuation`）。
#
# ⚠️ `--enable` 收的是**逗号分隔的一个列表**，不是可重复的开关（fcitx5 --help
# 原文）。写成两个 `--enable` 后者会**覆盖**前者：`--enable opi_fcitx5
# --enable punctuation` 的 Override 实际是 {testui, punctuation}，opi 被挤掉。
# 而 opi **照样能按键出字**（它是 profile 的 DefaultIM，IM addon 会被强制加载），
# 于是这个错误在日志里只表现为 Override 那一行少一项 —— 极易漏过去。
# pinyin 也要在列表里：punctopi 变体靠它把 libpunctuation 拉起来（见 opi_e2e.py
# 的 punctopi 说明）；不在 --enable 列表里的 addon 会被 Override 挡下。
fcitx5 --ui=testui --enable opi_fcitx5,punctuation,pinyin > "$LOG" 2>&1 &
pid=$!
# 等 addon 真的加载起来再送按键。固定 sleep 是假故障源：慢机器上 5s 不够，
# e2e 会连上守护进程但连不到输入法，报出来一堆「无面板更新信号」。
for _ in $(seq 1 80); do
    grep -q 'Loaded addon opi_fcitx5' "$LOG" && break
    kill -0 "$pid" 2>/dev/null || { echo "!! fcitx5 加载完成前就退了，日志见 $LOG"; exit 1; }
    sleep 0.25
done
if ! grep -q 'Loaded addon opi_fcitx5' "$LOG"; then
    echo "!! 20s 内没等到 'Loaded addon opi_fcitx5'，日志见 $LOG"; exit 1
fi
python3 "$E2E_PY" $VARIANT
rc=$?
kill "$pid" 2>/dev/null || true
wait "$pid" 2>/dev/null || true
# 前台的守护进程没被收掉就是清理失败 —— 这正是 `-d` 那版掩盖掉的东西，
# 不能让它再静默一次。
if kill -0 "$pid" 2>/dev/null; then
    echo "!! 守护进程 $pid 仍在跑（$LOG），会污染下一次运行"; rc=1
fi
exit $rc
EOS
done

echo
echo "== 守护进程加载记录（$work/fcitx5-*.log）"
grep -H -E 'Loaded addon opi_fcitx5|Unloading addon opi_fcitx5|Override Enabled Addons' \
    "$work"/fcitx5-*.log || echo "  （没找到 opi_fcitx5 的加载记录 —— 上面 e2e 的信号全是假的，别采信）"
echo
echo "== 完成（e2e 退出码 $e2e_rc）。产物与日志留在 $work"
exit "$e2e_rc"
