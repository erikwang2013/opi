// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 皮肤与状态栏的**枚举 + 纯函数**（2026-10-14 从 `Settings.kt` 拆出：加拼音方式/
//! 双拼布局两组名字表后那个文件 511 行撞 500 行门禁）。同包移动，零引用改动。
//!
//! 拆的界是「**数据与判定** vs **配置模型**」：本文件不碰 ini（读写都在 `Settings.kt`
//! 的 `OpSettings`），只放「哪些值合法、哪个按钮该上条」这类纯判定 —— 它们被状态栏
//! 窗、设置窗、测试三处引用，是稳定的公共面。

package xyz.erik.opi.candidate

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp

// ---------- 状态栏尺寸表（2026-10-21 从 StatusBar.kt 拆出：那边 510 行撞 500 行门禁） ----------

internal const val BTN = 30 // 100% 时的按钮边长
private const val GAP = 4
private const val PAD = 6
private const val BAR_H = 42

/**
 * Windows 状态栏的**紧凑系数**（2026-10-24 用户报障「win端的状态栏整体大了」）：
 * 只乘在绘制尺寸上 —— 配置里的 `StatusBarScale`（75–150）与设置页档位**原样保留**，
 * 100% 在 Windows 上画成 Linux 尺寸的 85%，用户仍可调到 150%。Linux 不乘（未报障，
 * 行为一字不变）。纯常量；系数只有 [barScaleFactor] 一个乘法口。
 */
internal const val WINDOWS_BAR_FACTOR = 0.85f

/** 缩放总系数 = 用户档位 ×（Windows 紧凑系数）。尺寸表的唯一乘法口，纯函数。 */
internal fun barScaleFactor(scale: Int, windows: Boolean): Float =
    statusBarScalePercent(scale) / 100f * (if (windows) WINDOWS_BAR_FACTOR else 1f)

/**
 * 状态栏的**唯一尺寸表**（2026-10-19 用户裁决「统一调整所有状态栏键大小」）：
 * 按钮、间距、内边距、条高全乘 [StatusBarSettings.scale] 同一系数。100% 时逐项等于
 * BTN/GAP/PAD/BAR_H；enpro/en26 与其他键共用这里同一个 [button] 盒（键帽再按 26/30
 * 画在盒内，见 KeycapFace.kt）。别在调用点各自乘 —— 第二份尺寸就是漂移口。
 * `windows = true` 时另乘 [WINDOWS_BAR_FACTOR]。
 */
internal data class BarMetrics(val button: Dp, val gap: Dp, val pad: Dp, val height: Dp)

/** [scale] 先经 [statusBarScalePercent] 收口（越界值按边界算）。纯函数。 */
internal fun barMetrics(scale: Int, windows: Boolean = false): BarMetrics {
    val k = barScaleFactor(scale, windows)
    return BarMetrics((BTN * k).dp, (GAP * k).dp, (PAD * k).dp, (BAR_H * k).dp)
}

/** 条宽随按钮数变（隐藏的按钮不占位）。[scale] 与 [barMetrics] 同一系数。纯函数，单测钉住。 */
internal fun statusBarWidthDp(
    buttonCount: Int,
    scale: Int = STATUS_BAR_SCALE_DEFAULT,
    windows: Boolean = false,
): Float = (PAD * 2 + BTN * buttonCount + GAP * (buttonCount - 1).coerceAtLeast(0)) * barScaleFactor(scale, windows)

/**
 * 条的裁剪形状（2026-10-24 用户报障「边角也不是弧形」；2026-10-25 用户订正「四角弧度
 * 太大，微微一点就行」）：Windows 用 **4dp 小圆角**，与状态栏窗的透明背景
 *（StatusBar.kt 的 `transparent`）配合，四角透出桌面又不会像胶囊；Linux 未报障 ⇒
 * 保持直角。纯函数，单测钉住。
 */
internal fun barShape(windows: Boolean): Shape =
    if (windows) RoundedCornerShape(4.dp) else RectangleShape

/**
 * 纯函数：条该不该显示（[StatusBarApp] 的 visible 用它）。
 *
 * 规则（2026-10-10 下午用户裁决「切到其他输入法时状态栏消失」）：
 * `enabled && (im 未知 || im == "opi")` ——
 * · [buttons] 空 = master 隐藏 / 按钮全关 ⇒ 恒隐藏（老规则，优先级最高，与 [im] 无关）；
 * · [im] 缺键/空串 = **未知** ⇒ Linux 维持现状（可见）、Windows 隐藏（[requireActive]）——
 *   glue 的 `im=` 还没合时旧输出正好落这里，轮询读失败（中性缺省）也落这里；
 * · [im] 非空且 ≠ "opi"（当前是别的输入法）⇒ 隐藏；切回 opi 由 1s 轮询让它自己出现。
 * · Windows 的 [requireActive]：只有 `im == "opi"` 才显示；TSF 未连接时状态读不到，
 *   状态栏进程虽随登录常驻但不可见（否则安装完就出现一条点哪都报错的条）。
 *
 * ⚠️ `"opi"` 是**跨轨字面**（glue 侧追加的 `im=` 值 = 本 addon 的 uniqueName）。对不上的
 * 表现是「opi 激活时条一直藏着」——响的错，不是静默错；但改它必须两侧一起改。
 */
internal fun barVisible(buttons: List<StatusBarButton>, im: String, requireActive: Boolean = false): Boolean {
    if (buttons.isEmpty()) return false
    // Windows 常驻状态栏仅在 TSF 激活（im=opi）时显示；未知状态隐藏，免得安装完就出现
    // 一条点哪都报错的条。Linux 保留「未知 ⇒ 维持可见」（旧 glue 缺 im= 时不闪）。
    if (requireActive) return im == "opi"
    return im.isEmpty() || im == "opi"
}

/** 连续读不到状态多少拍后回到中性（[nextStatePoll]）。Windows 上 im 空 = 隐藏。 */
internal const val STATE_CLEAR_AFTER_FAILS = 3

/** [nextStatePoll] 的结果：新的失败拍数 + 本拍是否该清回中性状态。 */
internal data class StatePoll(val fails: Int, val clear: Boolean)

/**
 * 1s 轮询的状态读失败计数（纯函数，[StatusBarApp] 用它）。
 *
 * 2026-10-22 订正：旧内联实现把第 3 拍花在自增上、第 4 拍才清，比注释承诺的
 * 「连续 3 拍 ≈3s」多一秒；本函数按 lead 要求收口到**第 3 拍即清**。
 * 读到状态 ⇒ 计数归零；`im` 已经为空 ⇒ 没有可清的旧状态（`clear=false`）。
 */
internal fun nextStatePoll(previousFails: Int, readOk: Boolean, imPresent: Boolean): StatePoll {
    if (readOk) return StatePoll(fails = 0, clear = false)
    val fails = (previousFails + 1).coerceAtMost(STATE_CLEAR_AFTER_FAILS)
    return StatePoll(fails, clear = fails >= STATE_CLEAR_AFTER_FAILS && imPresent)
}

/**
 * 「皮肤」的合法值。**字面值就是写进配置的字符串** —— 与 fcitx5 侧 `OpiSkin`
 * 的枚举字面值（`opi_config.h` 的 `FCITX_CONFIG_ENUM_NAME_WITH_I18N`）逐字相同：
 * Rust/C++ 读侧按字面匹配（`unmarshallOption`），写错一个字节 = 静默回默认值。
 * 十款的顺序与 `opi_skin.h` 的 `opiSkinThemeName` 一一对应；**三端同集合同顺序**
 * 由 SettingsTest 的 `skin names stay in lockstep` 从四份源文件现读现比。
 */
enum class SkinOption(val value: String) {
    KEEP("不改动"),
    LIGHT("浅色"),
    DARK("深色"),
    PAPER("宣纸"),
    CINNABAR("朱砂"),
    FRESH("清爽"),
    AZURE("湛蓝"),
    LEMON("柠檬"),
    SUNSET("晚霞"),
    STARLIGHT("星光"),
    WISTERIA("紫藤"),
    PINE("松林"),
    ;

    companion object {
        /** 未知值（手改坏/别的版本写的）回默认清爽 —— 与 bool 的坏值处理同一档。 */
        fun fromValue(v: String?): SkinOption =
            entries.firstOrNull { it.value == v } ?: FRESH
    }
}

/**
 * 状态栏（悬浮条）设置：master 隐藏 + 9 个按钮的常驻开关（未常驻 = 不显示）。
 * 默认 = master 不隐藏，其余按钮常驻，**软键盘除外**：2026-10-10 早先裁决「8 按钮
 * 全上」被**同日中午**的用户裁决收窄 —— 软键盘默认关（键盘窗不是人人要，要用的
 * 在设置里自己开）；配置文件里的显式值照旧优先（读侧 [OpSettings.fromIni]）。
 *
 * **10 枚按钮里这里只放 9 个开关**：2026-10-14 加的模式按钮（[StatusBarButton.MODE]）
 * 没有开关、恒常驻，所以它既没有字段也没有 ini 键 —— 别按「漏了一个」把它补上
 * （补它要先加 `StatusBarMode=` 键，见 [statusButtonWired] 的 MODE 段）。
 */
data class StatusBarSettings(
    val hidden: Boolean = false,
    val vk: Boolean = false,
    val symbols: Boolean = true,
    /** 话筒（2026-11-02；语音总开关在 OpSettings.speechEnabled，默认关）。 */
    val mic: Boolean = true,
    val trad: Boolean = true,
    val punct: Boolean = true,
    val lang: Boolean = true,
    // 英文专业键（2026-10-19 用户裁决「设置增加是否隐藏控制」）：默认常驻；
    // 两平台都可用（Windows 走控制面，见 statusButtonWired）。
    val enpro: Boolean = true,
    val logo: Boolean = true,
    val fullwidth: Boolean = true,
    val settings: Boolean = true,
    /**
     * 全部条上键的**统一缩放**（2026-10-19 用户裁决；键 `StatusBarScale`）：整数百分比
     * 75–150，缺省 100。BTN/GAP/PAD/BAR_H/条窗宽高/键帽/图标/字形全从这一系数派生
     * （见本文件的 [barMetrics]）—— 别另存第二份尺寸。
     */
    val scale: Int = STATUS_BAR_SCALE_DEFAULT,
)

/** 统一缩放的合法区间与缺省（百分比）。解析（Settings.kt）与绘制（StatusBar.kt）共用。 */
internal const val STATUS_BAR_SCALE_MIN = 75
internal const val STATUS_BAR_SCALE_MAX = 150
internal const val STATUS_BAR_SCALE_DEFAULT = 100

/** 手改坏/别的版本写来的越界值收进合法区间；正常路径原样返回。 */
internal fun statusBarScalePercent(v: Int): Int =
    v.coerceIn(STATUS_BAR_SCALE_MIN, STATUS_BAR_SCALE_MAX)

/** 设置页的档位（都落在合法区间内）；存的是百分比整数，不是档位下标。 */
internal val STATUS_BAR_SCALE_CHOICES = listOf(75, 90, 100, 110, 125, 150)

/**
 * 状态栏按钮。**枚举顺序 = 条上从左到右的展示顺序**（用户 2026-10-10 **最终冻结**：
 * logo / 中英 / 。，标点 / 简繁 / 月相 / 键盘 / Ω符号大全 / 设置）。第 3 位 = 标点
 * 双态（。，/.,）、第 7 位 = 符号大全（静态 Ω）—— 与图标映射（StatusBar.kt 的
 * iconText）配套，别只改一处。验收订正留痕：2026-10-10 中午先按行为描述换了**图标**、
 * 枚举序漏跟，用户真机截图（`[小欧][英][Ω][简][🌓][。，][⊞]`）抓到后同日把
 * SYMBOLS/PUNCT 两键对调。
 *
 * **2026-10-14 增 MODE**（用户裁决「状态栏在简繁按钮后增加模式切换按钮」）：
 * 插在 TRAD 与 FULLWIDTH 之间 ——**枚举序 = 显示序**，所以这一枚的位置就写在这个
 * 枚举里，没有第二处（StatusBarTest 的 `button order is the display order` 按 label
 * 逐位钉住）。它与其余按钮的两点不同：没有常驻开关（恒在条上，见
 * [visibleStatusButtons]）、面由**档位 token** 驱动而不是 [ImeState]（见 [modeFace]）。
 *
 * **2026-10-19 移 ENPRO**（用户裁决「enpro 键移到简繁键后面」）：从 LANG/PUNCT 之间
 * 挪到 TRAD 之后、MODE 之前。旧裁决（「放在标点符号键前面」）已被覆盖，别按旧位置改回。
 *
 * `label` 兼作设置窗的开关名与状态栏的可访问性描述；条上的**图标**另有一套
 * （见本文件的 iconText 与 BarGlyphs.kt 的各面）。
 */
enum class StatusBarButton(val label: String, val hint: String) {
    LOGO("logo", "品牌标识兼拖动把手；单击换下一款皮肤"),
    // ⚠️ 通路是 opi-ctl lang，不是 fcitx5-remote -t（后者停用整个 fcitx5，
    // 2026-10-10 中午用户踩过；事故留痕见 Actions.kt 头部与 StatusBarTest 的映射钉）。
    LANG("中英文切换", "与 Shift 轻按同一条通路（opi-ctl lang）"),
    PUNCT("中英标点", "中英文标点切换（调 opi-ctl）"),
    TRAD("简繁转换", "简 ⇄ 繁（调 opi-ctl）"),
    // 英文普通/专业（2026-10-19）：白底圆角键帽里 en 居中、pro 在右下角；只有专业模式
    // 查词库。受 `StatusBarEnPro` 常驻开关控制（默认开、设置窗可隐藏）；点击只在英文
    // 输入法下生效。2026-10-19 用户裁决把它移到简繁之后（见枚举头）。
    ENPRO("英文专业", "英文26键：普通只输出字母 ⇄ 专业查单词库（仅英文输入法；opi-ctl englishpro）"),
    // 模式（2026-10-14 用户裁决，原话：「状态栏在简繁按钮后增加模式切换按钮，默认拼音，
    // 可切换五笔、英文26键，按钮缩写：拼、五、en图标，其中en右下角加26」）——
    // **恒常驻**：它没有 `StatusBarMode=` 这个 ini 键（本批不动 Settings.kt 的读写表），
    // 所以既不出现在设置窗（见 [statusButtonWired] 的 MODE 支），也不走 resident。
    // 面 = 档位 token 驱动（pinyin「拼」/ en26「en」+ 右下角标「26」/ wubi「五」，见
    // [modeFace]/[modeCorner]），摆法见 BarGlyphs.kt 的 modeSlots；token 取值域由
    // opi-ctl 侧钉死（pinyin/en26/wubi）。en26 面同样套白底键帽（BarGlyphs 的
    // StatusBarButtonFace），其余档位保持原样。
    MODE("模式", "拼音 → 五笔 → 英文26键循环一次；失败保持原档并报一句"),
    FULLWIDTH("全半角", "全角 ⇄ 半角（调 opi-ctl）"),
    VK("软键盘", "虚拟键盘窗：按键注入系统（会照常流经输入法）"),
    SYMBOLS("符号大全", "符号页入口"),
    // 语音输入（2026-11-02 用户要求）：**严格插在符号后、设置前**，枚举序 = 显示序。
    // 默认常驻；总开关 SpeechEnabled 默认关（点了只提示去设置，不开麦）。
    MIC("语音输入", "点击开始录音，再点停止并识别；本地离线识别，正文不记录"),
    SETTINGS("设置", "打开本设置窗"),
}

/** 按钮的悬停提示 = `label：hint`（唯一源是枚举；非空且含 label 由 StatusBarTooltipTest 钉住）。 */
internal fun tooltipText(b: StatusBarButton): String = "${b.label}：${b.hint}"

/**
 * 该按钮是否被设为常驻（用户开关）。MODE **恒常驻** —— 它没有自己的 ini 键
 * （见 [StatusBarButton.MODE] 头注释），恒真而不是「可拨但拨不动」。
 */
fun StatusBarSettings.resident(b: StatusBarButton): Boolean = when (b) {
    StatusBarButton.VK -> vk
    StatusBarButton.SYMBOLS -> symbols
    StatusBarButton.MIC -> mic
    StatusBarButton.TRAD -> trad
    StatusBarButton.PUNCT -> punct
    StatusBarButton.LANG -> lang
    StatusBarButton.LOGO -> logo
    StatusBarButton.FULLWIDTH -> fullwidth
    StatusBarButton.SETTINGS -> settings
    StatusBarButton.ENPRO -> enpro
    StatusBarButton.MODE -> true
}

/**
 * 改一个按钮的常驻位（设置窗与单测共用的纯函数）。MODE 不动：它没有可写的键位，
 * 而设置窗也**不会**给它出行（[statusButtonWired] 的 MODE 支）—— 这里是 `this`
 * 而不是「假装改了」：万一将来多出调用点，改不动比改了没落盘好判。
 */
fun StatusBarSettings.with(b: StatusBarButton, on: Boolean): StatusBarSettings = when (b) {
    StatusBarButton.VK -> copy(vk = on)
    StatusBarButton.SYMBOLS -> copy(symbols = on)
    StatusBarButton.MIC -> copy(mic = on)
    StatusBarButton.TRAD -> copy(trad = on)
    StatusBarButton.PUNCT -> copy(punct = on)
    StatusBarButton.LANG -> copy(lang = on)
    StatusBarButton.LOGO -> copy(logo = on)
    StatusBarButton.FULLWIDTH -> copy(fullwidth = on)
    StatusBarButton.SETTINGS -> copy(settings = on)
    StatusBarButton.ENPRO -> copy(enpro = on)
    StatusBarButton.MODE -> this
}

/**
 * 该按钮有没有动作、以及**设置窗该不该给它一行开关**（SettingsPages.kt 拿它当过滤器）。
 *
 * 2026-10-20 起**两个平台动作面相同**：Windows 的 trad/punct/fullwidth/lang/
 * englishpro/mode 走本机控制面（ControlPlane.kt → 候选窗进程 broker → TSF），
 * 不再有「Windows 上没有 opi-ctl ⇒ 按钮不上条」的缺口。`windows` 参数保留只为
 * 调用点形状稳定；真正的平台差异已经只剩动作的**实现**（`runStatusAction` 内分派）。
 *
 * ⚠️ MODE 仍返回 false，但理由换了：它**没有常驻开关**（没有 `StatusBarMode=` ini
 * 键），所以设置窗不出它那行 —— 出了就是拨了没反应的开关（拨掉后 1s 轮询把缺键读回
 * 默认 = 自己弹回来）。条上有没有它**不归这里管**：[visibleStatusButtons] 单独判。
 */
internal fun statusButtonWired(b: StatusBarButton, @Suppress("UNUSED_PARAMETER") windows: Boolean): Boolean =
    b != StatusBarButton.MODE

/**
 * 纯函数：状态栏该显示哪些按钮（**master 隐藏优先**；未常驻不显示）。
 * 状态栏窗与设置窗共用这一份判定，两处各自算必然漂移。
 *
 * 2026-10-20 起 Windows 与 Linux 同一条：动作面已由控制面补齐（见
 * [statusButtonWired]），不再按平台过滤。MODE 没有常驻开关 ⇒ 恒在条上。
 */
fun visibleStatusButtons(sb: StatusBarSettings, @Suppress("UNUSED_PARAMETER") windows: Boolean): List<StatusBarButton> {
    if (sb.hidden) return emptyList()
    return StatusBarButton.entries.filter { it == StatusBarButton.MODE || sb.resident(it) && statusButtonWired(it, windows) }
}

// ---------- 模式按钮的档位（2026-10-14 用户裁决「简繁后加模式切换按钮」）----------

/**
 * 模式按钮的**回落档位**（用户 2026-10-14 裁决「默认拼音」）：`opi-ctl state` 的
 * `mode=` 位拿不到可用值时画的档 —— 缺键（老版本插件）/ 域外值（解析层筛成 `""`）/
 * 整行读失败（[readImeState] 返回中性 [ImeState]）。判定只有一处：[barMode]。
 *
 * ⚠️ 它是**回落**，不是「引擎现在的档」：真值走 [barMode] 读 state 行（2026-10-14
 * modeplumb 给 `opi-ctl state` 补了 `mode=` 位，原先那条「条只记得自己点出来的档、
 * 重启后画错」的读侧缺口就此堵上）。别绕开 [barMode] 直接拿它画按钮。
 */
internal const val MODE_DEFAULT = "pinyin"

/**
 * 档位 token → 按钮面上的**缩写**（用户 2026-10-14 原话：「按钮缩写：拼、五、en图标，
 * 其中en右下角加26」）。取值域（`pinyin` / `en26` / `wubi`）的权威**不在本文件**：
 * `crates/opi-tools/src/bin/opi-ctl.rs` 的 `check_mode_shape` + `crates/fcitx5-opi/cpp/
 * opi_ctl_mode.h` 的派生表，桌面侧只认字面（认不得的应答在 Actions.cycleModeToken
 * 就被丢掉，流不到这里）。
 *
 * 未知 token 落「拼」这一支：画出来是个中性档位，不是空白按钮。
 *
 * **加一档的插入点**（别只加一处）：**面**这一层 = 本函数 + 需要角标时的 [modeCorner]；
 * token 取值域在 `Actions.kt` 的 `MODE_TOKENS`（认不得的应答在那里就被丢掉，流不到这里）；
 * 最后要**引擎侧真会发**这个 token。
 *
 * `wubi` 这一行**现在是真态**（2026-10-14 M4 第二波落地：引擎侧 `opi_ctl_mode.h` 的循环
 * 已是 **拼 → 五 → en26 → 拼**，`wubi` 在 `MODE_TOKENS` 里、可达）。它当初（同日早些时候，
 * 用户裁决「先两态，五笔随 M4 接」时）是**防假标签**：`MODE_TOKENS` 认 wubi 而面表没有它，
 * 引擎在五笔档时条上会写「拼」= 给用户看假档位。锁定它的门禁（放行 token 必须有面）在
 * `StatusBarTest` 的 `mode tokens map to the frozen abbreviations` —— 加第四档时那条先红。
 */
internal fun modeFace(mode: String): String = when (mode) {
    "en26" -> "en"
    "wubi" -> "五"
    else -> "拼" // pinyin（默认档）+ 任何认不得的值
}

/**
 * 档位 token → 右下角的**角标**（用户原话「其中en右下角加26」）：只有 en26 有，
 * 其余档位无角标（返回 null = 不占位，那一个字占满整块面）。
 */
internal fun modeCorner(mode: String): String? = if (mode == "en26") "26" else null

/**
 * `opi-ctl state` 的一行 → 条上该画的档位（2026-10-14，堵 [MODE_DEFAULT] 那条读侧缺口：
 * 条此前只能记住「自己点出来的」档，引擎若在别的档、条重启后就画错）。
 *
 * `st.mode` 由解析层按取值域筛过（域外/认不得 ⇒ `""`）⇒ 这里只做**回落**这一件事：
 * 空 = 老版本插件没发 `mode=` 位 / 通道不通整行读不到 ⇒ 默认档（用户裁决「默认拼音」）。
 * ⚠️ **回落刻意留在这里、不在解析层**（modeplumb 的决定）：解析层若代我回落，
 * 「老版本无 mode 位 ⇒ 拼音」这条门禁把 `ifEmpty` 删掉也照样绿 —— 那就没牙了。
 *
 * ⚠️ 别拿它反推「引擎在哪个模式」：档位面只有三档（拼音 / 英文26键 / 五笔），
 * **表达不了繁体/符号/数字/双拼**这些态。2026-10-14 lead 改判：不在循环面上的态服务端
 * 一律报 `pinyin`（旧口径是把它们收进 `en26` ⇒ 打繁体的人看见「en26」＝假标签，与
 * [modeFace] 那条 wubi 防假标签同类）。派生在 `crates/fcitx5-opi/cpp/opi_ctl_mode.h` 的
 * `opiCtlStateModeToken`。要表达繁体就看 [ImeState.trad]（条上 TRAD 那一枚负责它）——
 * 反推恒假：拼音态与繁体态的 `mode` 位**逐字相同**。
 */
internal fun barMode(st: ImeState): String = st.mode.ifEmpty { MODE_DEFAULT }

// ---------- 按钮面上的字（2026-10-24 从 StatusBar.kt 移来：那边加 Windows 紧凑/圆角
// 后撞 500 行门禁；同包 internal 引用不变，BarGlyphs 与单测照旧。纯函数） ----------

/**
 * 按钮面上的字。两轮裁决的**时间线**（防止按任一半截「改回去」）：
 * · 2026-10-10 深夜 B 版：SYMBOLS = 「。，」静态、PUNCT = 「Ω」静态；
 * · 同日**中午订正**（按用户对按钮行为的描述）：两枚**互换**且 PUNCT 变**双态** ——
 *   SYMBOLS（符号大全）= 静态「Ω」；PUNCT（中英标点）= punct=on「。，」/ off「.,」。
 * 字面都是用户写的：「。，」是中文侧（**先句号**，勿按惯例改成「，。」）；「.,」是
 * 英文侧（先点号）—— 都不是笔误。
 * · TRAD / LANG = 单字，且是**当前状态**（简/繁、中/英），随输入法状态翻转 ——
 *   用户原话「文字标签全图标化」：多字标签（简繁/中英）全去。
 * 全半角、设置、键盘、小欧不出字：画月相/四小方块/键盘/小欧。
 */
internal fun iconText(b: StatusBarButton, st: ImeState): String = when (b) {
    StatusBarButton.SYMBOLS -> "Ω"
    StatusBarButton.TRAD -> tradIcon(st)
    StatusBarButton.PUNCT -> punctIcon(st)
    StatusBarButton.LANG -> langIcon(st)
    // MODE 不从这里出字：它的面由**档位 token** 驱动（[modeFace]/[modeCorner]，面在
    // BarGlyphs.kt 的 modeSlots）—— 本函数的入参里没有档位，给不出正确字面。
    StatusBarButton.MODE -> ""
    StatusBarButton.ENPRO -> "" // 两行 en/pro 由 enproSlots 画，不走单字 iconText
    StatusBarButton.FULLWIDTH, StatusBarButton.SETTINGS, StatusBarButton.MIC,
    StatusBarButton.VK, StatusBarButton.LOGO -> "" // 这五个画图标，不出字
}

/** 简繁按钮面的字 = **当前输出**（转繁开着写「繁」）；用户裁决②：随真实状态翻转。 */
internal fun tradIcon(st: ImeState): String = if (st.trad) "繁" else "简"

/** 中英按钮面的字 = 当前模式（zh/en）。 */
internal fun langIcon(st: ImeState): String = if (st.zh) "中" else "英"

/**
 * 标点按钮面的字 = **当前标点态**（2026-10-10 中午订正：双态）。
 * on = 中文标点「。，」（用户字面序，先句号）；off = 英文「.,」（用户字面序，先点号）。
 */
internal fun punctIcon(st: ImeState): String = if (st.punct) "。，" else ".,"

