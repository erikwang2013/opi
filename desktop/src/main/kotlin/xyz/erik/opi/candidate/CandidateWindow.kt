// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 候选窗 UI：Compose 自绘（无 material3 依赖，见 build.gradle.kts 注释）；
//! 读 CandidateModel 渲染，交互回调交给 PipeServer 发消息（线协议全貌见
//! Main.kt 头部注释）。

package xyz.erik.opi.candidate

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import xyz.erik.opi.pet.OpiPet
import xyz.erik.opi.pet.PetMood
import xyz.erik.opi.pet.PetPalette

// ---------- 自绘配色（无 material3 依赖，见 build.gradle.kts 注释） ----------

private val BgColor = Color(0xFF2D2D2D)
private val TextColor = Color(0xFFE8E8E8)
private val DimColor = Color(0xFF9E9E9E)
private val AccentColor = Color(0xFF4C8DFF)
private val BtnColor = Color(0xFF3A3A3A)

/** 单页候选上限（与 Rust 侧 logic.rs 的 PAGE_SIZE 一致）。 */
private const val MAX_CANDIDATES = 8

// ---------- 字号表 ----------

/**
 * 候选窗字号表（2026-10-14）。用户原话「候选词有点小了至少12px」；随后经 lead 转述确认
 * **本窗也在内**（「浮在状态栏上方那个」）⇒ 本轮整体抬一档，并把「按窗口尺寸取较大值」
 * 做成 [fontSizeFor]（窗更大字同比更大、封顶 [MAX_FONT_SCALE]；窗更小也不缩）。
 *
 * 旧 → 新（sp）：Buffer 16→18、Mode 12→13、Empty 13→14、**Candidate 17→20**、
 * Index 13→14、Page 12→13、Arrow 16→17。
 *
 * 门禁 `CandidateFontGateTest` 钉四件事：反射枚举本对象的**每个成员**逐个 ≥ [MIN] 且名单
 * 无遗漏；`fontSizeFor` 的缩放语义；面板真画出来的候选墨迹 = 按 `fontSizeFor` 单画的墨迹
 * （防「门禁读表、面板写死字面量」）；真窗尺寸下现实页形（8 条 × 1/2/4/6 字）不溢出。
 * 真窗尺寸本身也**现读 Main.kt** 钉住，免得夹具与基准手抄第二份。
 *
 * **留白换字号**（2026-10-14 实测驱动）：真窗固定 320×168（Main.kt，不归本文件），本轮把
 * Candidate 抬到 20.sp 后，「8 条 × 6 字」这一页在本机（有 CJK 字体）要 165px —— 挤得下，
 * 但 **CI 会假红**：ubuntu runner 没有覆盖 CJK 的字体，回退字体（DejaVu 级）行盒更高，实测
 * 同一页要 175px > 168。两条出路：字号退到 18.sp（DejaVu 下只剩 1px 余量，换种字体又会红），
 * 或**把留白换成字号** —— 取后者：上下 padding 8→6、首行后 Spacer 6→4、候选行距 4→2、
 * 底行前 Spacer 4→2（4 行的页省 14px）。改完 20.sp 实测：本机 151px、CI 字体环境 161px。
 * 留白/行距只余「看得出分组」的量；再想加字号，先看门禁印出来的高度账。
 *
 * 余量（同一实验）：21.sp 两边都过得去（本机 155 / CI 字体 165）；**22.sp 两边都红** ——
 * 「6 字 × 8 条」由 4 行翻成 8 行（宽不够折行），高度跳到 255。要再抬得先加窗宽/窗高
 * （Main.kt）或改折行策略，那是另一个决定。
 *
 * sp→px：Compose Desktop 上 1.sp = density × fontScale；默认缩放下 1.sp = 1px，
 * 于是「至少 12px」= 「至少 12.sp」（HiDPI 下两者同比放大，不变式仍成立）。
 */
internal object CandidateFont {
    /** 用户给的线（至少 12px）：本表每个值都 ≥ 它，门禁逐项核。 */
    val MIN = 12.sp

    /** 缓冲行（正在输入的串）。 */
    val Buffer = 18.sp

    /** 模式标签（拼音/英文/数字/符号）。 */
    val Mode = 13.sp

    /** 「无匹配」。 */
    val Empty = 14.sp

    /** 候选词本身 —— 用户说「小了」的就是它，本轮 17 → 20。 */
    val Candidate = 20.sp

    /** 候选序号（`1.`）。 */
    val Index = 14.sp

    /** 页码 `n/m`。 */
    val Page = 13.sp

    /** 翻页箭头 `‹`/`›`。 */
    val Arrow = 17.sp
}

/** 设计窗尺寸 = Main.kt 的真窗（`rememberWindowState(width = 320.dp, height = 168.dp)`）；门禁现读 Main.kt 钉住。 */
internal val PANEL_DESIGN_SIZE = DpSize(320.dp, 168.dp)

/** 放大上限：窗比设计尺寸大再多，字号也不超过基准的这么多倍（免得大屏上出巨字）。 */
internal const val MAX_FONT_SCALE = 1.5f

/**
 * 按窗口尺寸取字号：`基准 × clamp(min(宽比, 高比), 1, [MAX_FONT_SCALE])`。
 *
 * 「取较大值」落在 clamp 的**下界 1**：窗比设计尺寸小也**不缩**（字号只升不降，下限就是表
 * 里的基准，已在用户线之上）；比设计尺寸大则同比放大、封顶。
 * 两个方向取 **min**：只加宽不加高时字不能变大（宽度有余但行高没余，字号大了反而溢出），
 * 只加高不加宽同理 —— 取「两边都放得下」的那个比例。
 *
 * 按**内容**尺寸自适应（条目少/短时把留白也吃进来放大）**没做**：那要 measure→resize 的
 * 反馈环（字号随候选条数跳、还可能来回抖），收益只是把留白吃掉；要就说一声。
 */
internal fun fontSizeFor(base: TextUnit, windowSize: DpSize): TextUnit = base *
    minOf(
        windowSize.width / PANEL_DESIGN_SIZE.width,
        windowSize.height / PANEL_DESIGN_SIZE.height,
    ).coerceIn(1f, MAX_FONT_SCALE)

// ---------- Compose UI ----------

@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun CandidatePanel(
    model: CandidateModel,
    onSelect: (Int) -> Unit,
    onNext: () -> Unit,
    onPrev: () -> Unit,
    // 窗尺寸从调用方来（Main.kt 目前用默认值 = 真窗尺寸）；字号按它缩放，见 fontSizeFor。
    windowSize: DpSize = PANEL_DESIGN_SIZE,
) {
    val fontFor: (TextUnit) -> TextUnit = { fontSizeFor(it, windowSize) }
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .background(BgColor)
            // 上下 8 → 6：见 CandidateFont 头部「留白换字号」。横向不动（12.dp 只影响宽，不参与
            // 那笔高度账）。
            .padding(horizontal = 12.dp, vertical = 6.dp),
    ) {
        Column {
            // 首行：缓冲 + 模式标签
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    model.buffer,
                    fontSize = fontFor(CandidateFont.Buffer),
                    fontWeight = FontWeight.Bold,
                    color = TextColor,
                )
                Spacer(Modifier.width(8.dp))
                Text(modeLabel(model.mode), fontSize = fontFor(CandidateFont.Mode), color = DimColor)
            }
            Spacer(Modifier.height(4.dp))
            if (model.candidates.isEmpty()) {
                // 无候选：小欧出来摊手，替掉一片空白（与 Android 候选栏同一处理）。
                // 尺寸与 Android 候选栏/鸿蒙候选栏统一为 30 —— 同一只小欧在三端应当一样大。
                Row(verticalAlignment = Alignment.CenterVertically) {
                    OpiPet(
                        mood = PetMood.PUZZLED,
                        palette = PetPalette.Dark,
                        size = 30.dp,
                    )
                    Spacer(Modifier.width(8.dp))
                    Text("无匹配", fontSize = fontFor(CandidateFont.Empty), color = DimColor)
                }
            } else {
                // 候选列表（最多 8 个/页；点击 → TSF 提交）
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(16.dp),
                    verticalArrangement = Arrangement.spacedBy(2.dp),
                ) {
                    model.candidates.take(MAX_CANDIDATES).forEachIndexed { index, text ->
                        CandidateItem(
                            index = index + 1,
                            text = text,
                            textSize = fontFor(CandidateFont.Candidate),
                            indexSize = fontFor(CandidateFont.Index),
                        ) { onSelect(index) }
                    }
                }
            }
            Spacer(Modifier.height(2.dp))
            // 底行：页码 + 翻页。
            // 两个箭头只发 next_page/prev_page 消息、**不改本地页码**：页码的唯一真源是
            // TSF 回发的 show（TSF 侧已接线，见 Main.kt 头注释「翻页」段）。enabled 只是
            // 禁用态提示，不参与置页 —— 点了箭头要等下一帧 show 回来才算翻页。
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                PageButton("‹", enabled = model.page > 1, size = fontFor(CandidateFont.Arrow), onClick = onPrev)
                Text(
                    "${model.page}/${model.pageCount}",
                    fontSize = fontFor(CandidateFont.Page),
                    color = DimColor,
                )
                PageButton("›", enabled = model.page < model.pageCount, size = fontFor(CandidateFont.Arrow), onClick = onNext)
            }
        }
    }
}

@Composable
private fun CandidateItem(
    index: Int,
    text: String,
    textSize: TextUnit,
    indexSize: TextUnit,
    onClick: () -> Unit,
) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier.clickable(onClick = onClick),
    ) {
        Text("$index.", fontSize = indexSize, color = DimColor)
        // 候选词**不加省略/不缩字号**（本文件既有的唯一取舍就是「换行、整词可读」）：
        // 候选是拿来选的字串，截成「中华人民共和…」就分不出是哪一条了 —— 那正是本轮
        // 用户抱怨的「看不清」的反面。长候选在行内折行，页形上限见 CandidateFontGateTest。
        Text(text, fontSize = textSize, color = TextColor)
    }
}

/** 翻页按钮：无 material3 的 TextButton 平替。 */
@Composable
private fun PageButton(text: String, enabled: Boolean, size: TextUnit, onClick: () -> Unit) {
    val color = if (enabled) AccentColor else DimColor
    Box(
        modifier = Modifier
            .background(BtnColor)
            .clickable(enabled = enabled, onClick = onClick)
            .padding(horizontal = 14.dp, vertical = 2.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(text, fontSize = size, color = color)
    }
}

/**
 * 模式 → 中文标签（协议字符串见 `candidate_io.rs` 的 `mode_str`）。
 *
 * `else -> "拼音"` 是**载荷分支**，不是兜底美化：Rust 侧有意把 `Mode::Traditional`
 * 也编码成 `"pinyin"`（`mode_str` 的注释：候选窗无简繁概念，新增 `"traditional"`
 * 只会造成协议漂移），所以繁体态落到这里显示「拼音」是两端约定好的行为。
 * 改这个 else 之前先读 `mode_str` 的注释 —— 改坏了两端都不报错，只会静默显示错标签。
 *
 * `internal` 而非 `private`：让 WireContractTest 钉住上面这条跨轨约定。
 */
internal fun modeLabel(mode: String): String = when (mode) {
    "english" -> "英文"
    "number" -> "数字"
    "symbol" -> "符号"
    else -> "拼音"
}
