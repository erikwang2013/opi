// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * 9 键布局表（拼音 9 键 / 大 9 键共用；大 9 只差字号）。
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
 * 发送键都取引擎中文标点表（punctuation.rs 的 CHINESE_PUNCT）**已有的键**：
 * `,`→，`.`→。`\`→、`?`→？`!`→！ —— 上层不用另造标点通路。
 */
internal val nineKeyStripPuncts: List<NineKey> = listOf(
    NineKey("，", ","),
    NineKey("。", "."),
    NineKey("、", "\\"),
    NineKey("？", "?"),
    NineKey("！", "!"),
)

/** 分词键的发送键（音节边界分隔符 = 协议值，与 `scheme.rs` 的分隔符同一个字）：
 * 布局表与点击分流共用，别各写一份字面量。 */
internal const val SEGMENT_KEY = "'"

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
 * 拼音 9 键（T9）。[big] = 大 9 键（同布局放大键面，对齐主流输入法「拼音大9键」）。
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
    /** 重输键（2026-10-10 用户裁决「增加重输按钮，去掉分词按钮」）：清掉当前组合重来。
     * 缓冲空时是空操作（[xyz.erik.opi.engine.EngineController.clear] 本身幂等）。
     * 手动分词键 2026-10-15 又放回主区域第一格（见上）。 */
    onRetype: () -> Unit,
    /** 数字键盘切换（2026-10-10 用户裁决：底行新增）：进数字面板（与 26 键的 123 同路）。 */
    onNumber: () -> Unit,
    /** 英文键盘切换（同上）：切到英文 26 键输入模式（与输入模式面板的「英文26键」同一条
     * selectInputMode 路——它会先提交 pending 缓冲，那是「明确换布局」的既有语义）。 */
    onEnglish: () -> Unit,
    /** 候选拼音读数（9 键左列逐行显示，如 `ni'hao`；**按读音去重保序** —— 同一读音
     * 只出一行，引擎出口已去重，2026-10-12 用户裁决「左列去重」）。
     * 空列表 = 没在输入/无候选，左列显示标点）。 */
    readings: List<String> = emptyList(),
    /** 点第 i 行拼音 = 钉该读音（**不确认**，候选收窄到**读音恰等**的条目；见 onPickReading 的注释）。 */
    onPickReading: (Int) -> Unit = {},
    /** 当前钉住的读音（空串 = 未钉；命中的行高亮强调色）。 */
    pinnedReading: String = "",
    /** 读数列的滚动复位键（传缓冲：缓冲一变整列回顶部，别停在旧位置）。 */
    scrollKey: String = "",
    modeLabel: String = "中",
    big: Boolean = false,
) {
    // 键面按主流输入法排：**字母为主、数字缩小到下面**（2026-10-09 用户裁决「现在的
    // 排版太不适应」）。此前是反的（数字 22sp、字母 11sp）—— 人们认键靠字母。
    val letterSize = if (big) 30.sp else 21.sp
    val digitSize = if (big) 15.sp else 12.sp
    Column(modifier = modifier.fillMaxSize()) {
        Row(modifier = Modifier.weight(3f).fillMaxSize()) {
            // ---- 左侧一列（2026-10-09，主流输入法 9 键同款）----
            // 空闲 = 一列标点（可点，走引擎标点表）；输入时**整列换成候选拼音读数**
            // （一个读音一行、已去重：你好→ni'hao；2026-10-09 用户裁决「一列多个、
            // 可滑动查看后面候选/下一页」；2026-10-12 用户裁决「左列去重」）——
            // 「拼音是输入才替换到左侧一列」（用户原话）。
            // 左列宽度 0.72 → 1.0 键宽（2026-10-10 用户裁决「把左侧一列按钮大了」；
            // 右列要与它同宽，见下）。
            Box(modifier = Modifier.weight(1f).fillMaxHeight()) {
                if (readings.isNotEmpty()) {
                    // 读数态的整块在 CandidateReadingColumn（2026-10-16 抽出：平铺候选
                    // 视图要求同语义的第二处）。抽出的口径：点击=钉读音、读屏串、滚动
                    // 复位、封顶格数都逐字同构 —— 别在本文件里再抄一份。
                    CandidateReadingColumn(
                        readings = readings,
                        pinnedReading = pinnedReading,
                        onPick = onPickReading,
                        scrollKey = scrollKey,
                        cells = nineKeyStripPuncts.size,
                        fontSize = 15.sp,
                        modifier = Modifier.fillMaxSize(),
                    )
                } else {
                    Column(modifier = Modifier.fillMaxSize()) {
                        for (k in nineKeyStripPuncts) {
                            KeyBox(
                                modifier = Modifier.weight(1f),
                                description = nineKeySpoken(k),
                                onTap = { onKey(k.key) },
                            ) {
                                Text(k.label, fontSize = letterSize)
                            }
                        }
                    }
                }
            }
            // ---- 右侧 3×3 数字格 ----
            Column(modifier = Modifier.weight(3f)) {
                for (row in nineKeyRows) {
                    Row(modifier = Modifier.weight(1f).fillMaxSize()) {
                        for (k in row) {
                            KeyBox(
                                modifier = Modifier.weight(1f),
                                description = nineKeySpoken(k),
                                // 分词键（发 `'`）走 onSegment（宿主在空缓冲时拦住）；
                                // 其余键照常走 onKey（数字 = 输入键）。
                                onTap = { if (k.key == SEGMENT_KEY) onSegment() else onKey(k.key) },
                            ) {
                                Column(
                                    horizontalAlignment = Alignment.CenterHorizontally,
                                    verticalArrangement = Arrangement.Center,
                                ) {
                                    if (k.hint.isEmpty()) {
                                        // 无字母组的键（分词）：单行大字，与旁边键的
                                        // 字母提示同号 —— 2026-10-15 起这格只有两个字，
                                        // 原先为「，。?！」四个字形收一档的特例随之下线。
                                        Text(k.label, fontSize = letterSize)
                                    } else {
                                        Text(k.hint, fontSize = letterSize)
                                        Text(
                                            k.label,
                                            fontSize = digitSize,
                                            color = LocalKeyboardPalette.current.barDimFg,
                                        )
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // ---- 右侧一列（2026-10-10 用户裁决）----
            // ⌫ / 重输 / 空格 从底行搬到右列：三格各占三分之一格高、**与左列同宽**
            // （「大小和左侧一列一样」）。空格挪到高键上更好按，⌫ 也顺手。
            Column(modifier = Modifier.weight(1f).fillMaxHeight()) {
                KeyButton("⌫", modifier = Modifier.weight(1f).fillMaxWidth(), onTap = onBackspace)
                KeyButton("重输", modifier = Modifier.weight(1f).fillMaxWidth(), onTap = onRetype)
                KeyButton("空格", modifier = Modifier.weight(1f).fillMaxWidth(), onTap = onSpace)
            }
        }
        // 底行（2026-10-10 用户裁决）：[符][中/繁] + **新增 [123] 数字键盘与 [英文] 键盘
        // 切换**，↵ 压尾。⌫/重输/空格 已移入右列。重输是 9 键专属（14/26/笔画无此行）。
        Row(modifier = Modifier.weight(1f).fillMaxSize()) {
            KeyButton("符", modifier = Modifier.weight(1f), onTap = onSymbol)
            // 模式键在 9 键上**只切简/繁**（2026-10-15 用户裁决「简繁按钮就只切换简繁」；
            // 进英文走右边那枚 [英文] 键）—— 键面仍是「中/繁」两个既有字（= 点它会
            // 切到哪），但读屏不能沿用 spokenKeyName 对这两字的默认读法「中英切换」，
            // 故覆写名字；状态用 14 键/笔画同一处 [modeStateDescription]（「繁体」等），
            // 不为这枚键再造一套串。
            KeyButton(
                modeLabel,
                modifier = Modifier.weight(1f),
                onTap = onModeSwitch,
                description = "简繁切换",
                stateDescription = modeStateDescription(modeLabel),
            )
            KeyButton("123", modifier = Modifier.weight(1f), onTap = onNumber)
            KeyButton("英文", modifier = Modifier.weight(1f), onTap = onEnglish)
            KeyButton("↵", modifier = Modifier.weight(1f), onTap = onEnter)
        }
    }
}

/** 9 键键位的读屏名：「数字 2，字母 a b c」—— 只念数字会丢掉「这键打什么」。 */
internal fun nineKeySpoken(k: NineKey): String = when {
    k.hint.isNotEmpty() -> "数字 ${k.label}，字母 ${k.hint.toCharArray().joinToString(" ")}"
    // 按**发送键**判，不按键面（多标点那代定的规矩，键面与语义不再一一对应）：
    // `,` = 逗号（左列那枚）；`'` = 分词键（2026-10-15 起在主区域第一格，面「分词」）。
    k.key == "," -> "逗号"
    k.key == "'" -> "分词"
    else -> spokenKeyName(k.label)
}

/**
 * 键位容器（9 键 / 14 键 / 笔画共用外观）：圆角底 + 点击 + 读屏名。
 * 与 [KeyButton] 同观感，区别只在内容不是单行文本（数字 + 字母提示两行）。
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
