// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.pet

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * 钉住贴纸文件名契约：十二个可挑选形态一一对应、SLEEPY/DEGRADED 不在其中。
 * 期望表**手写**（不按被测规则再生成一遍），这样改枚举名/规则都会红。
 */
class PetStickerAssetsTest {

    private val expected = mapOf(
        PetMood.IDLE to "idle.png",
        PetMood.WAITING to "waiting.png",
        PetMood.PUZZLED to "puzzled.png",
        PetMood.CRYING to "crying.png",
        PetMood.TERRIFIED to "terrified.png",
        PetMood.ANGRY to "angry.png",
        PetMood.COQUETTISH to "coquettish.png",
        PetMood.CUTE to "cute.png",
        PetMood.PLEASING to "pleasing.png",
        PetMood.KNOCKING to "knocking.png",
        PetMood.FACE_PALM to "face-palm.png",
        PetMood.TONGUE_OUT to "tongue-out.png",
    )

    @Test
    fun `the twelve sticker moods map to their file names`() {
        assertEquals(12, expected.size)
        // 除 SLEEPY / DEGRADED 外**全部**枚举成员都在这张表里（新增形态会红）。
        assertEquals(
            PetMood.entries.toSet() - setOf(PetMood.SLEEPY, PetMood.DEGRADED),
            expected.keys,
        )
        for ((mood, file) in expected) {
            assertEquals(file, PetStickerAssets.stickerFile(mood))
            assertEquals(file.removeSuffix(".png") + ".gif", PetStickerAssets.gifFile(mood))
        }
    }
}
