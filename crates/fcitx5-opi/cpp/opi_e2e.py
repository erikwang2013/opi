#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""端到端验证：在**真 fcitx5 守护进程**里选中 opi，送按键，看它推出什么信号。

手工 harness，不进 CI（CI 既没有 fcitx5 也没有 dbus 会话）。跑法：用同目录
run-harness.sh（它会建好私有 dbus 会话、独立 XDG_CONFIG_HOME、addons 目录、
词库），或自己起好守护进程后直接：
    dbus-run-session -- bash -c '...起 fcitx5...; python3 opi_e2e.py basic'

前置条件（照着 README「验证 harness」一节做，否则看到的是假故障）：
  - fcitx5 5.1.12 运行中，`--ui=testui --enable opi_fcitx5`（testui 会把其它
    addon 全禁掉，不额外 --enable 则本 addon 根本不加载）
  - **独立 XDG_CONFIG_HOME**，其 profile 的输入法组里必须**同时**有
    keyboard-us 和目标 IM。否则 SetCurrentIM("opi") 静默无效（不报错、不切换、
    连 CurrentIM 信号都不发），表现为「按键被吃了但一个字都不出」。
  - 本脚本必须在**私有** dbus 会话里跑（`dbus-run-session`），否则会连到用户
    真正的 fcitx5 实例上去。

模式：
  basic        逐键 n i h a o space，声明面板能力位，打印全部信号（默认）
  page         n 之后对比 UI 的 NextPage/PrevPage 与 PageUp/PageDown 键
  caps <0x..>  用指定 capability 跑 basic（对照面板去向，见下）

信号即「用户最终能看见什么」：CommitString / UpdateClientSideUI(预编辑+候选)。
⚠️ 面板**去向由能力位决定**：有 ClientSideInputPanel(1<<39) 才推给客户端
（才有 UpdateClientSideUI 可看），没有则推给 UI addon —— 而 --ui=testui 不
打印面板内容，于是两边都看不见，看着就像「什么都没发生」。
"""

import sys

import dbus
import dbus.mainloop.glib
from gi.repository import GLib

DEST = "org.fcitx.Fcitx5"
IC_IFACE = "org.fcitx.Fcitx.InputContext1"

# CapabilityFlag: Preedit(1<<1) | FormattedPreedit(1<<4) | ClientSideInputPanel(1<<39)
CAPS_PANEL = (1 << 1) | (1 << 4) | (1 << 39)

KEYS = [("n", 0x6E), ("i", 0x69), ("h", 0x68), ("a", 0x61), ("o", 0x6F),
        ("space", 0x20)]

events = []
_fd_keepalive = None  # 见 open_input_context()：不握住这个 fd，IC 会被销毁


def fmt_csui(args):
    """UpdateClientSideUI(preedit a(si), cursor i, aux, aux, cands a(ss), ...)."""
    preedit = "".join(str(seg[0]) for seg in args[0])
    cands = [str(c[1]) for c in args[4]]
    return f'preedit="{preedit}" cursor={int(args[1])} 候选={len(cands)} {cands}'


def open_input_context(bus):
    global _fd_keepalive
    im = dbus.Interface(
        bus.get_object(DEST, "/org/freedesktop/portal/inputmethod"),
        "org.fcitx.Fcitx.InputMethod1",
    )
    path, fd = im.CreateInputContext([("program", "opi-e2e"), ("display", "e2e")])
    # CreateInputContext 的第二个返回值是 IC 的生命线 fd：一旦被 GC 关掉，IC 立刻
    # 销毁，后续调用全变 UnknownObject。必须在整个运行期持有引用。
    _fd_keepalive = fd
    path = str(path)
    ic = dbus.Interface(bus.get_object(DEST, path), IC_IFACE)
    for sig, handler in (
        ("CommitString", lambda s: events.append(("CommitString", str(s)))),
        ("UpdateClientSideUI",
         lambda *a: events.append(("UpdateClientSideUI", fmt_csui(a)))),
        ("UpdatePreedit",
         lambda *a: events.append(("UpdatePreedit", str([str(x) for x in a])))),
        ("CurrentIM",
         lambda *a: events.append(("CurrentIM", str([str(x) for x in a])))),
    ):
        bus.add_signal_receiver(handler, signal_name=sig,
                                dbus_interface=IC_IFACE, path=path)
    return path, ic


def plan(mode, ic):
    """返回 [(标签, 可调用)]；可调用为 None 表示只打印一行分隔。"""
    if mode == "page":
        return [
            ("输入 'n'", lambda: ic.ProcessKeyEvent(0x6E, 0, 0, False, 0)),
            # UI 的翻页接口：本轮实测是空操作（见 README 已知边界）
            ("UI NextPage()", lambda: ic.NextPage()),
            ("UI PrevPage()", lambda: ic.PrevPage()),
            ("键 PageDown(0xff56)", lambda: ic.ProcessKeyEvent(0xFF56, 0, 0, False, 0)),
            ("键 PageUp(0xff55)", lambda: ic.ProcessKeyEvent(0xFF55, 0, 0, False, 0)),
        ]
    return [
        (f"key {name!r} (0x{val:x})",
         (lambda v: lambda: ic.ProcessKeyEvent(v, 0, 0, False, 0))(val))
        for name, val in KEYS
    ]


def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else "basic"
    if mode == "caps":
        caps = int(sys.argv[2], 0) if len(sys.argv) > 2 else 0
    elif mode in ("basic", "page"):
        caps = CAPS_PANEL
    else:
        print(f"未知模式 {mode!r}；可用: basic | page | caps <0x..>")
        return 2

    dbus.mainloop.glib.DBusGMainLoop(set_as_default=True)
    bus = dbus.SessionBus()
    path, ic = open_input_context(bus)
    print(f"[e2e] 模式={mode} 输入上下文 {path}", flush=True)

    ctrl = dbus.Interface(bus.get_object(DEST, "/controller"),
                          "org.fcitx.Fcitx.Controller1")

    ic.SetCapability(dbus.UInt64(caps))
    print(f"[e2e] SetCapability(0x{caps:x})", flush=True)
    ic.FocusIn()
    print("[e2e] FocusIn", flush=True)
    ctrl.SetCurrentIM("opi")
    print("[e2e] SetCurrentIM('opi')", flush=True)

    steps = plan(mode, ic)

    def report(i, label, ret, before):
        print(f"--- {label} -> 返回 {ret}", flush=True)
        new = events[before:]
        for name, payload in new:
            print(f"    {name}: {payload}", flush=True)
        if not new:
            print("    （无面板更新信号）", flush=True)
        GLib.timeout_add(50, step, i)
        return False

    def step(i):
        if i >= len(steps):
            print(f"[e2e] --- 全部信号（{len(events)} 条）---", flush=True)
            for name, payload in events:
                print(f"  {name}: {payload}", flush=True)
            if not events:
                print("  （一个信号都没有）", flush=True)
            GLib.timeout_add(200, loop.quit)
            return False
        label, fn = steps[i]
        before = len(events)
        try:
            ret = fn()
        except Exception as exc:  # noqa: BLE001
            ret = f"异常 {exc}"
        GLib.timeout_add(300, report, i + 1, label, ret, before)
        return False

    GLib.timeout_add(600, step, 0)
    loop.run()
    return 0


loop = GLib.MainLoop()
sys.exit(main())
