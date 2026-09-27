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
  passthrough  ESC/Right/F5 这类直通键**不该**重推面板（期望 0 条信号）
  caps <0x..>  用指定 capability 跑 basic（对照面板去向，见下）

信号即「用户最终能看见什么」：
  CommitString           提交进客户端
  UpdateClientSideUI     客户端自己画面板时才发（预编辑+光标+候选+候选光标）
  UpdateFormattedPreedit 客户端有 Preedit 能力时的**内联预编辑**通道

⚠️ 面板**去向由能力位决定**，两条通道互不替代：
  - `ClientSideInputPanel(1<<39)`   → 引擎推的 InputPanel 交给客户端，
    走 `UpdateClientSideUI`；没有这一位则交给 UI addon。
  - `Preedit(1<<1)`                 → 内联预编辑走 `UpdateFormattedPreedit`。
    不看这一位，客户端既看不到内联预编辑，切窗口时已输入的拼音还会**静默丢失**
    （core 对「有 clientPreedit 的客户端失焦」有默认提交行为）。
  而 `--ui=testui` 不打印面板内容，所以「推给 UI addon」那条投递路径**没有
  可观测记录**，可观测的只有客户端这两条。

断言（不再是只打印）：每个按键步骤都要求「该客户端按能力位应收到的信号」
至少一条（`page` 的 UI NextPage/PrevPage、`passthrough` 的直通键是**已知不该
发信号**的，反过来要求 **0 条**）。任何一步不符即退出码 1 —— 「订阅了一个不存在
的信号所以永远静默」这类问题就是这样被发现的（见 SIGNALS 的注释）。
"""

import re
import sys

import dbus
import dbus.mainloop.glib
from gi.repository import GLib

DEST = "org.fcitx.Fcitx5"
IC_IFACE = "org.fcitx.Fcitx.InputContext1"
CONTROLLER_IFACE = "org.fcitx.Fcitx.Controller1"
IM_IFACE = "org.fcitx.Fcitx.InputMethod1"

# 订阅的信号名必须是这个接口**真有**的：dbus-python 订阅一个不存在的信号
# 不报错、只是永远静默。这里原先写的是 "UpdatePreedit" —— 5.1.12 的真名是
# "UpdateFormattedPreedit"（introspection 已确认），于是「每键都有预编辑信号」
# 从来没被验过，反倒让人以为预编辑通路已经通了。check_signals_known() 启动时
# 对着 introspection 核对，写错就直接红，不再靠人看输出。
SIGNALS = ("CommitString", "UpdateClientSideUI", "UpdateFormattedPreedit",
           "CurrentIM")

# CapabilityFlag（fcitx-utils/capabilityflags.h）
CAP_PREEDIT = 1 << 1
CAP_FORMATTED_PREEDIT = 1 << 4
CAP_CLIENT_SIDE_PANEL = 1 << 39
CAPS_PANEL = CAP_PREEDIT | CAP_FORMATTED_PREEDIT | CAP_CLIENT_SIDE_PANEL

KEYS = [("n", 0x6E), ("i", 0x69), ("h", 0x68), ("a", 0x61), ("o", 0x6F),
        ("space", 0x20)]

PAGE_DOWN = 0xFF56
PAGE_UP = 0xFF55

events = []
_fd_keepalive = None  # 见 open_input_context()：不握住这个 fd，IC 会被销毁
_failures = []


def fmt_csui(args):
    """UpdateClientSideUI(preedit a(si), cursor i, auxUp a(si), auxDown a(si),
    candidates a(ss), candidateCursor i, layoutHint i, hasPrev b, hasNext b)。

    `cursor` 是**预编辑串内**的光标（引擎 setCursor 的那个）；`candidateCursor`
    才是候选栏的高亮位（CandidateList::cursorIndex，-1 = 无高亮）。
    """
    preedit = "".join(str(seg[0]) for seg in args[0])
    cands = [str(c[1]) for c in args[4]]
    return (f'preedit="{preedit}" 预编辑光标={int(args[1])} '
            f'候选光标={int(args[5])} 候选={len(cands)} {cands}')


def check_signals_known(bus, path):
    """核对订阅的信号名都在接口的 introspection 里。返回缺失的名字列表。"""
    obj = bus.get_object(DEST, path)
    xml = str(obj.Introspect(dbus_interface="org.freedesktop.DBus.Introspectable"))
    known = set(re.findall(r'<signal name="([^"]+)"', xml))
    return [s for s in SIGNALS if s not in known], sorted(known)


def open_input_context(bus):
    global _fd_keepalive
    im = dbus.Interface(
        bus.get_object(DEST, "/org/freedesktop/portal/inputmethod"), IM_IFACE,
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
        ("UpdateFormattedPreedit",
         lambda *a: events.append((
             "UpdateFormattedPreedit",
             f'preedit="{"".join(str(seg[0]) for seg in a[0])}" '
             f'预编辑光标={int(a[1])}'))),
        ("CurrentIM",
         lambda *a: events.append(("CurrentIM", str([str(x) for x in a])))),
    ):
        bus.add_signal_receiver(handler, signal_name=sig,
                                dbus_interface=IC_IFACE, path=path)
    return path, ic


def plan(mode, ic, caps):
    """返回 [(标签, 可调用, 期望)]；期望 True=至少一条信号 False=0 条 None=不管。"""
    panel_channel = bool(caps & (CAP_PREEDIT | CAP_CLIENT_SIDE_PANEL))
    if mode == "page":
        return [
            ("输入 'n'", lambda: ic.ProcessKeyEvent(0x6E, 0, 0, False, 0), True),
            # UI 的翻页接口：本轮实测是空操作（见 README 已知边界）
            ("UI NextPage()", lambda: ic.NextPage(), False),
            ("UI PrevPage()", lambda: ic.PrevPage(), False),
            (f"键 PageDown(0x{PAGE_DOWN:x})",
             lambda: ic.ProcessKeyEvent(PAGE_DOWN, 0, 0, False, 0), True),
            (f"键 PageUp(0x{PAGE_UP:x})",
             lambda: ic.ProcessKeyEvent(PAGE_UP, 0, 0, False, 0), True),
        ]
    if mode == "passthrough":
        # 直通键（Rust 侧 action==0）：不消费按键、引擎状态不变。期望 **0 条**
        # 面板信号 —— 内容与上一次完全相同，重推只是白付一次跨 FFI + 客户端重绘。
        # 缓冲必须非空才测得到：空缓冲时这些键走的是「直传客户端」另一条路。
        return [
            ("输入 'ni'", lambda: ic.ProcessKeyEvent(0x6E, 0, 0, False, 0), True),
            ("输入 'i'", lambda: ic.ProcessKeyEvent(0x69, 0, 0, False, 0), True),
        ] + [
            (f"直通键 {name}(0x{val:x})，缓冲不变",
             (lambda v: lambda: ic.ProcessKeyEvent(v, 0, 0, False, 0))(val), False)
            for name, val in (("ESC", 0x1B), ("Right", 0xFF53), ("F5", 0xFFC2))
        ]
    return [
        (f"key {name!r} (0x{val:x})",
         (lambda v: lambda: ic.ProcessKeyEvent(v, 0, 0, False, 0))(val),
         True if panel_channel else None)
        for name, val in KEYS
    ]


def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else "basic"
    if mode == "caps":
        caps = int(sys.argv[2], 0) if len(sys.argv) > 2 else 0
    elif mode in ("basic", "page", "passthrough"):
        caps = CAPS_PANEL
    else:
        print(f"未知模式 {mode!r}；可用: basic | page | passthrough | caps <0x..>")
        return 2

    dbus.mainloop.glib.DBusGMainLoop(set_as_default=True)
    bus = dbus.SessionBus()
    path, ic = open_input_context(bus)
    print(f"[e2e] 模式={mode} 输入上下文 {path}", flush=True)

    missing, known = check_signals_known(bus, path)
    if missing:
        # 订阅不存在的信号不会报错，只会永远静默 —— 这正是「预编辑通路其实没通
        # 却一直显示正常」的成因。这里直接失败，别让静默变成假绿。
        print(f"[e2e] !! 订阅了本接口不存在的信号 {missing}", flush=True)
        print(f"[e2e]    本接口真有的信号: {known}", flush=True)
        return 1

    ctrl = dbus.Interface(bus.get_object(DEST, "/controller"), CONTROLLER_IFACE)

    ic.SetCapability(dbus.UInt64(caps))
    print(f"[e2e] SetCapability(0x{caps:x})", flush=True)
    ic.FocusIn()
    print("[e2e] FocusIn", flush=True)
    ctrl.SetCurrentIM("opi")
    print("[e2e] SetCurrentIM('opi')", flush=True)

    steps = plan(mode, ic, caps)

    def report(i, label, ret, before, expect):
        print(f"--- {label} -> 返回 {ret}", flush=True)
        new = events[before:]
        for name, payload in new:
            print(f"    {name}: {payload}", flush=True)
        if not new:
            print("    （无面板更新信号）", flush=True)
        # 断言：不是「打印一下就完」。期望一条没有、或期望 0 条却来了，都记失败。
        if expect is True and not new:
            _failures.append(f"{label}: 期望至少一条信号，实际 0 条")
        elif expect is False and new:
            names = ", ".join(n for n, _ in new)
            _failures.append(f"{label}: 期望 0 条信号（已知空操作），实际 {names}")
        GLib.timeout_add(50, step, i)
        return False

    def step(i):
        if i >= len(steps):
            print(f"[e2e] --- 全部信号（{len(events)} 条）---", flush=True)
            for name, payload in events:
                print(f"  {name}: {payload}", flush=True)
            if not events:
                print("  （一个信号都没有）", flush=True)
            if _failures:
                print(f"[e2e] !! 断言失败 {len(_failures)} 项:", flush=True)
                for f in _failures:
                    print(f"  - {f}", flush=True)
            else:
                print("[e2e] 断言全过", flush=True)
            GLib.timeout_add(200, loop.quit)
            return False
        label, fn, expect = steps[i]
        before = len(events)
        try:
            ret = fn()
        except Exception as exc:  # noqa: BLE001
            ret = f"异常 {exc}"
        GLib.timeout_add(300, report, i + 1, label, ret, before, expect)
        return False

    GLib.timeout_add(600, step, 0)
    loop.run()
    return 1 if _failures else 0


loop = GLib.MainLoop()
sys.exit(main())
