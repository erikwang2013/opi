// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 按钮**面**：字形的「按盒取最大且不溢出」排版、三个自绘图标、以及每枚按钮的槽位表
//! （2026-10-14 从 `StatusBar.kt` 拆出 —— 那个文件 494 行贴着 500 行门禁）。
//!
//! 用户原话（2026-10-14，本文件所有排版的裁决来源）：「状态栏的中文、英文标点符号切换
//! 按钮：要句号在按钮中左上侧，逗号在右下侧，根据按钮显示最大，不要溢出；符号按钮中的
//! 符号也根据按钮显示最大，不要溢出；其他图标、文字等也在自己所在的按钮显示最大，
//! 不要溢出」。
//!
//! ## 为什么不用 Compose 的 `Text` 排版
//!
//! 它量的是**步进盒**（advance box），不是墨迹 —— 而标点的墨迹只占 em 盒的一小块
//! （本机 Noto 实测，100px 字号：「。」墨迹 32×31、「，」14×23、「en」高 46）。
//! 按步进盒填槽位的话，标点看起来跟旧的 12sp 一样小（填进 15×15 象限只有 ~5px 的点），
//! 正是用户报的这个毛病。本文件改量**墨迹紧框**（skia `Font.getBounds`，逐字形并上
//! 步进偏移），按墨迹取最大，再用**同一支字体**画 —— 画的就是量到的那一支，
//! 没有第二条字体链（GlyphCoverage.kt 的文件头有取证：无自定义 FontFamily 时，
//! 渲染与 `FontMgr.default` 的回退链同源）。
//!
//! ## 缺字形的环境（CI 的 ubuntu runner 没有 CJK 字体）
//!
//! `getBounds` 给全零 ⇒ 退到「步进宽 + ascent/descent」的排版盒（[metricBox] 末段），
//! 排布照常、不崩、不除零；真画出来是**空白**（本机实测缺字形一个像素都不画，
//! 见 SymbolRenderProbeTest 的注释）。所以门禁不许依赖「某个汉字画得出来」——
//! 见 StatusBarRenderProbeTest 的两支夹具。
//!
//! ⚠️ 单字体、不做**逐字**回退：本文件的槽位文本都是同一种字面（一对标点、en+26、
//! 单字），按首码位问一次够用。将来若有混字面槽位，得按 run 拆开问，别直接塞进来。
//!
//! ## 整体缩放（2026-10-15 用户裁决）
//!
//! 「状态栏图标、文字在自己所在的按钮显示太大，缩小40％」⇒ 面内容整体 × [FACE_SCALE]
//! （= 0.6，四周各留 20% 余量）。**上面那套「按墨迹取最大」的排版一行没改** —— 只是
//! 内盒从「按钮本身」变成「按钮 × 0.6」，取最大/居中/不溢出的语义原样成立。

package xyz.erik.opi.candidate

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.skiaCanvas
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import org.jetbrains.skia.Font
import org.jetbrains.skia.FontMgr
import org.jetbrains.skia.FontStyle
import org.jetbrains.skia.Paint
import org.jetbrains.skia.Rect
import org.jetbrains.skia.Typeface
import xyz.erik.opi.pet.OpiPet
import xyz.erik.opi.pet.PetMood
import xyz.erik.opi.pet.PetPalette
import java.util.Optional
import java.util.concurrent.ConcurrentHashMap

// ---------- 槽位与拟合（纯函数 + Skia 度量，无 Compose） ----------

/**
 * 一个字形槽位：要画的文本 + 它在**按钮面内区**里占的比例矩形（0..1，左上原点）。
 * 用比例不用像素：按钮边长、DPI 变了，槽位自己跟着走。
 */
internal data class GlyphSlot(
    val text: String,
    val left: Float = 0f,
    val top: Float = 0f,
    val right: Float = 1f,
    val bottom: Float = 1f,
) {
    /** 槽位盒（像素）：面为 [w]×[h] 时这块地方的矩形。 */
    fun boxIn(w: Float, h: Float): Rect =
        Rect(left * w, top * h, right * w, bottom * h)
}

/** 排好的一个字形：[x]/[y] = 画点（skia 口径：基线原点），[ink] = 墨迹紧框（同一坐标系）。 */
internal class PlacedGlyph(val text: String, val font: Font, val x: Float, val y: Float, val ink: Rect)

/**
 * 文本的字体：`FontMgr.default` 回退链，按首码位问。问不到（本机没有覆盖它的字体）
 * 就退到**默认家族** —— 排布仍是确定的（画不画得出归渲染那一层，见文件头缺字形段）。
 *
 * ⚠️ 缓存是必须的：fontconfig 单次查询实测 ~2.5ms（GlyphCoverage 的文件头），
 * 而这里是悬停重绘都会走的地方。字体在一次进程里不会中途换 ⇒ 缓存永不失效。
 */
private val typefaceCache = ConcurrentHashMap<Int, Optional<Typeface>>()

private val defaultTypeface: Typeface? by lazy {
    FontMgr.default.matchFamilyStyle(null, FontStyle.NORMAL)
}

internal fun resolveTypeface(text: String): Typeface? {
    if (text.isEmpty()) return null
    val cp = text.codePointAt(0)
    val tf = typefaceCache.computeIfAbsent(cp) {
        Optional.ofNullable(FontMgr.default.matchFamilyStyleCharacter(null, FontStyle.NORMAL, null, cp))
    }.orElse(null)
    return tf ?: defaultTypeface
}

/**
 * 文本在字号 [sizePx] 下的**墨迹紧框**（相对基线原点：x 向右、y 向下为正）。
 * 就是画出来的那个东西的紧框，不是步进盒 —— 本文件全部排版都建在它上面。
 *
 * 本机没有这个字形（skia 给全零框）⇒ 退到排版盒：宽 = 步进宽、上下 = ascent/descent。
 * 这条退路是**给缺字形环境**的（CI 没有 CJK 字体）：尺寸照样算得出来、排布确定，
 * 只是画出来是空白。别把它读成「字形在」。
 */
internal fun metricBox(text: String, font: Font): Rect {
    val glyphs = font.getStringGlyphs(text)
    if (glyphs.isNotEmpty()) {
        val xs = font.getXPositions(glyphs)
        val bs = font.getBounds(glyphs)
        var l = Float.MAX_VALUE
        var t = Float.MAX_VALUE
        var r = -Float.MAX_VALUE
        var b = -Float.MAX_VALUE
        for (i in glyphs.indices) {
            l = minOf(l, xs[i] + bs[i].left)
            r = maxOf(r, xs[i] + bs[i].right)
            t = minOf(t, bs[i].top)
            b = maxOf(b, bs[i].bottom)
        }
        if (r > l && b > t) return Rect(l, t, r, b)
    }
    val m = font.metrics
    val w = maxOf(font.measureTextWidth(text), font.size)
    return Rect(0f, minOf(m.ascent, -font.size * 0.5f), w, maxOf(m.descent, font.size * 0.5f))
}

/**
 * 把 [text] 的墨迹放进 ([boxW]×[boxH]) 里的**最大**字号（px）。
 *
 * 量-缩-再量：字号与墨迹基本线性，但字体暗示/取整让它不是严格线性 ⇒ 迭代收敛；
 * 收敛后仍差一点点就**只缩不放**（宁可小一丁点，不许溢出）。
 */
internal fun fitGlyphPx(text: String, boxW: Float, boxH: Float): Float {
    if (text.isEmpty() || boxW <= 0f || boxH <= 0f) return 0f
    val tf = resolveTypeface(text) ?: return boxH * 0.8f // 连默认家族都没有：保守值，不除零
    val font = Font(tf, boxH)
    try {
        var size = boxH
        var box = metricBox(text, font)
        repeat(6) {
            val k = minOf(boxW / box.width, boxH / box.height)
            if (k > 0.999f && k < 1.001f) return size
            size *= k
            if (size <= 0.5f) return 0.5f
            font.size = size
            box = metricBox(text, font)
        }
        var guard = 0
        while ((box.width > boxW || box.height > boxH) && guard++ < 8) {
            size *= 0.98f
            font.size = size
            box = metricBox(text, font)
        }
        return size
    } finally {
        // Font 是 native 资源（Managed）；局部量用完即还，别指望 GC 及时。
        font.close()
    }
}

/**
 * 槽位表 → 可画字形。每个槽位：墨迹按该槽的盒取最大，并**在盒里居中**。
 * 返回的 [PlacedGlyph.font] 生命周期归调用方（Compose 侧用 DisposableEffect 关）。
 */
internal fun layoutGlyphs(slots: List<GlyphSlot>, w: Float, h: Float): List<PlacedGlyph> =
    slots.mapNotNull { s ->
        val tf = resolveTypeface(s.text) ?: return@mapNotNull null
        val box = s.boxIn(w, h)
        val font = Font(tf, fitGlyphPx(s.text, box.width, box.height))
        val ink = metricBox(s.text, font)
        val tx = box.left + (box.width - ink.width) / 2f
        val ty = box.top + (box.height - ink.height) / 2f
        PlacedGlyph(
            text = s.text,
            font = font,
            x = tx - ink.left,
            y = ty - ink.top,
            // 放好之后的墨迹框 == 上面算的落点（正是门禁要断的那条「不溢出」）
            ink = Rect(tx, ty, tx + ink.width, ty + ink.height),
        )
    }

/** 画排好的一排字形（颜色只影响这一笔，不影响布局）。 */
internal fun DrawScope.drawGlyphs(glyphs: List<PlacedGlyph>, color: Color) {
    if (glyphs.isEmpty()) return
    val paint = Paint()
    try {
        paint.isAntiAlias = true // skia 的 Paint 默认关抗锯齿，文字会锯
        paint.color = color.toArgb()
        drawIntoCanvas { canvas ->
            for (g in glyphs) canvas.skiaCanvas.drawString(g.text, g.x, g.y, g.font, paint)
        }
    } finally {
        paint.close()
    }
}

/** 在 [box] 见方的面里画 [slots]（每槽按墨迹最大化、居中）。换色不重排。 */
@Composable
internal fun ButtonGlyphs(slots: List<GlyphSlot>, box: Dp, color: Color) {
    val px = with(LocalDensity.current) { box.toPx() }
    val layout = remember(slots, px) { layoutGlyphs(slots, px, px) }
    DisposableEffect(layout) { onDispose { layout.forEach { it.font.close() } } }
    Canvas(Modifier.size(box)) { drawGlyphs(layout, color) }
}

// ---------- 每枚按钮的槽位表（纯函数；改摆法只改这里 + 门禁） ----------

/**
 * 按钮 → 槽位表。字面仍全从 [iconText] 取（唯一源）：标点在这里**拆成两枚**摆对角，
 * 不出字的四枚（VK/FULLWIDTH/SETTINGS/LOGO）由 [StatusBarButtonFace] 画图标、不经过这里。
 */
internal fun buttonSlots(b: StatusBarButton, st: ImeState, mode: String): List<GlyphSlot> = when (b) {
    StatusBarButton.PUNCT -> punctSlots(st)
    StatusBarButton.MODE -> modeSlots(mode)
    else -> listOf(GlyphSlot(iconText(b, st)))
}

/**
 * 标点按钮：**句号在左上、逗号在右下**（用户 2026-10-14 原话），两枚各占一个象限、
 * 互不相交，各自按墨迹最大化。中英两态同一摆法（英文态 = 「.」「,」），字面来自
 * [punctIcon] 这个唯一源 —— 不在这里重抄「。，」「.,」（抄一份就是下次改标点时的漂移口）。
 */
internal fun punctSlots(st: ImeState): List<GlyphSlot> {
    val pair = punctIcon(st) // 两字符：中文「。，」/ 英文「.,」（BMP 单码元，substring 安全）
    return listOf(
        GlyphSlot(pair.substring(0, 1), left = 0f, top = 0f, right = 0.5f, bottom = 0.5f),
        GlyphSlot(pair.substring(1, 2), left = 0.5f, top = 0.5f, right = 1f, bottom = 1f),
    )
}

/**
 * 模式按钮（用户 2026-10-14 裁决：简繁之后增加，缩写「拼」「五」「en 右下角 26」）。
 * · 无角标态（拼音/五笔）：单字占满整块面；
 * · en26：「en」占上部 +「26」占右下角 —— 两槽**按分数就不相交**（同一个 0.55 分界，
 *   en 槽底 = 角标槽顶），所以「en 的字不会压到 26 上」是**与字体无关**的结构保证，
 *   不是「这个字体恰好没压上」。⚠️ 别把 en 的槽底调过 0.55（早先写成 0.58 = 两槽重叠
 *   0.03 面高，压不压上全看字体的墨迹比例 —— 门禁量的是结果，那种写法门禁会飘）。
 * 字面来自 [modeFace] / [modeCorner]（档位 token → 缩写/角标，唯一源在 SkinAndStatus.kt）；
 * 五笔本批只到「按 token 出字」这一层：切换是 Actions.kt 的事（引擎侧还没有五笔方案，
 * 见 lead 裁决），加一档 = 在 [modeFace] 加一行。
 */
internal fun modeSlots(mode: String): List<GlyphSlot> = buildList {
    val corner = modeCorner(mode)
    add(
        GlyphSlot(
            text = modeFace(mode),
            left = 0f,
            top = 0f,
            right = 1f,
            bottom = if (corner == null) 1f else 0.55f,
        ),
    )
    if (corner != null) add(GlyphSlot(corner, left = 0.55f, top = 0.55f, right = 1f, bottom = 1f))
}

// ---------- 一个按钮的面 ----------

/**
 * 按钮面内容的**整体缩放**（用户 2026-10-15 裁决：「状态栏图标、文字在自己所在的按钮显示
 * 太大，缩小40％」）—— 保留 60%、四周各留 20% 余量。
 *
 * 这是 2026-10-14「按按钮显示最大、不要溢出」之后的**再裁决**（用户看过填满的效果嫌大；
 * 那套效果的人眼图由 StatusBarRenderProbeTest 落盘在 /tmp/opi-render-probe/）：**排版机制一字未改**
 * （仍按墨迹取最大、居中、不溢出），只是内盒 = 按钮 × 本系数。所以：
 *  · 字形（[layoutGlyphs]/[fitGlyphPx] 那一套）、三个自绘图标**全走这一处** ——
 *    它们的内盒都是 [StatusBarButtonFace] 里算出来的同一个 `face`；
 *  · ⚠️ **小欧（LOGO）例外**：它是品牌标识，2026-10-15 二次裁决「logo 小了，按最大显示，
 *    不要溢出」⇒ 不走本系数，走 [`PET_FIT`]（见那条常量）；
 *  · ⚠️ 别在图标/字形里再各乘一遍：乘两处 = 两个真相源，改一处漏一处（那正是「图标比
 *    文字小一截」这种只在人眼里存在的错）。
 *  · ⚠️ 系数变了要同步 StatusBarRenderProbeTest 里按它算的期望（那边逐条标了「缩放
 *    系数变了要同步这里」）。
 */
internal const val FACE_SCALE = 0.6f

/**
 * **小欧（LOGO）的按最大系数** = 240/250（用户裁决 2026-10-15：「logo 小了，按最大显示，
 * 不要溢出」）。为什么不直接写 1.0：小欧的画布是 **240 宽 × 250 高**（`shared/pet/OpiPet.kt`
 * 的 `CANVAS_W/CANVAS_H`，内容竖向占满 236/240 个单位）⇒ `size = 按钮` 时**高**会伸出按钮
 * 1.25dp（就是「溢出」）。按**高**收口：宽 = 按钮 × 240/250 ⇒ 高恰好 = 按钮，宽留 4% 余量。
 * 它是本文件里**唯一**不走 [FACE_SCALE] 的面（只有这一处调用）。
 */
internal const val PET_FIT = 240f / 250f

/**
 * 一枚按钮的**面**（图标或字形），画在 [box] 见方的盒里居中。BarButton（StatusBar.kt）
 * 只管悬停底/点击/拖动/semantics —— 面单独一支是为了让门禁能**离屏直接渲它**
 * （StatusBarRenderProbeTest：无窗、无进程、无 X）。
 *
 * [box] = **按钮那块盒**（BarButton 传的就是它）；面内容实际占的是居中的 `box ×
 * FACE_SCALE`（2026-10-15 裁决）—— 缩小落在这里、且只落在这里，调用方不用自己乘。
 */
@Composable
internal fun StatusBarButtonFace(b: StatusBarButton, st: ImeState, mode: String, box: Dp) {
    val face = box * FACE_SCALE // 唯一落点：下面所有分支都画进这块内盒
    when (b) {
        // 品牌标识兼拖动把手：设置页那只小欧缩到按钮里（2026-10-13 用户「logo 大点」）。
        // 配色随条底深浅切：浅底上用深底版那套（近白身 + 淡青描边）会糊进底色。
        // ⚠️ 2026-10-15 用户裁决「logo 小了，按最大显示，不要溢出」⇒ 尺寸用 [PET_FIT]
        // （= 按**高**收进按钮），**不走** `face`（那条 0.6 是给字形与图标的）。
        StatusBarButton.LOGO -> {
            val palette = LocalBarPalette.current
            OpiPet(
                mood = PetMood.IDLE,
                palette = if (palette.bg.luminance() < 0.5f) PetPalette.Dark else PetPalette.Default,
                size = box * PET_FIT,
            )
        }
        StatusBarButton.VK -> VkIcon(face)
        StatusBarButton.FULLWIDTH -> MoonIcon(st.fullwidth, face)
        StatusBarButton.SETTINGS -> SettingsIcon(face)
        else -> ButtonGlyphs(buttonSlots(b, st, mode), face, LocalBarPalette.current.fg)
    }
}

// ---------- 自绘图标（2026-10-14 从 StatusBar.kt 搬来：尺寸改由调用方给，几何全按盒比例） ----------

/**
 * 全半角按钮 = 月相（用户裁决②：满月 = 全角、半月 = 半角）。**填满给它的盒**（用户
 * 2026-10-14「其他图标…显示最大」；盒自 2026-10-15 起 = 按钮 × [FACE_SCALE]，由
 * [StatusBarButtonFace] 传入）⇒ 半径按盒算、描边收在盒内（不外溢）。
 * 半月 = 描边圆 + 右半实心（缺的半边露的是条自己的底 —— 窗不透明，不会透出背后的窗）。
 */
@Composable
internal fun MoonIcon(fullwidth: Boolean, box: Dp) {
    val fg = LocalBarPalette.current.fg
    Canvas(Modifier.size(box)) {
        val d = size.minDimension
        val stroke = d * 0.09f
        val r = d / 2f - stroke / 2f
        val center = Offset(size.width / 2f, size.height / 2f)
        if (fullwidth) {
            drawCircle(color = fg, radius = r)
        } else {
            drawCircle(color = fg, radius = r, style = Stroke(stroke))
            drawArc(
                color = fg,
                startAngle = -90f,
                sweepAngle = 180f, // 右半
                useCenter = true,
                topLeft = Offset(center.x - r, center.y - r),
                size = Size(r * 2f, r * 2f),
            )
        }
    }
}

/** 设置按钮 = 四小方块（用户裁决②：多字的「设」去掉，画图标）；2026-10-14 起填满盒
 *  （2026-10-15 起该盒 = 按钮 × [FACE_SCALE]）。 */
@Composable
internal fun SettingsIcon(box: Dp) {
    val fg = LocalBarPalette.current.fg
    Canvas(Modifier.size(box)) {
        val d = size.minDimension
        val gap = d * 0.23f // 原 13dp 面里 5dp 块 + 3dp 缝 ⇒ 缝占 0.23
        val s = (d - gap) / 2f
        for (row in 0 until 2) {
            for (col in 0 until 2) {
                drawRoundRect(
                    color = fg,
                    topLeft = Offset(col * (s + gap), row * (s + gap)),
                    size = Size(s, s),
                    cornerRadius = CornerRadius(s * 0.2f),
                )
            }
        }
    }
}

/** 小键盘图标：一个键帽外框 + 六个键点；2026-10-14 起填满盒
 *  （2026-10-15 起该盒 = 按钮 × [FACE_SCALE]），几何全按盒比例。 */
@Composable
internal fun VkIcon(box: Dp) {
    val fg = LocalBarPalette.current.fg
    Canvas(Modifier.size(box)) {
        val d = size.minDimension
        val stroke = d * 0.09f
        val h = d * 0.72f
        val top = (size.height - h) / 2f
        // 外框左右各让开半个描边：不让的话填满按钮面后描边外侧被画布裁掉半条。
        drawRoundRect(
            color = fg,
            topLeft = Offset(stroke / 2f, top),
            size = Size(d - stroke, h),
            cornerRadius = CornerRadius(d * 0.15f),
            style = Stroke(stroke),
        )
        for (r in 0 until 2) {
            for (c in 0 until 3) {
                drawCircle(
                    color = fg,
                    radius = d * 0.065f,
                    center = Offset(d * (0.28f + c * 0.22f), top + h * (0.35f + r * 0.3f)),
                )
            }
        }
    }
}
