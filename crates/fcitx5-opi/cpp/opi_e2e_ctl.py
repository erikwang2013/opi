#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""opi-ctl 通道的端到端变体（2026-10-10）—— 从 opi_e2e.py 分出来，同
opi_e2e_modes.py 的先例（那个文件贴 500 行硬线）。

状态栏按钮走的就是这条链，**只有在这里**四样同时为真：

    desktop-ui → ProcessBuilder(/usr/bin/opi-ctl <action>) → dbus-send --session
      → 私有会话总线 → 守护进程里 addon 挂的 /opi（org.fcitx.Fcitx5.Opi1.Trigger）
      → OpiEngine::handleCtlAction_ → 回复 "ok" / "err:<码>:<原因>" → opi-ctl 退出码

  · 真守护进程（addon 真的把 /opi 注册上了）、真会话总线（dbus-send 真的到得了）、
    真 opi-ctl 二进制（它的退出码映射真跑了一遍）。
  C++ 驱动（opi_ctl_driver）只能测 handleCtlAction_ 本身；opi-ctl 的 cargo 测试
  只能测它的客户侧（拿假 dbus-send 喂应答）。**两端的字面契约**（路径/接口/方法名/
  应答形状）只有本变体覆盖 —— 写错任一处，前面的测试全绿，用户的按钮没反应。

断言（都是「两端对上没有」，不是「引擎行为对不对」——后者归 opi_ctl_driver）：
  · lang 的 W1：先打 nihao，再 opi-ctl lang —— IC 必须收到 CommitString「你好」；
    然后英文模式真的生效（再按字母直传上屏）—— 这两条合起来证明「作用于当前 IC」
    真的作用到了那个 IC 上。
  · trad/切回：退出码 0 且 stderr 为空（应答被正确解析成成功）。
  · 未知动作走**线上**：dbus-send 直接发 string:bogus，回复必须是 "err:5:未知动作…"
    —— 这是服务端回复形态，跨进程协议的另一半（opi-ctl 把它映射成退出码 5 的行为
    在它的 cargo 测试里）。
  · symbols：**真链路成功面**（2026-10-10 引擎侧落地后）：run-harness 把入库的
    symbol_categories.tsv 布置进 XDG 数据目录 ⇒ 必须 rc=0；且「浏览已开」要能**看见**
    —— 引擎契约 ① 说浏览态劫持 buffer() 为标题（一级「符号」），glue 的成功分支
    紧接着 refreshingPanelFor(ic) 推面板 ⇒ 客户端应收到带「符号」标题的面板信号。
    再补一步「浏览中按 '1'」：类目下钻的二级标题同样要推到客户端。
    失败面（未接线 / 数据缺失）由 opi_ctl_driver §7e 在**没有**数据的那份 XDG 目录里
    用假 hook 测 —— 那两条在本变体里已经不可达（入口已接线、数据已布置）。
"""

import os
import subprocess

# run-harness.sh 编好并传进来的 opi-ctl（cargo 产物）。缺了不是本变体的错：
# 报清楚、让 check 记一条失败（别静默跳过 —— 「没测」会被读成「测了且过」）。
CTL = os.environ.get("OPI_CTL_BIN", "")
DEST = "org.fcitx.Fcitx5"
OPI_PATH = "/opi"
OPI_IFACE = "org.fcitx.Fcitx5.Opi1"

# 每次调用的结果：{标签: (rc, stdout, stderr)}。信号窗口（_windows）只装 dbus 信号，
# 装不下进程退出码，故这里单独记一份 —— check_ctl 两样都读。
RESULTS = {}


def run_ctl(label, action):
    """跑一次真 opi-ctl。返回一句话给 report 打印。"""
    if not CTL or not os.path.exists(CTL):
        RESULTS[label] = (-1, "", f"OPI_CTL_BIN 没指到可执行文件：{CTL!r}")
        return f"!! 找不到 opi-ctl（OPI_CTL_BIN={CTL!r}）—— run-harness.sh 会编它"
    proc = subprocess.run([CTL, action], capture_output=True, text=True, timeout=20)
    RESULTS[label] = (proc.returncode, proc.stdout.strip(), proc.stderr.strip())
    return (f"退出码 {proc.returncode} stdout={proc.stdout.strip()!r} "
            f"stderr={proc.stderr.strip()!r}")


def run_dbus_send(label, arg):
    """绕过 opi-ctl，直接按线上形态发一条 —— 验服务端的回复协议本身。"""
    argv = ["dbus-send", "--session", "--print-reply", "--reply-timeout=5000",
            f"--dest={DEST}", OPI_PATH, f"{OPI_IFACE}.Trigger", f"string:{arg}"]
    proc = subprocess.run(argv, capture_output=True, text=True, timeout=20)
    RESULTS[label] = (proc.returncode, proc.stdout.strip(), proc.stderr.strip())
    return (f"退出码 {proc.returncode} stdout={proc.stdout.strip()!r} "
            f"stderr={proc.stderr.strip()!r}")


def plan_ctl(ic):
    """步骤表。每个 opi-ctl 调用的 expect 都是 None：这些动作的**结果**是提交文本
    与退出码（在 check_ctl 里断言），面板推不推是另一件事，钉死条数只会白红。"""
    key = lambda v: (lambda: ic.ProcessKeyEvent(v, 0, 0, False, 0))  # noqa: E731
    steps = [
        ("输入 'n'", key(0x6E), True),
        ("输入 'i'", key(0x69), True),
        ("输入 'h'", key(0x68), True),
        ("输入 'a'", key(0x61), True),
        ("输入 'o'", key(0x6F), True),
        ("opi-ctl lang（切英文）", lambda: run_ctl("lang1", "lang"), None),
        ("英文模式：字母 'x'", key(0x78), None),
        ("opi-ctl lang（切回拼音）", lambda: run_ctl("lang2", "lang"), None),
        ("opi-ctl trad", lambda: run_ctl("trad", "trad"), None),
        ("opi-ctl trad 切回", lambda: run_ctl("trad2", "trad"), None),
        ("线上未知动作（dbus-send string:bogus）",
         lambda: run_dbus_send("bogus", "bogus"), None),
        ("opi-ctl symbols", lambda: run_ctl("symbols", "symbols"), None),
        # 浏览中按 '1'：引擎侧 1-9 只在**浏览态**有专属路由（类目下钻/符号上屏），
        # 非浏览态它只是普通数字键 —— 它的面板结果因此是「浏览真的开着」的进一步
        # 证据，而不是「动作被接受了」的重复。
        ("浏览中按 '1'（下钻类目 1）", key(0x31), None),
    ]
    return steps


def _commits(events):
    return [p for n, p in events if n == "CommitString"]


def check_ctl(windows, failures):
    by_label = {label: events for label, events in windows}
    print(f"[e2e/ctl] 调用结果: {RESULTS}", flush=True)

    # 1) lang 的 W1：待定拼音必须**提交进那个 IC**（「作用于当前 IC」的实锤：
    #    handleCtlAction_ 取的 mostRecentInputContext() 要真的是这个 IC）。
    lang_events = by_label.get("opi-ctl lang（切英文）", [])
    lang_commits = _commits(lang_events)
    if "你好" not in lang_commits:
        failures.append(
            f"opi-ctl lang 之后 IC 没收到 CommitString「你好」（实际 {lang_commits}）"
            f" —— W1 提交没作用到当前 IC 上")
    else:
        print(f"[e2e/ctl] lang 的 W1 提交到位：{lang_commits}", flush=True)

    # 2) 英文模式真的切了：下一个字母直传上屏（拼音模式下它会进缓冲、不提交）。
    x_commits = _commits(by_label.get("英文模式：字母 'x'", []))
    if "x" not in x_commits:
        failures.append(
            f"lang 之后按 'x' 没有直传上屏（实际 {x_commits}）—— 模式没真的切到英文")

    # 3) 正向：应答被解析成成功（退出码 0 + stderr 干净）。trad/lang 都不依赖
    #    引擎侧符号浏览的实现，是本变体的**阳性对照** —— 少了它，「symbols 拿到了
    #    err:4」分不清是「符号入口未落地」还是「/opi 根本没注册、总线上谁都找不到」。
    for label, what in (("lang1", "opi-ctl lang"), ("trad", "opi-ctl trad"),
                        ("trad2", "opi-ctl trad 切回")):
        rc, _out, err = RESULTS.get(label, (None, "", ""))
        if rc != 0:
            failures.append(f"{what} 退出码 {rc}（应 0），stderr={err!r}")
        elif err:
            failures.append(f"{what} 退出码 0 但 stderr 有内容（应静默）：{err!r}")

    # 4) 线上回复协议：服务端对未知动作必须回 err:5:…（这一条不经过 opi-ctl，
    #    验的是 /opi 的应答形状本身）。
    rc, out, err = RESULTS.get("bogus", (None, "", ""))
    if "err:5" not in out:
        failures.append(
            f"未知动作的线上回复不是 err:5（rc={rc} stdout={out!r} stderr={err!r}）"
            f" —— 两端共享的回复协议对不上了")

    # 5) symbols：真链路成功面（见模块头）。rc 必须 0，且「浏览已开」要能**看见**：
    #    引擎契约 ① 说浏览态劫持 buffer() 为标题（一级「符号」），glue 的成功分支
    #    紧接着 refreshingPanelFor(ic) 推面板 ⇒ 这一步的信号窗口里必须有带「符号」
    #    字样的面板信号。rc==0 而窗口空 = 「按钮报成功、用户看不见」的静默形态。
    #    关键词只钉「符号」不钉整串标题：标题格式（「符号·<名>」）归引擎侧，
    #    格式漂了不该让本变体红 —— 红的是「标题整个没了」这件事。
    def panel_texts(label):
        return [p for n, p in by_label.get(label, []) if n.startswith("Update")]

    rc, out, err = RESULTS.get("symbols", (None, "", ""))
    if rc != 0:
        failures.append(
            f"opi-ctl symbols 退出码 {rc}（应为 0：引擎入口已接线，且 run-harness 已把 "
            f"symbol_categories.tsv 布置进 XDG 数据目录）：stderr={err!r} stdout={out!r}")
    else:
        print(f"[e2e/ctl] symbols 返回 0（真入口 + 真数据）：{out!r}", flush=True)
        sym_panel = panel_texts("opi-ctl symbols")
        titled = [p for p in sym_panel if "符号" in p]
        if not sym_panel:
            failures.append(
                "opi-ctl symbols 返回 0 但客户端一条面板信号都没有 —— 浏览开了却看不见"
                "（glue 成功分支应 refreshingPanelFor(ic) 把标题推出去）")
        elif not titled:
            failures.append(f"面板信号里没有「符号」标题（浏览态没推到客户端）：{sym_panel}")
        else:
            print(f"[e2e/ctl] 浏览已开且推到客户端：{titled[0]!r}", flush=True)
        drill = panel_texts("浏览中按 '1'（下钻类目 1）")
        d_titled = [p for p in drill if "符号" in p]
        if not d_titled:
            failures.append(
                f"浏览中按 '1' 没有带「符号」标题的面板更新：{drill}"
                f" —— 浏览态的键路由（类目下钻）没走到面板")
        else:
            print(f"[e2e/ctl] 下钻生效（二级标题推到客户端）：{d_titled[0]!r}", flush=True)
