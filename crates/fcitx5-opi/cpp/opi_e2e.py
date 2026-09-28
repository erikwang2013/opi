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
    addon 全禁掉）
    ⚠️ 订正（2026-09-28 实测）：**`--enable opi_fcitx5` 不是本 addon 加载的原因。**
    单变量对照 —— 两份只差「组里有没有 opi 那一项」的 profile，**两臂都不带
    `--enable`**：组里有 → 加载 1 次、`nihao` 提交「你好」；组里没有 → 加载 0 次、
    每键返回 0、`SetCurrentIM("opi")` 静默无效。**真正的触发条件是「opi 在 profile
    的输入法组里 + 切到它」** —— 也就是紧跟其后的那条前置条件本身。
    （本行原写「不额外 --enable 则本 addon 根本不加载」，与实测不符。）
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
  punct [im]   标点模块探针：送 `,` `.` `;`，打印客户端收到的 CommitString。
               im 缺省 opi，可传 keyboard-us 等做**对照**（见下）
  punctopi     决定性探针：先切 pinyin（libpinyin 会去加载 libpunctuation），
               再切回 opi 送同样三个键 —— 用来分辨「标点模块是全局的、只是没人
               为 opi 加载它」与「它本来就跳过 opi」
  modes        模式热键（B0/B5）：Ctrl+' 切英文、Ctrl+; 切符号，各切进再切出。
               断言不是「有没有信号」（面板重推也会给信号），而是**同一个键在三
               种模式下的结果必须不同** —— 见 check_modes()

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

# B0/B5 的步骤与断言（同目录；脚本以绝对路径运行，sys.path[0] = 脚本所在目录）。
from opi_e2e_modes import check_fullwidth, check_modes, plan_fullwidth, plan_modes

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
_windows = []  # [(步骤标签, 该步产生的信号)]，report() 逐条追加；check_modes 用
CURRENT_IM = "opi"  # main() 按模式改写；plan() 只借它打标签
CTRL = None  # main() 里建好；punctopi 模式要用它切换 IM


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
    if mode == "punct":
        # 标点模块（libpunctuation）在 PreInputMethod 阶段劫持标点键并直接
        # commitString 全角标点。要判断的是**它对 opi 生不生效**，做法是同一份
        # 脚本跑两个 IM（opi / keyboard-us）当对照 —— memory
        # [[fcitx5-local-verification-harness]] 的教训：「测不出来」不是关于被测
        # 对象的证据，必须有一个**已知会出标点**的对照组。字符本身是结论，故
        # expect=None 只打印；结构性断言（每键恰好 1 条 CommitString）见 main()。
        return [
            (f"IM={CURRENT_IM} 键 ',' (0x2c)",
             lambda: ic.ProcessKeyEvent(0x2C, 0, 0, False, 0), None),
            (f"IM={CURRENT_IM} 键 '.' (0x2e)",
             lambda: ic.ProcessKeyEvent(0x2E, 0, 0, False, 0), None),
            (f"IM={CURRENT_IM} 键 ';' (0x3b)",
             lambda: ic.ProcessKeyEvent(0x3B, 0, 0, False, 0), None),
        ]
    if mode == "punctopi":
        # 机制（strings + 实测日志）：libpunctuation 是 OnDemand 模块，**core 里
        # 没有任何对它的引用**，全机只有 libpinyin.so / libtable.so 两个 IM 引擎
        # 提到它 —— 即「谁想用谁去加载」。实测：切到 pinyin 的那一瞬间日志里同时
        # 出现 `Loaded addon punctuation` 与 `Loaded addon pinyin`；opi 全程不触发
        # 加载。所以「模块存不存在」与「模块对 opi 生不生效」是两个问题，本模式
        # 把后者单独拎出来，且**同一个守护进程内**先跑阳性对照再跑待测项 ——
        # 跨进程比对照更容易被环境差异骗过去。
        return [
            ("切 pinyin（触发 libpinyin 加载 libpunctuation）",
             lambda: CTRL.SetCurrentIM("pinyin"), None),
            ("[阳性对照] IM=pinyin 键 ',' (0x2c)",
             lambda: ic.ProcessKeyEvent(0x2C, 0, 0, False, 0), None),
            ("[阳性对照] IM=pinyin 键 '.' (0x2e)",
             lambda: ic.ProcessKeyEvent(0x2E, 0, 0, False, 0), None),
            ("切 keyboard-us（模块已加载，问：模块是全局的吗）",
             lambda: CTRL.SetCurrentIM("keyboard-us"), None),
            ("[待测2] IM=keyboard-us 键 ',' (0x2c)",
             lambda: ic.ProcessKeyEvent(0x2C, 0, 0, False, 0), None),
            ("切回 opi", lambda: CTRL.SetCurrentIM("opi"), None),
            ("[待测] IM=opi 键 ',' (0x2c)",
             lambda: ic.ProcessKeyEvent(0x2C, 0, 0, False, 0), None),
            ("[待测] IM=opi 键 '.' (0x2e)",
             lambda: ic.ProcessKeyEvent(0x2E, 0, 0, False, 0), None),
        ]
    if mode == "modes":
        # B0/B5 的步骤在 opi_e2e_modes.py（本文件已贴 500 行硬线，
        # 那一块自成一体的：只依赖 ProcessKeyEvent 与每步的信号窗口）。
        return plan_modes(ic)

    if mode == "fullwidth":
        # 全角 ⇄ 半角切换键（Shift+Space），同上放模块里。
        return plan_fullwidth(ic)

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
    global CURRENT_IM, CTRL
    mode = sys.argv[1] if len(sys.argv) > 1 else "basic"
    im = "opi"
    if mode == "caps":
        caps = int(sys.argv[2], 0) if len(sys.argv) > 2 else 0
    elif mode in ("basic", "page", "passthrough"):
        caps = CAPS_PANEL
    elif mode in ("punctopi", "modes", "fullwidth"):
        caps = CAPS_PANEL
    elif mode == "punct":
        caps = CAPS_PANEL
        # 对照组的 IM 名（缺省仍是 opi）。profile 的组里同时有 keyboard-us
        # 与 opi，故 SetCurrentIM 对两者都真的生效（见 memory：不在组里会静默无效）。
        im = sys.argv[2] if len(sys.argv) > 2 else "opi"
    else:
        print(f"未知模式 {mode!r}；可用: basic | page | passthrough | "
              f"punct [im] | modes | caps <0x..>")
        return 2
    CURRENT_IM = im

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

    ctrl = CTRL = dbus.Interface(bus.get_object(DEST, "/controller"),
                                 CONTROLLER_IFACE)

    ic.SetCapability(dbus.UInt64(caps))
    print(f"[e2e] SetCapability(0x{caps:x})", flush=True)
    ic.FocusIn()
    print("[e2e] FocusIn", flush=True)
    ctrl.SetCurrentIM(CURRENT_IM)
    print(f"[e2e] SetCurrentIM({CURRENT_IM!r})", flush=True)

    steps = plan(mode, ic, caps)

    def report(i, label, ret, before, expect):
        print(f"--- {label} -> 返回 {ret}", flush=True)
        new = events[before:]
        _windows.append((label, new))
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

    def commits_by_im():
        """按 CurrentIM 信号把 CommitString 分桶：{'pinyin': [...], 'opi': [...]}。
        CurrentIM 的 payload 形如 ['OPI 拼音', 'opi', 'zh_CN']，第二项是唯一名，
        用它分流（'OPI 拼音' 里没有 'pinyin' 子串，'拼音' 里没有 'opi'）。"""
        out = {}
        bucket = None
        for name, payload in events:
            if name == "CurrentIM":
                if "'pinyin'" in payload:
                    bucket = out.setdefault("pinyin", [])
                elif "'opi'" in payload:
                    bucket = out.setdefault("opi", [])
                elif "'keyboard-us'" in payload:
                    bucket = out.setdefault("keyboard-us", [])
            elif name == "CommitString" and bucket is not None:
                bucket.append(payload)
        return out

    def check_punct():
        """punct 模式的两条结构性断言（字符本身是结论，不在此断言）。"""
        # 1) SetCurrentIM 必须真的生效。它静默失效时 IM 仍是组里的 DefaultIM
        #    （=opi），于是 keyboard-us「对照组」其实还在测 opi —— 两边结果当然
        #    相等，直接导出「标点对谁都不生效」的假结论。只有 IC 发出的
        #    CurrentIM 信号能证明切换真的发生了。
        ims = [p for n, p in events if n == "CurrentIM"]
        if not any(CURRENT_IM in p for p in ims):
            _failures.append(
                f"SetCurrentIM({CURRENT_IM!r}) 未生效（收到的 CurrentIM: {ims}）"
                f" —— 对照实验无效")
        # 2) 【2026-09-27 订正，断言已翻转】原断言写于「opi 打不出中文标点」时期：
        #    期望**就是 0 条**（把已查明的事实钉住，而不是留一条永远红的期望）。
        #    标点层（engine-core/src/punctuation.rs）落地后该前提不成立 ——
        #    按原注释留下的指示「那时是好事，把期望改成每键 1 条」翻转。
        #    只压**全角**这一条不变式：半角泄漏才是缺陷（`，` 与 `,` 是用户可
        #    分辨的差别，且在中文语境下是错的）；具体出哪个全角符号归标点表管，
        #    随表增删而变，钉死它等于每改一次标点表就红一次。
        if mode == "punct":
            commits = [p for n, p in events if n == "CommitString"]
            bad = [c for c in commits if any(ch in c for ch in ",.;")]
            if bad:
                _failures.append(
                    f"opi 标点键提交了半角字符 {bad}（应为全角），全部提交 {commits}")
            elif not commits and CURRENT_IM == "opi":
                # 只有 opi 这一支才要求「打得出」。`punct keyboard-us` 是同脚本的
                # **对照组**（同一个守护进程只是换 IM），它**就该是 0 条** ——
                # 在对照组上也要求提交，等于把对照实验判成失败。
                _failures.append(
                    "opi 标点键一条提交都没有（中文标点又没了？阳性对照见 punctopi）")
            return
        if mode != "punctopi":
            return
        # 3) punctopi：先证明「模块在本环境里确实会出全角标点」（阳性对照），
        #    再报待测项。对照组不成立时，opi 的 0 条**不能**当结论用 ——
        #    那正是 memory 里「测不出来不是关于被测对象的证据」。
        by_im = commits_by_im()
        pin, opi = by_im.get("pinyin", []), by_im.get("opi", [])
        kb = by_im.get("keyboard-us", [])
        print(f"[e2e] 分桶提交: pinyin={pin} keyboard-us={kb} opi={opi}", flush=True)
        if pin != ["，", "。"]:
            _failures.append(
                f"阳性对照失败：pinyin 下 ',' '.' 应提交 ['，','。']，实际 {pin}"
                f" —— 本环境测不出标点模块，opi 的结论无效")
            return
        # 【2026-09-27 订正】上面那条分桶此前被读成「标点模块对 opi 生效」。
        # 标点层落地后**不能**再这么读：opi 的提交来自它**自己的引擎**
        # （engine-core 的标点表），与 libpunctuation 无关 —— 换句话说，
        # 「opi 打得出中文标点」成立，「标点模块对 opi 生效」不成立，两者是不同的
        # 命题，别把后者当前者的解释。
        #
        # 真正的机制判别在 keyboard-us 这一桶：模块若真是**全局**的，那么「模块
        # 已加载」之后换任何 IM 都该出全角标点。实测它不出 → 出全角标点的从来
        # 不是这个模块（pinyin 那一桶是 **libpinyin 自己**，它加载 libpunctuation
        # 只为读 punctuationmap 配置）。故键盘-us 这桶是本变体唯一的对照。
        print(f"[e2e] 机制判别: keyboard-us（模块已加载）={'出标点' if kb else '不出标点'}",
              flush=True)
        if kb:
            _failures.append(
                "keyboard-us 也出了标点 —— 模块是全局的（本变体的机制判别失效），"
                f"那么 opi 那桶就不能用来自证引擎侧实现，实际 {kb}")
        else:
            print("[e2e] 模块**不是**全局的（keyboard-us 不出标点）；"
                  "pinyin 那桶是 libpinyin 自带的，opi 那桶是 opi 引擎自带的",
                  flush=True)
        if opi:
            print(f"[e2e] opi 出了全角标点 {opi} —— 来自 opi 自己的标点层",
                  flush=True)

    def finish():
        """收尾：打印全部信号 + 断言结论，然后退出 mainloop。只走一次。"""
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

    def step(i):
        # GLib 回调里未捕获的异常**不会**让进程退出：mainloop 继续转，脚本挂到
        # 外层 timeout 为止（实测：check_punct 里一个 `label` 写成 `_` 的
        # TypeError 让整轮 harness 卡满 500s 才被 timeout 杀掉，日志里只有
        # traceback、没有「失败」二字）。挂起是最难查的失败形态 —— 看起来像
        # 「慢」而不是「坏」—— 故任何异常都转成一条断言失败并正常收尾。
        try:
            if i >= len(steps):
                if mode in ("punct", "punctopi"):
                    check_punct()
                elif mode == "modes":
                    # windows/failures 由调用方传入，模块不反向 import。
                    check_modes(_windows, _failures)
                elif mode == "fullwidth":
                    check_fullwidth(_windows, _failures)
                finish()
                return False
            label, fn, expect = steps[i]
            before = len(events)
            try:
                ret = fn()
            except Exception as exc:  # noqa: BLE001
                ret = f"异常 {exc}"
            GLib.timeout_add(300, report, i + 1, label, ret, before, expect)
            return False
        except Exception as exc:  # noqa: BLE001
            import traceback
            traceback.print_exc()
            _failures.append(f"步骤 {i} 抛出异常: {exc!r}")
            finish()
            return False

    GLib.timeout_add(600, step, 0)
    loop.run()
    return 1 if _failures else 0


loop = GLib.MainLoop()
sys.exit(main())
