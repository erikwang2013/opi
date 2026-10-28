// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 候选窗 UI：Compose 自绘（无 material3 依赖，见 build.gradle.kts 注释）；
//! 读 CandidateModel 渲染，交互回调交给 PipeServer 发消息（线协议全貌见
//! Main.kt 头部注释）。

package xyz.erik.opi.candidate

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.WindowPosition
import java.awt.Toolkit
import xyz.erik.opi.pet.OpiPet
import xyz.erik.opi.pet.PetMood
import xyz.erik.opi.pet.PetPalette

// ---------- 自绘配色 ----------
//
// 2026-10-22 起**不再写死**：候选窗跟着状态栏 logo 换的那款皮肤走（用户报障
// 「点击状态栏logo切换背景，候选词框的背景没变化」）。色值唯一源 = SkinPalette.kt
// 的 `skinColors`（与状态栏同一份），根部由 Main.kt 用 `LocalBarPalette` provide
// 一次；本文件只按角色取色（bg/fg/accent/line/hover），不新造颜色。
// 「不改动」= KeepColors，五个值就是原先这里那组深色常量，行为一字不变。

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
 * **2026-10-21：Windows 候选窗改成搜狗式横条**（用户裁决「长条的，和搜狗输入法一样」）。
 * **2026-10-22：拼音行挪到候选上方**（用户裁决「候选词中的拼音要在候选词上方」）——
 * 横条改为固定两行、76dp 高：第一行拼音 + 模式，第二行候选 + 翻页控件；宽度按两行里
 * 更宽的那一行实测（[rememberCandidateBarSize]）。字号表本身不变（20.sp 的候选仍由
 * 门禁钉住）。
 *
 * 门禁 `CandidateFontGateTest` 钉：字号下限/覆盖、缩放语义、面板真用这张表（像素接线）、
 * **拼音墨迹在候选墨迹上方**、真窗下现实页形（8 条 × 1/2/4/6 字）高度不变且翻页控件固定
 * 在右端（候选溢出时也不许被挤出窗）。真窗尺寸现读 Main.kt，免得夹具与基准手抄两份。
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

/**
 * 设计窗尺寸 = Main.kt 的**初始**窗尺寸。门禁现读 Main.kt 钉住。实际宽度由
 * [rememberCandidateBarSize] 按每页内容在 [BAR_MIN_WIDTH]–[BAR_MAX_WIDTH] 间调整；
 * 高度**恒定 76dp**（2026-10-22 用户裁决「拼音要在候选词上方」：两行 ——
 * 上拼音/模式、下候选/翻页，两行都不折行）。
 *
 * 720 是「8 条 2–3 字候选 + 翻页」的常用宽度；更长的词条把窗拉宽，超屏则在候选
 * 区横向滚动（[CandidatePanel]），翻页控件始终钉在右端。
 */
internal val PANEL_DESIGN_SIZE = DpSize(720.dp, 76.dp)

/** 横条宽度上下限（dp）。上限还会按屏幕宽收口（见 [rememberCandidateBarSize]）。 */
internal const val BAR_MIN_WIDTH = 360f
internal const val BAR_MAX_WIDTH = 1000f

// 横条的间距账（[candidateBarWidthDp] 与布局共用同一批数字 —— 别在调用点各写一份）。
private const val BAR_PAD_H = 10f
private const val BAR_PAD_V = 6f
private const val BAR_GAP_BUFFER = 6f
private const val BAR_GAP_ITEM = 12f
private const val BAR_GAP_PAGE = 8f
private const val BAR_GAP_ROWS = 2f
/** 拼音行最多占多宽（超出省略；候选是拿来选的字串，候选行绝不省略）。 */
private const val BAR_BUFFER_MAX_WIDTH = 360f
private const val PAGE_BTN_PAD_H = 14f // 与 PageButton 的 horizontal padding 同步
private const val PAGE_BTN_GAP = 6f
private const val EMPTY_PET = 30f
private const val EMPTY_PET_GAP = 8f
private const val INDEX_TEXT_GAP = 2f
/** CandidateItem 的左右 padding 合计（2dp × 2）—— 测量必须算进去，否则每项少 4dp。 */
private const val ITEM_PAD_H = 4f

/**
 * 横条需要的宽度（dp）。**纯函数**：输入是已测好的文字宽度，便于单测钉住上下限与
 * 「页控件始终有位置」。`items` 为空时用 `emptyContent`（小欧 + 无匹配）。
 *
 * 两行布局（2026-10-22）：宽度 = **两行里更宽的那一行**
 * · 第一行 = 拼音缓冲 + 模式；
 * · 第二行 = 候选 + 翻页控件（翻页控件**始终**占位，不从前一行挤）。
 */
internal fun candidateBarWidthDp(
    buffer: Float,
    mode: Float,
    items: List<Float>,
    emptyContent: Float,
    page: Float,
    min: Float = BAR_MIN_WIDTH,
    max: Float = BAR_MAX_WIDTH,
): Float {
    val bufferRow = BAR_PAD_H * 2 + buffer + BAR_GAP_BUFFER + mode
    val candidates = if (items.isEmpty()) {
        emptyContent
    } else {
        items.sum() + BAR_GAP_ITEM * (items.size - 1)
    }
    val candidateRow = BAR_PAD_H * 2 + candidates + BAR_GAP_PAGE + page
    return maxOf(bufferRow, candidateRow).coerceIn(min, max)
}

/**
 * 按当前页内容算出横条尺寸（宽动态、高固定）。**在组合里调用**：文字宽度用
 * [rememberTextMeasurer]（与 Text 同一套排版），所以不存在「估算少 1px 把候选裁掉」。
 *
 * [fontScale] = 设置里的候选字号档位（[candidateFontScale]）。**测量必须与绘制用
 * 同一个字号**：只给 Text 缩放而不给测量缩放，窗宽会按旧字号算 ⇒ 「大」档候选被裁、
 * 「小」档窗里空一大截。
 *
 * 屏幕宽按本模块既有约定：AWT `screenSize` 的像素数当逻辑像素（见 Main.kt 的
 * WindowPosition 注释 —— CMP 1.11 的窗口定位不乘 density）。上限留 80dp 边距。
 */
@Composable
internal fun rememberCandidateBarSize(model: CandidateModel, fontScale: Float = 1f): DpSize {
    val density = LocalDensity.current
    // 默认 cacheSize=8，而一次测量就有 ~20 条文本（8 候选 × 序号+词、缓冲、模式、
    // 空态、箭头、页码）。按键时候选文本全变；缓存只有 8 条时，序号/模式/箭头/
    // 页码这些**每键都一样**的标签会在下一键前被挤出 ⇒ 每键重排十几次。
    // 64 条足够盖住一整页 + 常驻标签；候选文本变了自然 miss 换入，无需手动失效。
    val measurer = rememberTextMeasurer(cacheSize = 64)
    val screenMax = (Toolkit.getDefaultToolkit().screenSize.width - 80)
        .coerceAtLeast(BAR_MIN_WIDTH.toInt()).toFloat()
    return remember(
        model.buffer, model.candidates, model.page, model.pageCount, model.mode, density,
        screenMax, fontScale,
    ) {
        // 高度固定 = PANEL_DESIGN_SIZE.height ⇒ 真实运行期的窗口系数恒为 1；这里用
        // fontSizeFor 而不是裸乘，是为了与面板的 MIN 下限/缩放口径逐字一致。
        fun textW(text: String, size: TextUnit, weight: FontWeight = FontWeight.Normal): Float =
            if (text.isEmpty()) 0f else with(density) {
                val px = fontSizeFor(size, PANEL_DESIGN_SIZE, fontScale)
                measurer.measure(text, TextStyle(fontSize = px, fontWeight = weight))
                    .size.width.toDp().value
            }
        // wire 的 candidates 已是**当前页**（Main.kt 协议头），本窗口不再自设上限。
        val shown = model.candidates
        val items = shown.mapIndexed { i, text ->
            // 与 CandidateItem 的实际盒宽一致：左右 padding + 序号 + 间隙 + 候选。
            // 漏掉 padding 的话 8 条候选的窗会窄 32dp（review 2026-10-22）。
            ITEM_PAD_H + textW("${i + 1}.", CandidateFont.Index) +
                INDEX_TEXT_GAP + textW(text, CandidateFont.Candidate)
        }
        val empty = EMPTY_PET + EMPTY_PET_GAP + textW("无匹配", CandidateFont.Empty)
        val arrow = textW("‹", CandidateFont.Arrow).coerceAtLeast(textW("›", CandidateFont.Arrow))
        val page = 2 * (arrow + PAGE_BTN_PAD_H * 2) + PAGE_BTN_GAP * 2 +
            textW("${model.page}/${model.pageCount}", CandidateFont.Page)
        val width = candidateBarWidthDp(
            // 与布局同一条上限：拼音行超出 360dp 就省略，窗宽不再跟着涨
            buffer = textW(model.buffer, CandidateFont.Buffer, FontWeight.Bold)
                .coerceAtMost(BAR_BUFFER_MAX_WIDTH),
            mode = textW(modeLabel(model.mode), CandidateFont.Mode),
            items = items,
            emptyContent = empty,
            page = page,
            max = screenMax,
        )
        DpSize(width.dp, PANEL_DESIGN_SIZE.height)
    }
}

/**
 * 横条的默认落点：水平居中、垂直沿用原先候选窗的高度（320dp，屏幕放不下就上移）。
 * 位置单位与 WindowPosition 一致（逻辑像素 = dp，见 Main.kt 注释）。
 */
internal fun candidateBarPosition(size: DpSize): WindowPosition {
    val screen = Toolkit.getDefaultToolkit().screenSize
    val w = size.width.value.toInt()
    val x = ((screen.width - w) / 2).coerceIn(20, (screen.width - w - 20).coerceAtLeast(20))
    val y = minOf(320, screen.height - size.height.value.toInt() - 80).coerceAtLeast(40)
    return WindowPosition(x.dp, y.dp)
}

/**
 * 虚拟桌面（所有显示器并集）的逻辑像素范围。Main.kt 用它把「贴着 caret」的窗口
 * 收回屏内：`Toolkit.screenSize` 只有主屏，副屏（尤其左侧 `x<0`）上会把窗口拽回
 * 主屏。AWT 的 `GraphicsDevice.bounds` 与 `WindowPosition` 同一坐标空间。
 */
internal fun virtualScreenBounds(): java.awt.Rectangle {
    val screens = java.awt.GraphicsEnvironment.getLocalGraphicsEnvironment().screenDevices
    return screens.map { it.defaultConfiguration.bounds }
        .reduceOrNull { a, b -> a.union(b) }
        ?: java.awt.Rectangle(Toolkit.getDefaultToolkit().screenSize)
}

/** 放大上限：窗比设计尺寸大再多，字号也不超过基准的这么多倍（免得大屏上出巨字）。 */
internal const val MAX_FONT_SCALE = 1.5f

/**
 * 候选字号三档（设置窗「外观 → 候选字号」）→ 缩放系数。与 `ClassicUiFont.kt` 的
 * pango 三档同比：小 10pt、中 12pt（基准）、大 14pt ⇒ 10/12、1、14/12。认不得的名字
 * 按中档（与 `OpSettings.fromIni` 的回落同一口径）。
 *
 * Windows 专用：Linux 的候选面板是 fcitx5 classicui 画的，字号走 `Font=`（见
 * ClassicUiFont.kt）；本函数只喂小欧自绘候选窗。
 */
internal fun candidateFontScale(name: String): Float = when (name) {
    "小" -> 10f / 12f
    "大" -> 14f / 12f
    else -> 1f
}

/**
 * 按窗口尺寸取字号：`基准 × 用户档位 × clamp(min(宽比, 高比), 1, [MAX_FONT_SCALE])`，
 * 最后**不低于** [CandidateFont.MIN]（用户线：至少 12px；「小」档只缩得动大字号，
 * 序号/页码不会被缩到看不清）。
 *
 * 「取较大值」落在 clamp 的**下界 1**：窗比设计尺寸小也**不缩**（字号只升不降，下限就是表
 * 里的基准，已在用户线之上）；比设计尺寸大则同比放大、封顶。
 * 两个方向取 **min**：只加宽不加高时字不能变大（宽度有余但行高没余，字号大了反而溢出），
 * 只加高不加宽同理 —— 取「两边都放得下」的那个比例。
 *
 * 按**内容**尺寸自适应（条目少/短时把留白也吃进来放大）**没做**：那要 measure→resize 的
 * 反馈环（字号随候选条数跳、还可能来回抖），收益只是把留白吃掉；要就说一声。
 */
internal fun fontSizeFor(
    base: TextUnit,
    windowSize: DpSize,
    userScale: Float = 1f,
): TextUnit {
    val scaled = base * (minOf(
        windowSize.width / PANEL_DESIGN_SIZE.width,
        windowSize.height / PANEL_DESIGN_SIZE.height,
    ).coerceIn(1f, MAX_FONT_SCALE) * userScale)
    // TextUnit 没有 Comparable 扩展（Compose 1.11 实测），按数值比：
    // 本函数只收 sp 基准，下界也用 sp，量纲一致。
    return if (scaled.value < CandidateFont.MIN.value) CandidateFont.MIN else scaled
}

// ---------- Compose UI ----------

@Composable
internal fun CandidatePanel(
    model: CandidateModel,
    onSelect: (Int) -> Unit,
    onNext: () -> Unit,
    onPrev: () -> Unit,
    // 窗尺寸从调用方来（Main.kt 传动态横条尺寸）；字号按它缩放，见 fontSizeFor。
    windowSize: DpSize = PANEL_DESIGN_SIZE,
    // 设置里的候选字号档位（[candidateFontScale]）；1 = 中档（既有行为）。
    fontScale: Float = 1f,
) {
    val fontFor: (TextUnit) -> TextUnit = { fontSizeFor(it, windowSize, fontScale) }
    val palette = LocalBarPalette.current
    // 「次要文字」没有独立角色：用 fg 降透明度，深底/浅底皮肤都自动成立。
    val dim = palette.fg.copy(alpha = 0.62f)
    val scroll = rememberScrollState()
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .height(windowSize.height)
            .background(palette.bg)
            .padding(horizontal = BAR_PAD_H.dp, vertical = BAR_PAD_V.dp),
        verticalArrangement = Arrangement.spacedBy(BAR_GAP_ROWS.dp),
    ) {
        // 第一行 = 拼音（用户裁决 2026-10-22「候选词中的拼音要在候选词上方」）。
        // 太长时省略 —— 它是输入提示、不是候选，省掉最后几个字母不会选错词；
        // 候选本身**绝不省略**（见 CandidateItem）。
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            Text(
                model.buffer,
                fontSize = fontFor(CandidateFont.Buffer),
                fontWeight = FontWeight.Bold,
                color = palette.fg,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.widthIn(max = BAR_BUFFER_MAX_WIDTH.dp),
            )
            Spacer(Modifier.width(BAR_GAP_BUFFER.dp))
            Text(modeLabel(model.mode), fontSize = fontFor(CandidateFont.Mode), color = dim, maxLines = 1)
        }
        // 第二行 = 候选 + 翻页控件。
        Row(Modifier.fillMaxWidth().weight(1f), verticalAlignment = Alignment.CenterVertically) {
            if (model.candidates.isEmpty()) {
                // 无候选：小欧出来摊手，替掉一片空白（与 Android 候选栏同一处理）。
                Row(Modifier.weight(1f), verticalAlignment = Alignment.CenterVertically) {
                    OpiPet(
                        mood = PetMood.PUZZLED,
                        palette = PetPalette.Dark,
                        size = EMPTY_PET.dp,
                    )
                    Spacer(Modifier.width(EMPTY_PET_GAP.dp))
                    Text("无匹配", fontSize = fontFor(CandidateFont.Empty), color = dim)
                }
            } else {
                // 候选一行排开；超出窗宽时**横向滚动**（滚轮/触控板），候选一个不少。
                // weight(1f) 保证翻页控件永远留在右端 —— 极端长词也挤不走它。
                Row(
                    Modifier.weight(1f).horizontalScroll(scroll),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(BAR_GAP_ITEM.dp),
                ) {
                    model.candidates.forEachIndexed { index, text ->
                        CandidateItem(
                            index = index + 1,
                            text = text,
                            textSize = fontFor(CandidateFont.Candidate),
                            indexSize = fontFor(CandidateFont.Index),
                        ) { onSelect(index) }
                    }
                }
            }
            Spacer(Modifier.width(BAR_GAP_PAGE.dp))
            // 翻页：两个箭头只发 next_page/prev_page 消息、**不改本地页码**：页码的唯一真源
            // 是 TSF 回发的 show（TSF 侧已接线，见 Main.kt 头注释「翻页」段）。enabled 只是
            // 禁用态提示，不参与置页 —— 点了箭头要等下一帧 show 回来才算翻页。
            Row(verticalAlignment = Alignment.CenterVertically) {
                PageButton("‹", enabled = model.page > 1, size = fontFor(CandidateFont.Arrow), onClick = onPrev)
                Spacer(Modifier.width(PAGE_BTN_GAP.dp))
                Text(
                    "${model.page}/${model.pageCount}",
                    fontSize = fontFor(CandidateFont.Page),
                    color = dim,
                    maxLines = 1,
                )
                Spacer(Modifier.width(PAGE_BTN_GAP.dp))
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
    val palette = LocalBarPalette.current
    val interaction = remember { MutableInteractionSource() }
    val hovered by interaction.collectIsHoveredAsState()
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .background(if (hovered) palette.hover else Color.Transparent)
            .clickable(interactionSource = interaction, indication = null, onClick = onClick)
            .padding(horizontal = 2.dp),
    ) {
        Text("$index.", fontSize = indexSize, color = palette.fg.copy(alpha = 0.62f))
        // 候选词**不加省略/不缩字号**：候选是拿来选的字串，截成「中华人民共和…」就分不出
        // 是哪一条了。横条放不下时由外层横向滚动（见 CandidatePanel），整词始终可读可点。
        Text(text, fontSize = textSize, color = palette.fg, softWrap = false)
    }
}

/** 翻页按钮：无 material3 的 TextButton 平替。颜色全取当前皮肤角色。 */
@Composable
private fun PageButton(text: String, enabled: Boolean, size: TextUnit, onClick: () -> Unit) {
    val palette = LocalBarPalette.current
    val color = if (enabled) palette.accent else palette.fg.copy(alpha = 0.45f)
    Box(
        modifier = Modifier
            .background(palette.line.copy(alpha = 0.45f))
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
