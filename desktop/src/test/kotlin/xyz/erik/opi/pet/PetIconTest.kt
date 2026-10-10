// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 窗口图标（小欧 Painter）的**纯**门禁（真渲染到 ImageBitmap，无窗口、无进程）。
//!
//! 为什么单列：icon 这类失败全是静默的 —— 忘挂（各造窗处的 Window(icon=…) 是
//! 编译层看不出来的遗漏，靠人核）、挂了但画不出（intrinsicSize 忘给 ⇒ CMP 栅格化
//! 成空白图，不报错不崩）。这里把「画得出」这一半钉住；「挂没挂」在各文件的
//! Window( 调用处有注释标记，代码评审时对照。

package xyz.erik.opi.pet

import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Canvas
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.drawscope.CanvasDrawScope
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.LayoutDirection
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class PetIconTest {

    @Test
    fun `intrinsic size is specified`() {
        // CMP 桌面（Painter.toAwtImage）按 intrinsicSize 栅格化窗口图标；留着默认的
        // Size.Unspecified 会渲成空白图 —— 表现是「图标装好了但是空的」，不报错。
        assertEquals(Size(256f, 256f), OpiPetWindowIcon.intrinsicSize)
    }

    @Test
    fun `painter draws non-blank pixels at every icon size`() {
        // 16（任务栏小档）/64/256（大档）都要画得出东西：断言至少一个非透明像素
        // （小欧有白色键帽面等不透明填充，空图/异常裁剪都会让整张全透明）。
        // 画法 = CMP 自己栅格化图标时用的同一条路（CanvasDrawScope + Painter.draw）。
        for (n in listOf(16, 64, 256)) {
            val bmp = ImageBitmap(n, n)
            CanvasDrawScope().draw(
                Density(1f),
                LayoutDirection.Ltr,
                Canvas(bmp),
                Size(n.toFloat(), n.toFloat()),
            ) {
                with(OpiPetWindowIcon) { draw(Size(n.toFloat(), n.toFloat())) }
            }
            val px = IntArray(n * n)
            bmp.readPixels(px)
            assertTrue("$n px 下全是透明像素（形状没画出来/被裁光）", px.any { (it ushr 24) != 0 })
        }
    }
}
