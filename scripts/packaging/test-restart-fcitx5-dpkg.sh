#!/bin/sh
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# 可选容器集成测试：真实 dpkg 下的两种升级路径（M3）。
#
# 6a) 旧包 → 本版：旧包（1.15.1 形态）的 prerm 在 upgrade 时无条件杀掉
#     opi-candidates，且不写 state；新 preinst 随后找不到进程 ⇒ state 为空。断言：
#     · 旧 prerm 确实杀了实例（模拟过渡的失败前提）；
#     · 新 postinst 在 state 为空 + 升级参数下按状态栏自启项补拉，实例以 testuser 存活；
#     · 卸载时新 prerm 仍照常停实例。
# 6b) 新 → 新：新 prerm upgrade 不杀；preinst 停+记；postinst 按记录拉起。
#
# 需要：docker + debian:12 镜像（不联网）。
#
# 需要：docker + debian:12 镜像（不联网）。
# 退出码：0 通过；1 失败；2 跳过（docker / 镜像不可用）。
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
command -v docker >/dev/null 2>&1 || { echo "SKIP: 没有 docker"; exit 2; }
docker image inspect debian:12 >/dev/null 2>&1 || { echo "SKIP: 没有 debian:12 镜像"; exit 2; }

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
cat > "$TMP/probe.sh" <<'PROBE'
#!/bin/sh
set -u
id testuser >/dev/null 2>&1 || useradd -m testuser
T=$(mktemp -d)
pids_named() { # $1 = comm
    for d in /proc/[0-9]*; do
        [ "$(cat "$d/comm" 2>/dev/null)" = "$1" ] || continue
        st=$(awk '/^State:/{print substr($2,1,1); exit}' "$d/status" 2>/dev/null)
        [ "$st" = Z ] && continue
        echo "${d##*/}"
    done
}
pids() { pids_named opi; }
pids_old() { pids_named opi-candidates; }
control() {
    printf 'Package: opi-test\nVersion: %s\nArchitecture: all\nMaintainer: t <t@t>\nDescription: t\n' "$1" > "$2/DEBIAN/control"
}
# ---- 旧包：只有无条件 prerm（1.15.1 的升级行为）----
mkdir -p "$T/v1/DEBIAN" "$T/v1/usr/bin"
cp /bin/sleep "$T/v1/usr/bin/opi-candidates"
cat > "$T/v1/DEBIAN/prerm" <<'OLD'
#!/bin/sh
pids() {
    for d in /proc/[0-9]*; do
        [ "$(cat "$d/comm" 2>/dev/null)" = opi-candidates ] || continue
        echo "${d##*/}"
    done
}
if [ -n "$(pids)" ]; then
    for pid in $(pids); do kill -TERM "$pid" 2>/dev/null; done
    i=0
    while [ -n "$(pids)" ] && [ "$i" -lt 30 ]; do sleep 0.1; i=$((i + 1)); done
    for pid in $(pids); do kill -KILL "$pid" 2>/dev/null; done
fi
exit 0
OLD
chmod 755 "$T/v1/DEBIAN/prerm"
control 1.15.1 "$T/v1"
dpkg-deb --build "$T/v1" "$T/v1.deb" >/dev/null || exit 1

# ---- 新包：工作区里的脚本 + 状态栏自启项 ----
mkdir -p "$T/v2/DEBIAN" "$T/v2/usr/bin" "$T/v2/usr/libexec/opi" "$T/v2/etc/xdg/autostart"
cp /repo/scripts/packaging/preinstall.sh "$T/v2/DEBIAN/preinst"
cp /repo/scripts/packaging/postinstall.sh "$T/v2/DEBIAN/postinst"
cp /repo/scripts/packaging/preremove.sh "$T/v2/DEBIAN/prerm"
cp /repo/scripts/packaging/restart-fcitx5.sh "$T/v2/usr/libexec/opi/restart-fcitx5.sh"
chmod 755 "$T/v2/DEBIAN/"* "$T/v2/usr/libexec/opi/restart-fcitx5.sh"
cp /bin/sleep "$T/v2/usr/bin/opi"
# 假二进制是 sleep，Exec 用合法间隔；字面量 `--statusbar` 的传参由主测试的假
# runuser 断言（这里若写 --statusbar，sleep 会立即报参数错退出）。
printf 'Exec=/usr/bin/opi 300\n' > "$T/v2/etc/xdg/autostart/opi-statusbar.desktop"
control 1.16.0 "$T/v2"
dpkg-deb --build "$T/v2" "$T/v2.deb" >/dev/null || exit 1

fail=0
dpkg -i "$T/v1.deb" >"$T/i1.log" 2>&1 || { echo "FAIL: 旧包装不上"; cat "$T/i1.log"; exit 1; }
# 用子 shell 双 fork 起旧实例：旧 prerm 杀掉它时父 shell 不会打 "Terminated" 噪声。
( runuser -u testuser -- /usr/bin/opi-candidates 300 >/dev/null 2>&1 & )
sleep 0.5
old=$(pids_old)
[ -n "$old" ] || { echo "FAIL: 旧包实例没起来"; exit 1; }
SUDO_USER=testuser dpkg -i "$T/v2.deb" >"$T/i2.log" 2>&1 \
    || { echo "FAIL: 升级失败"; cat "$T/i2.log"; exit 1; }

i=0
while [ -z "$(pids)" ] && [ "$i" -lt 20 ]; do sleep 0.1; i=$((i + 1)); done
np=$(pids)
echo "before-upgrade pids=$old after-upgrade pids=$np"
[ -n "$np" ] || { echo "FAIL: 升级后没有实例（迁移兜底没生效）"; fail=1; }
[ -z "$(pids_old)" ] || { echo "FAIL: 升级后旧版进程还在"; fail=1; }
for p in $np; do
    uid=$(awk '/^Uid:/{print $2; exit}' "/proc/$p/status" 2>/dev/null)
    cmd=$(tr '\0' ' ' < "/proc/$p/cmdline" 2>/dev/null)
    echo "  pid=$p uid=${uid:-?} cmd=$cmd"
    [ "${uid:-0}" != 0 ] || { echo "FAIL: 实例以 root 运行"; fail=1; }
    case "$cmd" in "/usr/bin/opi "*) ;; *) echo "FAIL: 不是自启项拉起的新实例"; fail=1 ;; esac
done
grep -q '从旧版升级：已按状态栏自启项恢复' "$T/i2.log" \
    || { echo "FAIL: postinst 没走迁移兜底"; fail=1; }
grep -q '升级完成后按记录自动拉起' "$T/i2.log" \
    && { echo "FAIL: 新 preinst 竟然记录到了实例（旧 prerm 没先杀？）"; fail=1; }

dpkg -r opi-test >"$T/r.log" 2>&1 || { echo "FAIL: 卸载失败"; cat "$T/r.log"; exit 1; }
[ -z "$(pids)" ] || { echo "FAIL: 卸载后实例还在"; fail=1; }
grep -q '卸载中' "$T/r.log" || { echo "FAIL: 卸载时 prerm 没停实例"; fail=1; }
echo "upgrade-log: $(tr '\n' '|' < "$T/i2.log")"
echo "remove-log:  $(tr '\n' '|' < "$T/r.log")"

# ---- 6b) 新 → 新：正常升级路径 ----
dpkg -i "$T/v2.deb" >"$T/n1.log" 2>&1 || { echo "FAIL: 新包首次安装失败"; cat "$T/n1.log"; exit 1; }
( runuser -u testuser -- /usr/bin/opi 300 >/dev/null 2>&1 & )
sleep 0.5
[ -n "$(pids)" ] || { echo "FAIL: 新包实例没起来"; exit 1; }
control 1.17.0 "$T/v2"
dpkg-deb --build "$T/v2" "$T/v3.deb" >/dev/null || exit 1
SUDO_USER=testuser dpkg -i "$T/v3.deb" >"$T/n2.log" 2>&1 \
    || { echo "FAIL: 新→新升级失败"; cat "$T/n2.log"; exit 1; }
i=0
while [ -z "$(pids)" ] && [ "$i" -lt 20 ]; do sleep 0.1; i=$((i + 1)); done
nn=$(pids)
echo "new-to-new after-upgrade pids=$nn"
[ -n "$nn" ] || { echo "FAIL: 新→新升级后没有实例"; fail=1; }
grep -q '升级完成后按记录自动拉起' "$T/n2.log" \
    || { echo "FAIL: 新→新升级 preinst 没停/记"; fail=1; }
grep -q '卸载中' "$T/n2.log" \
    && { echo "FAIL: 新→新升级时 prerm 抢先杀了实例"; fail=1; }
echo "new-to-new-log: $(tr '\n' '|' < "$T/n2.log")"
exit "$fail"
PROBE

timeout 120 docker run --rm -v "$TMP/probe.sh":/probe:ro \
    -v "$HERE":/repo/scripts/packaging:ro debian:12 sh /probe
