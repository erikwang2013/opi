// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import xyz.erik.opi.pet.PetMood
import xyz.erik.opi.pet.PetStickerAssets

/**
 * 「自有」= 小欧形象，2026-10-18 用户裁决后点击**插入 PNG 图片**（不再是 Unicode emoji）。
 *
 * 文件名契约的唯一真源是共享的 [PetStickerAssets]；本测试钉住 Android 侧两件事：
 * 12 个形态齐全（状态枚举不混进来）、assets 路径 = `pet/` + 契约文件名。
 */
class PetEmojiTest {

    @Test
    fun twelveMoodsInEnumOrder() {
        val expected = PetMood.entries.filter {
            it != PetMood.SLEEPY && it != PetMood.DEGRADED
        }
        assertEquals(12, expected.size)
        assertEquals("形态顺序 = 枚举序（跨端契约）", expected, PetEmoji.forms.map { it.mood })
    }

    @Test
    fun sleepyAndDegradedAreInputStatesNotForms() {
        // 不记事 / 引擎降级是状态，不是用户可挑的表情形态 —— 没有贴纸，也不进自有网格。
        val moods = PetEmoji.forms.map { it.mood }
        assertTrue(PetMood.SLEEPY !in moods)
        assertTrue(PetMood.DEGRADED !in moods)
    }

    @Test
    fun everyFormHasALabel() {
        for (f in PetEmoji.forms) {
            assertTrue("形态 ${f.mood} 缺无障碍标签", f.label.isNotEmpty())
        }
    }

    /** 图片插不进去时上屏 `[小欧·标签]`；标签必须取共享映射的中文名。 */
    @Test
    fun labelLookupMatchesFormLabels() {
        assertEquals("笑", PetEmoji.label(PetMood.IDLE))
        for (f in PetEmoji.forms) assertEquals(f.label, PetEmoji.label(f.mood))
    }

    /** 资产契约：小写 + 下划线换连字符；assets 路径只多一个 `pet/` 前缀。 */
    @Test
    fun stickerFileNamesFollowSharedContract() {
        assertEquals("idle.png", PetStickerAssets.stickerFile(PetMood.IDLE))
        assertEquals("face-palm.png", PetStickerAssets.stickerFile(PetMood.FACE_PALM))
        assertEquals("tongue-out.png", PetStickerAssets.stickerFile(PetMood.TONGUE_OUT))
        val files = PetEmoji.forms.map { PetStickerAssets.stickerFile(it.mood) }
        assertEquals("12 个文件名必须互不相同", 12, files.toSet().size)
        for (f in PetEmoji.forms) {
            assertEquals("pet/" + PetStickerAssets.stickerFile(f.mood), PetEmoji.assetPath(f.mood))
            assertEquals("pet-gif/" + PetStickerAssets.gifFile(f.mood), PetEmoji.gifAssetPath(f.mood))
        }
    }
}
