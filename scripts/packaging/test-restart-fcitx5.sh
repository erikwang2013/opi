#!/bin/sh
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# 单测：fcitx5 热重启 helper + pre/post 安装脚本的进程处理。
#
# **绝不碰本机真实的 /proc 与 /run/user**：helper 与 preinstall/postinstall 都支持
# OPI_PROC_ROOT / OPI_RUN_ROOT / OPI_STATE_DIR 测试钩子（只有非 root 才认）。
# 真实私有 dbus 用例用**合成 /proc 夹具**把 helper 限定到私有 fcitx5 实例上 ——
# 本机用户的真实 fcitx5 不会被选中（只按夹具里的 pid/uid/总线调用）。
#
# 覆盖：
#   1. 四个脚本 sh -n；
#   2. helper：多实例/多总线、去重、僵尸与别的 uid 不碰、无 fcitx5 静默跳过、
#      缺总线跳过、标准总线兜底、Restart 打在 GetNameOwner 的唯一名上、
#      总线无所有者时跳过（不调 Restart）、OPI_NO_RESTART_FCITX5=1、busctl 失败仍 exit 0；
#   3. preremove：升级（deb upgrade / rpm 1）不停实例，卸载才停；
#   4. preinstall：停 opi-candidates、写状态；不再动 fcitx5、不写旧状态文件；
#   5. postinstall：先调 helper 再拉起 opi-candidates；无 SUDO_USER 也热重启；
#      关闭开关生效；helper 失败不阻断；字体缓存分支调用 fc-cache（真实 fc-cache
#      在同一字体目录连跑两次，幂等）；
#   6. 真实私有 dbus（dbus-run-session + fcitx5 --ui=testui）：
#      Restart 返回 0、日志出现 Unloading/Loaded addon、服务仍存活；
#   7. 可选（独立脚本 test-restart-fcitx5-dpkg.sh）：真实 dpkg 旧→新迁移兜底 +
#      新→新升级/卸载序列；
#   8. 可选（独立脚本 test-restart-fcitx5-root.sh）：root 生产路径容器集成。
#
# ⚠️ root 路径：主测试以非 root 运行（脚本在 root 下忽略 OPI_PROC_ROOT 等钩子），
# 覆盖不到。第 8 节的可选容器用例实际执行 `SUDO_USER → 真 /proc → id -nu → runuser
# → gdbus Restart`；`PKEXEC_UID` / `DOAS_USER` / 无 SUDO_USER 扫 `/run/user` 分支
# 仍只有静态审查，容器无 CAP_SYS_PTRACE 也覆盖不到 environ 直读（走标准总线兜底）。
#
# 用法：sh scripts/packaging/test-restart-fcitx5.sh   （非 root 可跑）
set -u

# 被测试脚本在 root 下会忽略测试钩子（改用真 /proc、真 /run）——那会把本机正在跑的
# 进程当成夹具。必须用普通用户跑。
if [ "$(id -u)" -eq 0 ]; then
    echo "本测试必须用普通用户跑（root 下脚本会忽略 OPI_PROC_ROOT 等钩子）" >&2
    exit 1
fi

HERE="$(cd "$(dirname "$0")" && pwd)"
PRE="$HERE/preinstall.sh"
POST="$HERE/postinstall.sh"
PREUN="$HERE/preremove.sh"
HELPER="$HERE/restart-fcitx5.sh"

TMP="$(mktemp -d)"
SLEEP1=""
SLEEP2=""
cleanup() {
    [ -n "$SLEEP1" ] && kill "$SLEEP1" 2>/dev/null
    [ -n "$SLEEP2" ] && kill "$SLEEP2" 2>/dev/null
    rm -rf "$TMP"
}
trap cleanup EXIT

fails=0
ok()   { echo "ok   - $*"; }
bad()  { echo "FAIL - $*" >&2; fails=$((fails + 1)); }
check() { desc="$1"; shift; if "$@" >/dev/null 2>&1; then ok "$desc"; else bad "$desc"; fi; }

ME="$(id -un)"
MYUID="$(id -u)"
PROC="$TMP/proc"
STATEDIR="$TMP/state"
RUN="$TMP/run"
BIN="$TMP/bin"
BIN_FAIL="$TMP/bin-fail"
mkdir -p "$PROC" "$STATEDIR" "$RUN" "$BIN" "$BIN_FAIL"
BUSLOG="$TMP/busctl.log"
FONTDIR="$TMP/fonts/opi"
mkdir -p "$FONTDIR"
cp "$HERE/../../shared/pet/emoji-font/opi-emoji.ttf" "$FONTDIR/"
REAL_FC_CACHE="$(command -v fc-cache 2>/dev/null || true)"

# ---- 1. 语法 ----
for f in "$PRE" "$POST" "$PREUN" "$HELPER"; do
    check "sh -n $(basename "$f")" sh -n "$f"
done

# ---- 夹具 ----
# 假 busctl：记录「总线环境 + 参数」；GetNameOwner 默认回唯一名（helper 用它给
# Restart 定目标），$TMP/no-owner 存在时失败（模拟活进程在、目标总线无所有者）。
cat > "$BIN/busctl" <<EOF
#!/bin/sh
printf '%s|%s|%s\n' "\$XDG_RUNTIME_DIR" "\$DBUS_SESSION_BUS_ADDRESS" "\$*" >> "$BUSLOG"
case "\$*" in
    *GetNameOwner*)
        if [ -e "$TMP/no-owner" ]; then exit 1; else echo 's ":1.4242"'; fi ;;
esac
exit 0
EOF
chmod +x "$BIN/busctl"
printf '#!/bin/sh\nexit 1\n' > "$BIN_FAIL/busctl"
chmod +x "$BIN_FAIL/busctl"

# 假 runuser：记录 argv 后成功（只用于 postinstall 拉起 opi-candidates）
{
    echo '#!/bin/sh'
    echo "printf '%s\\n' \"\$@\" >> '$TMP/runuser.argv'"
    echo 'exit 0'
} > "$BIN/runuser"
chmod +x "$BIN/runuser"

# 假 fc-cache：只记录调用（postinstall 的字体缓存分支）；真实 fc-cache 的幂等性
# 在下面单独用 REAL_FC_CACHE 跑，避免把用户缓存当判据。
if [ -n "$REAL_FC_CACHE" ]; then
    {
        echo '#!/bin/sh'
        echo "printf '%s\\n' \"\$*\" >> '$TMP/fc-cache.log'"
        echo 'exit 0'
    } > "$BIN/fc-cache"
    chmod +x "$BIN/fc-cache"
fi

fresh_proc() { rm -rf "$PROC"; mkdir -p "$PROC"; }
mk_fx5() { # pid uid state [bus] [runtime]
    mkdir -p "$PROC/$1"
    printf 'fcitx5\n' > "$PROC/$1/comm"
    printf 'Uid:\t%s\t%s\t%s\t%s\nState:\t%s (test)\n' \
        "$2" "$2" "$2" "$2" "$3" > "$PROC/$1/status"
    {
        [ -n "${4:-}" ] && printf 'DBUS_SESSION_BUS_ADDRESS=%s\n' "$4"
        [ -n "${5:-}" ] && printf 'XDG_RUNTIME_DIR=%s\n' "$5"
    } | tr '\n' '\000' > "$PROC/$1/environ"
}
run_helper() { # stdin/args unused；输出到 $TMP/helper.out
    env -u OPI_NO_RESTART_FCITX5 -u SUDO_USER -u PKEXEC_UID -u DOAS_USER \
        OPI_PROC_ROOT="$PROC" OPI_RUN_ROOT="$RUN" PATH="$BIN:$PATH" \
        sh "$HELPER" > "$TMP/helper.out" 2>&1
}
run_helper_fail() {
    env -u OPI_NO_RESTART_FCITX5 OPI_PROC_ROOT="$PROC" OPI_RUN_ROOT="$RUN" \
        PATH="$BIN_FAIL:$PATH" sh "$HELPER" > "$TMP/helper.out" 2>&1
}

# ---- 2. helper：多实例、多总线、去重 ----
fresh_proc
mk_fx5 800001 "$MYUID" S "unix:path=/tmp/opi-bus-a" "/run/user/$MYUID"
mk_fx5 800002 "$MYUID" S "unix:path=/tmp/opi-bus-b" "/run/user/$MYUID"
mk_fx5 800003 "$MYUID" S "unix:path=/tmp/opi-bus-a" "/run/user/$MYUID"  # 同总线，应去重
mk_fx5 800004 "$MYUID" Z "unix:path=/tmp/opi-bus-z" "/run/user/$MYUID"  # 僵尸
mk_fx5 800005 999997 S "unix:path=/tmp/opi-bus-other" "/run/user/999997"
: > "$BUSLOG"
run_helper
check "helper 多实例 exit 0" [ "$?" -eq 0 ]
check "两个不同总线各重启一次" [ "$(grep -c 'Controller1 Restart' "$BUSLOG")" -eq 2 ]
check "总线 A 被调用（带 XDG_RUNTIME_DIR）" \
    grep -F -- "/run/user/$MYUID|unix:path=/tmp/opi-bus-a|" "$BUSLOG"
check "总线 B 被调用" grep -F -- "|unix:path=/tmp/opi-bus-b|" "$BUSLOG"
check "僵尸未被调用" sh -c "! grep -q 'opi-bus-z' '$BUSLOG'"
check "别的 uid 未被调用" sh -c "! grep -q 'opi-bus-other' '$BUSLOG'"
check "每次重启前后都做所有者查询（2 次重启 = 4 次 GetNameOwner）" \
    [ "$(grep -c 'GetNameOwner' "$BUSLOG")" -eq 4 ]
check "Restart 打在 GetNameOwner 的唯一名上" \
    sh -c "grep 'Controller1 Restart' '$BUSLOG' | grep -q 'call :1.4242'"
check "Restart 不以 well-known name 为目标（不触发激活）" \
    sh -c "! grep 'Controller1 Restart' '$BUSLOG' | grep -q 'org.fcitx.Fcitx5'"
check "两条成功日志" [ "$(grep -c '已热重启 fcitx5' "$TMP/helper.out")" -eq 2 ]

# 活进程在、目标总线无所有者：不能调 Restart（否则激活文件会拉起新实例）
fresh_proc
mk_fx5 800006 "$MYUID" S "unix:path=/tmp/opi-bus-nobody" "/run/user/$MYUID"
: > "$BUSLOG"
: > "$TMP/no-owner"
run_helper
check "总线无所有者 exit 0" [ "$?" -eq 0 ]
check "总线无所有者做了所有者查询" grep -q 'GetNameOwner' "$BUSLOG"
check "总线无所有者不调 Restart" sh -c "! grep -q 'Controller1 Restart' '$BUSLOG'"
check "总线无所有者有跳过日志" grep -q "跳过重启" "$TMP/helper.out"
rm -f "$TMP/no-owner"

: > "$BUSLOG"
OPI_NO_RESTART_FCITX5=1 OPI_PROC_ROOT="$PROC" OPI_RUN_ROOT="$RUN" PATH="$BIN:$PATH" \
    sh "$HELPER" > "$TMP/helper.out" 2>&1
check "关闭开关 exit 0" [ "$?" -eq 0 ]
check "关闭开关不调用 D-Bus" [ ! -s "$BUSLOG" ]
check "关闭开关有日志" grep -q "OPI_NO_RESTART_FCITX5=1" "$TMP/helper.out"

# ---- 3. helper：没有 fcitx5 / 缺总线 / 标准总线兜底 / 失败 ----
fresh_proc
: > "$BUSLOG"
run_helper
check "无 fcitx5 时 exit 0" [ "$?" -eq 0 ]
check "无 fcitx5 时不调用 D-Bus" [ ! -s "$BUSLOG" ]

fresh_proc
mk_fx5 800010 "$MYUID" S "" ""
rm -rf "$RUN"; mkdir -p "$RUN"
: > "$BUSLOG"
run_helper
check "缺会话总线 exit 0" [ "$?" -eq 0 ]
check "缺会话总线不调用 D-Bus" [ ! -s "$BUSLOG" ]
check "缺会话总线有日志" grep -q "没有会话总线" "$TMP/helper.out"

# 标准会话总线兜底（environ 读不到总线时用 <run>/<uid>/bus）
mkdir -p "$RUN/$MYUID"
: > "$RUN/$MYUID/bus"
: > "$BUSLOG"
run_helper
check "标准总线兜底 exit 0" [ "$?" -eq 0 ]
check "标准总线兜底用 unix:path" \
    grep -F -- "|unix:path=$RUN/$MYUID/bus|" "$BUSLOG"
check "标准总线兜底 runtime 为 <run>/<uid>" \
    grep -F -- "$RUN/$MYUID|unix:path=$RUN/$MYUID/bus|" "$BUSLOG"
rm -rf "$RUN"; mkdir -p "$RUN"

fresh_proc
mk_fx5 800020 "$MYUID" S "unix:path=/tmp/opi-bus-fail" "/run/user/$MYUID"
: > "$BUSLOG"
run_helper_fail
check "busctl 失败仍 exit 0" [ "$?" -eq 0 ]
check "busctl 失败不调 Restart" [ ! -s "$BUSLOG" ]
check "busctl 失败有跳过日志" grep -q "跳过重启" "$TMP/helper.out"

# ---- 4. preremove：升级不停（停+记归 preinstall），卸载才停 ----
fresh_proc
sleep 300 & SLEEP1=$!
mkdir -p "$PROC/$SLEEP1"
printf 'opi-candidates\n' > "$PROC/$SLEEP1/comm"
OPI_PROC_ROOT="$PROC" sh "$PREUN" upgrade > "$TMP/preun.out" 2>&1
check "preremove deb upgrade exit 0" [ "$?" -eq 0 ]
check "preremove deb upgrade 不停实例" kill -0 "$SLEEP1"
OPI_PROC_ROOT="$PROC" sh "$PREUN" 1 > "$TMP/preun.out" 2>&1
check "preremove rpm 升级(1) 不停实例" kill -0 "$SLEEP1"
OPI_PROC_ROOT="$PROC" sh "$PREUN" remove > "$TMP/preun.out" 2>&1
check "preremove remove exit 0" [ "$?" -eq 0 ]
check "preremove remove 有停止日志" grep -q "已停止运行中的 opi-candidates" "$TMP/preun.out"
wait "$SLEEP1" 2>/dev/null
check "preremove remove 实例收到 SIGTERM" [ "$?" -eq 143 ]
SLEEP1=""

# ---- 5. preinstall：只停 opi-candidates，不碰 fcitx5 ----
fresh_proc
SLEEP1=""
SLEEP2=""
sleep 300 & SLEEP1=$!
sleep 300 & SLEEP2=$!
mkdir -p "$PROC/$SLEEP1"
printf 'opi-candidates\n' > "$PROC/$SLEEP1/comm"
printf '%s\0--statusbar\0' "$TMP/opi-candidates" > "$PROC/$SLEEP1/cmdline"
mk_fx5 "$SLEEP2" "$MYUID" S "unix:path=/tmp/opi-bus-real" "/run/user/$MYUID"
: > "$STATEDIR/opi-install.state"
SUDO_USER="$ME" OPI_PROC_ROOT="$PROC" OPI_STATE_DIR="$STATEDIR" \
    sh "$PRE" > "$TMP/pre.out" 2>&1
check "preinstall exit 0" [ "$?" -eq 0 ]
check "preinstall 有停止日志" grep -q "已停止运行中的 opi-candidates" "$TMP/pre.out"
check "preinstall 记录了 cmdline" grep -Fqx -- "$TMP/opi-candidates --statusbar " "$STATEDIR/opi-install.state"
check "preinstall 不写旧 fcitx5 状态" [ ! -e "$STATEDIR/opi-install.fcitx5" ]
check "preinstall 日志不提 fcitx5" sh -c "! grep -q fcitx5 '$TMP/pre.out'"
check "preinstall 参数里的 fcitx5 未被停" kill -0 "$SLEEP2"
wait "$SLEEP1" 2>/dev/null
check "被停实例收到 SIGTERM" [ "$?" -eq 143 ]
SLEEP1=""
kill "$SLEEP2" 2>/dev/null
wait "$SLEEP2" 2>/dev/null
SLEEP2=""

# ---- 6. postinstall：先热重启 fcitx5，再拉起 opi-candidates ----
printf '#!/bin/sh\nexit 0\n' > "$TMP/opi-candidates"
chmod +x "$TMP/opi-candidates"
fresh_proc
mk_fx5 800030 "$MYUID" S "unix:path=/tmp/opi-bus-post" "/run/user/$MYUID"
: > "$BUSLOG"
: > "$TMP/fc-cache.log"
rm -f "$TMP/runuser.argv"
printf '%s --statusbar\n' "$TMP/opi-candidates" > "$STATEDIR/opi-install.state"
SUDO_USER="$ME" OPI_PROC_ROOT="$PROC" OPI_STATE_DIR="$STATEDIR" \
    OPI_FONT_DIR="$FONTDIR" PATH="$BIN:$PATH" \
    sh "$POST" > "$TMP/post.out" 2>&1
check "postinstall exit 0" [ "$?" -eq 0 ]
check "postinstall 调了 helper 热重启" grep -q 'Controller1 Restart' "$BUSLOG"
check "postinstall 先热重启（日志在）" grep -q '已热重启 fcitx5' "$TMP/post.out"
check "postinstall 拉起 opi-candidates" grep -Fqx -- "--statusbar" "$TMP/runuser.argv"
check "postinstall runuser 用 -u <user> --" \
    sh -c "[ \"\$(sed -n '1,3p' '$TMP/runuser.argv' | tr '\n' ' ')\" = '-u $ME -- ' ]"
check "postinstall 删掉状态" [ ! -e "$STATEDIR/opi-install.state" ]
if [ -n "$REAL_FC_CACHE" ]; then
    check "postinstall 调了 fc-cache -f 字体目录" \
        grep -Fq -- "-f $FONTDIR" "$TMP/fc-cache.log"
    check "postinstall 无 fc-cache 失败日志" sh -c "! grep -q 'fc-cache' '$TMP/post.out'"
    check "真实 fc-cache 刷字体目录（第一次）" "$REAL_FC_CACHE" -f "$FONTDIR"
    check "真实 fc-cache 第二次仍成功（幂等）" "$REAL_FC_CACHE" -f "$FONTDIR"
    if command -v fc-scan >/dev/null 2>&1; then
        check "fc-scan 读出 family「Opi Emoji」" \
            sh -c "fc-scan --format '%{family}' '$FONTDIR/opi-emoji.ttf' | grep -q 'Opi Emoji'"
    else
        echo "skip - fc-scan 用例（本机没有 fc-scan）"
    fi
else
    echo "skip - fc-cache 用例（本机没有 fc-cache）"
fi

# 无 SUDO_USER：postinstall 仍调 helper（本测试非 root，helper 走「普通用户 = 自己」的
# 分支；生产里 root 的 SUDO_USER/PKEXEC_UID/扫 /run/user 发现路径**未被本测试覆盖**）；
# 实例不拉起
: > "$BUSLOG"
rm -f "$TMP/runuser.argv"
printf '%s --statusbar\n' "$TMP/opi-candidates" > "$STATEDIR/opi-install.state"
env -u SUDO_USER OPI_PROC_ROOT="$PROC" OPI_STATE_DIR="$STATEDIR" \
    OPI_FONT_DIR="$FONTDIR" PATH="$BIN:$PATH" \
    sh "$POST" > "$TMP/post-nouser.out" 2>&1
check "无 SUDO_USER postinstall exit 0" [ "$?" -eq 0 ]
check "无 SUDO_USER 仍热重启 fcitx5" grep -q 'Controller1 Restart' "$BUSLOG"
check "无 SUDO_USER 不拉起实例" [ ! -e "$TMP/runuser.argv" ]
check "无 SUDO_USER 有说明" grep -q "无 SUDO_USER" "$TMP/post-nouser.out"
check "无 SUDO_USER 清状态" [ ! -e "$STATEDIR/opi-install.state" ]

# 关闭开关：helper 不调 D-Bus，但 opi-candidates 照常拉起
: > "$BUSLOG"
rm -f "$TMP/runuser.argv"
printf '%s --statusbar\n' "$TMP/opi-candidates" > "$STATEDIR/opi-install.state"
OPI_NO_RESTART_FCITX5=1 SUDO_USER="$ME" OPI_PROC_ROOT="$PROC" \
    OPI_STATE_DIR="$STATEDIR" OPI_FONT_DIR="$FONTDIR" PATH="$BIN:$PATH" \
    sh "$POST" > "$TMP/post-off.out" 2>&1
check "关闭开关 postinstall exit 0" [ "$?" -eq 0 ]
check "关闭开关不调 D-Bus" [ ! -s "$BUSLOG" ]
check "关闭开关仍拉起实例" grep -Fqx -- "--statusbar" "$TMP/runuser.argv"

# helper 失败：安装继续、实例照常拉起
rm -f "$TMP/runuser.argv"
printf '%s --statusbar\n' "$TMP/opi-candidates" > "$STATEDIR/opi-install.state"
SUDO_USER="$ME" OPI_PROC_ROOT="$PROC" OPI_STATE_DIR="$STATEDIR" \
    OPI_FONT_DIR="$FONTDIR" PATH="$BIN_FAIL:$BIN:$PATH" \
    sh "$POST" > "$TMP/post-fail.out" 2>&1
check "helper 失败 postinstall exit 0" [ "$?" -eq 0 ]
check "helper 失败仍拉起实例" grep -Fqx -- "--statusbar" "$TMP/runuser.argv"
check "helper 失败状态已清" [ ! -e "$STATEDIR/opi-install.state" ]

# 旧包升级过渡：旧 prerm 已杀实例 ⇒ state 为空；仅升级时按状态栏自启项补拉
AUTOSTART="$TMP/opi-statusbar.desktop"
printf '[Desktop Entry]\nExec=%s --statusbar\n' "$TMP/opi-candidates" > "$AUTOSTART"
migrate() { env -u OPI_NO_RESTART_FCITX5 OPI_PROC_ROOT="$PROC" OPI_STATE_DIR="$STATEDIR" \
    OPI_AUTOSTART_FILE="$AUTOSTART" OPI_FONT_DIR="$FONTDIR" PATH="$BIN:$PATH" "$@"; }
rm -f "$TMP/runuser.argv"; : > "$STATEDIR/opi-install.state"
SUDO_USER="$ME" migrate sh "$POST" configure 1.15.1 > "$TMP/migrate.out" 2>&1
check "旧版升级迁移 exit 0" [ "$?" -eq 0 ]
check "旧版升级迁移拉起状态栏" grep -Fqx -- "--statusbar" "$TMP/runuser.argv"
check "旧版升级迁移有日志" grep -q "从旧版升级" "$TMP/migrate.out"
rm -f "$TMP/runuser.argv"; : > "$STATEDIR/opi-install.state"
SUDO_USER="$ME" migrate sh "$POST" 2 > "$TMP/migrate-rpm.out" 2>&1
check "rpm 升级(2)迁移拉起状态栏" grep -Fqx -- "--statusbar" "$TMP/runuser.argv"
rm -f "$TMP/runuser.argv"; : > "$STATEDIR/opi-install.state"
SUDO_USER="$ME" migrate sh "$POST" configure > "$TMP/migrate-install.out" 2>&1
check "首次安装不启动状态栏" [ ! -e "$TMP/runuser.argv" ]
check "首次安装无迁移日志" sh -c "! grep -q '从旧版升级' '$TMP/migrate-install.out'"

# ---- 7. 真实私有 dbus：Restart 的行为级证据 ----
if command -v fcitx5 >/dev/null 2>&1 && command -v dbus-run-session >/dev/null 2>&1 \
   && command -v busctl >/dev/null 2>&1; then
    cat > "$TMP/private-case.sh" <<'PRIVATE'
#!/bin/sh
set -u
T=$1; HELPER=$2; MYUID=$3
mkdir -p "$T/cfg" "$T/data" "$T/run" "$T/home" "$T/proc"
export XDG_CONFIG_HOME="$T/cfg" XDG_DATA_HOME="$T/data" \
       XDG_RUNTIME_DIR="$T/run" HOME="$T/home"
LOG="$T/fcitx5.log"

kill_private() {
    owner="$(busctl --user status org.fcitx.Fcitx5 2>/dev/null | sed -n 's/^PID=//p')"
    [ -n "$owner" ] && kill "$owner" 2>/dev/null
    # Restart 后 pid 会变：按精确 environ 匹配兜底（绝不匹配本机真实实例）
    for d in /proc/[0-9]*; do
        [ "$(cat "$d/comm" 2>/dev/null)" = "fcitx5" ] || continue
        tr '\0' '\n' < "$d/environ" 2>/dev/null | grep -Fqx "XDG_CONFIG_HOME=$T/cfg" \
            && kill "${d##*/}" 2>/dev/null
    done
}
trap 'kill_private; kill "$pid" 2>/dev/null; wait "$pid" 2>/dev/null' EXIT

fcitx5 --ui=testui > "$LOG" 2>&1 &
pid=$!

i=0
while [ "$i" -lt 150 ]; do
    if busctl --user call org.freedesktop.DBus /org/freedesktop/DBus \
        org.freedesktop.DBus NameHasOwner s org.fcitx.Fcitx5 2>/dev/null | grep -q true; then
        break
    fi
    sleep 0.1
    i=$((i + 1))
done
if [ "$i" -ge 150 ]; then
    echo "fcitx5 未在私有总线启动"
    tail -30 "$LOG"
    exit 1
fi

before_unload="$(grep -c 'Unloading addon' "$LOG")"
before_loaded="$(grep -c 'Loaded addon' "$LOG")"

# 合成 /proc：只让 helper 看见这个私有实例（uid/总线都来自当前会话）
mkdir -p "$T/proc/$pid"
printf 'fcitx5\n' > "$T/proc/$pid/comm"
printf 'Uid:\t%s\t%s\t%s\t%s\nState:\tS (test)\n' \
    "$MYUID" "$MYUID" "$MYUID" "$MYUID" > "$T/proc/$pid/status"
printf 'DBUS_SESSION_BUS_ADDRESS=%s\nXDG_RUNTIME_DIR=%s\n' \
    "$DBUS_SESSION_BUS_ADDRESS" "$XDG_RUNTIME_DIR" | tr '\n' '\000' \
    > "$T/proc/$pid/environ"

OPI_PROC_ROOT="$T/proc" sh "$HELPER" > "$T/helper.out" 2>&1
rc=$?
echo "helper rc=$rc"; cat "$T/helper.out"
[ "$rc" -eq 0 ] || exit 1
grep -q '已热重启 fcitx5' "$T/helper.out" || { echo "helper 没报成功"; exit 1; }

# 服务仍在：同名总线回答 NameHasOwner（Restart 是自重启，旧 pid 会消失）
owner="$(busctl --user status org.fcitx.Fcitx5 2>/dev/null | sed -n 's/^PID=//p')"
echo "started pid=$pid  owner-after-restart=$owner"
[ -n "$owner" ] || { echo "Restart 后没有总线名 owner"; exit 1; }
kill -0 "$owner" 2>/dev/null || { echo "Restart 后 owner 进程不存活"; exit 1; }
tr '\0' '\n' < "/proc/$owner/environ" 2>/dev/null | grep -Fqx "XDG_CONFIG_HOME=$T/cfg" \
    || { echo "重启后的实例不是本私有会话的（拒绝下结论）"; exit 1; }

after_unload="$(grep -c 'Unloading addon' "$LOG")"
after_loaded="$(grep -c 'Loaded addon' "$LOG")"
echo "Unloading addon: $before_unload -> $after_unload"
echo "Loaded addon:    $before_loaded -> $after_loaded"
[ "$after_unload" -gt "$before_unload" ] || { echo "日志没有 Unloading addon"; exit 1; }
[ "$after_loaded" -gt "$before_loaded" ] || { echo "日志没有重新 Loaded addon"; exit 1; }
exit 0
PRIVATE
    if dbus-run-session -- sh "$TMP/private-case.sh" "$TMP/priv" "$HELPER" "$MYUID" \
        > "$TMP/private.out" 2>&1; then
        ok "真实私有 dbus：Restart 返回 0 + Unloading/Loaded + 服务存活"
        grep -E 'helper rc=|owner-after-restart=|Unloading addon:|Loaded addon:' "$TMP/private.out" || true
    else
        bad "真实私有 dbus 用例"
        cat "$TMP/private.out"
    fi
else
    echo "skip - 真实私有 dbus 用例（缺 fcitx5 / dbus-run-session / busctl）"
fi

# ---- 8. 可选：真实 dpkg 升级序列（独立脚本；需 docker + debian:12）----
# 旧包→新版（state 为空时迁移兜底）与新→新（prerm upgrade 不杀、preinst 停+记、
# postinst 按记录拉起、卸载才停）两种序列都在脚本里跑真 dpkg。
DPKG_TEST="$HERE/test-restart-fcitx5-dpkg.sh"
if [ -f "$DPKG_TEST" ] && command -v docker >/dev/null 2>&1 \
   && docker image inspect debian:12 >/dev/null 2>&1; then
    timeout 200 sh "$DPKG_TEST" > "$TMP/dpkg.out" 2>&1
    rc=$?
    case "$rc" in
        0) ok "真实 dpkg：旧→新迁移兜底 + 新→新升级/卸载序列"
           grep -E 'before-upgrade|after-upgrade|new-to-new|pid=|upgrade-log:|remove-log:' "$TMP/dpkg.out" || true ;;
        2) echo "skip - 真实 dpkg 升级序列（docker/镜像不可用）"; tail -2 "$TMP/dpkg.out" ;;
        *) bad "真实 dpkg 升级序列（容器）"; cat "$TMP/dpkg.out" ;;
    esac
else
    echo "skip - 真实 dpkg 升级序列（缺 docker / debian:12 镜像 / 测试脚本）"
fi

# ---- 9. 可选：生产 root 路径（独立脚本；需 docker + debian:12 + 容器内 apt）----
ROOT_TEST="$HERE/test-restart-fcitx5-root.sh"
if [ -f "$ROOT_TEST" ] && command -v docker >/dev/null 2>&1 \
   && docker image inspect debian:12 >/dev/null 2>&1; then
    timeout 260 sh "$ROOT_TEST" > "$TMP/root.out" 2>&1
    rc=$?
    case "$rc" in
        0) ok "root 生产路径（容器）：SUDO_USER → runuser → Restart，Unloading/Loaded 增加"
           grep -E 'bus addr=|helper rc=|Unloading addon:|Loaded addon:|已热重启' "$TMP/root.out" || true ;;
        2) echo "skip - root 生产路径（容器内 apt 不可用）"; tail -2 "$TMP/root.out" ;;
        *) bad "root 生产路径（容器）"; cat "$TMP/root.out" ;;
    esac
else
    echo "skip - root 生产路径（缺 docker / debian:12 镜像 / 测试脚本）"
fi

echo
if [ "$fails" -eq 0 ]; then
    echo "全部通过"
else
    echo "失败 $fails 项" >&2
    exit 1
fi
