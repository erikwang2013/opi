// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.sp
import xyz.erik.opi.engine.ShiftState

/**
 * QWERTY 键盘。三种形态（2026-10-24 用户参考图 aa.jpg / cc.jpg）：
 * - 英文 26 键（[showProKey]）：顶部数字行；字母小写 + 上方符号提示；
 *   底行 `!?# 123 , 空格 . 中英 换行`。
 * - 拼音 26 键（[pinyin26]）：无数字行，Q..P 右上角标 1-0；字母大写；
 *   A/Z 行上方符号提示；底行 `符 123 , 空格 。 中英 换行`。
 * - 五笔 26 键（[wubi]，2026-10-24 参考图 ads）：键面同拼音 26 键，只差第三行最左
 *   「拼」（切拼音 26）与底行空格键面「五笔」。
 * - 其它（搜索叠盘等紧凑调用方）：三行字母 + 旧底行 `123 中英 空格 ⌫ ↵`。
 * 字母键长按/上滑出备选弹层：[大写, 符号(拼音 Q 行是数字), 小写]，默认选中符号。
 */
@Composable
fun QwertyKeyboard(
    modifier: Modifier = Modifier,
    onKey: (String) -> Unit,
    onSpace: () -> Unit,
    onBackspace: () -> Unit,
    onEnter: () -> Unit,
    onModeSwitch: () -> Unit,
    onShift: (() -> Unit)? = null,
    onShiftLongPress: (() -> Unit)? = null,
    onNumber: (() -> Unit)? = null,
    onSymbol: (() -> Unit)? = null,
    shiftState: ShiftState? = null,
    modeLabel: String = "中",
    /** 字母行上方加一行 0-9（英文 26 键；搜索叠盘等其它场景保持关闭）。 */
    showNumberRow: Boolean = false,
    /** 英文 26 键布局：底部 `!?#`+`123`、⌫ 放到第 3 行最右、中英键在句号后。 */
    showProKey: Boolean = false,
    /** 2026-10-24 参考图版英文底行不再有 pro 键；参数保留以兼容现有宿主调用，
     *  pro 入口待宿主侧重新裁决（当前键面上不可达）。 */
    proActive: Boolean = false,
    onProToggle: (() -> Unit)? = null,
    /** 拼音 26 键主盘（无数字行、Q 行数字角标、底行 `符`）。默认按 [onSymbol] 判定：
     *  只有 26 键主盘提供符号入口，搜索叠盘等紧凑调用方保持旧底行。 */
    pinyin26: Boolean = onSymbol != null,
    /** 五笔 26 键（2026-10-24 参考图 ads）：第三行最左「拼」、底行空格键面「五笔」。
     *  其余键面与拼音 26 键相同（弹层/角标/符号提示都不另做）。 */
    wubi: Boolean = false,
    /** 五笔态「拼」键：切回拼音 26 键（宿主传 [xyz.erik.opi.ime.ImeState.selectInputMode]）。 */
    onSwitchToPinyin: (() -> Unit)? = null,
    /** 模式键长按（打开输入方案菜单）；null = 不长按。 */
    onModeLongPress: (() -> Unit)? = null,
) {
    val shiftActive = shiftState != null && shiftState != ShiftState.OFF
    val englishLayout = showProKey
    val pinyinLayout = !englishLayout && pinyin26
    BoxWithConstraints(modifier = modifier.fillMaxSize()) {
        // 横屏矮键区 / 单手窄键区降一档字号，见 [compactKeys]。
        // 英文字号更大（2026-10-19 用户反馈数字/字母键偏小）；数字行压矮、字母行加高，
        // 把多出的一行高度还给字母，而不是让 5 行均分。
        val keyFont = when {
            compactKeys(maxWidth.value, maxHeight.value) -> 16.sp
            showProKey -> 23.sp
            else -> 21.sp
        }
        val rowsToDraw = qwertyRows(showNumberRow, englishLayout)
        Column(modifier = Modifier.fillMaxSize()) {
            rowsToDraw.forEachIndexed { index, row ->
                val rowWeight = when {
                    showProKey && showNumberRow && index == 0 -> 0.7f
                    showProKey -> 1.05f
                    else -> 1f
                }
                Row(modifier = Modifier.weight(rowWeight).fillMaxSize()) {
                    for (ch in row) {
                        when (ch) {
                            SHIFT_LABEL -> KeyButton(
                                qwertyLeadLabel(wubi),
                                modifier = Modifier.weight(1f),
                                // 五笔态这格是「拼」（切拼音 26），不是 ⇧。
                                onTap = if (wubi) onSwitchToPinyin else onShift,
                                onLongPress = if (wubi) null else onShiftLongPress,
                                highlighted = !wubi && shiftActive,
                                fontSize = keyFont,
                                description = if (wubi) "切换到拼音键盘" else null,
                                // SINGLE/LOCK 原本只靠蓝灰底区分：读屏读不到，色觉障碍也分不清
                                stateDescription = if (wubi) null else when (shiftState) {
                                    ShiftState.SINGLE -> "仅下一个字母大写"
                                    ShiftState.LOCK -> "大写锁定"
                                    else -> null
                                },
                            )
                            ENTER_LABEL -> KeyButton(
                                ENTER_LABEL,
                                modifier = Modifier.weight(1f),
                                onTap = onEnter,
                                fontSize = keyFont,
                            )
                            BACKSPACE_LABEL -> KeyButton(
                                BACKSPACE_LABEL,
                                modifier = Modifier.weight(1f),
                                onTap = onBackspace,
                                repeatOnLongPress = true,
                                fontSize = keyFont,
                            )
                            else -> {
                                // 26 键字母键（2026-10-24）：键面下方是符号提示 / 拼音数字角标，
                                // 长按或上滑弹 [大写, 符号, 小写]，抬手提交选中项（默认符号）。
                                // 拼音键面固定大写（参考图 cc）；英文仍随 ⇧ 显示大小写 ——
                                // **只改显示**：onKey 送出去的仍是原字符，大小写由 shift 语义
                                // /引擎决定（KeyRouter.handleKey 的英文直传）。
                                KeyButton(
                                    if (pinyinLayout || shiftActive) ch.uppercase() else ch,
                                    modifier = Modifier.weight(1f),
                                    onTap = { onKey(ch) },
                                    fontSize = keyFont,
                                    hint = when {
                                        englishLayout -> qwertySymbolHints[ch]
                                        pinyinLayout -> pinyinSymbolHints[ch]
                                        else -> null
                                    },
                                    cornerHint = if (pinyinLayout) pinyinDigitHints[ch] else null,
                                    alternatives = if (englishLayout || pinyinLayout) {
                                        qwertyAlternatives(ch, pinyinLayout)
                                    } else {
                                        emptyList()
                                    },
                                    defaultAlternative = QWERTY_ALTERNATIVE_DEFAULT,
                                    onAlternative = onKey,
                                )
                            }
                        }
                    }
                }
            }
            Row(modifier = Modifier.weight(if (showProKey) 0.95f else 1f).fillMaxSize()) {
                if (englishLayout || pinyinLayout) {
                    Qwerty26Bottom(
                        keys = qwertyBottomKeys(englishLayout),
                        modeLabel = modeLabel,
                        onKey = onKey,
                        onSpace = onSpace,
                        onEnter = onEnter,
                        onNumber = onNumber,
                        onSymbol = onSymbol,
                        onModeSwitch = onModeSwitch,
                        onModeLongPress = onModeLongPress,
                        spaceLabel = qwertySpaceLabel(wubi),
                        keyFont = keyFont,
                    )
                } else {
                    // 中文/其它键盘保持既有顺序：123 / 中英 / 空格 / ⌫ / ↵。
                    // 123 不挂长按：长按 500ms 吞 tap，导致面板打不开。
                    KeyButton(
                        "123",
                        modifier = Modifier.weight(1f),
                        onTap = onNumber,
                        fontSize = keyFont,
                    )
                    ModeKey(modeLabel, onModeSwitch, keyFont)
                    KeyButton(
                        "空格",
                        modifier = Modifier.weight(3f),
                        onTap = onSpace,
                        fontSize = keyFont,
                    )
                    KeyButton(
                        "⌫",
                        modifier = Modifier.weight(1f),
                        onTap = onBackspace,
                        // 按住连续删除；短按仍只删一个（2026-10-23 用户裁决）。
                        repeatOnLongPress = true,
                        fontSize = keyFont,
                    )
                    KeyButton(
                        "↵",
                        modifier = Modifier.weight(1f),
                        onTap = onEnter,
                        fontSize = keyFont,
                    )
                }
            }
        }
    }
}

/** 26 键底行一格；[weight] 是横向权重（空格最宽）。 */
internal data class QwertyBottomKey(val label: String, val weight: Float)

/**
 * 26 键底行（2026-10-24 参考图 aa/cc）：英文 `!?# 123 , 空格 . 中英 换行`，
 * 拼音 `符 123 , 空格 。 中英 换行`。模式键键面固定「中英」（参考图），
 * 当前/目标模式仍走读屏状态描述（[modeLabel]）。
 */
internal fun qwertyBottomKeys(english: Boolean): List<QwertyBottomKey> =
    if (english) {
        listOf(
            QwertyBottomKey("!?#", 1f),
            QwertyBottomKey("123", 1f),
            QwertyBottomKey(",", 1f),
            QwertyBottomKey("空格", 2.5f),
            QwertyBottomKey(".", 1f),
            QwertyBottomKey("中", 1f),
            QwertyBottomKey("换行", 1f),
        )
    } else {
        listOf(
            QwertyBottomKey("符", 1f),
            QwertyBottomKey("123", 1f),
            QwertyBottomKey(",", 1f),
            QwertyBottomKey("空格", 2.5f),
            QwertyBottomKey("。", 1f),
            QwertyBottomKey("英", 1f),
            QwertyBottomKey("换行", 1f),
        )
    }

@Composable
private fun RowScope.Qwerty26Bottom(
    keys: List<QwertyBottomKey>,
    modeLabel: String,
    onKey: (String) -> Unit,
    onSpace: () -> Unit,
    onEnter: () -> Unit,
    onNumber: (() -> Unit)?,
    onSymbol: (() -> Unit)?,
    onModeSwitch: () -> Unit,
    onModeLongPress: (() -> Unit)?,
    /** 空格键键面字（五笔态 =「五笔」，动作仍 [onSpace]）。 */
    spaceLabel: String,
    keyFont: TextUnit,
) {
    for (key in keys) {
        when {
            key.label == "!?#" || key.label == "符" -> KeyButton(
                key.label,
                modifier = Modifier.weight(key.weight),
                onTap = onSymbol,
                fontSize = keyFont,
            )
            key.label == "123" -> KeyButton(
                key.label,
                modifier = Modifier.weight(key.weight),
                onTap = onNumber,
                fontSize = keyFont,
            )
            key.label == "空格" -> KeyButton(
                spaceLabel,
                modifier = Modifier.weight(key.weight),
                onTap = onSpace,
                // 五笔键面写方案名，动作仍是打空格：读屏按动作念。
                description = if (spaceLabel == "空格") null else "空格",
                fontSize = keyFont,
            )
            // 2026-10-25：26 键底行模式键按**目标语言**写「中」/「英」，点一次即切
            // （不再走 中→繁→英 三态循环；繁体仍可从长按输入模式面板选）。
            key.label == "中" || key.label == "英" -> ModeKey(
                key.label,
                onModeSwitch,
                keyFont,
                onModeLongPress,
                label = key.label,
            )
            key.label == "换行" -> KeyButton(
                key.label,
                modifier = Modifier.weight(key.weight),
                onTap = onEnter,
                fontSize = keyFont,
            )
            else -> KeyButton(
                key.label,
                modifier = Modifier.weight(key.weight),
                onTap = { onKey(key.label) },
                fontSize = keyFont,
            )
        }
    }
}

@Composable
private fun RowScope.ModeKey(
    modeLabel: String,
    onModeSwitch: () -> Unit,
    keyFont: TextUnit = 21.sp,
    onLongPress: (() -> Unit)? = null,
    /** 键面字；默认按模式动态显示，26 键底行固定「中英」（参考图）。 */
    label: String = modeLabel,
) {
    KeyButton(
        label,
        modifier = Modifier.weight(1f),
        onTap = onModeSwitch,
        onLongPress = onLongPress,
        // 键面字是**点它会切到哪**（2026-10-12 晚裁决，见 ime/ImeScreen.kt 的
        // modeKeyLabelOf）—— 读屏按同一语义念成「切到X」，别念成当前模式。
        stateDescription = when (modeLabel) {
            "中" -> "切到中文"
            "繁" -> "切到繁体"
            "英" -> "切到英文"
            else -> null
        },
        fontSize = keyFont,
    )
}

private val rows = listOf(
    listOf("q", "w", "e", "r", "t", "y", "u", "i", "o", "p"),
    listOf("a", "s", "d", "f", "g", "h", "j", "k", "l"),
    // 退格与大写锁定同一行，退格最右（2026-10-23 用户裁决：与回车互换）。
    listOf("⇧", "z", "x", "c", "v", "b", "n", "m", "⌫"),
)
private val numberRow = listOf("1", "2", "3", "4", "5", "6", "7", "8", "9", "0")

/** 实际渲染的键位行（数字行在字母行上方）。JVM 单测直接核这张表。 */
internal fun qwertyRows(showNumberRow: Boolean, englishLayout: Boolean = false): List<List<String>> {
    // 拼音/英文 26 键的行表同构；englishLayout 仅为既有调用方与测试保留。
    val letters = rows
    return if (showNumberRow) listOf(numberRow) + letters else letters
}

/** 英文 26 键字母键上方的符号提示（aa.jpg）。 */
internal val qwertySymbolHints: Map<String, String> = mapOf(
    "q" to "`", "w" to "=", "e" to "+", "r" to "$", "t" to "…", "y" to "\"", "u" to "^",
    "i" to "[", "o" to "]", "p" to "|",
    "a" to "~", "s" to "!", "d" to "@", "f" to "#", "g" to "%", "h" to "'", "j" to "&",
    "k" to "*", "l" to "?",
    "z" to "(", "x" to ")", "c" to "-", "v" to "-", "b" to ":", "n" to ";", "m" to "/",
)

/** 拼音 26 键的符号提示只在 A/Z 行（cc.jpg）；H/J 是中文引号。 */
internal val pinyinSymbolHints: Map<String, String> =
    qwertySymbolHints.filterKeys { it !in "qwertyuiop" } + mapOf("h" to "“", "j" to "”")

/** 拼音 26 键 Q..P 右上角的数字 1-0。 */
internal val pinyinDigitHints: Map<String, String> =
    "1234567890".mapIndexed { index, digit -> "qwertyuiop"[index].toString() to digit.toString() }.toMap()

/** 字母键的备选表：[大写, 符号, 小写]；拼音 Q 行的中间项是角标数字。 */
internal fun qwertyAlternatives(ch: String, pinyin: Boolean): List<KeyAlternative> {
    val middle = if (pinyin) pinyinSymbolHints[ch] ?: pinyinDigitHints[ch] else qwertySymbolHints[ch]
    if (middle == null) return emptyList()
    return listOf(
        KeyAlternative(ch.uppercase(), ch.uppercase()),
        KeyAlternative(middle, middle),
        KeyAlternative(ch, ch),
    )
}

/** 26 键弹层默认选中「符号」这一行。 */
internal const val QWERTY_ALTERNATIVE_DEFAULT = 1

/** 五笔 26 键的键面字（ads.jpg）：第三行最左「拼」、底行空格键「五笔」；其余态不变。 */
internal fun qwertyLeadLabel(wubi: Boolean): String = if (wubi) "拼" else SHIFT_LABEL
internal fun qwertySpaceLabel(wubi: Boolean): String = if (wubi) "五笔" else "空格"

private const val SHIFT_LABEL = "⇧"
private const val ENTER_LABEL = "↵"
private const val BACKSPACE_LABEL = "⌫"
