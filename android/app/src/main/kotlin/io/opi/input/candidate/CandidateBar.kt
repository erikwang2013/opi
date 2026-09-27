// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package io.opi.input.candidate

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.KeyboardArrowLeft
import androidx.compose.material.icons.filled.KeyboardArrowRight
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.opi.input.engine.EngineController
import io.opi.input.engine.EngineMode
import io.opi.input.pet.OpiPet
import io.opi.input.pet.PetMood

/**
 * 候选栏高度（dp）。OpiImeService 把它算进 IME 窗口高度、ImeScreen 用它预留位置：
 * 三处必须一致，所以只在这里定义一次。
 */
const val CANDIDATE_BAR_HEIGHT_DP = 44

/**
 * 候选栏：拼音缓冲 + 每屏 8 候选，点击选择、长按删自造词；页数>1 时显示 ‹ n/m › 翻页。
 * 无状态：数据与翻页状态均在 EngineController（单一状态源）。
 * 两个回调都是**页内下标**（同 EngineController.selectFromPage）。
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun CandidateBar(controller: EngineController, onTap: (Int) -> Unit, onLongPress: (Int) -> Unit) {
    // english 模式：候选栏退化为模式条，切换中/英有明确区域反馈
    if (controller.mode == EngineMode.ENGLISH) {
        Row(
            modifier = Modifier.fillMaxWidth().height(CANDIDATE_BAR_HEIGHT_DP.dp)
                .background(Color(0xFFEEEEEE)),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                "EN",
                modifier = Modifier.padding(horizontal = 12.dp),
                fontSize = 16.sp,
                fontWeight = FontWeight.Bold,
                color = Color(0xFF455A64),
            )
            Text("字母直接上屏", fontSize = 13.sp, color = Color(0xFF757575))
        }
        return
    }
    val pageCount = controller.candidatePageCount
    val candidates = controller.pageCandidates
    Row(
        modifier = Modifier.fillMaxWidth().height(CANDIDATE_BAR_HEIGHT_DP.dp)
            .background(Color(0xFFEEEEEE)),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            controller.buffer,
            modifier = Modifier
                .padding(horizontal = 12.dp)
                .semantics {
                    // 播报候选变化只能挂在这里：读屏仅在「节点自身」语义变化时播报
                    // （Compose issue 225780131 —— 只改 text 不触发，须同块给 contentDescription），
                    // 而候选项是 clickable 子节点（自身合并语义），父节点合并不了它们。
                    // 于是把「缓冲 + 候选数」并进这一个节点一起播报。
                    contentDescription = composingAnnouncement(controller.buffer, candidates.size)
                    liveRegion = LiveRegionMode.Polite
                },
            fontSize = 18.sp,
            color = Color(0x8A000000),
        )
        Row(
            // 候选整屏换新时滚动位置必须回起点，否则新一批的头几个候选留在视口外
            // （看着像"这页只有 3 个词"）。两个触发点：翻页（页号变）、继续打字
            // （缓冲变 → 整批候选换掉）。
            modifier = Modifier.weight(1f).horizontalScroll(
                remember(controller.buffer, controller.candidatePage) { ScrollState(0) },
            ),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            // 序号用全列表下标（翻页后仍唯一），读屏才能区分同音候选
            val indexBase = controller.candidatePage * EngineController.pageSize
            for ((i, c) in candidates.withIndex()) {
                Text(
                    c,
                    // clickable 必须在 padding 之前：反过来的话热区只覆盖文字本身
                    // （20sp 高 ≈ 24dp），远小于候选栏 44dp，边距区域点了没反应。
                    modifier = Modifier
                        .semantics {
                            contentDescription = "第 ${indexBase + i + 1} 项 $c"
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
                    fontSize = 20.sp,
                )
            }
            // 拼音无候选：小欧出来摊手，比一行灰字更不像"卡住了"
            if (candidates.isEmpty() && controller.buffer.isNotEmpty()) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    OpiPet(
                        mood = PetMood.PUZZLED,
                        size = 30.dp,
                        modifier = Modifier.padding(start = 12.dp),
                    )
                    Text(
                        "无匹配",
                        modifier = Modifier.padding(start = 8.dp, end = 12.dp),
                        fontSize = 13.sp,
                        color = Color(0xFF9E9E9E),
                    )
                }
            }
        }
        if (pageCount > 1) {
            IconButton(onClick = controller::prevPage) {
                Icon(Icons.Filled.KeyboardArrowLeft, contentDescription = "上一页")
            }
            Text(
                "${controller.candidatePage + 1}/$pageCount",
                // "1/3" 读屏会念成「三分之一」之类，说明白是页码
                modifier = Modifier.semantics {
                    contentDescription = "第 ${controller.candidatePage + 1} 页，共 $pageCount 页"
                },
                fontSize = 13.sp,
            )
            IconButton(onClick = controller::nextPage) {
                Icon(Icons.Filled.KeyboardArrowRight, contentDescription = "下一页")
            }
        }
    }
}

/** 拼音缓冲 + 候选数的播报文案；无候选时明说「无候选」，别让读屏用户以为卡住了。 */
internal fun composingAnnouncement(buffer: String, candidateCount: Int): String {
    val head = if (buffer.isEmpty()) "" else "拼音 $buffer，"
    return if (candidateCount == 0) "${head}无候选" else "$head$candidateCount 个候选"
}

/** 读屏「操作」菜单里的删词动作名。 */
internal const val REMOVE_USER_WORD_ACTION = "删除自造词"
