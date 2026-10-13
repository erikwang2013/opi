// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import xyz.erik.opi.engine.EngineMode
import xyz.erik.opi.engine.InputScheme

/**
 * 「输入模式」= 键盘布局 × 引擎方案 × 语言模式的组合（2026-10-09，对齐搜狗移动端）。
 *
 * ⚠️ 三根轴分清楚，别混：
 *   · [InputScheme]（引擎）：按键 → 字母类（9 键 / 14 键 / 笔画 / 全键盘）；
 *   · [EngineMode]（引擎）：语言变体（拼音 / 繁体 / 英文 / …），**与方案正交**；
 *   · 本枚举（UI）：菜单里给用户看的那几项 —— 每项映射到上面两个引擎轴。
 *
 * [BIG9] 与 [PINYIN9] 是**同一引擎配置**（9 键只在大键面），区别只在布局的字号；
 * [RARE]（生僻字）**不进引擎** —— 它是「部首 + 笔画」浏览面板（RarePanel），
 * 提交走直提；选中它只切面板视图，不动引擎的 scheme/mode。
 */
enum class InputMode(
    val scheme: InputScheme,
    val engineMode: EngineMode,
    /** 大键面（大 9 键）：仅布局字号差异。 */
    val big: Boolean = false,
    /** 面板式（生僻字）：不进引擎，UI 层拦下。 */
    val panel: Boolean = false,
) {
    PINYIN26(InputScheme.FULL, EngineMode.PINYIN),
    ENGLISH26(InputScheme.FULL, EngineMode.ENGLISH),
    PINYIN9(InputScheme.NINE, EngineMode.PINYIN),
    PINYIN14(InputScheme.FOURTEEN, EngineMode.PINYIN),
    BIG9(InputScheme.NINE, EngineMode.PINYIN, big = true),
    STROKE(InputScheme.STROKE, EngineMode.PINYIN),
    RARE(InputScheme.FULL, EngineMode.PINYIN, panel = true),

    /**
     * 双拼（2026-10-13）：键面就是 26 键（`ImeScreen` 的 else 臂画 QwertyKeyboard），
     * 引擎方案不同 —— 解码与布局（自然码/微软/…，见设置页的「双拼布局」）在引擎侧。
     */
    SHUANGPIN(InputScheme.SHUANGPIN, EngineMode.PINYIN),
    ;

    /** 菜单里的名字（与搜狗同款措辞）。 */
    val label: String
        get() = when (this) {
            PINYIN26 -> "拼音26键"
            ENGLISH26 -> "英文26键"
            PINYIN9 -> "拼音9键"
            PINYIN14 -> "拼音14键"
            BIG9 -> "拼音大9键"
            STROKE -> "笔画键盘"
            RARE -> "生僻字键盘"
            SHUANGPIN -> "拼音双拼"
        }
}

/**
 * 从引擎现状反推当前 **布局**（用于「用户用别的途径（中/英 键、Ctrl 切模式）改了
 * 引擎之后，键盘画哪一套」）：面板式与 big 不进引擎、无法反推，调用方保留自己选过的
 * [InputMode]；本函数只回答「按 scheme+mode 应该画哪种键面」。
 *
 * ⚠️ 英文模式恒 26 键：9 键布局下切到英文（中/英 键）仍画 26 键 —— 字母盘打字在
 * 9 键上没有意义（数字键在英文模式是原文），这是与引擎一致的默认（engine 侧
 * English + Nine 时数字进缓冲没有候选路）。
 */
fun layoutFor(scheme: InputScheme, mode: EngineMode, previous: InputMode): InputMode = when {
    previous.panel && mode == EngineMode.PINYIN && scheme == previous.scheme -> previous
    mode == EngineMode.ENGLISH -> InputMode.ENGLISH26
    scheme == InputScheme.NINE -> if (previous.big) InputMode.BIG9 else InputMode.PINYIN9
    scheme == InputScheme.FOURTEEN -> InputMode.PINYIN14
    scheme == InputScheme.STROKE -> InputMode.STROKE
    // 双拼与全键盘**共用 26 键键面**，反推不出来是哪一个 ⇒ 只能报 [InputMode.SHUANGPIN]
    // 还是 26键 取决于引擎方案（这正是本函数存在的理由）；方案非双拼时 else 臂照旧。
    scheme == InputScheme.SHUANGPIN -> InputMode.SHUANGPIN
    else -> InputMode.PINYIN26
}
