// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * 皮肤的**契约门禁**：显示名与桌面/配置面同名、id 是持久化契约、四款色板必须真的不同。
 */
class SkinOptionTest {

    @Test
    fun fiveSkinsShareTheDesktopNames() {
        // 与桌面设置窗（desktop/.../Settings.kt 的 SkinOption）和 fcitx5 配置面
        // （opi_config.h 的 OpiSkin i18n 名）**逐字同名** —— 三处漂开就是三套词汇表。
        assertEquals(
            listOf("不改动", "浅色", "深色", "宣纸", "朱砂"),
            SkinOption.entries.map { it.label },
        )
    }

    @Test
    fun idsAreThePersistenceContract() {
        // id 落进 SharedPreferences：改 id = 老用户的皮肤偏好静默失效回默认。
        assertEquals(
            listOf("follow", "light", "dark", "paper", "cinnabar"),
            SkinOption.entries.map { it.id },
        )
        assertEquals(SkinOption.PAPER, SkinOption.fromId("paper"))
    }

    @Test
    fun unknownOrMissingIdFallsBackToFollow() {
        assertEquals(SkinOption.FOLLOW, SkinOption.fromId(null))
        assertEquals(SkinOption.FOLLOW, SkinOption.fromId("RAINBOW"))
        assertEquals(SkinOption.FOLLOW, SkinOption.fromId(""))
    }

    @Test
    fun paletteMappingFollowsThePriorityDoc() {
        assertEquals(LightPalette, SkinOption.FOLLOW.palette)
        assertEquals(LightPalette, SkinOption.LIGHT.palette)
        assertEquals(DarkPalette, SkinOption.DARK.palette)
        assertEquals(PaperPalette, SkinOption.PAPER.palette)
        assertEquals(CinnabarPalette, SkinOption.CINNABAR.palette)
    }

    @Test
    fun skinsActuallyDiffer() {
        // 复制粘贴一个色板 = 换肤肉眼无变化，而这种错**不报任何错**（本仓的老毛病）。
        val bgs = listOf(SkinOption.LIGHT, SkinOption.DARK, SkinOption.PAPER, SkinOption.CINNABAR)
            .map { it.palette.keyBg }
        assertEquals("四款皮肤的键面底色必须两两不同", 4, bgs.toSet().size)
        val fgs = listOf(SkinOption.LIGHT, SkinOption.DARK, SkinOption.PAPER, SkinOption.CINNABAR)
            .map { it.palette.keyFg }
        assertEquals("四款皮肤的键面字色必须两两不同", 4, fgs.toSet().size)
    }
}
