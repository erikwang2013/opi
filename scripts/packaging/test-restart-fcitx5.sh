#!/bin/sh
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# 单测：preinstall / postinstall 的 fcitx5「停 → 起」逻辑。
#
# **绝不碰本机真实的 /proc 与 /run**：两个脚本都支持 OPI_PROC_ROOT / OPI_STATE_DIR
# 测试钩子，这里指向合成树。本机桌面正跑着 fcitx5 与 opi-candidates（ps 可见），
# 用真 /proc 跑的测试会把它们杀掉 —— 所以这组用例只认合成树。
#
# 覆盖：
#   1. 三个包脚本 sh -n；
#   2. 无 SUDO_USER ⇒ 不停任何 fcitx5、不写状态；
#   3. 有 SUDO_USER ⇒ 只停 uid 匹配且带 HOME/显示/总线的实例（僵尸、别的 uid、
#      缺总线的实例都不动），状态格式逐键核对；
#   4. postinstall 用记录的 env 以桌面用户身份 runuser … fcitx5 -d --replace；
#      成功/失败分支都 exit 0，状态文件读完即删。
#
# 用法：sh scripts/packaging/test-restart-fcitx5.sh   （非 root 可跑）
set -u

# 被测试脚本在 root 下会忽略测试钩子（改用真 /proc、真 /run）——那会把本机正在跑的
# fcitx5 当成夹具杀掉。必须用普通用户跑。
if [ "$(id -u)" -eq 0 ]; then
    echo "本测试必须用普通用户跑（root 下脚本会忽略 OPI_PROC_ROOT/OPI_STATE_DIR）" >&2
    exit 1
fi

HERE="$(cd "$(dirname "$0")" && pwd)"
PRE="$HERE/preinstall.sh"
POST="$HERE/postinstall.sh"
PREUN="$HERE/preremove.sh"

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
# check <描述> <命令...>
check() { desc="$1"; shift; if "$@" >/dev/null 2>&1; then ok "$desc"; else bad "$desc"; fi; }

ME="$(id -un)"
MYUID="$(id -u)"
PROC="$TMP/proc"
STATEDIR="$TMP/state"
BIN="$TMP/bin"
mkdir -p "$PROC" "$STATEDIR" "$BIN"

# 假 fcitx5（postinstall 用 command -v 找它）与假 runuser（记录 argv 后直接成功）
printf '#!/bin/sh\nexit 0\n' > "$BIN/fcitx5"
chmod +x "$BIN/fcitx5"
{
    echo '#!/bin/sh'
    echo "printf '%s\\n' \"\$@\" > '$TMP/runuser.argv'"
    echo 'exit 0'
} > "$BIN/runuser"
chmod +x "$BIN/runuser"

# ---- 1. 语法 ----
for f in "$PRE" "$POST" "$PREUN"; do
    check "sh -n $(basename "$f")" sh -n "$f"
done

# ---- 2. 合成 /proc 夹具 ----
# 两个真实 sleep 进程充当「活着的 fcitx5」：一个可重启，一个缺会话总线必须不动。
sleep 300 & SLEEP1=$!
sleep 300 & SLEEP2=$!

mk_status() { # $1=pid $2=uid $3=state-char
    printf 'Uid:\t%s\t%s\t%s\t%s\nState:\t%s (test)\n' \
        "$2" "$2" "$2" "$2" "$3" > "$PROC/$1/status"
}
mk_environ() { # stdin = KEY=VALUE 行；转成 NUL 分隔的 environ
    tr '\n' '\000' > "$PROC/$1/environ"
}

mkdir "$PROC/$SLEEP1"
printf 'fcitx5\n' > "$PROC/$SLEEP1/comm"
mk_status "$SLEEP1" "$MYUID" S
printf 'HOME=/fake/home\nDISPLAY=:7\nDBUS_SESSION_BUS_ADDRESS=unix:path=/tmp/fake-bus\nXAUTHORITY=/fake/home/.Xauthority\nXDG_RUNTIME_DIR=/run/user/%s\nLANG=zh_CN.UTF-8\n' "$MYUID" \
    | mk_environ "$SLEEP1"

# 缺 DBUS_SESSION_BUS_ADDRESS ⇒ preinstall 必须跳过（不停、不写记录）
mkdir "$PROC/$SLEEP2"
printf 'fcitx5\n' > "$PROC/$SLEEP2/comm"
mk_status "$SLEEP2" "$MYUID" S
printf 'HOME=/fake/home\nDISPLAY=:7\n' | mk_environ "$SLEEP2"

# 僵尸（comm 仍是 fcitx5）与别的 uid 都不能被选中
mkdir "$PROC/999998"
printf 'fcitx5\n' > "$PROC/999998/comm"
mk_status 999998 "$MYUID" Z
printf 'HOME=/fake/home\nDISPLAY=:7\nDBUS_SESSION_BUS_ADDRESS=unix:path=/tmp/dead-bus\n' \
    | mk_environ 999998

mkdir "$PROC/999997"
printf 'fcitx5\n' > "$PROC/999997/comm"
mk_status 999997 0 S
printf 'HOME=/root\nDISPLAY=:0\nDBUS_SESSION_BUS_ADDRESS=unix:path=/tmp/root-bus\n' \
    | mk_environ 999997

# ---- 3. 无 SUDO_USER：只记日志，不动进程 ----
env -u SUDO_USER OPI_PROC_ROOT="$PROC" OPI_STATE_DIR="$STATEDIR" PATH="$BIN:$PATH" \
    sh "$PRE" > "$TMP/pre-nouser.out" 2>&1
check "无 SUDO_USER 时 exit 0" [ "$?" -eq 0 ]
check "无 SUDO_USER 时不动 fcitx5" kill -0 "$SLEEP1"
check "无 SUDO_USER 时不写状态" [ ! -e "$STATEDIR/opi-install.fcitx5" ]
check "无 SUDO_USER 时有日志" grep -q "无 SUDO_USER" "$TMP/pre-nouser.out"

# ---- 4. 有 SUDO_USER：停匹配实例，写块格式状态 ----
SUDO_USER="$ME" OPI_PROC_ROOT="$PROC" OPI_STATE_DIR="$STATEDIR" PATH="$BIN:$PATH" \
    sh "$PRE" > "$TMP/pre.out" 2>&1
check "preinstall exit 0" [ "$?" -eq 0 ]
check "preinstall 有停止日志" grep -q "已停止 1 个 fcitx5 实例" "$TMP/pre.out"
check "缺总线的实例未被停" kill -0 "$SLEEP2"

FSTATE="$STATEDIR/opi-install.fcitx5"
check "状态文件存在" [ -s "$FSTATE" ]
check "状态文件权限 0600（含会话路径）" [ "$(stat -c %a "$FSTATE")" = 600 ]
for kv in "pid=$SLEEP1" "user=$ME" "uid=$MYUID" \
          "HOME=/fake/home" "DISPLAY=:7" \
          "DBUS_SESSION_BUS_ADDRESS=unix:path=/tmp/fake-bus" \
          "XAUTHORITY=/fake/home/.Xauthority" "XDG_RUNTIME_DIR=/run/user/$MYUID" \
          "LANG=zh_CN.UTF-8"; do
    check "状态含 $kv" grep -Fqx -- "$kv" "$FSTATE"
done
check "状态只有一条记录" [ "$(grep -c '^pid=' "$FSTATE")" -eq 1 ]
check "僵尸未被记录" sh -c "! grep -q '^pid=999998$' '$FSTATE'"
check "别的 uid 未被记录" sh -c "! grep -q '^pid=999997$' '$FSTATE'"
check "缺总线的实例未被记录" sh -c "! grep -q '^pid=$SLEEP2$' '$FSTATE'"

# preinstall 的 TERM 是否真送达：wait 拿到 128+15
wait "$SLEEP1" 2>/dev/null
check "被停实例收到 SIGTERM" [ "$?" -eq 143 ]
# 合成 status 是静态文件，不会随进程消失变成 Z —— 必须手动清掉，
# 否则 postinstall 的「起来没有」会把死实例的旧夹具当成活进程。
rm -rf "$PROC/$SLEEP1"
SLEEP1=""

# SLEEP2 的夹具只用于上面「缺总线不被停」那条；清掉，否则它会冒充
# postinstall「重启后有没有起来」的阳性证据，让失败分支永远测不到。
rm -rf "$PROC/$SLEEP2"
kill "$SLEEP2" 2>/dev/null
wait "$SLEEP2" 2>/dev/null
SLEEP2=""

# 给 postinstall 的「起来没有」检查留一个活 fcitx5（真实进程不必要，读夹具即可）
mkdir "$PROC/999996"
printf 'fcitx5\n' > "$PROC/999996/comm"
mk_status 999996 "$MYUID" S

# ---- 5. postinstall：按记录的环境重启 ----
cp "$FSTATE" "$TMP/fstate.saved"
SUDO_USER="$ME" OPI_PROC_ROOT="$PROC" OPI_STATE_DIR="$STATEDIR" PATH="$BIN:$PATH" \
    sh "$POST" > "$TMP/post.out" 2>&1
check "postinstall exit 0" [ "$?" -eq 0 ]
check "postinstall 报重启成功" grep -q "已按原会话环境重启 fcitx5" "$TMP/post.out"
check "postinstall 删掉状态" [ ! -e "$FSTATE" ]

ARGV="$TMP/runuser.argv"
for kv in "HOME=/fake/home" "USER=$ME" "LOGNAME=$ME" "DISPLAY=:7" \
          "DBUS_SESSION_BUS_ADDRESS=unix:path=/tmp/fake-bus" \
          "XAUTHORITY=/fake/home/.Xauthority" "XDG_RUNTIME_DIR=/run/user/$MYUID" \
          "LANG=zh_CN.UTF-8" "$BIN/fcitx5" "-d" "--replace"; do
    check "runuser argv 含 $kv" grep -Fqx -- "$kv" "$ARGV"
done
check "runuser 用 -u <user> --" \
    sh -c "[ \"\$(sed -n '1,3p' '$ARGV' | tr '\n' ' ')\" = '-u $ME -- ' ]"

# ---- 6. 失败分支：重启后看不到进程，仍 exit 0 ----
rm -rf "$PROC/999996"
cp "$TMP/fstate.saved" "$FSTATE"
rm -f "$ARGV"
SUDO_USER="$ME" OPI_PROC_ROOT="$PROC" OPI_STATE_DIR="$STATEDIR" PATH="$BIN:$PATH" \
    sh "$POST" > "$TMP/post-fail.out" 2>&1
check "失败分支 exit 0" [ "$?" -eq 0 ]
check "失败分支有日志" grep -q "没看到存活进程" "$TMP/post-fail.out"
check "失败分支也删状态" [ ! -e "$FSTATE" ]
check "失败分支仍尝试过 runuser" [ -s "$ARGV" ]

# ---- 7. postinstall 无 SUDO_USER：不拉起、不残留 ----
cp "$TMP/fstate.saved" "$FSTATE"
rm -f "$ARGV"
env -u SUDO_USER OPI_PROC_ROOT="$PROC" OPI_STATE_DIR="$STATEDIR" PATH="$BIN:$PATH" \
    sh "$POST" > "$TMP/post-nouser.out" 2>&1
check "postinstall 无 SUDO_USER exit 0" [ "$?" -eq 0 ]
check "postinstall 无 SUDO_USER 有日志" grep -q "无 SUDO_USER" "$TMP/post-nouser.out"
check "postinstall 无 SUDO_USER 不 runuser" [ ! -e "$ARGV" ]
check "postinstall 无 SUDO_USER 清状态" [ ! -e "$FSTATE" ]

# ---- 8. 块格式：多条记录各用各的环境（本机实测同一用户可有两个 fcitx5）----
{
    printf 'pid=101\nuser=%s\nuid=%s\nHOME=/h1\nDISPLAY=:1\nDBUS_SESSION_BUS_ADDRESS=bus-one\n\n' "$ME" "$MYUID"
    printf 'pid=102\nuser=%s\nuid=%s\nHOME=/h2\nWAYLAND_DISPLAY=wayland-2\nDBUS_SESSION_BUS_ADDRESS=bus-two\n\n' "$ME" "$MYUID"
} > "$FSTATE"
{
    echo '#!/bin/sh'
    echo "printf '%s\\n' \"\$@\" >> '$TMP/runuser.multi'"
    echo 'exit 0'
} > "$BIN/runuser"
chmod +x "$BIN/runuser"
SUDO_USER="$ME" OPI_PROC_ROOT="$PROC" OPI_STATE_DIR="$STATEDIR" PATH="$BIN:$PATH" \
    sh "$POST" > "$TMP/post-multi.out" 2>&1
check "多条记录 exit 0" [ "$?" -eq 0 ]
check "两条记录各拉起一次" [ "$(grep -c '^--replace$' "$TMP/runuser.multi")" -eq 2 ]
check "第一条用 DISPLAY/HOME 1" sh -c "grep -qx 'DISPLAY=:1' '$TMP/runuser.multi' && grep -qx 'HOME=/h1' '$TMP/runuser.multi'"
check "第一条用总线 1" grep -Fqx -- "DBUS_SESSION_BUS_ADDRESS=bus-one" "$TMP/runuser.multi"
check "第二条用 WAYLAND/HOME 2" sh -c "grep -qx 'WAYLAND_DISPLAY=wayland-2' '$TMP/runuser.multi' && grep -qx 'HOME=/h2' '$TMP/runuser.multi'"
check "第二条用总线 2" grep -Fqx -- "DBUS_SESSION_BUS_ADDRESS=bus-two" "$TMP/runuser.multi"

echo
if [ "$fails" -eq 0 ]; then
    echo "全部通过"
else
    echo "失败 $fails 项" >&2
    exit 1
fi
