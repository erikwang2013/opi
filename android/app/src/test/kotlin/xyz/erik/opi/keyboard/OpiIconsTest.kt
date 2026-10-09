// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.VectorGroup
import androidx.compose.ui.graphics.vector.VectorNode
import androidx.compose.ui.graphics.vector.VectorPath
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 工具条图标的**形态门禁**：四枚一律线稿（`fill = null` + 有描边）。
 *
 * 为什么钉这个：首版 `stroke()` 辅助里写了 `fill = SolidColor(fill)`，于是**闭环
 * 子路径全被填成实心** —— 笑脸是一枚实心圆饼、键盘是一块实心方块（2026-10-09
 * 渲染复核抓到；纯数值/存在性检查看不见这种形态，正是本仓记过的那个盲区）。
 * 图标颜色由 `Icon(tint=…)` 统一染色，任何一条路径自带填充都是 bug。
 */
class OpiIconsTest {

    private fun pathsOf(v: ImageVector): List<VectorPath> {
        val out = mutableListOf<VectorPath>()
        fun walk(group: VectorGroup) {
            // VectorGroup 实现 Iterable（children 是 private，迭代器才是公开面 ——
            // 实测踩过：写 group.children 编译不过）。
            for (child: VectorNode in group) {
                when (child) {
                    is VectorPath -> out += child
                    is VectorGroup -> walk(child)
                    else -> Unit // 其它节点（裁剪/组等）本文件不用，也不下探
                }
            }
        }
        walk(v.root)
        return out
    }

    @Test
    fun allIconsAreStrokeOnly() {
        val icons = mapOf(
            "Emoji" to OpiIcons.Emoji,
            "InputMode" to OpiIcons.InputMode,
            "KeyboardMode" to OpiIcons.KeyboardMode,
            "Palette" to OpiIcons.Palette,
            "Close" to OpiIcons.Close,
        )
        for ((name, icon) in icons) {
            val paths = pathsOf(icon)
            assertTrue("$name 至少要有一条路径", paths.isNotEmpty())
            for ((i, path) in paths.withIndex()) {
                // 有填充 = 闭环会画成实心块（笑脸变圆饼那种）
                assertNull("$name 第 $i 条路径不该有填充", path.fill)
                assertNotNull("$name 第 $i 条路径该有描边", path.stroke)
            }
        }
    }
}
