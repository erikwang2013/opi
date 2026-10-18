// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.pet

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 共享情绪动效的纯逻辑门禁（Android 宠物条 / 桌面设置页 / 贴纸格共用）。
 *
 * 只钉「十二形态都不是死的、两个输入状态保持静止、相位确实驱动变换」；
 * 具体幅度好不好看由离屏渲染探针负责人眼核。
 */
class PetMotionTest {

    private val forms = PetMood.entries -
        setOf(PetMood.SLEEPY, PetMood.DEGRADED)

    @Test
    fun `all twelve forms move somewhere across the cycle`() {
        assertEquals(12, forms.size)
        for (mood in forms) {
            val specs = (0..7).map { petMotionSpec(mood, it / 8f) }
            val moves = specs.any {
                it.translationXDp != 0f || it.translationYDp != 0f ||
                    it.scaleX != 1f || it.scaleY != 1f || it.rotationZ != 0f
            }
            assertTrue("$mood 没有动效", moves)
        }
    }

    @Test
    fun `input states stay still`() {
        for (mood in listOf(PetMood.SLEEPY, PetMood.DEGRADED)) {
            for (step in 0..7) assertEquals(PetMotionSpec(), petMotionSpec(mood, step / 8f))
        }
    }

    @Test
    fun `phase changes the transform`() {
        // 相位 0 与 1 是同一圈的两端；中间相位必须和端点不同。
        assertNotEquals(
            petMotionSpec(PetMood.IDLE, 0f),
            petMotionSpec(PetMood.IDLE, 0.25f),
        )
        assertEquals(
            petMotionSpec(PetMood.IDLE, 0f).translationYDp,
            petMotionSpec(PetMood.IDLE, 1f).translationYDp,
            1e-5f,
        )
    }
}
