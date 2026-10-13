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
 * 状态栏（悬浮条）设置：master 隐藏 + 8 个按钮的常驻开关（未常驻 = 不显示）。
 * 默认 = master 不隐藏，其余按钮常驻，**软键盘除外**：2026-10-10 早先裁决「8 按钮
 * 全上」被**同日中午**的用户裁决收窄 —— 软键盘默认关（键盘窗不是人人要，要用的
 * 在设置里自己开）；配置文件里的显式值照旧优先（读侧 [OpSettings.fromIni]）。
 */
data class StatusBarSettings(
    val hidden: Boolean = false,
    val vk: Boolean = false,
    val symbols: Boolean = true,
    val trad: Boolean = true,
    val punct: Boolean = true,
    val lang: Boolean = true,
    val logo: Boolean = true,
    val fullwidth: Boolean = true,
    val settings: Boolean = true,
)

/**
 * 状态栏按钮。**枚举顺序 = 条上从左到右的展示顺序**（用户 2026-10-10 **最终冻结**：
 * logo / 中英 / 。，标点 / 简繁 / 月相 / 键盘 / Ω符号大全 / 设置）。第 3 位 = 标点
 * 双态（。，/.,）、第 7 位 = 符号大全（静态 Ω）—— 与图标映射（StatusBar.kt 的
 * iconText）配套，别只改一处。验收订正留痕：2026-10-10 中午先按行为描述换了**图标**、
 * 枚举序漏跟，用户真机截图（`[小欧][英][Ω][简][🌓][。，][⊞]`）抓到后同日把
 * SYMBOLS/PUNCT 两键对调。
 * `label` 兼作设置窗的开关名与状态栏的可访问性描述；条上的**图标**另有一套
 * （见 StatusBar.kt 的 iconText / MoonIcon / SettingsIcon）。
 */
enum class StatusBarButton(val label: String, val hint: String) {
    LOGO("logo", "品牌标识兼拖动把手；单击换下一款皮肤"),
    // ⚠️ 通路是 opi-ctl lang，不是 fcitx5-remote -t（后者停用整个 fcitx5，
    // 2026-10-10 中午用户踩过；事故留痕见 Actions.kt 头部与 StatusBarTest 的映射钉）。
    LANG("中英文切换", "与 Shift 轻按同一条通路（opi-ctl lang）"),
    PUNCT("中英标点", "中英文标点切换（调 opi-ctl）"),
    TRAD("简繁转换", "简 ⇄ 繁（调 opi-ctl）"),
    FULLWIDTH("全半角", "全角 ⇄ 半角（调 opi-ctl）"),
    VK("软键盘", "虚拟键盘窗：按键注入系统（会照常流经输入法）"),
    SYMBOLS("符号大全", "符号页入口"),
    SETTINGS("设置", "打开本设置窗"),
}

/** 该按钮是否被设为常驻（用户开关）。 */
fun StatusBarSettings.resident(b: StatusBarButton): Boolean = when (b) {
    StatusBarButton.VK -> vk
    StatusBarButton.SYMBOLS -> symbols
    StatusBarButton.TRAD -> trad
    StatusBarButton.PUNCT -> punct
    StatusBarButton.LANG -> lang
    StatusBarButton.LOGO -> logo
    StatusBarButton.FULLWIDTH -> fullwidth
    StatusBarButton.SETTINGS -> settings
}

/** 改一个按钮的常驻位（设置窗与单测共用的纯函数）。 */
fun StatusBarSettings.with(b: StatusBarButton, on: Boolean): StatusBarSettings = when (b) {
    StatusBarButton.VK -> copy(vk = on)
    StatusBarButton.SYMBOLS -> copy(symbols = on)
    StatusBarButton.TRAD -> copy(trad = on)
    StatusBarButton.PUNCT -> copy(punct = on)
    StatusBarButton.LANG -> copy(lang = on)
    StatusBarButton.LOGO -> copy(logo = on)
    StatusBarButton.FULLWIDTH -> copy(fullwidth = on)
    StatusBarButton.SETTINGS -> copy(settings = on)
}

/**
 * 该按钮在**本平台**有没有动作。Windows 上 `opi-ctl` 不存在（那**四个**动作
 * trad/punct/fullwidth/lang 是 Linux 侧的；symbols 自 2026-10-12 起两端都开符号页窗、
 * 不是命令）—— 没有动作的按钮不上条、也不在设置窗出开关
 * （「宁可少一页，不做点不动的开关」，见 SettingsWindow.kt 的既有原则）。
 */
internal fun statusButtonWired(b: StatusBarButton, windows: Boolean): Boolean = !windows ||
    b == StatusBarButton.VK || b == StatusBarButton.SYMBOLS ||
    b == StatusBarButton.LOGO || b == StatusBarButton.SETTINGS

/**
 * 纯函数：状态栏该显示哪些按钮（**master 隐藏优先**；未常驻不显示；Windows 过滤
 * 无动作项）。状态栏窗与设置窗共用这一份判定，两处各自算必然漂移。
 */
fun visibleStatusButtons(sb: StatusBarSettings, windows: Boolean): List<StatusBarButton> {
    if (sb.hidden) return emptyList()
    return StatusBarButton.entries.filter { sb.resident(it) && statusButtonWired(it, windows) }
}

