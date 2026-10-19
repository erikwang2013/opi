#!/bin/sh
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# 可选容器集成测试：helper 的**生产 root 路径**（主测试以非 root 跑，覆盖不到）。
#
# 需要：docker、debian:12 镜像、容器内 apt 可用（装 fcitx5 / dbus / gdbus）。
# 覆盖：真 /proc 里的 fcitx5 → SUDO_USER=testuser → id -nu → runuser →
# gdbus `org.fcitx.Fcitx.Controller1.Restart` → Unloading/Loaded addon → 服务存活。
#
# ⚠️ 容器默认没有 CAP_SYS_PTRACE，root 读不了 testuser 的 /proc/pid/environ，
# 因此走 helper 的 `/run/user/<uid>/bus` 标准兜底（正常桌面本来也是这个地址）。
# environ 直读路径没有在这里覆盖。
#
# 退出码：0 通过；1 失败；2 跳过（docker / 镜像 / apt 不可用）。
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
command -v docker >/dev/null 2>&1 || { echo "SKIP: 没有 docker"; exit 2; }
docker image inspect debian:12 >/dev/null 2>&1 || { echo "SKIP: 没有 debian:12 镜像"; exit 2; }

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
cat > "$TMP/probe.sh" <<'PROBE'
#!/bin/sh
set -u
timeout 90 apt-get update -qq >/dev/null 2>&1 \
    && timeout 90 apt-get install -y -qq --no-install-recommends fcitx5 dbus libglib2.0-bin >/dev/null 2>&1 \
    || { echo "SKIP: 容器内 apt 不可用或超时"; exit 2; }
id testuser >/dev/null 2>&1 || useradd -m testuser
uid=$(id -u testuser)
T=$(mktemp -d)
chmod 777 "$T"
mkdir -p "$T/cfg" "$T/data" "$T/home" "/run/user/$uid"
chown -R testuser "$T/cfg" "$T/data" "$T/home" "/run/user/$uid"
chmod 700 "/run/user/$uid"
addr="unix:path=/run/user/$uid/bus"
runuser -u testuser -- dbus-daemon --session --address="$addr" \
    --fork --print-address=1 --print-pid=1 > "$T/bus.info" 2>"$T/bus.err"
buspid=$(sed -n 2p "$T/bus.info")
echo "bus addr=$addr pid=$buspid"
runuser -u testuser -- env HOME="$T/home" XDG_CONFIG_HOME="$T/cfg" \
    XDG_DATA_HOME="$T/data" XDG_RUNTIME_DIR="/run/user/$uid" \
    DBUS_SESSION_BUS_ADDRESS="$addr" fcitx5 --ui=testui > "$T/fcitx5.log" 2>&1 &
i=0
out=""
while [ "$i" -lt 150 ]; do
    out=$(runuser -u testuser -- env XDG_RUNTIME_DIR="/run/user/$uid" \
        DBUS_SESSION_BUS_ADDRESS="$addr" gdbus call --session \
        --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
        --method org.freedesktop.DBus.NameHasOwner org.fcitx.Fcitx5 2>/dev/null)
    case "$out" in *true*) break ;; esac
    sleep 0.1
    i=$((i + 1))
done
echo "namehasowner after $i tries: $out"
[ "$i" -lt 150 ] || { echo "FAIL: 私有总线上没有 fcitx5"; tail -20 "$T/fcitx5.log"; exit 1; }
before_u=$(grep -c 'Unloading addon' "$T/fcitx5.log")
before_l=$(grep -c 'Loaded addon' "$T/fcitx5.log")
SUDO_USER=testuser sh /repo/scripts/packaging/restart-fcitx5.sh > "$T/helper.out" 2>&1
rc=$?
echo "helper rc=$rc"
cat "$T/helper.out"
after_u=$(grep -c 'Unloading addon' "$T/fcitx5.log")
after_l=$(grep -c 'Loaded addon' "$T/fcitx5.log")
echo "Unloading addon: $before_u -> $after_u"
echo "Loaded addon:    $before_l -> $after_l"
owner=$(runuser -u testuser -- env XDG_RUNTIME_DIR="/run/user/$uid" \
    DBUS_SESSION_BUS_ADDRESS="$addr" gdbus call --session \
    --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus \
    --method org.freedesktop.DBus.NameHasOwner org.fcitx.Fcitx5 2>/dev/null)
echo "owner after: $owner"
fail=0
[ "$rc" -eq 0 ] || fail=1
grep -q '已热重启 fcitx5（用户 testuser）' "$T/helper.out" \
    || { echo "FAIL: 没有成功日志"; fail=1; }
[ "$after_u" -gt "$before_u" ] || { echo "FAIL: 没有 Unloading addon"; fail=1; }
[ "$after_l" -gt "$before_l" ] || { echo "FAIL: 没有重新 Loaded addon"; fail=1; }
case "$owner" in *true*) ;; *) echo "FAIL: Restart 后总线无所有者"; fail=1 ;; esac
exit "$fail"
PROBE

timeout 240 docker run --rm -v "$TMP/probe.sh":/probe:ro \
    -v "$HERE":/repo/scripts/packaging:ro debian:12 sh /probe
