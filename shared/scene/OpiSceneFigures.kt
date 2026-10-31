// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 动态背景里的**角色**：猫、兔子、小人、气球。全部是比例几何（`s` = 角色尺度），
//! 同一个函数同时服务手机整幅背景与桌面条外角色 —— 桌面那版只是把 `detail` 提到 1、
//! `muted` 压低，不做第二套形状。
//!
//! 花草/云/藤蔓/蝴蝶在 `OpiSceneNature.kt`；相位公式与调色在 `OpiScene.kt`。

package xyz.erik.opi.scene

import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.rotate

// 固定色只作色相种子；最终都经 sceneFill 混进当前皮肤底色（见 OpiScene.kt）。
private val CatOrange = Color(0xFFF2A65A)
private val CatCream = Color(0xFFFFE8C2)
private val RabbitCream = Color(0xFFFFF1E2)
internal val RabbitPink = Color(0xFFF2A7B3)
private val PersonSkin = Color(0xFFF4C9A3)
private val PersonHair = Color(0xFF5B4038)
private val ShirtTeal = Color(0xFF4FA7A0)
private val BalloonCoral = Color(0xFFEC6D62)

/** 猫：坐着，头/耳朵露出条沿，身体可被条本体的不透明底挡住。 */
internal fun DrawScope.drawCatFigure(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
    detail: Float,
    fillColor: Color = CatOrange,
    creamColor: Color = CatCream,
) {
    val fill = sceneFill(fillColor, p, muted)
    val cream = sceneFill(creamColor, p, muted)
    val edge = sceneEdge(p, 0.35f + 0.35f * detail)
    val line = s * (0.022f + 0.01f * detail)
    val bob = sceneSway(phase, 0.15f) * s * 0.018f
    val c = Offset(center.x, center.y + bob)
    val head = Offset(c.x, c.y - s * 0.28f)
    val headR = s * 0.42f

    // 尾巴（先画在身体后面）。
    drawPath(
        Path().apply {
            moveTo(c.x + s * 0.40f, c.y + s * 0.30f)
            quadraticTo(
                c.x + s * 0.82f,
                c.y + s * 0.12f + sceneSway(phase, 0.5f) * s * 0.06f,
                c.x + s * 0.64f,
                c.y - s * 0.20f,
            )
        },
        edge,
        style = Stroke(s * 0.10f, cap = StrokeCap.Round),
    )
    // 身体 + 肚子。
    drawOval(fill, Offset(c.x - s * 0.48f, c.y - s * 0.10f), Size(s * 0.96f, s * 0.82f))
    drawOval(cream, Offset(c.x - s * 0.27f, c.y + s * 0.10f), Size(s * 0.54f, s * 0.52f))
    drawOval(edge, Offset(c.x - s * 0.48f, c.y - s * 0.10f), Size(s * 0.96f, s * 0.82f), style = Stroke(line))
    // 耳朵（三角）+ 内耳。
    for (dir in listOf(-1f, 1f)) {
        val ex = head.x + dir * headR * 0.62f
        val ey = head.y - headR * 0.55f
        val ear = Path().apply {
            moveTo(ex - s * 0.17f, ey + s * 0.06f)
            lineTo(ex, ey - s * 0.30f)
            lineTo(ex + s * 0.17f, ey + s * 0.06f)
            close()
        }
        drawPath(ear, fill)
        drawPath(ear, edge, style = Stroke(line, join = StrokeJoin.Round))
        val inner = Path().apply {
            moveTo(ex - s * 0.08f, ey + s * 0.02f)
            lineTo(ex, ey - s * 0.18f)
            lineTo(ex + s * 0.08f, ey + s * 0.02f)
            close()
        }
        drawPath(inner, sceneFill(RabbitPink, p, muted))
    }
    // 头 + 眼睛/鼻子/胡须。
    drawCircle(fill, headR, head)
    drawCircle(edge, headR, head, style = Stroke(line))
    val eyeY = head.y + s * 0.02f
    if (sceneBlink(phase, 0.2f)) {
        for (dir in listOf(-1f, 1f)) {
            drawLine(
                edge,
                Offset(head.x + dir * headR * 0.26f - s * 0.045f, eyeY),
                Offset(head.x + dir * headR * 0.26f + s * 0.045f, eyeY),
                strokeWidth = line * 1.2f,
                cap = StrokeCap.Round,
            )
        }
    } else {
        for (dir in listOf(-1f, 1f)) {
            drawCircle(edge, s * 0.055f, Offset(head.x + dir * headR * 0.26f, eyeY))
        }
    }
    drawPath(
        Path().apply {
            moveTo(head.x - s * 0.045f, head.y + s * 0.14f)
            lineTo(head.x + s * 0.045f, head.y + s * 0.14f)
            lineTo(head.x, head.y + s * 0.20f)
            close()
        },
        sceneFill(RabbitPink, p, muted),
    )
    for (dir in listOf(-1f, 1f)) {
        drawLine(
            edge.copy(alpha = edge.alpha * 0.55f),
            Offset(head.x + dir * headR * 0.45f, head.y + s * 0.16f),
            Offset(head.x + dir * headR * 1.05f, head.y + s * 0.12f),
            strokeWidth = line * 0.7f,
            cap = StrokeCap.Round,
        )
        drawLine(
            edge.copy(alpha = edge.alpha * 0.55f),
            Offset(head.x + dir * headR * 0.45f, head.y + s * 0.20f),
            Offset(head.x + dir * headR * 1.00f, head.y + s * 0.24f),
            strokeWidth = line * 0.7f,
            cap = StrokeCap.Round,
        )
    }
    // 前爪。
    for (dir in listOf(-1f, 1f)) {
        drawOval(cream, Offset(c.x + dir * s * 0.24f - s * 0.11f, c.y + s * 0.28f), Size(s * 0.22f, s * 0.18f))
    }
}

/** 兔子：长耳朵会随相位摆，身体和猫同一个坐在条沿上的构图。 */
internal fun DrawScope.drawRabbitFigure(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
    detail: Float,
) {
    val fill = sceneFill(RabbitCream, p, muted)
    val pink = sceneFill(RabbitPink, p, muted)
    val edge = sceneEdge(p, 0.35f + 0.35f * detail)
    val line = s * (0.022f + 0.01f * detail)
    val bob = sceneSway(phase, 0.6f) * s * 0.015f
    val c = Offset(center.x, center.y + bob)
    val head = Offset(c.x, c.y - s * 0.28f)
    val headR = s * 0.38f
    val earSway = sceneSway(phase, 0.35f) * 7f

    // 耳朵（圆角长条，绕耳根旋转）。
    for (dir in listOf(-1f, 1f)) {
        val px = head.x + dir * headR * 0.34f
        val py = head.y - headR * 0.55f
        rotate(degrees = dir * 8f + earSway, pivot = Offset(px, py)) {
            val earTop = py - s * 0.62f
            drawRoundRect(
                fill,
                topLeft = Offset(px - s * 0.10f, earTop),
                size = Size(s * 0.20f, s * 0.66f),
                cornerRadius = CornerRadius(s * 0.10f),
            )
            drawRoundRect(
                pink,
                topLeft = Offset(px - s * 0.05f, earTop + s * 0.10f),
                size = Size(s * 0.10f, s * 0.44f),
                cornerRadius = CornerRadius(s * 0.05f),
            )
            drawRoundRect(
                edge,
                topLeft = Offset(px - s * 0.10f, earTop),
                size = Size(s * 0.20f, s * 0.66f),
                cornerRadius = CornerRadius(s * 0.10f),
                style = Stroke(line),
            )
        }
    }
    // 身体、头、肚子、脚。
    drawOval(fill, Offset(c.x - s * 0.44f, c.y - s * 0.12f), Size(s * 0.88f, s * 0.80f))
    drawOval(
        sceneFill(Color.White, p, (muted + 0.1f).coerceAtMost(0.85f)),
        Offset(c.x - s * 0.25f, c.y + s * 0.08f),
        Size(s * 0.50f, s * 0.50f),
    )
    drawOval(edge, Offset(c.x - s * 0.44f, c.y - s * 0.12f), Size(s * 0.88f, s * 0.80f), style = Stroke(line))
    drawCircle(fill, headR, head)
    drawCircle(edge, headR, head, style = Stroke(line))
    // 眼睛 + 鼻子 + 腮红。
    for (dir in listOf(-1f, 1f)) {
        val eye = Offset(head.x + dir * headR * 0.34f, head.y + s * 0.01f)
        if (sceneBlink(phase + 0.4f, 0.5f)) {
            drawLine(edge, eye + Offset(-s * 0.04f, 0f), eye + Offset(s * 0.04f, 0f), line * 1.2f, StrokeCap.Round)
        } else {
            drawCircle(edge, s * 0.05f, eye)
        }
        drawCircle(pink.copy(alpha = 0.55f), s * 0.07f, Offset(head.x + dir * headR * 0.62f, head.y + s * 0.13f))
    }
    drawPath(
        Path().apply {
            moveTo(head.x - s * 0.04f, head.y + s * 0.13f)
            lineTo(head.x + s * 0.04f, head.y + s * 0.13f)
            lineTo(head.x, head.y + s * 0.19f)
            close()
        },
        pink,
    )
    for (dir in listOf(-1f, 1f)) {
        drawOval(
            edge,
            Offset(c.x + dir * s * 0.24f - s * 0.10f, c.y + s * 0.56f),
            Size(s * 0.20f, s * 0.12f),
            style = Stroke(line * 0.8f),
        )
    }
}

/** 小人：圆头、头发、T 恤，一只手挥动。头部/手会露在条沿外。 */
internal fun DrawScope.drawPersonFigure(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
    detail: Float,
) {
    val skin = sceneFill(PersonSkin, p, muted)
    val hair = sceneFill(PersonHair, p, muted)
    val shirt = sceneFill(ShirtTeal, p, muted)
    val edge = sceneEdge(p, 0.30f + 0.35f * detail)
    val line = s * (0.02f + 0.01f * detail)
    val bob = sceneSway(phase, 0.8f) * s * 0.02f
    val c = Offset(center.x, center.y + bob)
    val head = Offset(c.x, c.y - s * 0.34f)
    val headR = s * 0.24f

    // 腿（被条挡住也保留，移动端整身可见）。
    drawLine(edge, Offset(c.x - s * 0.10f, c.y + s * 0.30f), Offset(c.x - s * 0.13f, c.y + s * 0.62f), line * 1.4f, StrokeCap.Round)
    drawLine(edge, Offset(c.x + s * 0.10f, c.y + s * 0.30f), Offset(c.x + s * 0.13f, c.y + s * 0.62f), line * 1.4f, StrokeCap.Round)
    // 身体（圆角矩形）。
    drawRoundRect(
        shirt,
        topLeft = Offset(c.x - s * 0.20f, c.y - s * 0.08f),
        size = Size(s * 0.40f, s * 0.42f),
        cornerRadius = CornerRadius(s * 0.10f),
    )
    drawRoundRect(
        edge,
        topLeft = Offset(c.x - s * 0.20f, c.y - s * 0.08f),
        size = Size(s * 0.40f, s * 0.42f),
        cornerRadius = CornerRadius(s * 0.10f),
        style = Stroke(line),
    )
    // 手：一只垂下，一只举起来挥。
    drawLine(skin, Offset(c.x - s * 0.18f, c.y + s * 0.02f), Offset(c.x - s * 0.30f, c.y + s * 0.28f), line * 1.6f, StrokeCap.Round)
    drawCircle(skin, s * 0.045f, Offset(c.x - s * 0.30f, c.y + s * 0.29f))
    val wave = sceneSway(phase, 0.1f) * 8f
    rotate(degrees = -28f + wave, pivot = Offset(c.x + s * 0.18f, c.y + s * 0.02f)) {
        drawLine(skin, Offset(c.x + s * 0.18f, c.y + s * 0.02f), Offset(c.x + s * 0.34f, c.y - s * 0.24f), line * 1.6f, StrokeCap.Round)
        drawCircle(skin, s * 0.05f, Offset(c.x + s * 0.34f, c.y - s * 0.25f))
    }
    // 头、头发（帽檐 + 一小撮呆毛）、脸。
    drawCircle(skin, headR, head)
    drawPath(
        Path().apply {
            moveTo(head.x - headR * 1.04f, head.y - headR * 0.05f)
            quadraticTo(head.x - headR * 0.9f, head.y - headR * 1.12f, head.x + headR * 0.2f, head.y - headR * 0.92f)
            quadraticTo(head.x + headR * 0.95f, head.y - headR * 0.82f, head.x + headR * 1.02f, head.y - headR * 0.02f)
            quadraticTo(head.x + headR * 0.4f, head.y - headR * 0.42f, head.x - headR * 0.2f, head.y - headR * 0.30f)
            close()
        },
        hair,
    )
    drawCircle(edge, headR, head, style = Stroke(line))
    for (dir in listOf(-1f, 1f)) {
        drawCircle(edge, s * 0.032f, Offset(head.x + dir * headR * 0.36f, head.y - headR * 0.02f))
    }
    drawArc(
        edge,
        startAngle = 20f,
        sweepAngle = 140f,
        useCenter = false,
        topLeft = Offset(head.x - headR * 0.30f, head.y + headR * 0.02f),
        size = Size(headR * 0.60f, headR * 0.50f),
        style = Stroke(line * 0.9f, cap = StrokeCap.Round),
    )
}

/** 气球：圆 + 高光 + 打结 + 飘线；[stringEnd] 非空时线一定牵到那只手上。 */
internal fun DrawScope.drawBalloon(
    p: ScenePalette,
    center: Offset,
    s: Float,
    phase: Float,
    muted: Float,
    stringEnd: Offset? = null,
) {
    val bob = sceneSway(phase, 0.25f) * s * 0.10f
    val c = Offset(center.x + sceneSway(phase, 0.9f) * s * 0.06f, center.y + bob)
    val fill = sceneFill(BalloonCoral, p, muted)
    val edge = sceneEdge(p, 0.25f + 0.35f * (1f - muted))
    val r = s * 0.46f
    drawCircle(fill, r, c)
    drawCircle(edge, r, c, style = Stroke(s * 0.055f))
    drawOval(
        Color.White.copy(alpha = 0.35f),
        Offset(c.x - r * 0.45f, c.y - r * 0.55f),
        Size(r * 0.30f, r * 0.42f),
    )
    // 打结 + 线（线垂到条沿/小人手边）。
    drawPath(
        Path().apply {
            moveTo(c.x - s * 0.06f, c.y + r)
            lineTo(c.x, c.y + r + s * 0.10f)
            lineTo(c.x + s * 0.06f, c.y + r)
            close()
        },
        fill,
    )
    val stringStart = Offset(c.x, c.y + r + s * 0.08f)
    val stringTip = stringEnd ?: Offset(c.x + s * 0.10f, c.y + r + s * 0.95f)
    drawPath(
        Path().apply {
            moveTo(stringStart.x, stringStart.y)
            quadraticTo(
                (stringStart.x + stringTip.x) * 0.5f + s * 0.12f,
                (stringStart.y + stringTip.y) * 0.5f,
                stringTip.x,
                stringTip.y,
            )
        },
        edge.copy(alpha = edge.alpha * 0.65f),
        style = Stroke(s * 0.035f, cap = StrokeCap.Round),
    )
}
