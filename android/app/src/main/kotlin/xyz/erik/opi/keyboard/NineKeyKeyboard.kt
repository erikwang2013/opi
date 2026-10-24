// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * 9 键布局表（拼音 9 键 / 大 9 键共用；大 9 只差字号与「不画数字徽标」）。
 *
 * ⚠️ **发送的键值（[NineKey.key]）= 协议**：
 * · `'2'..'9'` 必须落在 engine-core `scheme.rs` 的 Nine 类表里（各映射三四个字母），
 *   hint 是键面下的字母提示，且**必须正是该数字在 `scheme.rs` 里带的类** ——
 *   `NineKeyLayoutTest` 按类表交叉核对，两边任何一处改了都会红；
 * · `'` 是**音节边界分隔符**（`scheme.rs::matches_segmented`），不在类表里。
 *
 * 标点键（发送 `,` 等）沿用引擎的标点表：中文模式下由标点层出「，」，与 26 键盘的
 * `,` 是**同一条路**（不是 UI 直提）—— 主区域第一格 2026-10-15 换成了分词键，
 * 标点只剩左列（[nineKeyStripPuncts]）。
 */
internal data class NineKey(val label: String, val key: String, val hint: String = "")

/**
 * **左侧一列**的默认内容 = 标点列（主流同款：空闲时一列标点，输入时整列换成拼音）。
 * 2026-10-24 起整列**可竖向滚动**（条目多于可视三行），末尾追加「自定义」格。
 * 发送键都取引擎中文标点表（punctuation.rs 的 CHINESE_PUNCT）**已有的键**：
 * `,`→，`.`→。`\`→、`?`→？`!`→！…… —— 上层不用另造标点通路。
 */
internal val nineKeyStripPuncts: List<NineKey> = listOf(
    NineKey("，", ","),
    NineKey("。", "."),
    NineKey("、", "\\"),
    NineKey("？", "?"),
    NineKey("！", "!"),
    NineKey("：", ":"),
    NineKey("；", ";"),
    NineKey("（", "("),
    NineKey("）", ")"),
    NineKey("“", "\""),
    NineKey("‘", "'"),
    NineKey("【", "["),
    NineKey("】", "]"),
    NineKey("《", "<"),
    NineKey("》", ">"),
    NineKey("～", "~"),
)

/** 分词键的发送键（音节边界分隔符 = 协议值，与 `scheme.rs` 的分隔符同一个字）：
 * 布局表与点击分流共用，别各写一份字面量。 */
internal const val SEGMENT_KEY = "'"

/** 主区域格子的数字徽标（2026-10-24）：**按格序** 1..9，[index] 从 0 起。
 * 数字键发送的仍是 T9 的 2..9（徽标只表示位置，不改协议值）。 */
internal fun nineKeyBadge(index: Int): String = (index + 1).toString()

internal val nineKeyRows: List<List<NineKey>> = listOf(
    // 主区域第一格 = **分词键**（2026-10-15 用户裁决「第一符号按钮改成分词按钮」，
    // 换回 2026-10-09 的形态；此前这格是面「，。?!」、发半角逗号）：发送 `'` 往引擎
    // 缓冲插音节边界（`xi|an` 这类强制分词，见 `scheme.rs::matches_segmented`）。
    // ⚠️ **缓冲空时由宿主拦住**（ImeScreen 的 onSegment 守卫）—— 空缓冲的 `'` 在
    // 引擎里是引号，不是分词符（`engine_punct.rs`）。
    listOf(NineKey("分词", SEGMENT_KEY), NineKey("2", "2", "abc"), NineKey("3", "3", "def")),
    listOf(NineKey("4", "4", "ghi"), NineKey("5", "5", "jkl"), NineKey("6", "6", "mno")),
    listOf(NineKey("7", "7", "pqrs"), NineKey("8", "8", "tuv"), NineKey("9", "9", "wxyz")),
)

/**
 * 拼音 9 键（T9）。[big] = 大 9 键（同布局放大键面；2026-10-24 起大 9 **不画数字徽标**，
 * 对齐主流「大9键」的纯字母键面）。
 * 数字键在此方案下**是输入键**（引擎 Nine 方案收进缓冲），不需要任何数字选词 UI。
 */
@Composable
fun NineKeyKeyboard(
    modifier: Modifier = Modifier,
    onKey: (String) -> Unit,
    onSpace: () -> Unit,
    onBackspace: () -> Unit,
    onEnter: () -> Unit,
    /** 底行模式键：9 键上**只切简/繁**（2026-10-15 用户裁决），宿主传
     * [xyz.erik.opi.ime.ImeState.toggleTraditional]（不是 26 键的 cycleMode）。 */
    onModeSwitch: () -> Unit,
    onSymbol: () -> Unit,
    /** 分词键（主区域第一格，主流同款）：往引擎缓冲插音节边界（`xi|an`）。
     * **由宿主在缓冲空时拦住** —— 空缓冲点它没有边界可切，且空缓冲的 `'` 在引擎里
     * 是引号（不是分词符）。 */
    onSegment: () -> Unit,
    /** 重输键：清掉当前组合重来。缓冲空时是空操作（clear 本身幂等）。 */
    onRetype: () -> Unit,
    /** 数字键盘切换（底行 [123]，与 26 键的 123 同路）。 */
    onNumber: () -> Unit,
    /** 英文键盘切换（底行 [英文]）：切到英文 26 键输入模式（与输入模式面板同一条路）。 */
    onEnglish: () -> Unit,
    /** 候选拼音读数（9 键左列逐行显示，如 `ni'hao`；**按读音去重保序**）。
     * 空列表 = 没在输入/无候选，左列显示标点列）。 */
    readings: List<String> = emptyList(),
    /** 点第 i 行拼音 = 钉该读音（**不确认**，候选收窄到**读音恰等**的条目）。 */
    onPickReading: (Int) -> Unit = {},
    /** 当前钉住的读音（空串 = 未钉；命中的行高亮强调色）。 */
    pinnedReading: String = "",
    /** 读数列的滚动复位键（传缓冲：缓冲一变整列回顶部，别停在旧位置）。 */
    scrollKey: String = "",
    modeLabel: String = "中",
    big: Boolean = false,
    /** 左列末尾「自定义」标点的当前值（空 = 未选，按钮面「自定义」）。 */
    customPunct: String = "",
    /** 选了一个自定义标点：宿主落盘；下次点该格即发送它。 */
    onPickCustomPunct: (String) -> Unit = {},
    /** 自定义符号选择网格的数据源（引擎「常用」符号；只在选择器打开时取）。 */
    commonSymbols: () -> List<String> = { emptyList() },
    /** 自定义标点发送通道：任意符号不是键盘键码，走直提（符号面板同一条）。 */
    onCommitText: (String) -> Unit = onKey,
) {
    BoxWithConstraints(modifier = modifier.fillMaxSize()) {
        // 横屏矮键区 / 单手窄键区降一档，见 [compactKeys]。
        val compact = compactKeys(maxWidth.value, maxHeight.value)
        val letterSize = when {
            big && compact -> 24.sp
            big -> 30.sp
            compact -> 18.sp
            else -> 21.sp
        }
        val digitSize = when {
            big && compact -> 12.sp
            big -> 15.sp
            compact -> 10.sp
            else -> 12.sp
        }
        val keyFont = if (compact) 17.sp else 21.sp
        // 左列标点字号**独立一档**（2026-10-25 用户：标点键太大）—— 不再跟主区字母同字号。
        val stripFont = if (compact) 15.sp else 17.sp
        var pickerOpen by remember { mutableStateOf(false) }
        Column(modifier = Modifier.fillMaxSize()) {
            Row(modifier = Modifier.weight(3f).fillMaxSize()) {
                // ---- 左侧一列（2026-10-24：整列可滚动 + 末尾「自定义」）----
                // 空闲 = 标点列（点它走引擎标点表）；输入时**整列换成候选拼音读数**
                // （一个读音一行、已去重）——「拼音是输入才替换到左侧一列」。
                // 左列比主区窄一档（2026-10-25 用户：标点键太大）。
                Box(modifier = Modifier.weight(0.8f).fillMaxHeight()) {
                    if (readings.isNotEmpty()) {
                        // 读数态：格数与标点列同节奏（可视三行 ⇒ cells = 3）。
                        CandidateReadingColumn(
                            readings = readings,
                            pinnedReading = pinnedReading,
                            onPick = onPickReading,
                            scrollKey = scrollKey,
                            cells = 3,
                            fontSize = 15.sp,
                            modifier = Modifier.fillMaxSize(),
                        )
                    } else {
                        BoxWithConstraints(modifier = Modifier.fillMaxSize()) {
                            val cell = maxHeight / 3
                            LazyColumn(modifier = Modifier.fillMaxSize()) {
                                items(nineKeyStripPuncts.size) { i ->
                                    val k = nineKeyStripPuncts[i]
                                    KeyButton(
                                        k.label,
                                        modifier = Modifier.fillMaxWidth().height(cell),
                                        onTap = { onKey(k.key) },
                                        description = nineKeyStripSpoken(k),
                                        fontSize = stripFont,
                                    )
                                }
                                // 自定义格：未选 = 点它开选择器；已选 = 点它发送、长按换。
                                item {
                                    val face = customPunct.ifEmpty { "自定义" }
                                    KeyButton(
                                        face,
                                        modifier = Modifier.fillMaxWidth().height(cell),
                                        onTap = {
                                            if (customPunct.isEmpty()) pickerOpen = true
                                            else onCommitText(customPunct)
                                        },
                                        onLongPress = { pickerOpen = true },
                                        description = if (customPunct.isEmpty()) {
                                            "自定义符号，点按选择"
                                        } else {
                                            "自定义符号 $customPunct，点按输入，长按更换"
                                        },
                                        fontSize = if (customPunct.isEmpty()) stripFont else letterSize,
                                    )
                                }
                            }
                        }
                    }
                }
                // ---- 中间 3×3 数字格（2026-10-24：右上角数字徽标）----
                Column(modifier = Modifier.weight(3f)) {
                    nineKeyRows.forEachIndexed { r, row ->
                        Row(modifier = Modifier.weight(1f).fillMaxSize()) {
                            row.forEachIndexed { c, k ->
                                // 长按/上滑候选：字母（字号递减）+ **徽标序号**收尾；
                                // 弹层默认选中末项（普通点击仍发协议键，见 onTap）。
                                val alts = nineKeyAlternatives(k, r * 3 + c, letterSize, digitSize)
                                KeyButton(
                                    // 字母为主（主流同款）；大 9 只画字母、不画徽标。
                                    label = k.hint.ifEmpty { k.label },
                                    modifier = Modifier.weight(1f),
                                    onTap = {
                                        if (k.key == SEGMENT_KEY) onSegment() else onKey(k.key)
                                    },
                                    description = nineKeySpoken(k),
                                    fontSize = letterSize,
                                    alternatives = alts,
                                    defaultAlternative = alts.lastIndex,
                                    onAlternative = onKey,
                                    cornerHint = if (big) null else nineKeyBadge(r * 3 + c),
                                )
                            }
                        }
                    }
                }
                // ---- 右侧一列：⌫ / 重输 / 空格（与左列同宽）----
                Column(modifier = Modifier.weight(1f).fillMaxHeight()) {
                    KeyButton(
                        "⌫",
                        modifier = Modifier.weight(1f).fillMaxWidth(),
                        onTap = onBackspace,
                        // 按住连续删除；短按仍只删一个（2026-10-23 用户裁决）。
                        repeatOnLongPress = true,
                        fontSize = keyFont,
                    )
                    KeyButton(
                        "重输",
                        modifier = Modifier.weight(1f).fillMaxWidth(),
                        onTap = onRetype,
                        fontSize = keyFont,
                    )
                    KeyButton(
                        // 2026-10-25 用户裁决：右列空格换成 0 键（数字 0 直提），
                        // 空格移到底行中间并加宽。
                        "0",
                        modifier = Modifier.weight(1f).fillMaxWidth(),
                        onTap = { onKey("0") },
                        description = "数字 0",
                        fontSize = keyFont,
                    )
                }
            }
            // 底行：[符][中/繁][123][空格(宽)][英][↵]。
            Row(modifier = Modifier.weight(1f).fillMaxSize()) {
                KeyButton("符", modifier = Modifier.weight(0.9f), onTap = onSymbol, fontSize = keyFont)
                KeyButton(
                    modeLabel,
                    modifier = Modifier.weight(0.9f),
                    onTap = onModeSwitch,
                    description = "简繁切换",
                    stateDescription = modeStateDescription(modeLabel),
                    fontSize = keyFont,
                )
                KeyButton("123", modifier = Modifier.weight(0.9f), onTap = onNumber, fontSize = keyFont)
                KeyButton("空格", modifier = Modifier.weight(2.4f), onTap = onSpace, fontSize = keyFont)
                KeyButton("英", modifier = Modifier.weight(0.9f), onTap = onEnglish, fontSize = keyFont)
                KeyButton("↵", modifier = Modifier.weight(0.9f), onTap = onEnter, fontSize = keyFont)
            }
        }
        if (pickerOpen) {
            CustomSymbolPicker(
                symbols = commonSymbols(),
                onPick = { symbol ->
                    onPickCustomPunct(symbol)
                    pickerOpen = false
                },
                onClose = { pickerOpen = false },
                modifier = Modifier.fillMaxSize(),
            )
        }
    }
}

/**
 * 9 键弹层的发送值（2026-10-24 澄清）：字母逐个在前，**右上角徽标序号**收尾。
 * 九个格子都按格序 1..9（分词格 = 1）；选中末项 = 输入该序号数字。
 * 普通点击不走这里，仍发协议值（分词 `'` / T9 `2..9`）。
 * 分词格的「1」在 Nine 里没有键类，宿主把它当空缓冲分词键直提（ImeScreen 的 onKey）。
 */
internal fun nineKeyAlternativeValues(k: NineKey, index: Int): List<String> =
    k.hint.map(Char::toString) + nineKeyBadge(index)

/** 弹层键面：字母**字号递减**，序号数字用 [digitSize]（末项 = 默认高亮）。 */
private fun nineKeyAlternatives(
    k: NineKey,
    index: Int,
    firstSize: TextUnit,
    digitSize: TextUnit,
): List<KeyAlternative> {
    val values = nineKeyAlternativeValues(k, index)
    return values.mapIndexed { i, v ->
        KeyAlternative(
            label = v,
            value = v,
            fontSize = if (i == values.lastIndex) {
                digitSize
            } else {
                (firstSize.value - i * 3f).coerceAtLeast(12f).sp
            },
        )
    }
}

/** 9 键键位的读屏名：「数字 2，字母 a b c」—— 只念数字会丢掉「这键打什么」。 */
internal fun nineKeySpoken(k: NineKey): String = when {
    k.hint.isNotEmpty() -> "数字 ${k.label}，字母 ${k.hint.toCharArray().joinToString(" ")}"
    // 按**发送键**判，不按键面（多标点那代定的规矩，键面与语义不再一一对应）：
    // `,` = 逗号（主区域已无此键，留在这是给读屏一个稳定名字）；
    // `'` = 分词键（2026-10-15 起在主区域第一格，面「分词」）。
    k.key == "," -> "逗号"
    k.key == "'" -> "分词"
    else -> spokenKeyName(k.label)
}

/** 左列标点格的读屏名（与主区域分开：同一个 `'` 在主区域是分词、在标点列是单引号）。 */
internal fun nineKeyStripSpoken(k: NineKey): String = when (k.key) {
    "," -> "逗号"
    "." -> "句号"
    "\\" -> "顿号"
    "?" -> "问号"
    "!" -> "感叹号"
    ":" -> "冒号"
    ";" -> "分号"
    "(" -> "左括号"
    ")" -> "右括号"
    "[" -> "左方括号"
    "]" -> "右方括号"
    "<" -> "左书名号"
    ">" -> "右书名号"
    "\"" -> "双引号"
    "'" -> "单引号"
    "~" -> "波浪号"
    else -> spokenKeyName(k.label)
}

/**
 * 键位容器（14 键 / 笔画共用外观）：圆角底 + 点击 + 读屏名。
 * 与 [KeyButton] 同观感，区别只在内容不是单行文本。
 */
@Composable
internal fun KeyBox(
    modifier: Modifier = Modifier,
    description: String,
    onTap: () -> Unit,
    content: @Composable () -> Unit,
) {
    Box(
        modifier = modifier
            .padding(1.dp)
            .clip(RoundedCornerShape(6.dp))
            .background(LocalKeyboardPalette.current.keyBg)
            .clickable(onClick = onTap)
            .semantics {
                role = Role.Button
                contentDescription = description
            }
            .fillMaxSize(),
        contentAlignment = Alignment.Center,
    ) { content() }
}
