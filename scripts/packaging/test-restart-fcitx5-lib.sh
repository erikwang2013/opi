#!/bin/sh
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# test-restart-fcitx5.sh 的夹具与断言工具；**被它 source，不单独运行**。
# 单独成文件是为了守住 500 行源码门禁（crates/opi-ffi/tests/line_limit.rs）。
case "$0" in
    *test-restart-fcitx5-lib.sh)
        echo "本文件是夹具，请运行 test-restart-fcitx5.sh" >&2
        exit 1 ;;
esac

# 被测试脚本在 root 下会忽略测试钩子（改用真 /proc、真 /run）——那会把本机正在跑的
# 进程当成夹具。必须用普通用户跑。
if [ "$(id -u)" -eq 0 ]; then
    echo "本测试必须用普通用户跑（root 下脚本会忽略 OPI_PROC_ROOT 等钩子）" >&2
    exit 1
fi

PRE="$HERE/preinstall.sh"
POST="$HERE/postinstall.sh"
PREUN="$HERE/preremove.sh"
HELPER="$HERE/restart-fcitx5.sh"

TMP="$(mktemp -d)"
SLEEP1=""
SLEEP2=""
SLEEP_OLD=""
cleanup() {
    [ -n "$SLEEP1" ] && kill "$SLEEP1" 2>/dev/null
    [ -n "$SLEEP2" ] && kill "$SLEEP2" 2>/dev/null
    [ -n "$SLEEP_OLD" ] && kill "$SLEEP_OLD" 2>/dev/null
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

# ---- 夹具 ----
# 假 busctl：记录「总线环境 + 参数」并按 $TMP/bus-mode 模拟 Restart 的几种结局：
#   （空）/respawn      Restart 后出现不同的唯一名（正常自重启）
#   vanish              Restart 后进程消失且不再回来（等 fcitx5 拉起后才回来）
#   fail-call           Restart 调用报错 + 进程消失（同上恢复路径）
#   no-respawn-live     进程还在、但总线名没了（正在启动，不能重复拉起）
#   hold                唯一名一直不变（Restart 没生效）
# $TMP/launched 由假 fcitx5 创建，表示「已经被重新拉起」。
cat > "$BIN/busctl" <<EOF
#!/bin/sh
printf '%s|%s|%s\n' "\$XDG_RUNTIME_DIR" "\$DBUS_SESSION_BUS_ADDRESS" "\$*" >> "$BUSLOG"
mode=""
[ -f "$TMP/bus-mode" ] && mode="\$(cat "$TMP/bus-mode")"
n=0
[ -f "$TMP/restart-count" ] && n="\$(cat "$TMP/restart-count")"
case "\$*" in
    *GetNameOwner*)
        if [ -e "$TMP/no-owner" ]; then exit 1; fi
        if [ -e "$TMP/launched" ]; then echo 's ":1.9001"'; exit 0; fi
        if [ "\$n" -gt 0 ]; then
            case "\$mode" in
                vanish|fail-call|no-respawn-live) exit 1 ;;
                hold) echo 's ":1.4242"' ;;
                *) printf 's ":1.%s"\\n' "\$((4242 + n))" ;;
            esac
        else
            echo 's ":1.4242"'
        fi
        ;;
    *Controller1*Restart*)
        echo \$((n + 1)) > "$TMP/restart-count"
        case "\$mode" in
            # 整个 /proc 条目一起消失：fallback 不可能再从旧 pid 读 environ，
            # 只有 Restart 前的预抓取才能让拉起拿到显示环境。
            vanish|fail-call) rm -rf "$TMP/proc"/* ;;
        esac
        case "\$mode" in
            fail-call) exit 1 ;;
        esac
        ;;
esac
exit 0
EOF
chmod +x "$BIN/busctl"
printf '#!/bin/sh\nexit 1\n' > "$BIN_FAIL/busctl"
chmod +x "$BIN_FAIL/busctl"

# 假 fcitx5：记录拉起参数与环境（fallback 恢复路径用）。
cat > "$BIN/fcitx5" <<EOF
#!/bin/sh
printf '%s\n' "\$*" >> "$TMP/fcitx5.argv"
{
    printf 'XDG_RUNTIME_DIR=%s\n' "\$XDG_RUNTIME_DIR"
    printf 'DBUS_SESSION_BUS_ADDRESS=%s\n' "\$DBUS_SESSION_BUS_ADDRESS"
    printf 'DISPLAY=%s\n' "\${DISPLAY:-}"
    printf 'WAYLAND_DISPLAY=%s\n' "\${WAYLAND_DISPLAY:-}"
    printf 'XAUTHORITY=%s\n' "\${XAUTHORITY:-}"
} > "$TMP/fcitx5.env"
: > "$TMP/launched"
exit 0
EOF
chmod +x "$BIN/fcitx5"

# 假 runuser：记录 argv，并执行 `--` 之后的命令 —— postinstall 传的 `env` 赋值
# 因此真正生效，状态栏夹具记录到的是自己的环境（而不是只证明 argv 里有几个词）。
{
    echo '#!/bin/sh'
    echo "printf '%s\\n' \"\$@\" >> '$TMP/runuser.argv'"
    echo 'while [ "$#" -gt 0 ] && [ "$1" != "--" ]; do shift; done'
    echo '[ "$#" -gt 0 ] && shift'
    echo 'exec "$@"'
} > "$BIN/runuser"
chmod +x "$BIN/runuser"

fresh_proc() { rm -rf "$PROC"; mkdir -p "$PROC"; }
# 标准会话总线夹具：$RUN_ROOT/<uid>/bus（生产是 socket，测试用普通文件）
fresh_bus() { rm -rf "$RUN"; mkdir -p "$RUN/$MYUID"; : > "$RUN/$MYUID/bus"; }
reset_bus() {
    : > "$BUSLOG"
    rm -f "$TMP/restart-count" "$TMP/launched" "$TMP/bus-mode" "$TMP/no-owner"
    rm -f "$TMP/fcitx5.argv" "$TMP/fcitx5.env"
}
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
mk_env() { # pid key value（追加到合成 environ）
    printf '%s=%s\000' "$2" "$3" >> "$PROC/$1/environ"
}
run_helper() { # 输出到 $TMP/helper.out；OPI_WAIT_TICKS 缩短失败路径的等待
    env -u OPI_NO_RESTART_FCITX5 -u SUDO_USER -u PKEXEC_UID -u DOAS_USER \
        OPI_PROC_ROOT="$PROC" OPI_RUN_ROOT="$RUN" OPI_WAIT_TICKS=5 \
        PATH="$BIN:$PATH" sh "$HELPER" > "$TMP/helper.out" 2>&1
}
run_helper_fail() {
    env -u OPI_NO_RESTART_FCITX5 OPI_PROC_ROOT="$PROC" OPI_RUN_ROOT="$RUN" \
        OPI_WAIT_TICKS=5 PATH="$BIN_FAIL:$PATH" \
        sh "$HELPER" > "$TMP/helper.out" 2>&1
}
