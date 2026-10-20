// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import xyz.erik.opi.pet.PetMood
import kotlin.random.Random

/**
 * 顶部宠物条十二形态的规格（2026-10-17 用户裁决「改成项目宠物的多形态：
 * 笑、欢乐、哭、沮丧、惊恐、愤怒，要是动态的、有色彩感」；2026-10-18 追加
 * 撒娇、卖萌、讨好、敲打、捂脸、吐舌头）。
 *
 * 2026-10-20 用户裁决：单形态轮播改成**随机同时显示多个不同形态（不能重复），
 * 数量 1..12** —— 本文件钉纯函数 [petStripSelection] 的三条契约。
 * 「画出来好不好看、十二张脸是不是真的不一样」由离屏渲染探针
 * `desktop/src/test/.../OpiPetFormsRenderProbeTest.kt` 出 PNG 人眼核 + 像素判据把住。
 */
class PetStripTest {

    @Test
    fun `twelve forms in the order the user asked`() {
        assertEquals(
            listOf(
                PetMood.IDLE, // 笑
                PetMood.WAITING, // 欢乐
                PetMood.CRYING, // 哭
                PetMood.PUZZLED, // 沮丧
                PetMood.TERRIFIED, // 惊恐
                PetMood.ANGRY, // 愤怒
                PetMood.COQUETTISH, // 撒娇
                PetMood.CUTE, // 卖萌
                PetMood.PLEASING, // 讨好
                PetMood.KNOCKING, // 敲打
                PetMood.FACE_PALM, // 捂脸
                PetMood.TONGUE_OUT, // 吐舌头
            ),
            PET_STRIP_FORMS,
        )
        assertEquals("十二形态", 12, PET_STRIP_FORMS.size)
        // 重复 = 有一个形态永远看不到；漏一个 = 点名的形态没上。
        assertEquals("形态不能重复", 12, PET_STRIP_FORMS.distinct().size)
    }

    @Test
    fun `selection is 1 to all forms and never repeats`() {
        val random = Random(42)
        repeat(1_000) {
            val picked = petStripSelection(random)
            assertTrue(
                "数量必须在 1..12：${picked.size}",
                picked.size in 1..PET_STRIP_FORMS.size,
            )
            assertEquals("同一组里形态不能重复", picked.size, picked.distinct().size)
            assertTrue("只能从十二形态里取", PET_STRIP_FORMS.containsAll(picked))
        }
    }

    @Test
    fun `fixed seed can fill all twelve forms`() {
        // 「数量从 1 到全部占满」：固定种子下必有一次抽满 12 枚，且恰好覆盖全部形态。
        val random = Random(20261020)
        val full = generateSequence { petStripSelection(random) }
            .take(1_000)
            .firstOrNull { it.size == PET_STRIP_FORMS.size }
        assertNotNull("固定种子下 1000 次抽取应至少出现一次全 12 形态", full)
        assertEquals("全 12 时恰好覆盖全部形态", PET_STRIP_FORMS.toSet(), full!!.toSet())
    }

    @Test
    fun `capacity clamps to the available forms`() {
        assertEquals("下限 1", 1, petStripSelection(Random(1), capacity = 1).size)
        assertEquals("0 也钳到 1", 1, petStripSelection(Random(1), capacity = 0).size)
        val all = petStripSelection(Random(1), capacity = 99)
        assertTrue("上限 = 12", all.size in 1..12)
    }

    @Test
    fun `new forms are appended to the cross-platform contract`() {
        // 鸿蒙端逐值镜像 PetMood（见 shared/pet/OpiPet.kt 的枚举注释与 harmony 的
        // OpiPet.ets）：已有五枚的序号不能动，新成员只能加在末尾。
        assertEquals(0, PetMood.IDLE.ordinal)
        assertEquals(1, PetMood.WAITING.ordinal)
        assertEquals(2, PetMood.PUZZLED.ordinal)
        assertEquals(3, PetMood.SLEEPY.ordinal)
        assertEquals(4, PetMood.DEGRADED.ordinal)
        assertEquals(5, PetMood.CRYING.ordinal)
        assertEquals(6, PetMood.TERRIFIED.ordinal)
        assertEquals(7, PetMood.ANGRY.ordinal)
        // 2026-10-18 追加六形态同样只占末尾。
        assertEquals(8, PetMood.COQUETTISH.ordinal)
        assertEquals(9, PetMood.CUTE.ordinal)
        assertEquals(10, PetMood.PLEASING.ordinal)
        assertEquals(11, PetMood.KNOCKING.ordinal)
        assertEquals(12, PetMood.FACE_PALM.ordinal)
        assertEquals(13, PetMood.TONGUE_OUT.ordinal)
    }
}
