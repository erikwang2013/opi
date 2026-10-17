// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import androidx.compose.foundation.background
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import xyz.erik.opi.candidate.composingAnnouncement
import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.engine.InputScheme
import xyz.erik.opi.engine.composingText
import xyz.erik.opi.keyboard.LocalKeyboardPalette

/**
 * 拼音行高度（dp）。与 [xyz.erik.opi.candidate.CANDIDATE_BAR_HEIGHT_DP] 同一条纪律：
 * 只在这里定义一次，`OpiImeService` 把它算进 IME 窗口高度、本行自己用它占位。
 */
const val COMPOSING_BAR_HEIGHT_DP = 38

/**
 * **拼音行**（2026-10-15 用户裁决「移动端的拼音在候选词上方显示，不要在输入框里」）。
 *
 * 这是对 2026-10-10「组合串进字段」那条裁决的**改判**：拼音不再靠
 * `InputConnection.setComposingText` 推进输入框，而是画在**候选行上方**这一行里
 * （主流移动输入法同款位置）。落点分工：
 * · 本行读 `EngineController.composingText()` —— 9 键显示分词读数（`ni'hao`），
 *   26/14/双拼/五笔/笔画显示原缓冲（字母/码串），读数空时 9 键**留空不落数字**
 *   （2026-10-11 那条仍在）；
 * · 输入框侧：`OpiImeService.pushComposingText` 现在只负责**收干净**可能残留的组合区
 *   （见那儿的注释），不再往里写拼音。
 *
 * 槽位**常驻**（空时不铺底色、只留高度）：与候选槽同一条理由 —— 条件挂载会让键区在
 * 「有没有拼音」之间伸缩，本机实测过那条路（键高 117px ↔ 89px）。
 *
 * 无障碍：**候选播报挂在本行**（缓冲 + 候选数 + 五笔编码提示）。它原先挂在候选栏的
 * 缓冲首格上，那格随本裁决撤掉（拼音已独立成行），语义节点跟着搬到这里 ——
 * 读屏只在「节点自身」语义变化时播报（Compose issue 225780131），故 contentDescription
 * 与 liveRegion 必须在同一块里给。
 */
@Composable
fun ComposingBar(controller: EngineController, modifier: Modifier = Modifier) {
    val p = LocalKeyboardPalette.current
    val shown = controller.composingText()
    val hint = controller.wubiHint
    Row(
        modifier = modifier.fillMaxWidth().height(COMPOSING_BAR_HEIGHT_DP.dp)
            // 空槽不铺底（与候选槽同一条 2026-10-09 裁决）；有拼音时才起一条底色带。
            .let { if (shown.isEmpty() && hint.isEmpty()) it else it.background(p.barBg) },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            shown,
            modifier = Modifier
                .padding(horizontal = 10.dp)
                .semantics {
                    contentDescription = composingAnnouncement(
                        shown,
                        controller.candidates.size,
                        label = if (controller.inputScheme == InputScheme.WUBI) "编码" else "拼音",
                    ) + if (hint.isNotEmpty()) {
                        "编码提示 ${hint.toCharArray().joinToString(" ")}"
                    } else {
                        ""
                    }
                    liveRegion = LiveRegionMode.Polite
                },
            fontSize = 18.sp,
            color = p.barBufferFg,
        )
        // 编码提示（五笔，2026-10-14 接线）：与码串同一行、其后一串暗色小字。
        // 空串 = 非五笔态 / 无接续 ⇒ **不画**（画空 Text 会让布局里多一格、
        // 也会让读屏多念一次空气）。
        if (hint.isNotEmpty()) {
            Text(hint, fontSize = 13.sp, color = p.barDimFg)
        }
    }
}

/**
 * 顶部快捷条（2026-10-17 用户裁决「那你觉得放什么，默认顶部好空」）：与拼音行**共用**
 * 那条 38dp 槽位 —— 输入中画 [ComposingBar]，空闲画本行。条目来自 [QuickPhrases]
 * （最近上屏的词/短语，种子是内置常用语）；**点一下直接上屏、长按删除**。
 * 底色与拼音行同一条 `barBg`（有内容才有带 —— 与候选槽的同一条纪律）。
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun QuickPhraseBar(
    phrases: List<String>,
    onTap: (String) -> Unit,
    onRemove: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    val p = LocalKeyboardPalette.current
    Row(
        modifier = modifier
            .fillMaxWidth()
            .height(COMPOSING_BAR_HEIGHT_DP.dp)
            .background(p.barBg)
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        phrases.forEach { text ->
            Row(
                modifier = Modifier
                    .clip(RoundedCornerShape(14.dp))
                    .background(p.keyBg)
                    .combinedClickable(
                        onClick = { onTap(text) },
                        onLongClick = { onRemove(text) },
                    )
                    .padding(horizontal = 10.dp, vertical = 3.dp)
                    .semantics {
                        contentDescription = text
                        // 长按删除不可发现：读屏至少给一条能念出来的动作（同候选栏那条纪律）。
                        customActions = listOf(
                            CustomAccessibilityAction("从快捷条删除") {
                                onRemove(text)
                                true
                            },
                        )
                    },
            ) {
                Text(text, fontSize = 13.sp, color = p.keyFg)
            }
        }
    }
}
