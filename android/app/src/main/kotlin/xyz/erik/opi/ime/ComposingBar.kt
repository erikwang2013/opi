// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
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
 * 顶部槽位是否该画**拼音行**（而不是空闲态的宠物条）。
 *
 * 判据 = 显示串/五笔提示非空，**或**引擎仍有缓冲/候选。后两条不是冗余：
 * · 9 键整段无匹时 `composingText()` 是空串（2026-10-13 裁决不落数字），但用户正在
 *   输入 —— 槽位必须继续由拼音行占着，不能切回宠物条（那会让拼音行看起来被藏了）；
 * · 「有候选 ⇒ 有拼音行」是产品硬要求（任务 2026-10-23）：候选出现的那一拍，
 *   顶部这行绝不能消失或换成装饰条。
 *
 * 纯函数，JVM 单测钉住（[ComposingRowVisibilityTest]）。
 */
internal fun composingRowActive(
    composingText: String,
    wubiHint: String,
    buffer: String,
    candidateCount: Int,
): Boolean = composingText.isNotEmpty() || wubiHint.isNotEmpty() ||
    buffer.isNotEmpty() || candidateCount > 0

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
        // heightIn(min)：常规字号下仍是 38dp 的固定槽位（窗口高度按它算）；系统字体
        // 放到 1.6×/2.0× 时让行自己长高，而不是把 18sp 的字上下裁掉。
        modifier = modifier.fillMaxWidth().heightIn(min = COMPOSING_BAR_HEIGHT_DP.dp)
            // 空槽不铺底（与候选槽同一条 2026-10-09 裁决）；有拼音时才起一条底色带。
            .let { if (shown.isEmpty() && hint.isEmpty()) it else it.background(p.barBg) },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            shown,
            modifier = Modifier
                // fill=false：短缓冲保持固有宽度，五笔提示紧跟在码串后面（原布局）；
                // 长缓冲（五笔/双拼码串）吃掉剩余宽度并以省略号收尾，不把提示挤出屏幕。
                .weight(1f, fill = false)
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
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        // 编码提示（五笔，2026-10-14 接线）：与码串同一行、其后一串暗色小字。
        // 空串 = 非五笔态 / 无接续 ⇒ **不画**（画空 Text 会让布局里多一格、
        // 也会让读屏多念一次空气）。
        if (hint.isNotEmpty()) {
            Text(
                hint,
                modifier = Modifier.padding(end = 8.dp),
                fontSize = 13.sp,
                color = p.barDimFg,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}
