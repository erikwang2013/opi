#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

"""opi-ctl 变体的「配置面」与公用底层（2026-10-10）—— 从 opi_e2e_ctl.py 分
出来（那个文件贴 500 行硬线），切法同 opi_e2e_modes.py 的先例。

**③① 的步骤与断言在本文件**（opi_e2e_ctl.py 管基础通道面与 ② im 格）：

  · ③ opi-ctl punct = 运行期切换 + **写回配置**：conf 里 ChinesePunct 跟着翻面、
    state 的 punct 段与磁盘值一致（「显示与实际一致」的根治），再点一次两条都
    回原值。
  · ① ReloadAddonConfig 真链路（设置窗保存那条 wire）：**只改 conf 不重载** →
    state 与 '.' 键的引擎行为都不动（对照）；调 ReloadAddonConfig 后 → state 的
    punct 段随 conf 变，且 '.' 键的提交从「。」变「．」（拼音全角档的中文/机械
    标点分界 —— 引擎面证据，证明推的是活引擎、不只是配置内存）；Jianpin 走同一
    applyOpiSettings：关掉重载后 "nh" 不再出「你好」，开回来即还原。

跟着搬来的公用底层（上面两段的步骤/断言与 opi_e2e_ctl.py 都直接引用）：
常量、RESULTS、run_ctl / run_dbus_send / run_controller、conf 读写
（read_conf / patch_conf）、_commits / field_diff / conf_punct / flip。
opi_e2e_ctl.py 反向 import 本文件 —— 单向依赖，不回头。

wire 备注（别“顺手订正”）：/controller 上的接口名是 **org.fcitx.Fcitx.Controller1**
—— fcitx4 时代的接口名，fcitx5 原样沿用（实测；写成 org.fcitx.Fcitx5.Controller1
会 UnknownMethod rc=1）。ReloadAddonConfig 与 SetCurrentIM 都在这条接口上。
"""

import os
import subprocess

# run-harness.sh 编好并传进来的 opi-ctl（cargo 产物）。缺了不是本变体的错：
# 报清楚、让 check 记一条失败（别静默跳过 —— 「没测」会被读成「测了且过」）。
CTL = os.environ.get("OPI_CTL_BIN", "")
DEST = "org.fcitx.Fcitx5"
OPI_PATH = "/opi"
OPI_IFACE = "org.fcitx.Fcitx5.Opi1"

# 「通道不通」那条用的会话总线地址：指向一个不存在的 unix socket。dbus-send 连它
# 立刻失败（Failed to open connection ...）⇒ opi-ctl 判成 3。真地址由
# dbus-run-session 给，这一步显式盖掉它。
DEAD_BUS_ADDR = "unix:path=/tmp/opi-e2e-nonexistent-bus-socket"

# state 行的冻结键序与取值域（三处同步：cpp/opi_ctl.h 的 State 注释、opi-ctl.rs 的
# check_state_shape、desktop 的解析处）。桌面端**按位置**取值，键序漂了这里必须红。
# im（2026-10-10 追加，行尾）：任意串，**空串合法**（= 没有可报的输入法：没有 IC
# 且 ShareInputState=No 没兜底 —— 实测见 opi_e2e_ctl.py 的「无 IC 差分」臂）⇒
# 无取值域（None），但位置照样钉死。
# mode（2026-10-14 追加）：档位 token，取值域与 `opi-ctl mode` 的应答同域
# （服务端同一个派生函数，见 cpp/opi_ctl_mode.h）。
# enpro（2026-10-19 追加，真正的行尾）：英文联想开关，on/off。老插件只有 6 段；
# 本 harness 用本树 .so（必为 7 段），故这里按定长 7 段钉死。
STATE_FIELDS = (("lang", ("zh", "en")), ("trad", ("on", "off")),
                ("punct", ("on", "off")), ("fw", ("on", "off")),
                ("im", None), ("mode", ("pinyin", "en26", "wubi")),
                ("enpro", ("on", "off")))

# 每次调用的结果：{标签: (rc, stdout, stderr)}。信号窗口（_windows）只装 dbus 信号，
# 装不下进程退出码，故这里单独记一份 —— check_ctl 两样都读。
RESULTS = {}


def run_ctl(label, action, *extra, env=None):
    """跑一次真 opi-ctl（`extra` = 附加参数，只有 `insert <文本>` 用）。返回一句话给
    report 打印。`env` 整体替换子进程环境（只给「通道不通」那条用：把会话总线地址
    指到一个不存在的 socket）。"""
    if not CTL or not os.path.exists(CTL):
        RESULTS[label] = (-1, "", f"OPI_CTL_BIN 没指到可执行文件：{CTL!r}")
        return f"!! 找不到 opi-ctl（OPI_CTL_BIN={CTL!r}）—— run-harness.sh 会编它"
    proc = subprocess.run([CTL, action, *extra], capture_output=True, text=True,
                          timeout=20, env=env)
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


def run_dbus_search(label, keyword):
    """直接调线上方法 **Search**（2026-10-13 加的第二个方法，`s -> s`）。

    为什么不复用 `run_dbus_send`（Trigger）：两条是**不同的 D-Bus 方法**，注册面也不同
    —— C++ 驱动那份只调 `handleCtlSearch_`（handler），**vtable 里方法名/签名写错它照样绿**
    （`FCITX_OBJECT_VTABLE_METHOD(search, "Search", "s", "s")` 是宏里的一行字符串）。
    真守护进程上的这一问才是「注册有没有落到总线上」的证据 —— 与 Trigger 同一条理由：
    这种「有实现、没接线」的形态在本仓反复出现过。"""
    argv = ["dbus-send", "--session", "--print-reply", "--reply-timeout=5000",
            f"--dest={DEST}", OPI_PATH, f"{OPI_IFACE}.Search", f"string:{keyword}"]
    proc = subprocess.run(argv, capture_output=True, text=True, timeout=20)
    RESULTS[label] = (proc.returncode, proc.stdout.strip(), proc.stderr.strip())
    return (f"退出码 {proc.returncode} stdout={proc.stdout.strip()!r} "
            f"stderr={proc.stderr.strip()!r}")


# 设置窗（desktop Settings.kt 的 notifyFcitxReload）走的那条 wire：/controller 上的
# **org.fcitx.Fcitx.Controller1** —— fcitx4 时代的接口名，fcitx5 原样沿用（实测；
# 写成 org.fcitx.Fcitx5.Controller1 会 UnknownMethod rc=1，别“顺手订正”）。
# ReloadAddonConfig 与 SetCurrentIM 都在这条接口上。
CTRL_PATH = "/controller"
CTRL_IFACE = "org.fcitx.Fcitx.Controller1"


def run_controller(label, method, arg):
    """直发 Controller1 的方法（真 wire，同 run_dbus_send 的记法）。① 的
    ReloadAddonConfig 与 ② 的 SetCurrentIM 都从这里走。"""
    argv = ["dbus-send", "--session", "--print-reply", "--reply-timeout=5000",
            f"--dest={DEST}", CTRL_PATH, f"{CTRL_IFACE}.{method}", arg]
    proc = subprocess.run(argv, capture_output=True, text=True, timeout=20)
    RESULTS[label] = (proc.returncode, proc.stdout.strip(), proc.stderr.strip())
    return (f"退出码 {proc.returncode} stdout={proc.stdout.strip()!r} "
            f"stderr={proc.stderr.strip()!r}")


def run_global_reload(label):
    """Controller1.ReloadConfig（无参）：**全局**配置重载 —— ShareInputState 改完
    靠它活生效（fcitx5 instance.cpp 的 reloadConfig：setPropertyPropagatePolicy）。
    与 ReloadAddonConfig 不同：它**不动** addon 配置（① 的结论，opi 的四个开关
    不受影响）。"""
    argv = ["dbus-send", "--session", "--print-reply", "--reply-timeout=5000",
            f"--dest={DEST}", CTRL_PATH, f"{CTRL_IFACE}.ReloadConfig"]
    proc = subprocess.run(argv, capture_output=True, text=True, timeout=20)
    RESULTS[label] = (proc.returncode, proc.stdout.strip(), proc.stderr.strip())
    return (f"退出码 {proc.returncode} stdout={proc.stdout.strip()!r} "
            f"stderr={proc.stderr.strip()!r}")


def _conf_path():
    home = os.environ.get("XDG_CONFIG_HOME", "")
    return os.path.join(home, "fcitx5", "conf", "opi_fcitx5.conf") if home else ""


def read_conf(label):
    """读盘上的 addon 配置（③/① 的**磁盘面**证据）—— 整份内容进 RESULTS[label]。
    **文件不存在不是失败**（= 全默认，见 opi_config.h），内容记成空串。"""
    path = _conf_path()
    if not path:
        RESULTS[label] = (-1, "", "XDG_CONFIG_HOME 未设 —— 读不到配置落点")
        return "!! XDG_CONFIG_HOME 未设（run-harness.sh 会指到临时目录）"
    try:
        with open(path, encoding="utf-8") as f:
            text = f.read()
    except FileNotFoundError:
        RESULTS[label] = (0, "", "")
        return f"{path} 不存在（= 全默认）"
    except OSError as e:
        RESULTS[label] = (-1, "", f"读 {path} 失败：{e}")
        return f"!! 读不到 {path}: {e}"
    RESULTS[label] = (0, text, "")
    return f"{path}: {text!r}"


def patch_conf(label, **vals):
    """按设置窗保存的**同语义**改 conf（其余键原样保留 = mergeIni 的行为）——
    只动调用方点名的键；文件不存在就新建（与设置窗首次保存一致）。"""
    path = _conf_path()
    if not path:
        RESULTS[label] = (-1, "", "XDG_CONFIG_HOME 未设")
        return "!! XDG_CONFIG_HOME 未设"
    try:
        with open(path, encoding="utf-8") as f:
            lines = f.read().splitlines()
    except FileNotFoundError:
        lines = []
    except OSError as e:
        RESULTS[label] = (-1, "", f"读 {path} 失败：{e}")
        return f"!! 读不到 {path}: {e}"
    done = set()
    out = []
    for ln in lines:
        key = ln.split("=", 1)[0].strip() if "=" in ln else None
        if key in vals:
            out.append(f"{key}={vals[key]}")
            done.add(key)
        else:
            out.append(ln)
    for key, val in vals.items():
        if key not in done:
            out.append(f"{key}={val}")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        f.write("\n".join(out) + "\n")
    RESULTS[label] = (0, "\n".join(out), "")
    return f"改 {path}：{vals}"


def conf_punct(d):
    """conf 里 ChinesePunct 的**有效值**（键缺席 = 默认 True，见 opi_config.h）。"""
    return d.get("ChinesePunct", "True")


def _global_conf_path():
    """**全局** config 的落点（`$XDG_CONFIG_HOME/fcitx5/config`）—— ShareInputState
    住这里。别与 addon 的 conf/opi_fcitx5.conf（_conf_path）混了。"""
    home = os.environ.get("XDG_CONFIG_HOME", "")
    return os.path.join(home, "fcitx5", "config") if home else ""


def write_share_state(label, policy):
    """把全局 config 的 ShareInputState 写成 policy（只写这一节，别的选项留默认；
    本 fixture 原本没有这份文件 —— ② 的无 IC 差分臂用完会删掉它还原）。"""
    path = _global_conf_path()
    if not path:
        RESULTS[label] = (-1, "", "XDG_CONFIG_HOME 未设")
        return "!! XDG_CONFIG_HOME 未设"
    os.makedirs(os.path.dirname(path), exist_ok=True)
    text = f"[Behavior]\nShareInputState={policy}\n"
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)
    RESULTS[label] = (0, text.strip(), "")
    return f"写 {path}：ShareInputState={policy}"


def drop_share_state(label):
    """删掉全局 config（无 IC 差分臂的还原 —— fixture 原本没有这份文件）。"""
    path = _global_conf_path()
    if not path:
        RESULTS[label] = (-1, "", "XDG_CONFIG_HOME 未设")
        return "!! XDG_CONFIG_HOME 未设"
    try:
        os.remove(path)
    except FileNotFoundError:
        pass
    RESULTS[label] = (0, "", "")
    return f"删 {path}"


def field_diff(a, b):
    return {k: (a[k], b[k]) for k, _ in STATE_FIELDS if a[k] != b[k]}


def flip(v):
    return "off" if v == "on" else "on"


def _commits(events):
    return [p for n, p in events if n == "CommitString"]


def plan_conf_arms(ic):
    """③①（+简拼）的步骤片段：opi_e2e_ctl.py 的 plan_ctl 把它拼在基础步骤
    （trad 切回的 state3）之后 —— 顺序即协议（③ 整轮 → ① 对照/重载/还原 →
    简拼三口）。每个 opi-ctl 调用的 expect 都是 None：结果（conf/state/'.' 的
    提交）在 check_conf_arms 里断言，面板推不推是另一件事，钉死条数只会白红。"""
    key = lambda v: (lambda: ic.ProcessKeyEvent(v, 0, 0, False, 0))  # noqa: E731

    def backspaces(n):
        """连打 n 个退格清缓冲（引擎的缓冲编辑键；本变体里没有 Cancel 键 ——
        ESC 是直通键，不进缓冲处理）。"""
        def run():
            for _ in range(n):
                ic.ProcessKeyEvent(0xFF08, 0, 0, False, 0)
        return run

    return [
        # ---- ③ punct = 运行期切换 + 写回配置（conf 与活状态永远同一，2026-10-10）----
        # 磁盘面（read_conf）与 state 面两条都取，最后在 check_ctl 里判「同源」。
        ("读 conf（punct 前）", lambda: read_conf("conf_p0"), None),
        ("opi-ctl state（punct 前）", lambda: run_ctl("state_p0", "state"), None),
        ("opi-ctl punct", lambda: run_ctl("punct1", "punct"), None),
        ("读 conf（punct 后）", lambda: read_conf("conf_p1"), None),
        ("opi-ctl state（punct 后）", lambda: run_ctl("state_p1", "state"), None),
        ("opi-ctl punct 切回", lambda: run_ctl("punct2", "punct"), None),
        ("读 conf（切回后）", lambda: read_conf("conf_p2"), None),
        ("opi-ctl state（切回后）", lambda: run_ctl("state_p2", "state"), None),
        # ---- ① 改 conf → ReloadAddonConfig → 活引擎（设置窗保存那条链）----
        # '.' 是引擎面的探针键：拼音全角档下 chinese_punct 开 → 「。」、关 → 机械
        # 全角「．」（',' 两档都是「，」，分不开 —— 真值表在引擎测试里钉着）。
        ("键 '.'（重载前基线）", key(0x2E), None),
        ("写 conf：ChinesePunct=False（不重载）",
         lambda: patch_conf("conf_r_off", ChinesePunct="False"), None),
        ("opi-ctl state（只改文件、未重载）", lambda: run_ctl("state_r0", "state"), None),
        ("键 '.'（只改文件、未重载）", key(0x2E), None),
        ("ReloadAddonConfig（设置窗同款 wire）",
         lambda: run_controller("reload_off", "ReloadAddonConfig",
                                "string:opi_fcitx5"), None),
        ("opi-ctl state（重载后）", lambda: run_ctl("state_r1", "state"), None),
        ("键 '.'（重载后：引擎面证据）", key(0x2E), None),
        ("写 conf：ChinesePunct=True", lambda: patch_conf("conf_r_on", ChinesePunct="True"),
         None),
        ("ReloadAddonConfig（还原）",
         lambda: run_controller("reload_on", "ReloadAddonConfig",
                                "string:opi_fcitx5"), None),
        ("opi-ctl state（还原后）", lambda: run_ctl("state_r2", "state"), None),
        ("键 '.'（还原后）", key(0x2E), None),
        # ---- ① 扩展：Jianpin 走同一 applyOpiSettings —— 行为面（"nh" → 你好）----
        ("输入 'n'（简拼阳性对照）", key(0x6E), None),
        ("输入 'h'（简拼阳性对照：应有 你好）", key(0x68), None),
        ("清空缓冲#1（⌫×4）", backspaces(4), None),
        ("写 conf：Jianpin=False", lambda: patch_conf("conf_jp_off", Jianpin="False"), None),
        ("ReloadAddonConfig（关简拼）",
         lambda: run_controller("reload_jp_off", "ReloadAddonConfig",
                                "string:opi_fcitx5"), None),
        ("输入 'n'（关简拼后）", key(0x6E), None),
        ("输入 'h'（关简拼后：不应有 你好）", key(0x68), None),
        ("清空缓冲#2（⌫×4）", backspaces(4), None),
        ("写 conf：Jianpin=True", lambda: patch_conf("conf_jp_on", Jianpin="True"), None),
        ("ReloadAddonConfig（开简拼，还原）",
         lambda: run_controller("reload_jp_on", "ReloadAddonConfig",
                                "string:opi_fcitx5"), None),
        ("输入 'n'（还原后）", key(0x6E), None),
        ("输入 'h'（还原后：你好 应回来）", key(0x68), None),
        ("清空缓冲#3（⌫×4）", backspaces(4), None),
    ]


def check_conf_arms(by_label, failures, parse_state):
    """opi_e2e_ctl.py 的 check_ctl 8)–10) 段（③ 的两面一致 + ① 的对照/推送/还原
    + 简拼行为面）。`parse_state` 由调用方注入（它与 failures / by_label 是同一
    批对象）—— 模块不反向 import，同 opi_e2e_modes.py 的老规矩。"""
    # 8) ③ punct = 切换 + 落盘（conf 与活状态永远同一，2026-10-10）：磁盘面 +
    #    state 面 + 两面必须一致；切回两条都回原值。
    def parse_conf(label):
        """conf 全文 → {键: 值}。rc!=0 记失败；文件不存在 → {}（= 全默认）。"""
        rc, out, err = RESULTS.get(label, (None, "", ""))
        if rc != 0:
            failures.append(f"{label}: rc={rc} stderr={err!r}（conf 读不到 —— ③/① 的磁盘面测不了）")
            return None
        d = {}
        for ln in out.splitlines():
            if "=" in ln:
                k, v = ln.split("=", 1)
                d[k.strip()] = v.strip()
        return d

    s_p0, s_p1, s_p2 = (parse_state(l) for l in ("state_p0", "state_p1", "state_p2"))
    c0, c1, c2 = (parse_conf(l) for l in ("conf_p0", "conf_p1", "conf_p2"))
    rc, _, err = RESULTS.get("punct1", (None, "", ""))
    if rc != 0:
        failures.append(f"opi-ctl punct 退出码 {rc}（应 0 —— 这次切换要能落盘），stderr={err!r}")
    if c0 is not None and c1 is not None:
        v0, v1 = conf_punct(c0), conf_punct(c1)
        if v1 not in ("True", "False") or v1 == v0:
            failures.append(f"opi-ctl punct 没把 ChinesePunct 写进 conf：{v0!r} → {v1!r}"
                            f"（③：运行期切换必须同时写回配置）")
        else:
            print(f"[e2e/ctl] ③ punct 落盘：ChinesePunct {v0} → {v1}", flush=True)
    if c0 is not None and c2 is not None and conf_punct(c2) != conf_punct(c0):
        failures.append(f"punct 切回后 conf 没回原值：{conf_punct(c0)!r} → {conf_punct(c2)!r}")
    if s_p0 is not None and s_p1 is not None:
        want = {"punct": (s_p0["punct"], flip(s_p0["punct"]))}
        got = field_diff(s_p0, s_p1)
        if got != want:
            failures.append(f"punct 切换前后 state 应只有 punct 段翻面，实际差异 {got}"
                            f"（{s_p0} → {s_p1}）")
    if s_p0 is not None and s_p2 is not None and s_p2 != s_p0:
        failures.append(f"punct 切回后 state 没逐字回到基线：{s_p0} → {s_p2}")
    if s_p1 is not None and c1 is not None and conf_punct(c1) in ("True", "False"):
        if (conf_punct(c1) == "True") != (s_p1["punct"] == "on"):
            failures.append(f"③ 两条没同一：state punct={s_p1['punct']!r} 而 conf "
                            f"ChinesePunct={conf_punct(c1)!r}")

    # 9) ① ReloadAddonConfig 真链路：只改 conf 不重载 → 都不动（对照）；重载后 →
    #    state 的 punct 段**与** '.' 键的引擎行为都随 conf 变。「只改了配置内存」
    #    与「活引擎拿到了」在这里被 '.' 的提交字符分开。
    s_r0, s_r1, s_r2 = (parse_state(l) for l in ("state_r0", "state_r1", "state_r2"))
    for label, what in (("reload_off", "ReloadAddonConfig（设置窗同款）"),
                        ("reload_on", "ReloadAddonConfig（还原）"),
                        ("reload_jp_off", "ReloadAddonConfig（关简拼）"),
                        ("reload_jp_on", "ReloadAddonConfig（开简拼）")):
        rc, _, err = RESULTS.get(label, (None, "", ""))
        if rc != 0:
            failures.append(f"{what} 退出码 {rc}（应 0 —— 设置窗的这条 wire 必须到得了），"
                            f"stderr={err!r}")
    if s_r0 is not None and s_p2 is not None and s_r0 != s_p2:
        failures.append(f"① 对照臂破了：只改 conf 文件（不重载）state 就变了：{s_p2} → {s_r0}")
    if s_r0 is not None and s_r1 is not None:
        want = {"punct": (s_r0["punct"], flip(s_r0["punct"]))}
        got = field_diff(s_r0, s_r1)
        if got != want:
            failures.append(f"① ReloadAddonConfig 后 state 的 punct 段应随 conf 翻面，"
                            f"实际差异 {got}（{s_r0} → {s_r1}）—— reloadConfig 没把配置推给活引擎？")
        else:
            print(f"[e2e/ctl] ① 重载推送（state 面）：{s_r0} → {s_r1}", flush=True)
    if s_r2 is not None and s_p2 is not None and s_r2 != s_p2:
        failures.append(f"① 还原后 state 没逐字回到基线：{s_p2} → {s_r2}")
    if s_p0 is not None and c0 is not None:
        if s_p0["fw"] != "on":
            failures.append(f"前提：拼音档全角应为 on（state_p0 fw={s_p0['fw']!r}）——"
                            f" '.' 探针靠全角档才分得开中文/机械标点")
        else:
            cp_on = conf_punct(c0) == "True"
            exp_base = "。" if cp_on else "．"
            exp_off = "．" if cp_on else "。"
            for lbl, exp in (("键 '.'（重载前基线）", exp_base),
                             ("键 '.'（只改文件、未重载）", exp_base),
                             ("键 '.'（重载后：引擎面证据）", exp_off),
                             ("键 '.'（还原后）", exp_base)):
                got = _commits(by_label.get(lbl, []))
                if exp not in got:
                    failures.append(f"① {lbl} 应提交 {exp!r}，实际 {got}"
                                    f"（引擎面没跟着 conf 走）")
            if all(exp in _commits(by_label.get(lbl, []))
                   for lbl, exp in (("键 '.'（重载前基线）", exp_base),
                                    ("键 '.'（重载后：引擎面证据）", exp_off))):
                print(f"[e2e/ctl] ① 重载推送（引擎面）：'.' 提交 {exp_base} → {exp_off}", flush=True)

    # 10) ① 扩展：Jianpin（applyOpiSettings 推的四个旋钮之一）行为面 ——
    #     "nh" → 「你好」是简拼的招牌输出：关掉重载后必须消失、开回来重载后回来。
    #     阳性对照先行：本环境测不出简拼时，后面的结论一律无效（不是「修复了」）。
    def has_nihao(label):
        return any("你好" in p for n, p in by_label.get(label, []))

    pos = has_nihao("输入 'h'（简拼阳性对照：应有 你好）")
    if not pos:
        failures.append("前提：简拼开着时 'nh' 应出「你好」（阳性对照失败 —— 本环境"
                        "测不出简拼，10 的结论无效）")
    elif has_nihao("输入 'h'（关简拼后：不应有 你好）"):
        failures.append("① Jianpin=False + ReloadAddonConfig 后 'nh' 仍出「你好」——"
                        " 重载没推到引擎（Learn/Fuzzy 走同一 applyOpiSettings，同判）")
    elif not has_nihao("输入 'h'（还原后：你好 应回来）"):
        failures.append("① Jianpin 开回来 + 重载后 'nh' 没再出「你好」（还原这半没生效）")
    else:
        print("[e2e/ctl] ① Jianpin 行为面：开 → 关（无你好）→ 开，全过", flush=True)
