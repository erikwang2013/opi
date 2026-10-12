// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 状态栏按钮的**外部命令**部分：把点击变成一条命令，并把失败如实报回来。
//!
//! 契约（2026-10-10 冻结，Linux 侧由另一条轨实现，本侧**只调用**）：
//! **`opi-ctl <action>`**，action ∈ `symbols|trad|punct|fullwidth|lang`；`opi-ctl` 在
//! `/usr/bin/opi-ctl`（PATH 上，按名调）。
//!
//! ⚠️ **中英文切换只能走 `opi-ctl lang`，绝不能走 `fcitx5-remote -t`**（事故留痕，
//! 2026-10-10 中午用户报障）：`-t` 切换的是**整个 fcitx5 的启用态**，不是中英模式 ——
//! 用户点两下后整个输入法框架被停用，中文一个字打不出来（lead 在用户真机实测的指纹：
//! 点两次后 `fcitx5-remote` 报 2 = 全框架已禁用）。旧的 `-t` 映射来自更早的任务文档，
//! 就摆在状态栏 LANG 按钮下面；想「顺手」改回去之前先看 StatusBarTest 的映射回归钉。
//!
//! 状态通道（2026-10-10 深夜冻结）：**`opi-ctl state`** → 一行
//! `lang=zh|en;trad=on|off;punct=on|off;fw=on|off[;im=<uniqueName>]`（rc 0 成功；通道不通
//! rc≠0 且无输出）—— 状态栏的实时图标读它（[readImeState]）。`im` = 当前输入法
//! （2026-10-10 下午用户裁决「切到其他输入法时状态栏消失」的数据位，glue 侧在途追加；
//! 缺键/空串 = 未知，语义见 [ImeState.im] 与 StatusBar.kt 的 [barVisible]）。
//!
//! ⚠️ 这些动作的效果在**别处**（输入法状态），命令成败是本侧唯一看得见的东西 ——
//! 所以失败必须回到 UI 上（[runExternal] 的返回值就是给用户看的那句话）。

package xyz.erik.opi.candidate

import java.util.concurrent.TimeUnit

/**
 * 纯函数：按钮 → 要跑的命令；null = 本按钮不跑命令（另有动作：开窗/开关键盘/打开设置）。
 *
 * Windows 上没有 `opi-ctl` 也没有 fcitx5（那是 Linux 的输入法框架），所以调用方
 * 只在 Linux 上走这条路 —— 见 [statusButtonWired] 与 StatusBar.kt 的分派。
 */
fun statusActionCommand(b: StatusBarButton): List<String>? = when (b) {
    // SYMBOLS 2026-10-12 起**不再出命令**：用户裁决「点击后要是个大的页面，可以翻页
    // 选择」⇒ 两端都开符号页窗（见 StatusBar.openSymbols）。原来的
    // `opi-ctl symbols` 是 fcitx5 的小候选条浏览，命令本身还在（CLI 仍可用），
    // 只是**不再挂在这个按钮上** —— 别按老映射「改回去」。
    StatusBarButton.SYMBOLS -> null
    StatusBarButton.TRAD -> listOf("opi-ctl", "trad")
    StatusBarButton.PUNCT -> listOf("opi-ctl", "punct")
    StatusBarButton.FULLWIDTH -> listOf("opi-ctl", "fullwidth")
    // ⚠️ 事故（2026-10-10 中午用户报障）：这里曾是 listOf("fcitx5-remote", "-t") ——
    // `-t` 切的是**整个 fcitx5 的启用态**，不是中英模式。用户点两下 ⇒ 全框架被停用、
    // 中文打不出来（lead 真机实测指纹：点两次后 `fcitx5-remote` 报 2）。正确动作是契约
    // 里的 `opi-ctl lang`；有映射回归钉（StatusBarTest）。别改回去。
    StatusBarButton.LANG -> listOf("opi-ctl", "lang")
    StatusBarButton.VK, StatusBarButton.LOGO, StatusBarButton.SETTINGS -> null
}

// ---------- 跑命令 ----------

/** 一条命令的结局。Timeout 与 Failed 分开是因为给用户的话不一样（「没响应」/「跑不了」）。 */
private sealed interface RunResult {
    /** 起得来也退出了：退出码 + 输出首行（null = 没输出）。state 靠首行当载荷。 */
    class Exit(val code: Int, val firstLine: String?) : RunResult
    object Timeout : RunResult
    /** 进程都起不来（命令不存在/没权限）：`why` = 异常类名，够用户判断缺什么。 */
    class Failed(val why: String) : RunResult
}

/**
 * 跑命令并抓输出首行。超时由调用方定 —— 它不是防御性编程而是必须的：卡在等输入法
 * 应答上时同步等它，点一下按钮/一拍轮询就把 UI 冻住。到点 `destroyForcibly`。
 *
 * 只读**一行**、不另开读线程：我们自己的小工具输出一行以内；真卡住还有超时兜底。
 */
private fun runCapture(cmd: List<String>, timeoutMs: Long): RunResult = try {
    val proc = ProcessBuilder(cmd).redirectErrorStream(true).start()
    if (!proc.waitFor(timeoutMs, TimeUnit.MILLISECONDS)) {
        proc.destroyForcibly()
        RunResult.Timeout
    } else {
        RunResult.Exit(proc.exitValue(), proc.inputStream.bufferedReader().readLine()?.trim())
    }
} catch (e: Exception) {
    // 命令不存在（没装/不在 PATH）也走这里 —— 报出命令名，用户才知道缺什么。
    RunResult.Failed(e.javaClass.simpleName)
}

/** 跑一条外部命令。返回 null = 退出码 0；非 null = **给用户看的**失败短语。 */
fun runExternal(cmd: List<String>, timeoutMs: Long = 3000): String? {
    val name = cmd.joinToString(" ")
    return when (val r = runCapture(cmd, timeoutMs)) {
        is RunResult.Exit ->
            if (r.code == 0) null
            else "「$name」退出码 ${r.code}${if (r.firstLine.isNullOrEmpty()) "" else "：${r.firstLine}"}"
        RunResult.Timeout -> "「$name」超时（无响应 ${timeoutMs / 1000}s）"
        is RunResult.Failed -> "跑不了「${cmd.first()}」：${r.why}"
    }
}

// ---------- 输入法状态 ----------

/**
 * 输入法实时状态（`opi-ctl state` 的解析结果）。
 *
 * **默认值 = 中性缺省渲染**（2026-10-10 深夜用户裁决的定案）：通道读不到/坏串/缺键时
 * 就画这个 —— 中 / 简 / 半月。此前「读不到状态就不画成当前状态」的旧原则被该裁决
 * 取代：现在读得到就画真的，读不到画缺省（一个中文输入法最中性的样子），不空白、
 * 不报错。
 *
 * [punct] **参与渲染**：中英标点按钮双态（punct=on → 「。，」；off → 「.,」），图标
 * 选择见 StatusBar.kt 的 [punctIcon]。时间线（防回退）：2026-10-10 深夜 B 版曾裁决
 * 「punct 只解析、不参与渲染」→ **同日中午按用户对按钮行为的描述订正**（。，/., 两态
 * 归标点按钮、符号大全改静态 Ω）；B 版那条已被取代，别按它「改回去」。
 *
 * [im] = 当前输入法 uniqueName（`im=` 位，2026-10-10 下午用户裁决）。**只定条的存亡、
 * 不参与渲染**：缺键/空串 = 未知 ⇒ 状态栏判可见（维持现状，不闪）；非空且 ≠ `"opi"`
 * ⇒ 隐藏（判定在 StatusBar.kt 的 [barVisible]）。这是**加键**：既有四键的解析语义一字
 * 未动 —— 新 glue 没合时旧输出正好缺这个键，走「未知」分支。
 */
data class ImeState(
    val zh: Boolean = true,
    val trad: Boolean = false,
    val punct: Boolean = true,
    val fullwidth: Boolean = false,
    /** 当前输入法 uniqueName（`im=` 位）。缺键/空串 = 未知 —— 见类头 [im] 段。 */
    val im: String = "",
)

/**
 * 解析 `opi-ctl state` 的一行。**容错是规格的一部分**（缺省语义见 [ImeState]）：
 * · 空行/null、未知键、认不得的值、没有 `=` 的碎片 ⇒ 该处落缺省；
 * · 缺键 = 只有那个键落缺省，不把整行判坏（格式加字段时旧前端不至于全瞎）；
 * · `=`/`;` 两侧容忍空格（手写调试时难免）。
 */
fun parseImeState(line: String?): ImeState {
    var st = ImeState()
    for (kv in line.orEmpty().split(';')) {
        val eq = kv.indexOf('=')
        if (eq <= 0) continue // 无 '=' 或键为空：碎片，忽略
        val v = kv.substring(eq + 1).trim()
        val on: Boolean? = when (v) { "on" -> true; "off" -> false; else -> null }
        st = when (kv.substring(0, eq).trim()) {
            "lang" -> if (v == "zh" || v == "en") st.copy(zh = v == "zh") else st
            "trad" -> if (on != null) st.copy(trad = on) else st
            "punct" -> if (on != null) st.copy(punct = on) else st
            "fw" -> if (on != null) st.copy(fullwidth = on) else st
            // 任意串都收（uniqueName 没有枚举面）；空值/缺键归一成 "" = 未知（见 ImeState.im）
            "im" -> st.copy(im = v)
            else -> st // 以后多出来的键：忽略
        }
    }
    return st
}

/**
 * 读一次输入法状态。读不到（通道不通/超时/坏串）⇒ 中性缺省，**不弹提示** —— 条上
 * 每秒都在做这件事，弹提示只会刷屏；「通道不通」的可见反馈在动作命令那条路上。
 *
 * 超时 1.5s：轮询节奏本来就是 1s，查询卡住时最多把它拖成 ~2.5s 一拍，而不是 4s+。
 */
fun readImeState(): ImeState = when (val r = runCapture(listOf("opi-ctl", "state"), 1500)) {
    is RunResult.Exit -> if (r.code == 0) parseImeState(r.firstLine) else ImeState()
    RunResult.Timeout, is RunResult.Failed -> ImeState()
}

// ---------- 动作 + 补读 ----------

/**
 * 点按钮的统一收尾：跑动作 → **无论成败立刻补读一次状态**（裁决：点完马上看到翻转，
 * 这是「即时生效」的落点）。抽成纯函数只为一件事：让「动作返回后一定补读」有门禁
 * 钉住，而不是埋在 Compose 里 —— 埋着的话将来重构掉了它，唯一表现是「点完等 1s
 * 才翻转」，没有任何测试会红。
 *
 * 用 finally：动作抛异常也补读（否则一次异常之后状态可能一直停在旧值）。
 */
internal fun <R> actionWithStateRefresh(action: () -> R, refresh: () -> Unit): R = try {
    action()
} finally {
    refresh()
}
