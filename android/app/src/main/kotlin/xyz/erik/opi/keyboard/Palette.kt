// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color

/**
 * 键盘配色（2026-10-09 集中化）。**唯一色板** —— 此前各控件里散着字面量
 * （KeyButton/CandidateBar/SymbolPanel），「暗光模式」「长辈模式」要换肤就必须
 * 先收敛到这里；散着改 = 每加一个模式都要再翻一遍全部文件，必漏。
 *
 * 由 [xyz.erik.opi.ime.ImeScreen] 按当前键盘模式 provide 进
 * [LocalKeyboardPalette]，各控件读 `LocalKeyboardPalette.current`。
 * 没包 provider 的地方（测试、单件预览）拿到默认 [LightPalette]。
 *
 * ⚠️ 值本身是既有字面量的原样搬运（浅色板 = 改造前的观感），改配色只改本文件。
 */
data class KeyboardPalette(
    /** 普通键底色 / 键面字色。 */
    val keyBg: Color,
    val keyFg: Color,
    /** 高亮键（⇧ 激活态、候选选中项）底色 / 键面字色。 */
    val keyHighlightBg: Color,
    val keyHighlightFg: Color,
    /** 候选栏底色 / 缓冲字色（半透明）/ 副文字色。 */
    val barBg: Color,
    val barBufferFg: Color,
    val barDimFg: Color,
    /** 强调色（面板标题、页码等）。 */
    val accent: Color,
    /** 面板（符号/生僻字）底色与主字色。 */
    val panelBg: Color,
    val panelFg: Color,
    /** 工具条底色与图标色。 */
    val toolbarBg: Color,
    val toolbarFg: Color,
)

/** 浅色（默认，= 2026-10-09 之前的观感，逐值搬运）。 */
val LightPalette = KeyboardPalette(
    keyBg = Color(0xFFE0E0E0),
    keyFg = Color(0xFF1F1F1F),
    keyHighlightBg = Color(0xFF546E7A),
    keyHighlightFg = Color(0xFFFFFFFF),
    barBg = Color(0xFFEEEEEE),
    barBufferFg = Color(0x8A000000),
    barDimFg = Color(0xFF757575),
    accent = Color(0xFF455A64),
    panelBg = Color(0xFFEEEEEE),
    panelFg = Color(0xFF1F1F1F),
    toolbarBg = Color(0xFFE0E0E0),
    toolbarFg = Color(0xFF455A64),
)

/** 暗光模式：深底浅字（对比度按 WCAG AA 对 18pt 正文宽松档取）。 */
val DarkPalette = KeyboardPalette(
    keyBg = Color(0xFF2A2A2A),
    keyFg = Color(0xFFE6E6E6),
    keyHighlightBg = Color(0xFF4A6572),
    keyHighlightFg = Color(0xFFFFFFFF),
    barBg = Color(0xFF1F1F1F),
    barBufferFg = Color(0xCCE6E6E6),
    barDimFg = Color(0xFF9E9E9E),
    accent = Color(0xFF9FB4BE),
    panelBg = Color(0xFF1F1F1F),
    panelFg = Color(0xFFE6E6E6),
    toolbarBg = Color(0xFF2A2A2A),
    toolbarFg = Color(0xFFCFD8DC),
)

/** 长辈模式：高对比（纯白底 + 纯黑字 + 更大字号由 ImeScreen 的 elder 缩放管）。 */
val ElderPalette = KeyboardPalette(
    keyBg = Color(0xFFFAFAFA),
    keyFg = Color(0xFF000000),
    keyHighlightBg = Color(0xFF263238),
    keyHighlightFg = Color(0xFFFFFFFF),
    barBg = Color(0xFFFFFFFF),
    barBufferFg = Color(0xFF000000),
    barDimFg = Color(0xFF424242),
    accent = Color(0xFF263238),
    panelBg = Color(0xFFFFFFFF),
    panelFg = Color(0xFF000000),
    toolbarBg = Color(0xFFF5F5F5),
    toolbarFg = Color(0xFF263238),
)

/** 宣纸皮肤（2026-10-09）：底色/字色/强调取 fcitx5 侧 opi-paper 主题的同一套
 * （scripts/gen_fcitx5_themes.py 的 PALETTES：「宣纸」#F5EFE1 / #3D3222 / #8A5A2B），
 * 键面比底略深一档做分离。 */
val PaperPalette = KeyboardPalette(
    keyBg = Color(0xFFE7DFCC),
    keyFg = Color(0xFF3D3222),
    keyHighlightBg = Color(0xFF8A5A2B),
    keyHighlightFg = Color(0xFFFFFBF2),
    barBg = Color(0xFFF5EFE1),
    barBufferFg = Color(0xCC3D3222),
    barDimFg = Color(0xFF8A7A5B),
    accent = Color(0xFF8A5A2B),
    panelBg = Color(0xFFF5EFE1),
    panelFg = Color(0xFF3D3222),
    toolbarBg = Color(0xFFE7DFCC),
    toolbarFg = Color(0xFF8A5A2B),
)

/** 朱砂皮肤：fcitx5 opi-cinnabar 主题同套（#B3272D / #FFF6EC / #FFD9A0），
 * 键面取主题的 menu_bg 一档（#9E2228）压暗。 */
val CinnabarPalette = KeyboardPalette(
    keyBg = Color(0xFF9E2228),
    keyFg = Color(0xFFFFF6EC),
    keyHighlightBg = Color(0xFFFFD9A0),
    keyHighlightFg = Color(0xFF5A1012),
    barBg = Color(0xFF8A1C21),
    barBufferFg = Color(0xCCFFF6EC),
    barDimFg = Color(0xFFE8BCB4),
    accent = Color(0xFFFFD9A0),
    panelBg = Color(0xFF9E2228),
    panelFg = Color(0xFFFFF6EC),
    toolbarBg = Color(0xFF8A1C21),
    toolbarFg = Color(0xFFFFD9A0),
)

/** 清爽皮肤（2026-10-10）：清透浅青绿 —— fcitx5 opi-fresh 同套
 * （#E6F5EE / #16332B / #2E7D6B），键面比底深一档做分离。 */
val FreshPalette = KeyboardPalette(
    keyBg = Color(0xFFCFE9DF),
    keyFg = Color(0xFF16332B),
    keyHighlightBg = Color(0xFF2E7D6B),
    keyHighlightFg = Color(0xFFFFFFFF),
    barBg = Color(0xFFE6F5EE),
    barBufferFg = Color(0xCC16332B),
    barDimFg = Color(0xFF4F7A6C),
    accent = Color(0xFF2E7D6B),
    panelBg = Color(0xFFE6F5EE),
    panelFg = Color(0xFF16332B),
    toolbarBg = Color(0xFFCFE9DF),
    toolbarFg = Color(0xFF2E7D6B),
)

/** 湛蓝皮肤：深天空蓝 —— fcitx5 opi-azure 同套（#3A679E / #F4F8FD / #FFE3A8），
 * 选中态用浅天蓝底 + 深海军蓝字（对比度 ~9:1，不必借强调色）。 */
val AzurePalette = KeyboardPalette(
    keyBg = Color(0xFF3C6AA3),
    keyFg = Color(0xFFF4F8FD),
    keyHighlightBg = Color(0xFFCFE6FF),
    keyHighlightFg = Color(0xFF12395C),
    barBg = Color(0xFF3A679E),
    barBufferFg = Color(0xF0F4F8FD),
    barDimFg = Color(0xFFC9DCF2),
    accent = Color(0xFFFFE3A8),
    panelBg = Color(0xFF3A679E),
    panelFg = Color(0xFFF4F8FD),
    toolbarBg = Color(0xFF33598A),
    toolbarFg = Color(0xFFFFE3A8),
)

/** 柠檬皮肤：柠檬黄底 + 深橄榄字 —— fcitx5 opi-lemon 同套
 * （#FBEE7E / #3A3000 / #7E6200，2026-10-13 随 fcitx5 侧重色同步）；
 * 浅底必须深字，键面取主题 menu_bg 一档（#F9EA6C，比底黄深）。 */
val LemonPalette = KeyboardPalette(
    keyBg = Color(0xFFF9EA6C),
    keyFg = Color(0xFF3A3000),
    keyHighlightBg = Color(0xFF7E6200),
    keyHighlightFg = Color(0xFFFFFDF0),
    barBg = Color(0xFFFBEE7E),
    barBufferFg = Color(0xCC3A3000),
    barDimFg = Color(0xFF6E5F1E),
    accent = Color(0xFF7E6200),
    panelBg = Color(0xFFFBEE7E),
    panelFg = Color(0xFF3A3000),
    toolbarBg = Color(0xFFF9EA6C),
    toolbarFg = Color(0xFF5A4A00),
)

/** 晚霞皮肤：橙粉暖色 —— fcitx5 opi-sunset 同套
 * （#F7C79C / #4A2317 / #A63412，2026-10-13 随 fcitx5 侧重色同步），
 * 键面取主题 menu_bg 一档（#F4BC8A，比底深），强调 = 落日橘。 */
val SunsetPalette = KeyboardPalette(
    keyBg = Color(0xFFF4BC8A),
    keyFg = Color(0xFF4A2317),
    keyHighlightBg = Color(0xFFA63412),
    keyHighlightFg = Color(0xFFFFF3E8),
    barBg = Color(0xFFF7C79C),
    barBufferFg = Color(0xCC4A2317),
    barDimFg = Color(0xFF8E5A40),
    accent = Color(0xFFA63412),
    panelBg = Color(0xFFF7C79C),
    panelFg = Color(0xFF4A2317),
    toolbarBg = Color(0xFFF4BC8A),
    toolbarFg = Color(0xFFB4441F),
)

/** 星光皮肤（深色）：深靛蓝夜空底 + 亮星金强调 —— fcitx5 opi-starlight 同套
 * （#1B2138 / #E8EBF7 / #FFD966）。与「暗光」的区分度：暗光是中性灰 + 雾蓝强调，
 * 本款是靛蓝底 + 金色点缀（实测键面 #262E4A 也明显偏蓝）。 */
val StarlightPalette = KeyboardPalette(
    keyBg = Color(0xFF262E4A),
    keyFg = Color(0xFFE8EBF7),
    keyHighlightBg = Color(0xFFFFD966),
    keyHighlightFg = Color(0xFF23283F),
    barBg = Color(0xFF161B2E),
    barBufferFg = Color(0xCCE8EBF7),
    barDimFg = Color(0xFF9AA3C0),
    accent = Color(0xFFFFD966),
    panelBg = Color(0xFF1B2138),
    panelFg = Color(0xFFE8EBF7),
    toolbarBg = Color(0xFF262E4A),
    toolbarFg = Color(0xFFFFD966),
)

/** 紫藤皮肤（浅色）：淡紫底 + 藤紫高亮 —— fcitx5 opi-wisteria 同套
 * （底 #EDE7FB / 字 #3A2A63 / 强调 #6B4BC4），键面取主题 menu_bg 一档（#E3D9F8）。
 * 不复用既有款的判据（按底色 HSL 算）：本款 H258/L95，最近的「星光」（底 #1B2138）
 * H228/L16 —— 差的是**明度 79**，紫调只是看着相近。 */
val WisteriaPalette = KeyboardPalette(
    keyBg = Color(0xFFE3D9F8),
    keyFg = Color(0xFF3A2A63),
    keyHighlightBg = Color(0xFF6B4BC4),
    keyHighlightFg = Color(0xFFF7F3FF),
    barBg = Color(0xFFEDE7FB),
    barBufferFg = Color(0xCC3A2A63),
    barDimFg = Color(0xFF6E5E96),
    accent = Color(0xFF6B4BC4),
    panelBg = Color(0xFFEDE7FB),
    panelFg = Color(0xFF3A2A63),
    toolbarBg = Color(0xFFE3D9F8),
    toolbarFg = Color(0xFF5A3AA8),
)

/** 松林皮肤（深色）：松林绿底 + 浅苔文字 —— fcitx5 opi-pine 同套
 * （底 #1E3A2F / 字 #DCEEE2 / 强调 #7FD3A2），键面取主题 menu_bg 一档（#26473A）。
 * 判据同上：本款 H156/L17，与「清爽」（底 #E6F5EE）H152/L93 是同色相族的深色版
 * （明度差 76，是对照不是重复）；与「星光」同为深色但色相 156 vs 228。 */
val PinePalette = KeyboardPalette(
    keyBg = Color(0xFF26473A),
    keyFg = Color(0xFFDCEEE2),
    keyHighlightBg = Color(0xFF7FD3A2),
    keyHighlightFg = Color(0xFF17301F),
    barBg = Color(0xFF1E3A2F),
    barBufferFg = Color(0xCCDCEEE2),
    barDimFg = Color(0xFF9DBFAE),
    accent = Color(0xFF7FD3A2),
    panelBg = Color(0xFF1E3A2F),
    panelFg = Color(0xFFDCEEE2),
    toolbarBg = Color(0xFF26473A),
    toolbarFg = Color(0xFF7FD3A2),
)

/**
 * 皮肤（2026-10-09 用户裁决「移动端没有皮肤设置」；2026-10-10 加五款颜色；
 * 2026-10-13 加两款：紫藤/松林）。
 * **十二款与桌面/配置面同名**（不改动/浅色/深色/宣纸/朱砂/清爽/湛蓝/柠檬/晚霞/星光/紫藤/松林），
 * 配色与 fcitx5 主题同一套设计语言。
 *
 * 三条配色轴的**优先级**（ImeScreen 里那条 when 就是这个顺序）：
 *   长辈模式 > 暗光模式 > 皮肤 —— 模式是临时状态、皮肤是底：
 *   「深色皮肤」与「暗光模式」是同一个观感，两者同时开时暗光赢，不冲突。
 * 「不改动」= 不指定皮肤（默认观感，等于此前只有浅色板的行为）。
 */
enum class SkinOption(val id: String, val label: String) {
    FOLLOW("follow", "不改动"),
    LIGHT("light", "浅色"),
    DARK("dark", "深色"),
    PAPER("paper", "宣纸"),
    CINNABAR("cinnabar", "朱砂"),
    FRESH("fresh", "清爽"),
    AZURE("azure", "湛蓝"),
    LEMON("lemon", "柠檬"),
    SUNSET("sunset", "晚霞"),
    STARLIGHT("starlight", "星光"),
    WISTERIA("wisteria", "紫藤"),
    PINE("pine", "松林"),
    ;

    /** 皮肤 → 色板。「不改动」与「浅色」同板（默认观感）。 */
    val palette: KeyboardPalette
        get() = when (this) {
            FOLLOW, LIGHT -> LightPalette
            DARK -> DarkPalette
            PAPER -> PaperPalette
            CINNABAR -> CinnabarPalette
            FRESH -> FreshPalette
            AZURE -> AzurePalette
            LEMON -> LemonPalette
            SUNSET -> SunsetPalette
            STARLIGHT -> StarlightPalette
            WISTERIA -> WisteriaPalette
            PINE -> PinePalette
        }

    companion object {
        /** 未知 id（旧版本/手改坏了）→ 清爽（默认皮肤），不抛（与其它偏好同一条容错约定）。 */
        fun fromId(id: String?): SkinOption = entries.firstOrNull { it.id == id } ?: FRESH
    }
}

/**
 * 由 [xyz.erik.opi.ime.ImeScreen] 提供；`staticCompositionLocalOf` 的取舍：
 * 换肤时整棵键盘子树本来就要重组（颜色全换），不需要细粒度失效。
 */
val LocalKeyboardPalette = staticCompositionLocalOf { LightPalette }
