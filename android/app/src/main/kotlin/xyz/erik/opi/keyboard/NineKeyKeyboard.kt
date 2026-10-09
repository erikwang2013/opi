// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * 9 键布局表（拼音 9 键 / 大 9 键共用；大 9 只差字号）。
 *
 * ⚠️ **发送的键值（[NineKey.key]）= 协议**：必须落在 engine-core `scheme.rs` 的
 * Nine 类表里（`'2'..'9'` 各映射三四个字母）。hint 是键面下的字母提示，且**必须
 * 正是该数字在 `scheme.rs` 里带的类** —— `NineKeyLayoutTest` 按类表交叉核对，
 * 两边任何一处改了都会红。
 *
 * `，` 键（发送 `,`）沿用引擎的标点表：中文模式下由标点层出「，」，与 26 键盘的
 * `,` 是**同一条路**（不是 UI 直提）。
 */
internal data class NineKey(val label: String, val key: String, val hint: String = "")

/**
 * **左侧一列**的默认内容 = 标点列（搜狗同款：空闲时一列标点，输入时整列换成拼音）。
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

internal val nineKeyRows: List<List<NineKey>> = listOf(
    listOf(NineKey("，。?!", ","), NineKey("2", "2", "abc"), NineKey("3", "3", "def")),
    listOf(NineKey("4", "4", "ghi"), NineKey("5", "5", "jkl"), NineKey("6", "6", "mno")),
    listOf(NineKey("7", "7", "pqrs"), NineKey("8", "8", "tuv"), NineKey("9", "9", "wxyz")),
)

/**
 * 拼音 9 键（T9）。[big] = 大 9 键（同布局放大键面，对齐搜狗「拼音大9键」）。
 * 数字键在此方案下**是输入键**（引擎 Nine 方案收进缓冲），不需要任何数字选词 UI。
 */
@Composable
fun NineKeyKeyboard(
    modifier: Modifier = Modifier,
    onKey: (String) -> Unit,
    onSpace: () -> Unit,
    onBackspace: () -> Unit,
    onEnter: () -> Unit,
    onModeSwitch: () -> Unit,
    onSymbol: () -> Unit,
    /** 分词键（搜狗同款）：切音节边界（xi|an）。**由宿主在缓冲空时拦住** —— 空缓冲
     * 点它没有边界可切（引擎那边缓冲空时 `'` 是引号，不是分词符）。 */
    onSegment: () -> Unit,
    /** 逐条候选拼音读数（9 键左列按候选逐行显示，如 `ni'hao`；与候选同序等长。
     * 空列表 = 没在输入/无候选，左列显示标点）。 */
    readings: List<String> = emptyList(),
    /** 点第 i 行拼音 = 钉该读音（**不确认**，候选收窄到一族；见 onPickReading 的注释）。 */
    onPickReading: (Int) -> Unit = {},
    /** 当前钉住的读音（空串 = 未钉；命中的行高亮强调色）。 */
    pinnedReading: String = "",
    /** 读数列的滚动复位键（传缓冲：缓冲一变整列回顶部，别停在旧位置）。 */
    scrollKey: String = "",
    modeLabel: String = "中",
    big: Boolean = false,
) {
    // 键面按搜狗排：**字母为主、数字缩小到下面**（2026-10-09 用户裁决「现在的
    // 排版太不适应」）。此前是反的（数字 22sp、字母 11sp）—— 人们认键靠字母。
    val letterSize = if (big) 26.sp else 18.sp
    val digitSize = if (big) 13.sp else 10.sp
    Column(modifier = modifier.fillMaxSize()) {
        Row(modifier = Modifier.weight(3f).fillMaxSize()) {
            // ---- 左侧一列（2026-10-09，搜狗 9 键同款）----
            // 空闲 = 一列标点（可点，走引擎标点表）；输入时**整列换成逐条候选拼音**
            // （一个候选一行：你好→ni'hao；2026-10-09 用户裁决「一列多个、可滑动
            // 查看后面候选/下一页」）——「拼音是输入才替换到左侧一列」（用户原话）。
            Box(modifier = Modifier.weight(0.72f).fillMaxHeight()) {
                if (readings.isNotEmpty()) {
                    BoxWithConstraints(
                        modifier = Modifier.fillMaxSize().semantics {
                            // 念前几条就够（全量可能几十条）；逐条与候选的对应靠候选
                            // 自己的语义节点，这里只报「读数列有货」。
                            contentDescription = "候选拼音 " + readings.take(4).joinToString("、") +
                                if (readings.size > 4) "等" else ""
                        },
                    ) {
                        // 与标点列同一节奏：五格；多出来的候选（第 6 条起）滑动看。
                        val cell = maxHeight / nineKeyStripPuncts.size
                        Column(
                            // 新一批读数（缓冲变了）整列回顶部 —— 否则继续输入后视口
                            // 停在旧位置，与「当前读音」对不上（scrollKey 由宿主给缓冲）。
                            modifier = Modifier.fillMaxSize()
                                .verticalScroll(remember(scrollKey) { ScrollState(0) }),
                            horizontalAlignment = Alignment.CenterHorizontally,
                        ) {
                            for ((i, r) in readings.withIndex()) {
                                val pinned = r == pinnedReading && pinnedReading.isNotEmpty()
                                Box(
                                    // 点一行拼音 = **钉住该读音**（2026-10-09 用户裁决
                                    // 「选择后候选词跟着拼音变化，不要直接确认」）：
                                    // 候选列表收窄到该读音一族、不上屏；同一行再点 = 解除。
                                    modifier = Modifier.fillMaxWidth().height(cell)
                                        .clickable { onPickReading(i) }
                                        .semantics {
                                            role = Role.Button
                                            contentDescription = "第 ${i + 1} 项拼音 $r" +
                                                if (pinned) "，已选中，点击取消" else "，点击只看这一读音"
                                        },
                                    contentAlignment = Alignment.Center,
                                ) {
                                    Text(
                                        r,
                                        fontSize = 13.sp,
                                        color = if (pinned) {
                                            LocalKeyboardPalette.current.accent
                                        } else {
                                            LocalKeyboardPalette.current.barDimFg
                                        },
                                        fontWeight = if (pinned) FontWeight.Bold else FontWeight.Normal,
                                        maxLines = 1,
                                        overflow = TextOverflow.Ellipsis,
                                        textAlign = TextAlign.Center,
                                    )
                                }
                            }
                        }
                    }
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
                                onTap = { onKey(k.key) },
                            ) {
                                Column(
                                    horizontalAlignment = Alignment.CenterHorizontally,
                                    verticalArrangement = Arrangement.Center,
                                ) {
                                    if (k.hint.isEmpty()) {
                                        // 无字母组的键：单行大字。**多标点键**（，。?!）字号
                                        // 收一档 —— 四个字形在 1/3 宽的键里要放得下。
                                        Text(
                                            k.label,
                                            fontSize = if (k.label.length > 1) (digitSize.value + 3).sp else letterSize,
                                        )
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
        }
        // 底行按搜狗排：**[符] 在左、[中/英] 其次**（2026-10-09；此前是反的），
        // 再**分词**、空格、⌫、↵。分词是 9 键专属（14/26/笔画没有这一行）。
        Row(modifier = Modifier.weight(1f).fillMaxSize()) {
            KeyButton("符", modifier = Modifier.weight(1f), onTap = onSymbol)
            KeyButton(
                modeLabel,
                modifier = Modifier.weight(1f),
                onTap = onModeSwitch,
                stateDescription = modeStateDescription(modeLabel),
            )
            KeyButton("分词", modifier = Modifier.weight(1f), onTap = onSegment)
            KeyButton("空格", modifier = Modifier.weight(2f), onTap = onSpace)
            KeyButton("⌫", modifier = Modifier.weight(1f), onTap = onBackspace)
            KeyButton("↵", modifier = Modifier.weight(1f), onTap = onEnter)
        }
    }
}

/** 9 键键位的读屏名：「数字 2，字母 a b c」—— 只念数字会丢掉「这键打什么」。 */
internal fun nineKeySpoken(k: NineKey): String = when {
    k.hint.isNotEmpty() -> "数字 ${k.label}，字母 ${k.hint.toCharArray().joinToString(" ")}"
    // 按**发送键**判逗号，不按键面：左上角的面是多标点「，。?!」（2026-10-09）
    k.key == "," -> "逗号"
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
