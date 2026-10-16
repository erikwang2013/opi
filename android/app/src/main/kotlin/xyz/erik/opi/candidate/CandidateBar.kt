// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.candidate

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.keyboard.LocalKeyboardPalette
import xyz.erik.opi.engine.EngineMode
import xyz.erik.opi.pet.OpiPet
import xyz.erik.opi.pet.petMood

/**
 * 候选栏：拼音缓冲 + 每屏 8 候选，点击选择、长按删自造词。
 * 无状态：数据与翻页状态均在 EngineController（单一状态源）。
 * 两个回调都是**页内下标**（同 EngineController.selectFromPage）。
 *
 * 2026-10-16 裁决「候选词在功能按钮区域显示（替换掉功能区域）」：本栏画在
 * 「候选｜工具条」**共用一格**里（`ime/ImeScreen.kt`，格高 = 工具条那条常量）——
 * 自身不再定高（fillMaxHeight 由调用方给的槽决定）。
 * 同裁决「左右按钮换成 v 的图标……上下滑动翻页」：‹ n/m › 整段下线，右端只留一枚 ⌄
 * （[onExpand] 进 `CandidateTiledView` 平铺视图），翻页全部改到平铺态里上下滑。
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun CandidateBar(
    controller: EngineController,
    onTap: (Int) -> Unit,
    onLongPress: (Int) -> Unit,
    /** 右端 ⌄：进平铺候选视图（[xyz.erik.opi.ime.ImeState.expandCandidates]）。 */
    onExpand: () -> Unit,
    modifier: Modifier = Modifier,
) {
    // 配色走键盘色板（表情条/候选栏与键面同一套「暗光/长辈」换肤，见 Palette.kt）
    val p = LocalKeyboardPalette.current
    // english 模式：候选栏退化为模式条，切换中/英有明确区域反馈
    if (controller.mode == EngineMode.ENGLISH) {
        Row(
            modifier = modifier.fillMaxWidth().fillMaxHeight()
                .background(p.barBg),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                "EN",
                modifier = Modifier.padding(horizontal = 8.dp),
                fontSize = 16.sp,
                fontWeight = FontWeight.Bold,
                color = p.accent,
            )
            Text("字母直接上屏", fontSize = 13.sp, color = p.barDimFg)
        }
        return
    }
    val candidates = controller.pageCandidates
    Row(
        modifier = modifier.fillMaxWidth().fillMaxHeight()
            .background(p.barBg),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        // ⚠️ 2026-10-15 用户裁决改判：拼音（缓冲/读数）与五笔编码提示**不再画在本栏**——
        // 它们搬去了候选行**上方**的独立一行（`ime/ComposingBar.kt`，含读屏播报）。
        // 本栏自此只画候选本身。撤掉的那格见 git 历史与 docs/install/android.md 的订正。
        Row(
            // 候选整批换新时滚动位置必须回到起点，否则新一批的头几个候选留在视口外
            // （看着像"这页只有 3 个词"）。触发点：继续打字（缓冲变）、钉/解读音
            // （候选整批换）。翻页触发点随 2026-10-16 裁决（‹ n/m › 下线）一并撤掉。
            modifier = Modifier.weight(1f).horizontalScroll(
                remember(controller.buffer, controller.pinnedReading) { ScrollState(0) },
            ),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            for ((i, c) in candidates.withIndex()) {
                // 可见序号 = **页内**下标 + 1（翻页后从 1 重数）。两条理由：
                //   · 与数字选词的语义同源 —— 引擎里数字键选的是**页内索引**
                //     （`composer.rs` 的数字键约定），写成全列表下标就与「按 3 选本页第 3 个」对不上；
                //   · 与 Windows 候选窗同款（`CandidateWindow.kt` 的 `"$index."`，同样是页内）。
                // ⚠️ 播报用的 `n` 与可见序号是**同一个值**（`"$n."`）—— 不许一处页内、一处全列表。
                // （2026-10-09：曾整串去掉过一版 —— 用户澄清「候选词前的数字还要的，
                //  多出来的是 9 键那串键位数字」，故恢复。）
                val n = i + 1
                Row(
                    // clickable 必须在 padding 之前：反过来的话热区只覆盖文字本身
                    // （20sp 高 ≈ 24dp），远小于候选栏 44dp，边距区域点了没反应。
                    modifier = Modifier
                        .semantics {
                            // ⚠️ 反转记录：此处原为**全列表**下标（`indexBase + i + 1`），理由是
                            // 「翻页后仍唯一」。可见序号立起来之后那条理由不再成立 —— 同一项会出现
                            // 两个数（看得见的是 3、念出来的是 11），两者只能留一个：留下的必须与
                            // 看得见的一致。而「唯一」本身没丢：同一屏只渲染当前页，页内下标在本屏唯一。
                            contentDescription = "第 $n 项 $c"
                            // 长按删词是不可发现的交互：读屏至少要有一条能念出来的动作。
                            // 不往 contentDescription 里塞「长按可删除自造词」——候选每次刷新
                            // 8 条都念一遍太吵，而且对词典词（删不掉）是假承诺；
                            // customActions 会出现在 TalkBack 的「操作」菜单里，长按本身
                            // 也由 combinedClickable 报给读屏（双击并按住）。
                            customActions = listOf(
                                CustomAccessibilityAction(REMOVE_USER_WORD_ACTION) {
                                    onLongPress(i)
                                    true
                                },
                            )
                        }
                        // 长按不能吃掉点击提交：combinedClickable 里短按照走 onClick
                        // （长按 500ms 触发 onLongClick，两种情况互斥）
                        .combinedClickable(
                            onClick = { onTap(i) },
                            onLongClick = { onLongPress(i) },
                        )
                        .padding(horizontal = 12.dp, vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text("$n.", fontSize = 13.sp, color = p.barBufferFg, modifier = Modifier.padding(end = 3.dp))
                    Text(c, fontSize = 22.sp, color = p.keyFg)
                }
            }
            // 拼音无候选：小欧出来摊手，比一行灰字更不像"卡住了"。
            // 情绪不能写死 PUZZLED：这里的「无匹配」是**词典**的直接产物，降级时（引擎
            // 跑在内置 35 词库上）几乎每个输入都会走到这里，摊手就成了拿「你打错了」
            // 冒充「引擎坏了」（petMood 头注点名要避免的）。关掉学习同理 → SLEEPY。
            if (candidates.isEmpty() && controller.buffer.isNotEmpty()) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    OpiPet(
                        mood = petMood(
                            buffer = controller.buffer,
                            candidateCount = controller.candidates.size,
                            learnerEnabled = controller.learnerEnabled,
                            dictionaryDegraded = controller.dictionaryDegraded,
                        ),
                        size = 30.dp,
                        modifier = Modifier.padding(start = 8.dp),
                    )
                    Text(
                        "无匹配",
                        modifier = Modifier.padding(start = 8.dp, end = 8.dp),
                        fontSize = 13.sp,
                        color = p.barDimFg,
                    )
                }
            }
        }
        // 右端 ⌄（2026-10-16 裁决「左右按钮换成 v 的图标」）：展开 = 键盘区换成平铺候选
        // （`CandidateTiledView`）。「无匹配」那态（有缓冲没候选）不给入口 —— 展开一个
        // 空网格没有意义。
        if (candidates.isNotEmpty()) {
            IconButton(onClick = onExpand) {
                Icon(Icons.Filled.KeyboardArrowDown, contentDescription = "展开候选")
            }
        }
    }
}

/**
 * 候选栏最左格（缓冲格）显示的文本。
 *
 * **9 键方案下恒空**（2026-10-09 用户二次裁决：「输入无匹配的时候，候选出现 logo
 * 图标，**logo 图标前就不要显示数字了**」）—— 9 键的缓冲是数字串，候选区里没有它的
 * 位置：无匹配时那格就该只有小欧 +「无匹配」。其余方案（26/14/笔画）显示实际缓冲
 * （字母串本来就是拼音）。
 *
 * ⚠️ 别把「无候选时显示缓冲原文」加回来：2026-10-09 曾把它当作「无匹配时全屏无反馈」
 * 的修复，用户看过之后明确不要（数字串在候选区是噪音）。若又收到「无匹配时键盘像死了」，
 * 先与用户对反馈载体（键面/小欧/无匹配文案），别直接改这一格。
 */
/** 缓冲 + 候选数的播报文案；无候选时明说「无候选」，别让读屏用户以为卡住了。
 *
 * `label` = 那一格缓冲**是什么**（2026-10-14 加参数）：拼音方案念「拼音」，
 * 五笔念「编码」—— 五笔格子里放的是码串，念成拼音是假事实。
 */
internal fun composingAnnouncement(
    buffer: String,
    candidateCount: Int,
    label: String = "拼音",
): String {
    val head = if (buffer.isEmpty()) "" else "$label $buffer，"
    return if (candidateCount == 0) "${head}无候选" else "$head$candidateCount 个候选"
}

/** 读屏「操作」菜单里的删词动作名。 */
internal const val REMOVE_USER_WORD_ACTION = "删除自造词"
