// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.unit.dp
import xyz.erik.opi.keyboard.LocalKeyboardPalette
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.sin

/**
 * 顶部**四季花纹边框**（2026-10-17 用户裁决「把默认页最上面的记录短语去掉，改成一个
 * 凸起的动态花纹边框，这个动态花纹随一年四季不同而变化，花纹你来设计，要是传统的花纹」）。
 *
 * 位置：拼音行那条约 38dp 的槽位空闲时的内容（输入中仍是拼音行 —— 与既有共用槽位
 * 同一套哲学）。季节由 `SeasonTheme`（本地日期 + 锚点配置）决定；宿主每次键盘隐藏/
 * 关闭时重新校准（"每次隐藏、关闭时校准本地的时间锚点"），下一次显示就是新花色。
 *
 * 视觉：**凸起（浮雕）** = 同一笔画画三遍 —— 深色偏移 +1.2dp（阴影）、白色偏移 −1dp
 * （高光）、应季色正面；上下各一条细浮雕边线成"框"。传统纹样按季轮换：
 *  · 春：桃枝卷草（S 形藤 + 五瓣桃花 + 叶）
 *  · 夏：水波 + 荷花（双正弦水线 + 三瓣莲）
 *  · 秋：麦穗 + 菊花（斜秆谷粒 + 放射菊瓣）
 *  · 冬：雪花 + 梅花（六出雪花与五瓣梅交替）
 *
 * 纯装饰：读屏跳过（`clearAndSetSemantics`）。不依赖网络，只读公开的本地时钟。
 */
@Composable
fun SeasonBand(season: Season, modifier: Modifier = Modifier) {
    val p = LocalKeyboardPalette.current
    Canvas(
        modifier = modifier
            .fillMaxWidth()
            .height(COMPOSING_BAR_HEIGHT_DP.dp)
            .background(p.barBg)
            .clearAndSetSemantics {},
    ) {
        val tile = 92.dp.toPx()
        val count = (size.width / tile).toInt() + 1
        // 上下细浮雕边线（"边框"）：阴影在下、高光在上
        val inset = 3.dp.toPx()
        val shadow = Color.Black.copy(alpha = 0.10f)
        val light = Color.White.copy(alpha = 0.55f)
        for (y in listOf(inset, size.height - inset)) {
            drawLine(light, Offset(0f, y - 1.dp.toPx()), Offset(size.width, y - 1.dp.toPx()), 1f)
            drawLine(shadow, Offset(0f, y + 1.dp.toPx()), Offset(size.width, y + 1.dp.toPx()), 1f)
        }
        for (i in 0 until count) {
            val cx = i * tile + tile / 2f
            // 交替纹样（同一季里两种母题错落），偶数位大母题、奇数位小母题
            when (season) {
                Season.SPRING -> if (i % 2 == 0) blossom(cx, size.height / 2f, SPRING_PINK) else vine(cx, size.height / 2f)
                Season.SUMMER -> if (i % 2 == 0) lotus(cx, size.height / 2f + 2.dp.toPx()) else wave(cx, size.height / 2f)
                Season.AUTUMN -> if (i % 2 == 0) chrysanthemum(cx, size.height / 2f) else wheat(cx, size.height / 2f)
                Season.WINTER -> if (i % 2 == 0) snowflake(cx, size.height / 2f) else blossom(cx, size.height / 2f, PLUM_RED)
            }
        }
    }
}

private val SPRING_PINK = Color(0xFFD98C9E)
private val SUMMER_TEAL = Color(0xFF4E9A96)
private val AUTUMN_AMBER = Color(0xFFC99A3E)
private val WINTER_BLUE = Color(0xFF7E93B3)
private val PLUM_RED = Color(0xFFB06A78)

/**
 * 三遍浮雕绘制的小工具：同一条路径按「阴影→高光→正面」画三次。
 * shadow/light 取色与 alpha 集中在这里（全带同一条光照方向：左上亮、右下影）。
 */
private fun DrawScope.reliefPath(color: Color, path: Path, width: Float = 1.6.dp.toPx()) {
    val dy = 1.2.dp.toPx()
    val down = Path().apply { addPath(path, Offset(0f, dy)) }
    val up = Path().apply { addPath(path, Offset(0f, -1.dp.toPx())) }
    drawPath(down, Color.Black.copy(alpha = 0.18f), style = Stroke(width))
    drawPath(up, Color.White.copy(alpha = 0.65f), style = Stroke(width))
    drawPath(path, color.copy(alpha = 0.9f), style = Stroke(width))
}

private fun DrawScope.reliefCircle(color: Color, center: Offset, radius: Float) {
    val dy = 1.2.dp.toPx()
    drawCircle(Color.Black.copy(alpha = 0.18f), radius, center + Offset(0f, dy))
    drawCircle(Color.White.copy(alpha = 0.65f), radius, center + Offset(0f, -1.dp.toPx()))
    drawCircle(color.copy(alpha = 0.9f), radius, center)
}

/** 五瓣花（桃/梅）：中心点 + 五瓣圆 + 花心。 */
private fun DrawScope.blossom(cx: Float, cy: Float, color: Color) {
    val r = 2.4.dp.toPx()
    for (k in 0 until 5) {
        val a = (k * 72.0) * PI / 180.0
        reliefCircle(color, Offset(cx + (4.dp.toPx() * cos(a)).toFloat(), cy + (4.dp.toPx() * sin(a)).toFloat()), r)
    }
    reliefCircle(if (color == PLUM_RED) Color(0xFFE6C86E) else Color(0xFFE9C46A), Offset(cx, cy), 1.6.dp.toPx())
}

/** 春 · 桃枝卷草：一段 S 形藤 + 一片叶。 */
private fun DrawScope.vine(cx: Float, cy: Float) {
    val w = 26.dp.toPx()
    val path = Path().apply {
        moveTo(cx - w / 2f, cy)
        cubicTo(cx - w / 4f, cy - 8.dp.toPx(), cx + w / 4f, cy + 8.dp.toPx(), cx + w / 2f, cy)
    }
    reliefPath(SPRING_PINK, path)
    // 叶：小椭圆（直接给绝对 topLeft，不依赖 transform 助手）
    drawOval(
        Color(0xFF7FA86B).copy(alpha = 0.85f),
        topLeft = Offset(cx - w / 4f - 3.dp.toPx(), cy - 4.dp.toPx() - 1.6.dp.toPx()),
        size = Size(6.dp.toPx(), 3.2.dp.toPx()),
    )
}

/** 夏 · 水波：两条错相位的正弦细线。 */
private fun DrawScope.wave(cx: Float, cy: Float) {
    for ((phase, dy) in listOf(0.0 to -3.dp.toPx(), PI to 3.dp.toPx())) {
        val path = Path()
        val w = 30.dp.toPx()
        for (step in 0..24) {
            val t = step / 24f
            val x = cx - w / 2f + t * w
            val y = cy + dy + sin(t * 2 * PI + phase).toFloat() * 2.dp.toPx()
            if (step == 0) path.moveTo(x, y) else path.lineTo(x, y)
        }
        reliefPath(SUMMER_TEAL, path, width = 1.3.dp.toPx())
    }
}

/** 夏 · 荷花：三瓣扇 + 水线托底。 */
private fun DrawScope.lotus(cx: Float, cy: Float) {
    val h = 10.dp.toPx()
    for (angle in listOf(-0.56, 0.0, 0.56)) {
        // 以 (cx, cy) 为轴，把"尖端向上"的花瓣点旋转 angle 弧度（不用 drawscope.rotate）
        fun r(dx: Float, dy: Float): Offset {
            val c = cos(angle.toFloat())
            val s = sin(angle.toFloat())
            return Offset(cx + dx * c - dy * s, cy + dx * s + dy * c)
        }
        val petal = Path().apply {
            val tip = r(0f, -h)
            moveTo(tip.x, tip.y)
            val c1 = r(4.dp.toPx(), -h / 2)
            val c2 = r(4.dp.toPx(), -1.dp.toPx())
            val base = r(0f, 0f)
            cubicTo(c1.x, c1.y, c2.x, c2.y, base.x, base.y)
            val c3 = r(-4.dp.toPx(), -1.dp.toPx())
            val c4 = r(-4.dp.toPx(), -h / 2)
            cubicTo(c3.x, c3.y, c4.x, c4.y, tip.x, tip.y)
        }
        reliefPath(SUMMER_TEAL, petal, width = 1.5.dp.toPx())
    }
}

/** 秋 · 麦穗：斜秆 + 交错谷粒。 */
private fun DrawScope.wheat(cx: Float, cy: Float) {
    val stalk = Path().apply {
        moveTo(cx - 8.dp.toPx(), cy + 8.dp.toPx())
        lineTo(cx + 6.dp.toPx(), cy - 8.dp.toPx())
    }
    reliefPath(AUTUMN_AMBER, stalk, width = 1.4.dp.toPx())
    for (k in 0 until 4) {
        val t = 0.25f + k * 0.2f
        val x = cx - 8.dp.toPx() + t * 14.dp.toPx()
        val y = cy + 8.dp.toPx() - t * 16.dp.toPx()
        val side = if (k % 2 == 0) 1f else -1f
        reliefCircle(AUTUMN_AMBER, Offset(x + side * 2.dp.toPx(), y - 1.5.dp.toPx()), 1.8.dp.toPx())
    }
}

/** 秋 · 菊花：八瓣放射。 */
private fun DrawScope.chrysanthemum(cx: Float, cy: Float) {
    for (k in 0 until 8) {
        val a = (k * 45.0) * PI / 180.0
        val path = Path().apply {
            moveTo(cx + (2.dp.toPx() * cos(a)).toFloat(), cy + (2.dp.toPx() * sin(a)).toFloat())
            lineTo(cx + (7.dp.toPx() * cos(a)).toFloat(), cy + (7.dp.toPx() * sin(a)).toFloat())
        }
        reliefPath(AUTUMN_AMBER, path, width = 1.6.dp.toPx())
    }
    reliefCircle(Color(0xFFE9C46A), Offset(cx, cy), 2.dp.toPx())
}

/** 冬 · 雪花：六出（三线交叉 + 六枝小叉）。 */
private fun DrawScope.snowflake(cx: Float, cy: Float) {
    val r = 9.dp.toPx()
    for (k in 0 until 3) {
        val a = (k * 60.0) * PI / 180.0
        val dx = (r * cos(a)).toFloat()
        val dy = (r * sin(a)).toFloat()
        val axis = Path().apply {
            moveTo(cx - dx, cy - dy)
            lineTo(cx + dx, cy + dy)
        }
        reliefPath(WINTER_BLUE, axis, width = 1.5.dp.toPx())
        // 两端各一对小叉（与主轴成 ±0.6 弧度）
        for (end in listOf(1f, -1f)) {
            val ex = cx + dx * end
            val ey = cy + dy * end
            for (b in listOf(-0.6, 0.6)) {
                val bx = ex - (3.dp.toPx() * cos(a + b) * end).toFloat()
                val by = ey - (3.dp.toPx() * sin(a + b) * end).toFloat()
                reliefPath(WINTER_BLUE, Path().apply { moveTo(ex, ey); lineTo(bx, by) }, width = 1.2.dp.toPx())
            }
        }
    }
}
