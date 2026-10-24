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
 * 14 键布局表。
 *
 * ⚠️ **协议级约束（与 9 键同款）**：发送的键值（[FourteenKey.key]）必须等于
 * engine-core `scheme.rs` 里 `Scheme::Fourteen.key_class` 的**规范符号**（两字母键
 * 取字母对的**首个字母**：`qw→q`），而 label 的两个字母必须**恰好**是那一对
 * （或单字母键的单个字母）。`FourteenKeyLayoutTest` 与 `scheme.rs` 的类表逐格交叉
 * 核对；改这张表 = 改协议，四端一起改。
 *
 * 布局（2026-10-24 参考图改版）：`qw er ty ui op / as df gh jk l / zx cv bn m`，
 * 前两行带数字提示 1–5 / 6–0，第三行四枚字母键带上符号提示（见 [fourteenSymbols]）。
 */
internal data class FourteenKey(val label: String, val key: String)

internal val fourteenKeyRows: List<List<FourteenKey>> = listOf(
    listOf(
        FourteenKey("qw", "q"),
        FourteenKey("er", "e"),
        FourteenKey("ty", "t"),
        FourteenKey("ui", "u"),
        FourteenKey("op", "o"),
    ),
    listOf(
        FourteenKey("as", "a"),
        FourteenKey("df", "d"),
        FourteenKey("gh", "g"),
        FourteenKey("jk", "j"),
        FourteenKey("l", "l"),
    ),
    listOf(
        FourteenKey("zx", "z"),
        FourteenKey("cv", "c"),
        FourteenKey("bn", "b"),
        FourteenKey("m", "m"),
    ),
)

/** 前两行的数字提示：1–5 跟着第一行、6–0 跟着第二行（键面小字，不发送）。 */
internal val fourteenDigitBadges = listOf("1", "2", "3", "4", "5", "6", "7", "8", "9", "0")

/** 第三行四枚字母键上方的符号提示（发送键 = 字母；符号是长按/上滑候选之一）。 */
internal val fourteenSymbols = mapOf("z" to "~", "c" to "!", "b" to "@", "m" to "?")

/** 一枚 14 键候选的字面与发送值（纯数据：JVM 单测可核，不必碰 Compose）。 */
internal data class FourteenAlt(val label: String, val value: String)

/** 弹层默认选中 = 符号那枚（2026-10-24：键面上方就是符号；普通点击仍发协议字母）。 */
internal const val FOURTEEN_DEFAULT_ALT_INDEX = 1

/**
 * 字母键的长按/上滑候选（2026-10-24）：`[大写对, 符号, 小写对]`。
 * 普通点击仍发**协议字母**（14 键方案要的是 `q/e/t/...`，符号发不出去拼音）——
 * 大小写两枚的 `value` 都是同一个协议字母（引擎把大写折成小写），符号那枚直发符号。
 */
internal fun fourteenAlternatives(k: FourteenKey): List<FourteenAlt> {
    val symbol = fourteenSymbols[k.key] ?: return emptyList()
    return listOf(
        FourteenAlt(k.label.uppercase(), k.key.uppercase()),
        FourteenAlt(symbol, symbol),
        FourteenAlt(k.label.lowercase(), k.key),
    )
}

/** 14 键底行的动作（纯数据；UI 按 [FourteenBottomKey.action] 接既有回调）。 */
internal enum class FourteenBottomAction { Symbol, Number, Comma, Space, Period, Mode, Enter }

internal data class FourteenBottomKey(
    val label: String,
    val action: FourteenBottomAction,
    val weight: Float,
)

/** 底行 2026-10-24 版式：符 | 123 | , | 空格（宽）| 。| 中英 | 换行。 */
internal val fourteenBottomKeys = listOf(
    FourteenBottomKey("符", FourteenBottomAction.Symbol, 1.3f),
    FourteenBottomKey("123", FourteenBottomAction.Number, 1.3f),
    FourteenBottomKey(",", FourteenBottomAction.Comma, 1.0f),
    FourteenBottomKey("空格", FourteenBottomAction.Space, 2.7f),
    FourteenBottomKey("。", FourteenBottomAction.Period, 1.0f),
    FourteenBottomKey("英", FourteenBottomAction.Mode, 1.3f),
    FourteenBottomKey("换行", FourteenBottomAction.Enter, 1.3f),
)

/**
 * 一枚字母键（字母对 + 上方小字提示）。长按/上滑候选由 [fourteenAlternatives] 给：
 * 只有第三行四枚键有（前两行返回空表，普通点击即协议字母）。
 */
@Composable
private fun RowScope.FourteenPairKey(
    k: FourteenKey,
    hint: String,
    weight: Float,
    pairSize: TextUnit,
    shiftActive: Boolean,
    onKey: (String) -> Unit,
) {
    val alts = fourteenAlternatives(k).mapIndexed { i, a ->
        KeyAlternative(
            label = a.label,
            value = a.value,
            fontSize = (pairSize.value - i * 3f).coerceAtLeast(12f).sp,
        )
    }
    KeyButton(
        label = k.label.uppercase(),
        modifier = Modifier.weight(weight),
        // ⇧ 激活时发大写（引擎把拼音大写折成小写；协议值不变）。
        onTap = { onKey(if (shiftActive) k.key.uppercase() else k.key) },
        description = "字母 ${k.label.toCharArray().joinToString(" ")}",
        fontSize = pairSize,
        alternatives = alts,
        defaultAlternative = if (alts.isEmpty()) 0 else FOURTEEN_DEFAULT_ALT_INDEX,
        onAlternative = onKey,
        hint = hint.ifEmpty { null },
    )
}

/**
 * 拼音 14 键（2026-10-24 版式）：两行 5 键 + 一行 ⇧/四字母/⌫ + 底行 7 键。
 *
 * [onModeSwitch] = 底行「中英」**点击**（中文 ⇄ 英文，宿主传
 * [xyz.erik.opi.ime.ImeState.toggleChineseEnglish]）；[onModeLongPress] = 长按开
 * 输入模式面板。⇧ 与 26 键同一条状态机（点击 = 锁定、长按 = 单次）。
 */
@Composable
fun FourteenKeyKeyboard(
    modifier: Modifier = Modifier,
    onKey: (String) -> Unit,
    onSpace: () -> Unit,
    onBackspace: () -> Unit,
    onEnter: () -> Unit,
    onModeSwitch: () -> Unit,
    onModeLongPress: () -> Unit = {},
    onSymbol: () -> Unit,
    onNumber: () -> Unit,
    onShift: (() -> Unit)? = null,
    onShiftLongPress: (() -> Unit)? = null,
    shiftState: ShiftState? = null,
) {
    val shiftActive = shiftState != null && shiftState != ShiftState.OFF
    BoxWithConstraints(modifier = modifier.fillMaxSize()) {
        val compact = compactKeys(maxWidth.value, maxHeight.value)
        val keyFont = if (compact) 16.sp else 21.sp
        val pairSize = if (compact) 18.sp else 24.sp
        val bottomFont = if (compact) 15.sp else 18.sp

        Column(modifier = Modifier.fillMaxSize()) {
            // 第一行：数字提示 1–5
            Row(modifier = Modifier.weight(1f).fillMaxSize()) {
                fourteenKeyRows[0].forEachIndexed { i, k ->
                    FourteenPairKey(k, fourteenDigitBadges[i], 1f, pairSize, shiftActive, onKey)
                }
            }
            // 第二行：数字提示 6–0
            Row(modifier = Modifier.weight(1f).fillMaxSize()) {
                fourteenKeyRows[1].forEachIndexed { i, k ->
                    FourteenPairKey(k, fourteenDigitBadges[5 + i], 1f, pairSize, shiftActive, onKey)
                }
            }
            // 第三行：⇧ | ZX(~) CV(!) BN(@) M(?) | ⌫
            Row(modifier = Modifier.weight(1f).fillMaxSize()) {
                KeyButton(
                    "⇧",
                    modifier = Modifier.weight(1.2f),
                    onTap = onShift,
                    onLongPress = onShiftLongPress,
                    highlighted = shiftActive,
                    stateDescription = when (shiftState) {
                        ShiftState.SINGLE -> "仅下一个字母大写"
                        ShiftState.LOCK -> "大写锁定"
                        else -> null
                    },
                    fontSize = keyFont,
                )
                for (k in fourteenKeyRows[2]) {
                    val symbol = fourteenSymbols[k.key] ?: ""
                    FourteenPairKey(
                        k, symbol, if (k.key == "m") 0.95f else 1.55f,
                        pairSize, shiftActive, onKey,
                    )
                }
                KeyButton(
                    "⌫",
                    modifier = Modifier.weight(1.2f),
                    onTap = onBackspace,
                    // 按住连续删除；短按仍只删一个（2026-10-23 用户裁决）。
                    repeatOnLongPress = true,
                    fontSize = keyFont,
                )
            }
            // 底行：符 | 123 | , | 空格（宽）| 。| 中英 | 换行
            Row(modifier = Modifier.weight(1f).fillMaxSize()) {
                for (b in fourteenBottomKeys) {
                    val m = Modifier.weight(b.weight)
                    when (b.action) {
                        FourteenBottomAction.Symbol ->
                            KeyButton(b.label, m, onTap = onSymbol, fontSize = bottomFont)
                        FourteenBottomAction.Number ->
                            KeyButton(b.label, m, onTap = onNumber, fontSize = bottomFont)
                        FourteenBottomAction.Comma ->
                            KeyButton(b.label, m, onTap = { onKey(",") }, fontSize = keyFont)
                        FourteenBottomAction.Space ->
                            KeyButton(b.label, m, onTap = onSpace, fontSize = bottomFont)
                        FourteenBottomAction.Period ->
                            KeyButton(b.label, m, onTap = { onKey(".") }, fontSize = keyFont)
                        FourteenBottomAction.Mode -> KeyButton(
                            b.label,
                            m,
                            onTap = onModeSwitch,
                            onLongPress = onModeLongPress,
                            description = "中英切换",
                            fontSize = bottomFont,
                        )
                        FourteenBottomAction.Enter ->
                            KeyButton(b.label, m, onTap = onEnter, fontSize = bottomFont)
                    }
                }
            }
        }
    }
}
