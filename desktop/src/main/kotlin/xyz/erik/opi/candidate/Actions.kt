// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 状态栏按钮的**外部命令**部分：把点击变成一条命令，并把失败如实报回来。
//!
//! 契约（2026-10-10 冻结，Linux 侧由另一条轨实现，本侧**只调用**）：
//! **`opi-ctl <action>`**，action ∈ `symbols|trad|punct|fullwidth|lang|englishpro|mode`；`opi-ctl` 在
//! `/usr/bin/opi-ctl`（PATH 上，按名调）。`mode`（2026-10-14 加）是**唯一**成功应答
//! 不是 "ok" 的动作：它回新状态 token（见 [cycleMode]），别按「回 ok 才成功」读它。
//!
//! ⚠️ **中英文切换只能走 `opi-ctl lang`，绝不能走 `fcitx5-remote -t`**（事故留痕，
//! 2026-10-10 中午用户报障）：`-t` 切换的是**整个 fcitx5 的启用态**，不是中英模式 ——
//! 用户点两下后整个输入法框架被停用，中文一个字打不出来（lead 在用户真机实测的指纹：
//! 点两次后 `fcitx5-remote` 报 2 = 全框架已禁用）。旧的 `-t` 映射来自更早的任务文档，
//! 就摆在状态栏 LANG 按钮下面；想「顺手」改回去之前先看 StatusBarTest 的映射回归钉。
//!
//! 状态通道（2026-10-10 深夜冻结）：**`opi-ctl state`** → 一行
//! `lang=zh|en;trad=on|off;punct=on|off;fw=on|off[;im=<uniqueName>][;mode=<档位>][;enpro=on|off]`
//! （rc 0 成功；通道不通 rc≠0 且无输出）—— 状态栏的实时图标读它（[readImeState]）。
//! `im` = 当前输入法（2026-10-10 下午用户裁决「切到其他输入法时状态栏消失」的数据位，
//! glue 侧在途追加；缺键/空串 = 未知，语义见 [ImeState.im] 与 StatusBar.kt 的 [barVisible]）。
//! `mode` = 档位 token（2026-10-14 追加，**行尾**）：取值域同 [cycleMode] 的应答
//! （[MODE_TOKENS]），缺键/认不得 = **未知（空串）**，与 `im` 同一档 —— 老插件的输出
//! 没有这一格，不许变成解析失败；「未知 → 拼音」的回落归**消费方**（状态栏那条，
//! 见 SkinAndStatus.kt 的 barMode）—— 见 [ImeState.mode]。
//!
//! ⚠️ 这些动作的效果在**别处**（输入法状态），命令成败是本侧唯一看得见的东西 ——
//! 所以失败必须回到 UI 上（[runExternal] 的返回值就是给用户看的那句话）。

package xyz.erik.opi.candidate

import java.util.concurrent.TimeUnit

/**
 * 按钮 → 控制面动作名（平台中立的部分）。Windows 走 [runStatusAction] 的
 * `\\.\pipe\opi-control`（ControlPlane.kt）；Linux 把它翻成 `opi-ctl <动作>`。
 * null = 这个按钮没有控制面动作（开窗/切键盘/换肤等由 StatusBar.dispatch 自己处理）。
 */
internal fun statusActionOp(b: StatusBarButton): String? = when (b) {
    StatusBarButton.TRAD -> "trad"
    StatusBarButton.PUNCT -> "punct"
    StatusBarButton.FULLWIDTH -> "fullwidth"
    StatusBarButton.LANG -> "lang"
    StatusBarButton.ENPRO -> "englishpro"
    // MODE 走 [cycleMode]（要读新 token），不在这张「动作名」表里。
    StatusBarButton.MODE, StatusBarButton.SYMBOLS,
    StatusBarButton.VK, StatusBarButton.LOGO, StatusBarButton.SETTINGS -> null
}

/**
 * 跑一个状态栏动作（两个平台的**唯一**分派点）。返回 null = 成功；非 null = 给用户看的原因。
 * Windows：控制面（TSF 动作）；Linux：`opi-ctl <动作>`（既有契约）。
 */
fun runStatusAction(b: StatusBarButton, timeoutMs: Long = 3000): String? {
    val op = statusActionOp(b) ?: return null
    if (isWindows()) return controlAction(op, timeoutMs)
    val cmd = statusActionCommand(b) ?: return null
    return runExternal(cmd, timeoutMs)
}

/**
 * 纯函数：按钮 → 要跑的命令；null = 本按钮不跑命令（另有动作：开窗/开关键盘/打开设置）。
 *
 * Windows 上没有 `opi-ctl` 也没有 fcitx5（那是 Linux 的输入法框架），所以这条命令表
 * 只在 Linux 上走 —— 分派见 [runStatusAction]（Windows 先转控制面动作名）。
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
    StatusBarButton.ENPRO -> listOf("opi-ctl", "englishpro")
    // MODE 也不出命令：它走 [cycleMode]（进程内一次 `opi-ctl mode` 循环，返回新档位
    // token），与这张「按钮 → 命令行」的表不是一件事（分派见 StatusBar.dispatch）。
    StatusBarButton.MODE -> null
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

/**
 * 符号页/表情页的上屏（**两个宿主共用**：状态栏进程与设置窗）。
 * 返回 null = 送上去了；非 null = 给用户看的失败原因（页面原样显示）。
 *
 * · Linux：`opi-ctl insert <文本>`（D-Bus → addon 的 `ic->commitString`）—— 只要
 *   fcitx5 里有焦点输入框就能上屏，与进程拓扑无关；失败短语来自 [runExternal]，逐字有用。
 * · Windows：本机控制面 `\\.\pipe\opi-control` → 候选窗进程 broker → TSF
 *   （ControlPlane.kt）。TSF 把文本排进上屏队列，由下一个按键事件落进文档（插入
 *   延迟的既有语义，见 crates/tsf-opi/src/state.rs 的线程模型）。
 *
 * ⚠️ 两个宿主（状态栏、设置窗）**必须走这一条**：各写一份平台分支必然漂（此前
 * StatusBarApp 的调用方把 false 翻成写死的 Windows 解释，Linux 上的真实原因被顶掉了）。
 */
fun desktopSymbolCommit(text: String): String? =
    if (isWindows()) controlCommit(text)
    else runExternal(listOf("opi-ctl", "insert", text))

// ---------- 符号/表情搜索（2026-10-13） ----------

/**
 * 一次搜索的结局。**Failed 与「命中为空」必须分开**：前者是通道/协议坏了（原因要如实
 * 显示），后者是引擎说没搜到（正常的空结果）—— 把坏载荷画成「没搜到」正是最要防的假绿。
 */
sealed interface SymbolSearch {
    /** 引擎命中的条目文本（保序、已按引擎口径去重 —— 本侧不重排、不合并）。 */
    class Hits(val texts: List<String>) : SymbolSearch

    /** 通道/协议失败：`why` = **给用户看的**那句话（与上屏失败同一条纪律）。 */
    class Failed(val why: String) : SymbolSearch
}

/**
 * `opi-ctl search` 的一次应答 → 结局（纯函数：退出码 + 输出首行）。抽出来只为一件事：
 * 让「退出码/载荷怎么翻译成用户看到的话」有测试钉住 —— 进程那一段（[runCapture]）
 * 与 `insert`/`state` 是同一条路，不重复测。
 *
 * 载荷 = **一整行** JSON 字符串数组（`["…","…"]`；serde_json 写出，控制字符转义成
 * `\n` 这类转义序列，所以永远是一行）。退出码语义与其它动作同（`opi-ctl` 头部那张表）
 * —— 2=用法错误、3=通道不通、4=功能不可用、5=应答异形；这里不分类，只把原始原因带出来。
 */
internal fun searchResult(code: Int, firstLine: String?): SymbolSearch {
    if (code != 0) {
        return SymbolSearch.Failed(
            "「opi-ctl search」退出码 $code${if (firstLine.isNullOrEmpty()) "" else "：$firstLine"}",
        )
    }
    val texts = parseJsonStringArray(firstLine.orEmpty())
        ?: return SymbolSearch.Failed(
            "「opi-ctl search」应答看不懂（不是 JSON 文本数组）：${firstLine.orEmpty().take(120)}",
        )
    return SymbolSearch.Hits(texts)
}

/**
 * 搜一次。关键词必须非空 —— 空关键词在 `opi-ctl` 侧是**用法错误**（退出码 2），
 * 界面清空搜索框时该回浏览态，不该发这一问；真被问到就回「没有命中」，不把
 * 「搜索框空着」这种事报成错误。
 *
 * · Linux：`opi-ctl search <关键词>`（D-Bus `Search(string)` → 引擎 `search_symbols`）。
 * · Windows：控制面 search（候选窗进程 broker → TSF 的引擎）。
 *
 * 超时 3s：与状态栏动作同档。搜索是**打一个字发一次**（防抖在调用方），挂住不能等。
 */
fun desktopSymbolSearch(query: String, timeoutMs: Long = 3000): SymbolSearch {
    if (query.isBlank()) return SymbolSearch.Hits(emptyList())
    if (isWindows()) return controlSearch(query)
    return when (val r = runCapture(listOf("opi-ctl", "search", query), timeoutMs)) {
        is RunResult.Exit -> searchResult(r.code, r.firstLine)
        RunResult.Timeout -> SymbolSearch.Failed("「opi-ctl search」超时（无响应 ${timeoutMs / 1000}s）")
        is RunResult.Failed -> SymbolSearch.Failed("跑不了「opi-ctl」：${r.why}")
    }
}

// ---------- 模式循环（2026-10-14） ----------

/**
 * `opi-ctl mode` 能回的模式 token —— 与 `crates/opi-tools/src/bin/opi-ctl.rs` 的
 * `check_mode_shape`、C++ 侧 `opi_ctl_mode.h` 的派生表**同一张枚举**（三处同改）。
 * 三档都可达（循环 = 拼音(全拼) → 五笔 → 英文26键 → 拼音）。
 */
internal val MODE_TOKENS = setOf("pinyin", "en26", "wubi")

/**
 * 一次 `opi-ctl mode` 的应答 → 新模式 token（纯函数）。**空串 = 失败/认不得**：
 * 退出码非 0、或首行不在取值域里（`ok`、空行、大小写不对、带尾巴……）都回空串 ——
 * 认不得的应答绝不能当模式名画到条上（最要紧的指纹是 `ok`：插件把 mode 接到了旧路径）。
 *
 * 与 [searchResult] 的差别：那边失败要**带原因给用户看**，这边契约（2026-10-14 冻结）
 * 就是「token 或空串」，调用方按「空串 = 什么都没切成」处理。
 */
internal fun cycleModeToken(code: Int, firstLine: String?): String {
    if (code != 0) return ""
    val t = firstLine.orEmpty()
    return if (t in MODE_TOKENS) t else ""
}

/**
 * 点一下模式按钮：循环 **拼音(全拼) → 五笔 → 英文26键 → 拼音**。
 * 返回**新模式 token**（`pinyin` / `wubi` / `en26`）；**空串 = 没切成**
 * （通道不通/超时/应答异形）—— 此时显示保持原样，别把旧值当新档画。
 *
 * · Linux：`opi-ctl mode`（D-Bus Trigger → addon 的 `opiCtlCycleMode`，作用于当前 IC）。
 * · Windows：控制面 `action=mode`（TSF 的 `ctl.rs` 同一条循环：拼→五→en26→拼）。
 * 超时 3s：与状态栏动作同档。
 *
 * ⚠️ 与 [StatusBarButton.LANG] 的 `opi-ctl lang` **不是一条路**（2026-10-14 裁决）：
 * lang 只切中英，mode 是循环档位；今天两者结果恰好同形，五笔落地后才分家 ——
 * 别把中英按钮改到这条路上（StatusBarTest 有映射回归钉）。
 */
fun cycleMode(timeoutMs: Long = 3000): String {
    if (isWindows()) return controlMode()
    return when (val r = runCapture(listOf("opi-ctl", "mode"), timeoutMs)) {
        is RunResult.Exit -> cycleModeToken(r.code, r.firstLine)
        RunResult.Timeout, is RunResult.Failed -> ""
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
 *
 * [mode] = 档位 token（`mode=` 位，2026-10-14 追加，**行尾**）。**定状态栏模式按钮的
 * 初始档**：此前那条只能记住「自己点出来的档」，条一重启（或从别处改了模式）就与实际
 * 引擎不符，要点一下才对齐。取值域同 [cycleMode] 的应答（[MODE_TOKENS]）；缺键/认不得
 * = **未知（空串）** —— 与 [im] 的「缺键 = 未知」同一族（且与 interface 对齐过的形状：
 * statusbar 侧读 [ImeState.mode]，`""` 回落到「拼音」）。这里**不代为回落**：「未知 ⇒
 * 拼音」是消费方（状态栏）的回落，落在这边的话状态栏那道「老版本无 mode 位 ⇒ 拼音」
 * 的门禁就算删掉回落代码也照样绿 —— 回落归它，门禁才有牙。老插件的输出本来就没有这一
 * 格（那时也确实只有拼音/英文两档，且英文档那半由旧条自己的点击记住），**不许把老 CLI
 * 的输出变成解析失败**。⚠️ 服务端对「不在循环面上的态」（繁体/符号/双拼）报的是
 * **pinyin**（面外态收成「拼」：读法仍是拼音；2026-10-14 改判，见 cpp/opi_ctl_mode.h
 * 的改判记录）—— 所以 `trad=on` 与 `mode=pinyin` 同现是**正常**的（简繁由 `trad`
 * 格自述，两格各管各）。初版收的是 en26，那会让繁体中文输入时条上画英文档位 = 假
 * 标签；别把这条改回去，也别在这里「修」成别的。
 */
data class ImeState(
    val zh: Boolean = true,
    val trad: Boolean = false,
    val punct: Boolean = true,
    val fullwidth: Boolean = false,
    /** 当前输入法 uniqueName（`im=` 位）。缺键/空串 = 未知 —— 见类头 [im] 段。 */
    val im: String = "",
    /**
     * 档位 token（`mode=` 位）。缺键/认不得 = `""` = 未知（**不回落**；回落「拼音」在
     * 消费方）—— 见类头 [mode] 段。
     */
    val mode: String = "",
    /** 英文专业模式（`enpro=` 位，2026-10-19 追加）：缺键 = false（普通）。 */
    val englishPro: Boolean = false,
    /**
     * `enpro=` 位**在不在**（2026-10-19）：缺键 = 本机加载的还是老插件 ⇒ `englishpro`
     * 动作会被服务端回 err:5「未知动作」。与 [englishPro] 的值分开记 —— 缺键不是
     * 「普通模式」，是「这个能力还没有」；状态栏据此提示重启而不是弹裸错误。
     */
    val englishProKnown: Boolean = false,
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
            // 档位 token（`mode=` 位，2026-10-14 追加）：取值域内原样收，域外（"ok"/
            // 大小写/将来才有的 token）归一成 "" = 未知 —— **不原样收**：im 那边「任意
            // 串都收」是因为 uniqueName 没有枚举面，mode 有，收下等于把来路不明的串
            // 交给界面画档位。也不把整行判坏（老插件没这一格，硬判 = 把「版本旧」变成
            // 状态栏可见的失败）。
            "mode" -> st.copy(mode = if (v in MODE_TOKENS) v else "")
            "enpro" -> if (on != null) st.copy(englishPro = on, englishProKnown = true) else st
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
fun readImeState(): ImeState {
    if (isWindows()) return controlImeState() ?: ImeState()
    return when (val r = runCapture(listOf("opi-ctl", "state"), 1500)) {
        is RunResult.Exit -> if (r.code == 0) parseImeState(r.firstLine) else ImeState()
        RunResult.Timeout, is RunResult.Failed -> ImeState()
    }
}

/**
 * 英文专业键（en/pro）的**生效范围**（2026-10-19 用户裁决）：只控制英文是否查单词库，
 * 且只在英文输入法下有效。返回 null = 放行；非 null = 给用户看的原因，**不发动作**。
 *
 * 老插件（state 行没有 `enpro=` 位）也在这里挡住：动作会回 err:5「未知动作」，
 * 与其把裸错误弹给用户，不如直接告诉他重启输入法加载新插件。
 */
internal fun enproBlockReason(st: ImeState): String? = when {
    !st.englishProKnown -> "英文专业键暂不可用：请重启输入法后重试"
    st.zh -> "英文专业键只在英文输入法下有效"
    else -> null
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
