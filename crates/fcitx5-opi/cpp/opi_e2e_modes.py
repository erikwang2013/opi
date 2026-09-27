#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
"""opi_e2e.py 的 `modes` 变体：B0/B5 —— 桌面两端模式切换的步骤与断言。

**单独成文件**：opi_e2e.py 已贴 500 行硬线，而这一块自成一体的 —— 只依赖
ProcessKeyEvent 与"每一步产生了哪些信号"，不碰信号订阅 / IC 生命周期那些管道。
接口只有两个函数：`plan_modes(ic)` 出步骤，`check_modes(windows, failures)` 出断言
（`windows` = opi_e2e.py 的 `_windows`，`failures` = `_failures`，都由调用方传入，
故本模块不反向 import，没有循环依赖）。

**判据**全部落在"同一个键在不同模式下的结果差异"上。"有没有面板信号"不是证据：
面板重推与模式真变了是两回事，切没切都可能有信号 —— 第一版就是只看条数，
切失败与切成功分不出来。

环境隔离、私有 dbus 会话等都在 opi_e2e.py；本模块被 `python3 opi_e2e.py modes`
经 sys.path[0]（脚本所在目录）导入。
"""

import re

# xkb 修饰位（ProcessKeyEvent 的第三个参数 state）：Shift=1 Lock=2 Control=4。
# 用字面量而非 dbus 侧的枚举 —— 这一层走的是 X11 state 掩码，不是 fcitx5 的
# KeyState（后者 Ctrl 是 1<<2 = 4，**恰好同值**，但别指望下一位还撞得上）。
CTRL_MASK = 4

# 重复位：fcitx5 `KeyState::Repeat = 1U << 31`（fcitx-utils/keysym.h:57，5.0.4 起）。
# ProcessKeyEvent 的 state 参数直接落进 `KeyStates`（`Flags` 的构造函数不掩位，
# flags.h:37），故按住不放的重复事件可以从这一层构造出来。
# ⚠️ 判读：这一步红了要先排除「位 31 被 dbus 前端滤掉、退化成普通按下」——
# 那红的是环境不是代码。区分办法是**前后各有一次普通按下**（本计划里都有）：
# 只有中间那步不动、前后两步各翻一次，才说明位 31 真的到了插件。
REPEAT_MASK = 0x80000000


def plan_modes(ic):
    """B0/B5 的按键序列。返回 [(标签, 可调用, 期望)]，期望一律 None。

    期望一律 None：**内容才是结论**（结构性断言在 check_modes），条数会被
    "切没切都推一次面板"骗过去 —— 面板刷新与模式是否真变了是两回事。
    标签带 m: 前缀且各不相同：check_modes 按标签取回每一步的信号窗口。
    """
    return [
        ("m:拼音基线 n", lambda: ic.ProcessKeyEvent(0x6E, 0, 0, False, 0), None),
        ("m:Ctrl+apos 切英文",
         lambda: ic.ProcessKeyEvent(0x27, 0, CTRL_MASK, False, 0), None),
        ("m:英文下的 n", lambda: ic.ProcessKeyEvent(0x6E, 0, 0, False, 0), None),
        ("m:Ctrl+apos 切回拼音",
         lambda: ic.ProcessKeyEvent(0x27, 0, CTRL_MASK, False, 0), None),
        ("m:英文切回后的 n", lambda: ic.ProcessKeyEvent(0x6E, 0, 0, False, 0), None),
        # 按住不放的对照：带 REPEAT 位的 Ctrl+' **不许**再切一次模式（否则按住期间
        # 模式疯狂来回切）。下一步用同一个 'n' 读结果：还在拼音则入缓冲、不提交。
        ("m:Ctrl+apos 带 REPEAT 位（按住不放）",
         lambda: ic.ProcessKeyEvent(0x27, 0, CTRL_MASK | REPEAT_MASK, False, 0), None),
        ("m:REPEAT 后的 n（必须仍是拼音）",
         lambda: ic.ProcessKeyEvent(0x6E, 0, 0, False, 0), None),
        ("m:Ctrl+bslash 切符号",
         lambda: ic.ProcessKeyEvent(0x5C, 0, CTRL_MASK, False, 0), None),
    ] + [
        (f"m:符号下的 {c}",
         (lambda v: lambda: ic.ProcessKeyEvent(v, 0, 0, False, 0))(v), None)
        for c, v in (("d", 0x64), ("u", 0x75), ("n", 0x6E))
    ] + [
        ("m:Ctrl+bslash 切回拼音",
         lambda: ic.ProcessKeyEvent(0x5C, 0, CTRL_MASK, False, 0), None),
    ] + [
        (f"m:回拼音后的 {c}",
         (lambda v: lambda: ic.ProcessKeyEvent(v, 0, 0, False, 0))(v), None)
        for c, v in (("d", 0x64), ("u", 0x75), ("n", 0x6E))
    ] + [
        # 阴性对照，**必须放最后**：剪贴板面板一旦弹出来会吃掉后续按键，
        # 夹在中间会把后面每一步都污染成假红。
        # 计划原本建议 Ctrl+; 当符号触发键，实测被剪贴板模块抢走 ——
        # `strings .../fcitx5/libclipboard.so` 里就一行 `Control+semicolon`，
        # 且该模块 Category=Module + OnDemand=False，恒加载、不分输入法。
        # 这条**只打印不断言**：客户端侧唯一能看到的「被抢走」的证据是候选栏
        # 冒出剪贴板历史，而内容取决于当时剪贴板里有什么，拿它做断言是给
        # 自己埋雷。真正的锁在 opi_fcitx5.cpp 的 handleModeHotkey 注释里
        # （strings 实测的那张占用表）。
        # ⚠️ **判读陷阱**：全新会话的剪贴板历史是**空的**，于是这一步大概率
        # 只看到 `候选=0 []` —— 那是**不结论**，不是「Ctrl+; 没被占用」。要复现
        # 占用得先往剪贴板放点东西。07-xx 那次能看到 `systemctl status …` 条目，
        # 是因为宿主剪贴板里本来就有东西。
        ("m:[阴性对照] Ctrl+semi 被剪贴板占用",
         lambda: ic.ProcessKeyEvent(0x3B, 0, CTRL_MASK, False, 0), None),
    ]


# xkb 的 Shift 位（ProcessKeyEvent 第三个参数 state）：Shift=1 Lock=2 Control=4。
SHIFT_MASK = 1

# 探针键的**选取本身就是判据的一部分**（2026-09-28 拆开关后重挑）：
#
# 全角/半角与中文标点拆成两档之后（`Engine::chinese_punct` / `Engine::fullwidth`，
# 真值表见 crates/engine-core/tests/punctuation_switches.rs），`. ` `,` `\` 这些
# **表内键**由 `chinese_punct` 管，Shift+Space 再也改不动它们 —— 拿 `.` 当探针的话
# 无论全角开还是关都出 `。`，本变体会**恒红**，而且红得看不出原因。
#
# 全角档唯一还能观察到的地方是**表外键**：不在 CHINESE_PUNCT 里、落到
# `ascii_fullwidth` 机械全角（+0xFEE0）的那些。取 `^` → `＾`(U+FF3E)。
# 关掉全角后 `^` 是**直通**（`input_punct` 返回 None → 客户端收到 PassThrough），
# 不是 CommitString —— 判据因此落在「同键在不同全角态下结果不同」上。
#
# ⚠️ 「`^` 保持表外」是这条判据的**前提**。前提若破，**本变体**会变成恒绿 ——
# 往 CHINESE_PUNCT 里加任何 `^` 映射，`^` 就变成表内键、归 `chinese_punct` 管，
# Shift+Space 再也改不动它 ⇒ 四条断言无条件成立、不再测量任何东西。
#
# 但前提破**不是无人区**：引擎侧有**两道**网会在同一次改动上当场红（变异实测过：
# 加 ('^','＾') → 两条都红），所以静默的只是本变体，整个套件会先响：
#   1. crates/engine-core/tests/punctuation_switches.rs 的
#      `chinese_punct_and_fullwidth_are_independent` —— 真值表 (表开, 全角关)
#      那格对 `^` 期望 `None`；表命中会让 `or_else` 短路、得到 `＾`
#   2. crates/engine-core/src/punctuation.rs 的
#      `chinese_lookup_does_not_fall_back_to_mechanical_fullwidth` ——
#      直接断言 `chinese('^')` 是 `None`
# 这里写明前提是为了**指出它归谁守**，不是警告一片没有守卫的地方。改表前先看那两条。
FULLWIDTH_PROBE = "＾"


def plan_fullwidth(ic):
    """全角 ⇄ 半角切换键（Shift+Space）的步骤。

    判据与 `plan_modes` 同一条：**同一个键在不同全角态下的结果差异**。
    探针取表外键 `^`（理由见 `FULLWIDTH_PROBE`）：拼音模式默认全角，基线 `^`
    必然出 `＾`；按一次 Shift+Space 后同一个 `^` 必须**不再**出 `＾`；再按一次
    必须回到 `＾` —— 能进能出。
    """
    return [
        ("f:基线 ^（默认全角）",
         lambda: ic.ProcessKeyEvent(0x5E, 0, 0, False, 0), None),
        ("f:Shift+Space 关全角",
         lambda: ic.ProcessKeyEvent(0x20, 0, SHIFT_MASK, False, 0), None),
        ("f:半角下的 ^",
         lambda: ic.ProcessKeyEvent(0x5E, 0, 0, False, 0), None),
        # 按住不放的对照：带 REPEAT 位的 Shift+Space **不许**再翻一次全角位
        # （否则按住期间全角位疯狂翻）。下一步用同一个 '^' 读结果。
        ("f:Shift+Space 带 REPEAT 位（按住不放）",
         lambda: ic.ProcessKeyEvent(0x20, 0, SHIFT_MASK | REPEAT_MASK, False, 0), None),
        ("f:REPEAT 后的 ^（必须仍是半角）",
         lambda: ic.ProcessKeyEvent(0x5E, 0, 0, False, 0), None),
        ("f:Shift+Space 开全角",
         lambda: ic.ProcessKeyEvent(0x20, 0, SHIFT_MASK, False, 0), None),
        ("f:回全角后的 ^",
         lambda: ic.ProcessKeyEvent(0x5E, 0, 0, False, 0), None),
        # 回归守：**裸空格必须仍是选词键**。全角键的条件若写宽（把裸空格也吃掉），
        # 这两步会红 —— 那是「打不了字」级别的缺陷，比全角键本身重要，故与它同组。
        ("f:裸空格回归 n",
         lambda: ic.ProcessKeyEvent(0x6E, 0, 0, False, 0), None),
        ("f:裸空格回归 space",
         lambda: ic.ProcessKeyEvent(0x20, 0, 0, False, 0), None),
    ]


def _win(w, label, failures):
    """按标签取该步的信号窗口。

    **标签写错必须报出来**：`w.get(label, [])` 会把「标签打错了」表现成「这一步
    一条信号都没产生」，于是断言恒真、静默假绿 —— 第一版的 `f:REPEAT 后的 period`
    就是这么漏的（计划的标签带 `（必须仍是半角）` 后缀，检查里没写全），拆掉
    实现的阴性对照一跑才发现：该红的没红。这一步是本模块唯一的取数口，都走它。
    """
    if label not in w:
        failures.append(f"标签打错（不是信号缺失）：{label!r} 不在实际步骤标签里")
        return []
    return w[label]


def check_fullwidth(windows, failures):
    """全角键的结构性断言。判据全部落在**同一个键在不同全角态下的结果差异**上。

    「有没有面板信号」不是证据（同 check_modes）：面板重推与全角位真变了是两回事。
    """
    w = dict(windows)

    def commits(label):
        return "".join(p for n, p in _win(w, label, failures) if n == "CommitString")

    # 1) 基线不成立就直接返回 —— 拿一个本来就错的基线做对照，比不做还糟。
    if FULLWIDTH_PROBE not in commits("f:基线 ^（默认全角）"):
        failures.append(
            f"基线不成立：拼音模式默认全角，表外键 '^' 应提交 '＾'，实际 "
            f"{w.get('f:基线 ^（默认全角）')!r}"
        )
        return
    # 2) 关：切到半角后同一个 '^' 必须**不再**出 '＾'（直通给客户端）。
    if FULLWIDTH_PROBE in commits("f:半角下的 ^"):
        failures.append(
            f"全角键没生效：半角态下 '^' 仍提交了 '＾'，"
            f"实际 {w.get('f:半角下的 ^')!r}"
        )
    # 2b) REPEAT 对照：按住不放补发的重复事件**不许**再翻一次全角位。
    #     判据与第 2 条同一条（同一个 '^' 的结果差异），故不依赖任何新观察点。
    if FULLWIDTH_PROBE in commits("f:REPEAT 后的 ^（必须仍是半角）"):
        failures.append(
            f"REPEAT 没排除：带重复位的 Shift+Space 又翻了一次全角位 —— "
            f"按住不放会疯狂翻。实际 {w.get('f:REPEAT 后的 ^（必须仍是半角）')!r}"
        )
    # 3) 开：再按一次必须回到全角 —— 「能进能出」，只进不出等于换个地方卡住。
    if FULLWIDTH_PROBE not in commits("f:回全角后的 ^"):
        failures.append(
            f"全角键出不来：再按一次 Shift+Space 后 '^' 仍不出 '＾'，"
            f"实际 {w.get('f:回全角后的 ^')!r}"
        )
    # 4) 裸空格没被偷：拼音下 'n' + 空格应提交一个汉字（选首候选）。
    #    这条守的是本改动唯一会造成「打不了字」的方向。
    if not re.search(r"[一-鿿]", commits("f:裸空格回归 space")):
        failures.append(
            f"裸空格回归失败：'n' + 空格应提交汉字，实际 "
            f"{w.get('f:裸空格回归 space')!r} —— 全角键把裸空格也吃了？"
        )


def check_modes(windows, failures):
    """B0/B5 结构性断言。

    判据全部落在**同一个键在不同模式下的结果差异**上。「有没有面板信号」不是
    证据：面板重推与模式真变了是两回事，切没切都可能有信号 —— 第一版就是只
    看条数，切失败与切成功分不出来。
    """
    w = dict(windows)

    def commits(label):
        return [p for n, p in _win(w, label, failures) if n == "CommitString"]

    def last_csui(label):
        """该步**最后**一条面板 payload（多键一步时末态才是结论）。"""
        got = [p for n, p in _win(w, label, failures) if n == "UpdateClientSideUI"]
        return got[-1] if got else ""

    def preedit(label):
        m = re.search(r'preedit="([^"]*)"', last_csui(label))
        return m.group(1) if m else ""

    def cands(label):
        """候选列表那一段。payload 形如
        `preedit="dun" 预编辑光标=3 候选光标=0 候选=1 ['、']` —— 必须按
        " 候选=" 切掉前面，否则 hanzi() 会把「预编辑光标」这几个**标签字**
        当成候选里的汉字（第一版就是这么红的）。"""
        return last_csui(label).split(" 候选=", 1)[-1]

    def hanzi(label):
        return re.findall(r"[一-鿿]", cands(label))

    # 1) 基线：拼音下 'n' 入缓冲（推预编辑）、**不提交**。基线不成立就直接返回 ——
    #    拿一个本来就错的基线做对照，比不做还糟。
    if commits("m:拼音基线 n") or preedit("m:拼音基线 n") != "n":
        failures.append(
            f"基线不成立：拼音下 'n' 应入缓冲不提交，实际 {w.get('m:拼音基线 n')}")
        return
    # 2) B0 进：英文模式空缓冲 + 字母 = **直传上屏**（input_method.rs 的
    #    Mode::English 分支）。切成功之后同一个 'n' 必须提交 "n"。
    got = commits("m:英文下的 n")
    if got != ["n"]:
        failures.append(f"B0 切英文未生效：'n' 应提交 ['n']，实际 {got}")
    # 3) B0 出：再按一次必须切回拼音，'n' 重新入缓冲、不提交。
    if commits("m:英文切回后的 n"):
        failures.append(
            f"B0 出不来：切回拼音后 'n' 不应提交，"
            f"实际 {commits('m:英文切回后的 n')}")
    # 3b) REPEAT 对照：按住 Ctrl+' 补发的重复事件**不许**再切一次模式。
    #     判据同第 2/3 条：模式若切到了英文，同一个 'n' 会直传上屏（提交 "n"）；
    #     还在拼音则入缓冲、不提交（第 1 条已证）。
    if commits("m:REPEAT 后的 n（必须仍是拼音）"):
        failures.append(
            f"REPEAT 没排除：带重复位的 Ctrl+' 又切了一次模式（切到英文后 'n' "
            f"会直传上屏）—— 按住不放会疯狂来回切。"
            f"实际 {commits('m:REPEAT 后的 n（必须仍是拼音）')!r}"
        )
    # 4) B5 进：符号模式的候选**只出符号、不与词库合并**（candidates.rs
    #    rank_and_pick 对 Mode::Symbol 直接 return symbols.search）。「一个汉字
    #    都没有」是符号模式独有的指纹，且不依赖词库收了哪些字 —— 换词库照样成立。
    if not _win(w, "m:符号下的 n", failures):
        failures.append("B5 切符号后 'dun' 一条信号都没有（模式没切过去）")
    elif hanzi("m:符号下的 n"):
        failures.append(
            f"B5 切符号未生效：候选里仍有汉字 {'、'.join(hanzi('m:符号下的 n'))}"
            f" —— 说明还在拼音模式")
    elif "、" not in cands("m:符号下的 n"):
        failures.append(
            f"B5 切符号后 'dun' 候选里没有 '、'，"
            f"实际 {cands('m:符号下的 n')!r}")
    # 5) B5 出：切回拼音后同样的 'dun' 必须**出汉字**（符号模式给不出汉字，
    #    所以这条与第 4 条互为反证，不靠词库里具体是哪个字）。
    if not hanzi("m:回拼音后的 n"):
        failures.append(
            f"B5 出不来：切回拼音后 'dun' 候选里没有汉字，"
            f"实际 {cands('m:回拼音后的 n')!r}")
