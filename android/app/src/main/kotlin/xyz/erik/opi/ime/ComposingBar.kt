// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.withFrameNanos
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
/**
 * 拼音行实际显示什么：引擎显示串优先；9 键整段无匹时显示串为空（引擎不把数字当
 * 拼音），此时回退**原按键序列**（数字键串），用次要色画 —— 行不能空白，否则用户
 * 看到的就是「拼音被隐藏」（2026-10-23 用户反馈）。
 * 这不是假拼音：T9 一位多字母，凭键位猜读音会骗人；回退只展示用户实际按下的键。
 */
internal fun composingRowText(display: String, buffer: String): String =
    display.ifEmpty { buffer }

/** 当前显示的是不是「无匹回退」的原按键序列（决定用次要色）。 */
internal fun composingRowIsFallback(display: String, buffer: String): Boolean =
    display.isEmpty() && buffer.isNotEmpty()

@Composable
fun ComposingBar(controller: EngineController, modifier: Modifier = Modifier) {
    val p = LocalKeyboardPalette.current
    val display = controller.composingText()
    val shown = composingRowText(display, controller.buffer)
    val fallback = composingRowIsFallback(display, controller.buffer)
    val hint = controller.wubiHint
    // 拼音/编码可能比屏幕宽：不省略，横向滑动看完整串（2026-10-23 用户裁决）。
    // 每次内容变化自动滚到最右端，最新输入的尾巴始终可见；用户仍可拖回前面。
    val scroll = rememberScrollState()
    LaunchedEffect(shown, hint) {
        // 等一帧拿到真实 maxValue（LaunchedEffect 早于布局时它是 0）。
        withFrameNanos { }
        scroll.animateScrollTo(scroll.maxValue)
    }
    Row(
        // heightIn(min)：常规字号下仍是 38dp 的固定槽位（窗口高度按它算）；系统字体
        // 放到 1.6×/2.0× 时让行自己长高，而不是把 18sp 的字上下裁掉。
        modifier = modifier.fillMaxWidth().heightIn(min = COMPOSING_BAR_HEIGHT_DP.dp)
            // 空槽不铺底（与候选槽同一条 2026-10-09 裁决）；有拼音时才起一条底色带。
            .let { if (shown.isEmpty() && hint.isEmpty()) it else it.background(p.barBg) },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Row(
            modifier = Modifier.weight(1f).horizontalScroll(scroll)
                .padding(horizontal = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                shown,
                modifier = Modifier.semantics {
                    contentDescription = composingAnnouncement(
                        shown,
                        controller.candidates.size,
                        label = when {
                            controller.inputScheme == InputScheme.WUBI -> "编码"
                            fallback -> "按键"
                            else -> "拼音"
                        },
                    ) + if (hint.isNotEmpty()) {
                        "编码提示 ${hint.toCharArray().joinToString(" ")}"
                    } else {
                        ""
                    }
                    liveRegion = LiveRegionMode.Polite
                },
                fontSize = 18.sp,
                color = if (fallback) p.barDimFg else p.barBufferFg,
                maxLines = 1,
                softWrap = false,
                overflow = TextOverflow.Clip,
            )
            // 编码提示（五笔，2026-10-14 接线）：与码串同一行、其后一串暗色小字。
            // 空串 = 非五笔态 / 无接续 ⇒ **不画**（画空 Text 会让布局里多一格、
            // 也会让读屏多念一次空气）。
            if (hint.isNotEmpty()) {
                Text(
                    hint,
                    modifier = Modifier.padding(start = 6.dp, end = 2.dp),
                    fontSize = 13.sp,
                    color = p.barDimFg,
                    maxLines = 1,
                    softWrap = false,
                    overflow = TextOverflow.Clip,
                )
            }
        }
    }
}
