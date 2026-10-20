// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 皮肤与状态栏的**枚举 + 纯函数**（2026-10-14 从 `Settings.kt` 拆出：加拼音方式/
//! 双拼布局两组名字表后那个文件 511 行撞 500 行门禁）。同包移动，零引用改动。
//!
//! 拆的界是「**数据与判定** vs **配置模型**」：本文件不碰 ini（读写都在 `Settings.kt`
//! 的 `OpSettings`），只放「哪些值合法、哪个按钮该上条」这类纯判定 —— 它们被状态栏
//! 窗、设置窗、测试三处引用，是稳定的公共面。

package xyz.erik.opi.candidate

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
        /** 未知值（手改坏/别的版本写的）回默认 —— 与 bool 的坏值处理同一档。 */
        fun fromValue(v: String?): SkinOption =
            entries.firstOrNull { it.value == v } ?: KEEP
    }
}

/**
 * 下一款皮肤（状态栏 LOGO 单击循环换肤用，2026-10-13 用户裁决「点击 logo 的颜色变化」）：
 * 按枚举序绕一圈，**含「不改动」**——转一圈能回到条的本色，不是单向跑掉。
 * 纯函数，首尾与「一圈不重样」由 StatusBarTest 钉住。
 */
internal fun nextSkin(skin: SkinOption): SkinOption =
    SkinOption.entries[(skin.ordinal + 1) % SkinOption.entries.size]

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
    val trad: Boolean = true,
    val punct: Boolean = true,
    val lang: Boolean = true,
    // 英文专业键（2026-10-19 用户裁决「设置增加是否隐藏控制」）：默认常驻，
    // 与其他 opi-ctl 动作一样只在 Linux 上可用（见 statusButtonWired）。
    val enpro: Boolean = true,
    val logo: Boolean = true,
    val fullwidth: Boolean = true,
    val settings: Boolean = true,
    /**
     * 全部条上键的**统一缩放**（2026-10-19 用户裁决；键 `StatusBarScale`）：整数百分比
     * 75–150，缺省 100。BTN/GAP/PAD/BAR_H/条窗宽高/键帽/图标/字形全从这一系数派生
     * （见 StatusBar.kt 的 [barMetrics]）—— 别另存第二份尺寸。
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
 * 逐位钉住）。它与其余按钮的两点不同：没有常驻开关（恒在 Linux 条上，见
 * [visibleStatusButtons]）、面由**档位 token** 驱动而不是 [ImeState]（见 [modeFace]）。
 *
 * **2026-10-19 移 ENPRO**（用户裁决「enpro 键移到简繁键后面」）：从 LANG/PUNCT 之间
 * 挪到 TRAD 之后、MODE 之前。旧裁决（「放在标点符号键前面」）已被覆盖，别按旧位置改回。
 *
 * `label` 兼作设置窗的开关名与状态栏的可访问性描述；条上的**图标**另有一套
 * （见 StatusBar.kt 的 iconText 与 BarGlyphs.kt 的各面）。
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
    MODE("模式", "拼音 ⇄ 英文26键循环一次（五笔待引擎侧落地）；失败保持原档并报一句"),
    FULLWIDTH("全半角", "全角 ⇄ 半角（调 opi-ctl）"),
    VK("软键盘", "虚拟键盘窗：按键注入系统（会照常流经输入法）"),
    SYMBOLS("符号大全", "符号页入口"),
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
 * 该按钮在**本平台**有没有动作、以及**设置窗该不该给它一行开关**（SettingsPages.kt
 * 拿它当过滤器，两处共用一份判定，各判一次必然漂移 —— 见那里的注释）。
 *
 * Windows 上 `opi-ctl` 不存在（trad/punct/fullwidth/lang/englishpro 这**五个**动作是
 * Linux 侧的；symbols 自 2026-10-12 起两端都开符号页窗、不是命令）—— 没有动作的按钮不上条、
 * 也不在设置窗出开关（「宁可少一页，不做点不动的开关」，见 SettingsWindow.kt 的既有原则）。
 *
 * ⚠️ MODE 只该管住**一头**（2026-10-14，别照前几枚的形状「修正」它）：它**没有常驻
 * 开关**（没有 `StatusBarMode=` ini 键），所以设置窗不出它那行 —— 出了就是拨了没反应的
 * 开关（拨掉后 1s 轮询把缺键读回默认 = 自己弹回来）。这里返回 false 说的正是这件事。
 * 条上有没有它**不归这里管**：[visibleStatusButtons] 单独判这一枚（按它自己的动作面，
 * 不是按这个开关面）。等 `StatusBarMode=` 键落地（Settings.kt 的读写表 + 本文件的
 * resident/with），把这一支删掉 —— 那时 MODE 归 resident 管，[visibleStatusButtons]
 * 的 MODE 支要**一并删**（两处都是「没有开关」这一个前提的下游，别只删一处）。
 */
internal fun statusButtonWired(b: StatusBarButton, windows: Boolean): Boolean = when {
    b == StatusBarButton.MODE -> false
    !windows -> true
    else -> b == StatusBarButton.VK || b == StatusBarButton.SYMBOLS ||
        b == StatusBarButton.LOGO || b == StatusBarButton.SETTINGS
}

/**
 * 纯函数：状态栏该显示哪些按钮（**master 隐藏优先**；未常驻不显示；Windows 过滤
 * 无动作项）。状态栏窗与设置窗共用这一份判定，两处各自算必然漂移。
 *
 * MODE 单列一支（见 [statusButtonWired] 的 MODE 段）：它没有常驻开关 ⇒ 不走 resident，
 * 恒在 Linux 的条上；而它的动作（`opi-ctl mode`）只有 Linux 侧有 ⇒ Windows 照老规矩
 * 不上条（「没有动作的按钮不上条」，与 statusButtonWired 里那四枚同一条理由）。
 */
fun visibleStatusButtons(sb: StatusBarSettings, windows: Boolean): List<StatusBarButton> {
    if (sb.hidden) return emptyList()
    return StatusBarButton.entries.filter {
        if (it == StatusBarButton.MODE) !windows
        else sb.resident(it) && statusButtonWired(it, windows)
    }
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

