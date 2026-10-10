#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""opi-ctl 通道的端到端变体（2026-10-10）—— 从 opi_e2e.py 分出来，同
opi_e2e_modes.py 的先例（那个文件贴 500 行硬线）；③① 与公用底层又分给
opi_e2e_ctl_conf.py（本文件再贴线），② im 与基础通道面留在本文件。

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
  · lang 的 W1：先打 nihao，再 opi-ctl lang —— IC 必须收到 CommitString **「nihao」
    （缓冲原文）**，并带负对照「不是首候选「你好」」；然后英文模式真的生效（再按
    字母直传上屏）—— 这几条合起来证明「作用于当前 IC」真的作用到了那个 IC 上。
    （2026-10-10 深夜订正：原断言是「必须收到「你好」」—— 白天 lead 裁决「有候选上
    首候选」，同夜用户裁决「切英文 = 默认是拼音，不要是候选词」后桌面统一为一律
    原文，本断言随之**有意**改写；移动端同裁决已先改并实证。同夜追加：一律原文
    **按模式分叉**，符号模式不上关键字 —— 本臂是拼音面，不受分叉影响；分叉的
    正控/负对照在 opi_ctl_driver §1/§2b。）
  · trad/切回：退出码 0 且 stderr 为空（应答被正确解析成成功）。
  · state（只读查询，桌面的轮询就是它）：无参 State 方法在真总线上可调；每行按
    **位置**解析（键序固定，桌面按位置取值）；切 trad 前后**只有 trad 格**变、
    再切回逐字回到基线（两向）；lang 格随 lang 动作变；通道不通时 rc=3 且 stdout
    为空（可与 0/4 区分）。
  · 未知动作走**线上**：dbus-send 直接发 string:bogus，回复必须是 "err:5:未知动作…"
    —— 这是服务端回复形态，跨进程协议的另一半（opi-ctl 把它映射成退出码 5 的行为
    在它的 cargo 测试里）。
  · symbols：**真链路成功面**（2026-10-10 引擎侧落地后）：run-harness 把入库的
    symbol_categories.tsv 布置进 XDG 数据目录 ⇒ 必须 rc=0；且「浏览已开」要能**看见**
    —— 引擎契约 ① 说浏览态劫持 buffer() 为标题（一级「符号」），glue 的成功分支
    紧接着 refreshingPanelFor(ic) 推面板 ⇒ 客户端应收到带「符号」标题的面板信号。
    再补一步「浏览中按 '1'」：类目下钻的二级标题同样要推到客户端。
    失败面（未接线 / 数据缺失）由 opi_ctl_driver §7e 在**符号数据装载失败**的那份
    XDG 目录里用假 hook 测（data-nosym：零字节 tsv，见 run-harness.sh 的 stage 4 段）
    —— 那两条在本变体里已经不可达（入口已接线、数据已布置）。

  · （2026-10-10 加）③ punct 动作 = 运行期切换 + **写回配置**：opi-ctl punct 之后
    conf 文件里的 ChinesePunct 跟着翻面、state 的 punct 段与磁盘值一致（conf 与
    活状态永远同一 —— 「显示与实际一致」的根治），再点一次两条都回原值。
  · （2026-10-10 加）① ReloadAddonConfig 真链路（设置窗保存那条 wire）：**只改
    conf 不重载** → state 与 '.' 键的引擎行为都不动（对照）；调 ReloadAddonConfig
    后 → state 的 punct 段随 conf 变，且 '.' 键的提交从「。」变「．」（拼音全角
    档的中文/机械标点分界 —— 引擎面证据，证明推的是活引擎、不只是配置内存）；
    Jianpin 走同一 applyOpiSettings：关掉重载后 "nh" 不再出「你好」，开回来即还原。
  · （2026-10-10 加）② im 格：state 行尾的 im = 当前输入法唯一名 —— 切到
    keyboard-us 后跟着变、切回跟着回，且只有这一格变（其余四格逐字不变；切换的
    前提由 IC 自己发的 CurrentIM 信号证明）。销毁 IC（InputContext1.DestroyIC ——
    实测 CreateInputContext 第二返回值是 token 不是 fd，IC 生死跟创建连接走）后
    state 照常回一行、rc 0；im 那一刻的值 = fcitx5 的 currentInputMethod()
    （mostRecentInputContext 的 IM）：没有 IC 时看 ShareInputState —— All 给
    dummy IC 兜底（= 下一扇窗会拿到的 IM，非空），No 才是**空串**（前端约定：
    未知 = 维持现状）。两档都在本变体里**差分**测（写全局 config +
    ReloadConfig 活生效），不赌机器默认是哪档。
"""

import os

# 本变体的公用底层与 ③①（配置面：写回配置 / 重载推送）在 opi_e2e_ctl_conf.py
# —— 那边是下发层，本文件（通道面 + ② im）反向 import 它，单向依赖不回头。
from opi_e2e_ctl_conf import (DEAD_BUS_ADDR, RESULTS, STATE_FIELDS, _commits,
                              _global_conf_path, check_conf_arms, drop_share_state,
                              field_diff, plan_conf_arms, run_controller, run_ctl,
                              run_dbus_send, run_global_reload, write_share_state)


def plan_ctl(ic, destroy=None):
    """步骤表。每个 opi-ctl 调用的 expect 都是 None：这些动作的**结果**是提交文本
    与退出码（在 check_ctl 里断言），面板推不推是另一件事，钉死条数只会白红。
    `destroy` = 销毁 IC 的回调（opi_e2e.py 传进来：走 IC 接口的 DestroyIC）
    —— ② 的「没有 IC」面要真销毁，必须是最后一步。"""
    key = lambda v: (lambda: ic.ProcessKeyEvent(v, 0, 0, False, 0))  # noqa: E731

    steps = [
        ("输入 'n'", key(0x6E), True),
        ("输入 'i'", key(0x69), True),
        ("输入 'h'", key(0x68), True),
        ("输入 'a'", key(0x61), True),
        ("输入 'o'", key(0x6F), True),
        ("opi-ctl lang（切英文）", lambda: run_ctl("lang1", "lang"), None),
        ("opi-ctl state（英文侧）", lambda: run_ctl("state_en", "state"), None),
        ("英文模式：字母 'x'", key(0x78), None),
        ("opi-ctl lang（切回拼音）", lambda: run_ctl("lang2", "lang"), None),
        ("opi-ctl state（trad 切换前基线）", lambda: run_ctl("state1", "state"), None),
        ("opi-ctl trad", lambda: run_ctl("trad", "trad"), None),
        ("opi-ctl state（trad 切换后）", lambda: run_ctl("state2", "state"), None),
        ("opi-ctl trad 切回", lambda: run_ctl("trad2", "trad"), None),
        ("opi-ctl state（trad 再切回后）", lambda: run_ctl("state3", "state"), None),
    ]
    # ③①（写回配置 / 重载推送）的步骤在 opi_e2e_ctl_conf.py —— 顺序即协议：
    # ③ 整轮 → ① 对照/重载/还原 → 简拼三口，夹在 state3 与 ② im 臂之间。
    steps += plan_conf_arms(ic)
    steps += [
        # ---- ② im 格：状态栏「本输入法在不在前台」的依据（2026-10-10）----
        ("opi-ctl state（im 基线）", lambda: run_ctl("state_im0", "state"), None),
        ("切 keyboard-us（Controller1.SetCurrentIM）",
         lambda: run_controller("im_us", "SetCurrentIM", "string:keyboard-us"), None),
        ("opi-ctl state（切 IM 后）", lambda: run_ctl("state_im1", "state"), None),
        ("切回 opi（SetCurrentIM）",
         lambda: run_controller("im_back", "SetCurrentIM", "string:opi"), None),
        ("opi-ctl state（切回 IM 后）", lambda: run_ctl("state_im2", "state"), None),
        # 通道不通：把会话总线地址指到一个不存在的 socket（dbus-run-session 给的是
        # 真地址，这里显式盖掉）。真 opi-ctl + 真 dbus-send 的连接失败路径真跑一遍 ——
        # 「rc 可区分」只有在这里能测（假 dbus-send 的 cargo 测试喂的是人造 stderr）。
        ("opi-ctl state（不存在的会话总线）",
         lambda: run_ctl("state_dead", "state",
                         env=dict(os.environ,
                                  DBUS_SESSION_BUS_ADDRESS=DEAD_BUS_ADDR)), None),
        ("线上未知动作（dbus-send string:bogus）",
         lambda: run_dbus_send("bogus", "bogus"), None),
        ("opi-ctl symbols", lambda: run_ctl("symbols", "symbols"), None),
        # 浏览中按 '1'：引擎侧 1-9 只在**浏览态**有专属路由（类目下钻/符号上屏），
        # 非浏览态它只是普通数字键 —— 它的面板结果因此是「浏览真的开着」的进一步
        # 证据，而不是「动作被接受了」的重复。
        ("浏览中按 '1'（下钻类目 1）", key(0x31), None),
        # ---- ② 「没有 IC」面（必须最后：IC 一销毁，后面用 ic 的步骤全 UnknownObject）----
        ("销毁 IC（InputContext1.DestroyIC）", lambda: destroy(), None),
        ("opi-ctl state（没有 IC）", lambda: run_ctl("state_noic", "state"), None),
        # ---- ② 无 IC 的 im 语义差分（2026-10-10 实测 + fcitx5 源码钉死）----
        # im = currentInputMethod() = mostRecentInputContext() 的 IM；没有 IC 时由
        # ShareInputState 决定：All → dummy IC 兜底（注册在册、policy 检查在
        # mostRecentInputContext 里），给「下一扇窗会拿到的 IM」；No → 真没有 →
        # 空串。ReloadConfig 会把策略活推给 IC manager（instance.cpp：
        # setPropertyPropagatePolicy(shareInputState)），所以同一条链上先强制
        # All 再强制 No —— 只有这一个变量，im 非空 ⇄ 空串 互为对照。机器默认档
        # 是哪档随发行版（deepin 等把 All 当默认），不赌它，两条都显式写。
        ("写全局 config：ShareInputState=All",
         lambda: write_share_state("share_all", "All"), None),
        ("ReloadConfig（全局重载，All 活生效）",
         lambda: run_global_reload("reload_all"), None),
        ("opi-ctl state（无 IC + 共享态 All）",
         lambda: run_ctl("state_noshare_all", "state"), None),
        ("写全局 config：ShareInputState=No",
         lambda: write_share_state("share_no", "No"), None),
        ("ReloadConfig（No 活生效）", lambda: run_global_reload("reload_no"), None),
        ("opi-ctl state（无 IC + 共享态 No）",
         lambda: run_ctl("state_noshare_no", "state"), None),
        # 还原：删掉全局 config（fixture 原本没有这份文件）+ 重载回机器默认 ——
        # 不还原的话，下一次 harness 跑的守护进程会带着上一轮的策略启动。
        ("还原全局 config（删文件 + ReloadConfig）",
         lambda: (drop_share_state("share_cleanup"),
                  run_global_reload("reload_cleanup")), None),
    ]
    return steps


def check_ctl(windows, failures):
    by_label = {label: events for label, events in windows}
    print(f"[e2e/ctl] 调用结果: {RESULTS}", flush=True)

    # 1) lang 的 W1：待定拼音必须**提交进那个 IC**（「作用于当前 IC」的实锤：
    #    handleCtlAction_ 取的 mostRecentInputContext() 要真的是这个 IC）。
    #    2026-10-10 深夜订正（时间线）：白天 lead 裁决「有候选上首候选」，本断言原为
    #    「必须收到「你好」」；同夜用户裁决「切英文 = 默认是拼音，不要是候选词」
    #    （移动端先改并实证 [英文] → 字段收 ni'hao），桌面统一为一律原文 ⇒ 有意
    #    改写为「收到缓冲原文 nihao」+ 负对照「不是首候选「你好」」——把 select(0)
    #    加回去只应红本条的负对照（以及 opi_ctl_driver §1/§6 的同款负对照）。
    #    同夜追加：一律原文**按模式分叉**（符号模式不上关键字）—— 本臂是拼音面，
    #    分叉的正控/负对照在 opi_ctl_driver §1（拼音）/ §2b（符号）。
    lang_events = by_label.get("opi-ctl lang（切英文）", [])
    lang_commits = _commits(lang_events)
    if "nihao" not in lang_commits:
        failures.append(
            f"opi-ctl lang 之后 IC 没收到 CommitString「nihao」（缓冲原文，实际 "
            f"{lang_commits}）—— W1 提交没作用到当前 IC 上，或没走拼音面的"
            f"「一律原文」")
    elif "你好" in lang_commits:
        failures.append(
            f"opi-ctl lang 提交了首候选「你好」（实际 {lang_commits}）—— W1 又变成"
            f"挑候选了？（2026-10-10 深夜用户裁决：切英文（拼音面）一律缓冲原文）")
    else:
        print(f"[e2e/ctl] lang 的 W1 提交到位（原文，非首候选）：{lang_commits}",
              flush=True)

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

    # 6) state：只读查询的真链路（冻结格式 `lang=…;trad=…;punct=…;fw=…`，键序固定）。
    #    每一条都按**位置**解析（桌面就是按位置取值 —— 键序漂了在这里必须红，
    #    放过就是把值读错位）。先断「切 trad 前后输出不同」这条主契约，再断它由
    #    **哪一格**变（其余三段逐字不变），最后断退回来逐字回到基线（两个方向）。

    def parse_state(label):
        """state 行 → {键: 值}；rc/段数/键序/取值域任一不符 → 记失败并返回 None。"""
        rc, out, err = RESULTS.get(label, (None, "", ""))
        if rc != 0:
            failures.append(f"{label}: 退出码 {rc}（应 0：通道通、应答是冻结格式），"
                            f"stderr={err!r} stdout={out!r}")
            return None
        segs = out.split(";")
        if len(segs) != len(STATE_FIELDS):
            failures.append(f"{label}: 应为 {len(STATE_FIELDS)} 段，实际 {len(segs)}"
                            f" 段：{out!r}")
            return None
        vals = {}
        for i, (k, dom) in enumerate(STATE_FIELDS):
            if not segs[i].startswith(k + "="):
                failures.append(f"{label}: 第 {i + 1} 段不是 {k}=…（键序漂了 —— 桌面按"
                                f"位置取值会读错位）：{out!r}")
                return None
            vals[k] = segs[i][len(k) + 1:]
            if dom is not None and vals[k] not in dom:
                failures.append(f"{label}: {k}={vals[k]!r} 不在取值域 {dom}")
        return vals

    s_en = parse_state("state_en")
    s1 = parse_state("state1")
    s2 = parse_state("state2")
    s3 = parse_state("state3")
    if s_en is not None and s_en["lang"] != "en":
        failures.append(f"lang 切英文后 state 的 lang 段是 {s_en['lang']!r}（应 'en'）"
                        f" —— 状态没跟着真引擎模式走")
    if s1 is not None and s2 is not None:
        diff = {k: (s1[k], s2[k]) for k, _ in STATE_FIELDS if s1[k] != s2[k]}
        if diff != {"trad": ("off", "on")}:
            failures.append(
                f"切 trad 前后 state 的差异应为 {{'trad': ('off', 'on')}}，实际 {diff}"
                f"（state1={s1} state2={s2}）—— 要么输出没变，要么变了别的格")
        else:
            print(f"[e2e/ctl] state 切 trad 前后：{s1} → {s2}", flush=True)
    if s1 is not None and s1["lang"] != "zh":
        failures.append(f"切回拼音后 state 的 lang 段是 {s1['lang']!r}（应 'zh'）"
                        f" —— 英文那半没回来")
    if s1 is not None and s3 is not None and s3 != s1:
        failures.append(f"trad 再切回后 state 没有逐字回到基线：{s1} → {s3}"
                        f"（回落方向也要一致；不是只看它变了）")

    # 7) 通道不通：rc 必须可区分 —— 3（通道）而不是 0（假成功）或 4（与「插件没装」
    #    混了），且 stdout 不许留半行状态（桌面会把残留行当真状态画图标）。
    rc, out, err = RESULTS.get("state_dead", (None, "", ""))
    if rc != 3:
        failures.append(f"通道不通时 state 的退出码是 {rc}（应 3；0=假成功，4=与「插件"
                        f"没装/没加载」混了），stderr={err!r} stdout={out!r}")
    elif out:
        failures.append(f"通道不通时 stdout 不许有状态行：{out!r}")
    elif "通道不通" not in err:
        failures.append(f"通道不通的 stderr 要指明是通道问题：{err!r}")
    else:
        print(f"[e2e/ctl] 通道不通：rc=3 stderr={err!r}", flush=True)

    # 8)–10) ③① 的断言（③ conf/state 两面一致；① 对照 → 重载推送（state 面 +
    # '.' 的引擎面）→ 还原；简拼行为面）在 opi_e2e_ctl_conf.py：注入同一批
    # by_label / failures 与上面那个 parse_state（模块不反向 import）。
    check_conf_arms(by_label, failures, parse_state)

    # 11) ② im 格：切 IM 跟着变（前提由 IC 自己发的 CurrentIM 信号证明 —— 不看
    #     SetCurrentIM 的返回值）、只有这一格变；没有 IC 时 state 照常一行（im 的
    #     语义见 11b 的 ShareInputState 差分）。
    s_im0, s_im1, s_im2 = (parse_state(l) for l in ("state_im0", "state_im1", "state_im2"))
    if s_im0 is not None and s_im0["im"] != "opi":
        failures.append(f"② im 基线应为 'opi'（本变体没切过 IM），实际 {s_im0['im']!r}")
    im_win = by_label.get("切 keyboard-us（Controller1.SetCurrentIM）", [])
    if not any(n == "CurrentIM" and "keyboard-us" in p for n, p in im_win):
        failures.append(f"SetCurrentIM('keyboard-us') 后 IC 没发 CurrentIM 信号"
                        f"（{im_win}）—— im 断言的前提不成立")
    if s_im1 is not None and s_im1["im"] != "keyboard-us":
        failures.append(f"② 切到 keyboard-us 后 state 的 im 是 {s_im1['im']!r}"
                        f"（应 'keyboard-us'）")
    if s_im0 is not None and s_im1 is not None:
        got = field_diff(s_im0, s_im1)
        if got != {"im": (s_im0["im"], "keyboard-us")}:
            failures.append(f"② 切 IM 前后应只有 im 段变，实际差异 {got}"
                            f"（{s_im0} → {s_im1}）")
    if s_im0 is not None and s_im2 is not None and s_im2 != s_im0:
        failures.append(f"② 切回 opi 后 state 没逐字回基线：{s_im0} → {s_im2}")
    # 没有 IC 那一格：机器默认档只断「照常回一行」（rc/段数/键序由 parse_state
    # 钉住），**值只打印不判** —— 默认 ShareInputState 随发行版（见 11b）。
    s_noic = parse_state("state_noic")
    if s_noic is not None:
        print(f"[e2e/ctl] ② 没有 IC（机器默认档）：{s_noic}", flush=True)

    # 11b) ② 无 IC 的 im 语义差分（确定性，不赌机器默认）：同一条链上只改全局
    #      config + ReloadConfig —— All 档 dummy IC 兜底 ⇒ im 非空（「下一扇窗的
    #      IM」，具体串随此前的传播状态，不钉死）；No 档真没有 ⇒ im 空串（前端
    #      「未知 = 维持现状」）。两条互为对照：只写 config 不重载不会变（重载
    #      的活性由这对差分本身证明）；最后一个还原臂把 fixture 恢复原样。
    for label, what in (("reload_all", "ReloadConfig（→ All）"),
                        ("reload_no", "ReloadConfig（→ No）"),
                        ("reload_cleanup", "ReloadConfig（还原）")):
        rc, _, err = RESULTS.get(label, (None, "", ""))
        if rc != 0:
            failures.append(f"② {what} 退出码 {rc}（应 0 —— 这条 wire 得通），"
                            f"stderr={err!r}")
    s_all = parse_state("state_noshare_all")
    s_no = parse_state("state_noshare_no")
    if s_all is not None and s_all["im"] == "":
        failures.append("② ShareInputState=All 且没有 IC 时 im 不应为空串（dummy IC"
                        " 兜底 = 下一扇窗的 IM）—— All 这档没生效？")
    if s_no is not None and s_no["im"] != "":
        failures.append(f"② ShareInputState=No 且没有 IC 时 im 应为空串（前端按"
                        f"「未知 = 维持现状」处理），实际 {s_no['im']!r}")
    if s_all is not None and s_all["im"] != "" and s_no is not None and s_no["im"] == "":
        print(f"[e2e/ctl] ② 无 IC 差分：All → im={s_all['im']!r} ／ No → im=''",
              flush=True)
    if os.path.exists(_global_conf_path()):
        failures.append(f"② 还原臂没把全局 config 删掉（{_global_conf_path()} 还在）"
                        f" —— 会污染下一次 harness 跑的守护进程")
