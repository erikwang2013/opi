// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import org.junit.Assert.assertEquals
import org.junit.Test
import xyz.erik.opi.pet.PetMood

/**
 * 顶部宠物条六形态的规格（2026-10-17 用户裁决「改成项目宠物的多形态：
 * 笑、欢乐、哭、沮丧、惊恐、愤怒，要是动态的、有色彩感」）。
 *
 * 这里只钉**纯逻辑**（轮播次序 / 包裹）；「画出来好不好看、六张脸是不是真的不一样」
 * 由离屏渲染探针 `desktop/src/test/.../OpiPetFormsRenderProbeTest.kt` 出 PNG 人眼核 +
 * 像素判据把住。
 */
class PetStripTest {

    @Test
    fun `six forms in the order the user asked`() {
        assertEquals(
            listOf(
                PetMood.IDLE, // 笑
                PetMood.WAITING, // 欢乐
                PetMood.CRYING, // 哭
                PetMood.PUZZLED, // 沮丧
                PetMood.TERRIFIED, // 惊恐
                PetMood.ANGRY, // 愤怒
            ),
            PET_STRIP_FORMS,
        )
        assertEquals("六形态（笑/欢乐/哭/沮丧/惊恐/愤怒）", 6, PET_STRIP_FORMS.size)
        // 重复 = 六个里有一个永远看不到；漏一个 = 用户点名的形态没上。
        assertEquals("形态不能重复", 6, PET_STRIP_FORMS.distinct().size)
    }

    @Test
    fun `cycle wraps in both directions`() {
        assertEquals(PetMood.IDLE, petStripFormAt(0))
        assertEquals(PetMood.ANGRY, petStripFormAt(5))
        assertEquals(PetMood.IDLE, petStripFormAt(6))
        assertEquals(PetMood.WAITING, petStripFormAt(13))
        // 防御性包裹：轮播实现只加不减，但纯函数该自洽（负数不崩、落回同一圈）。
        assertEquals(PetMood.ANGRY, petStripFormAt(-1))
        assertEquals(PetMood.WAITING, petStripFormAt(-5))
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
    }
}
