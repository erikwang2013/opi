// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Test
import xyz.erik.opi.scene.SceneTheme

class ThemeSkinTest {
    @Test
    fun everyThemeCarriesItsOwnPalette() {
        for (theme in SceneTheme.entries) {
            assertNotEquals(
                "动态主题就是皮肤：${theme.id} 不能落到 KEEP（配色会脱离主题）",
                SkinOption.KEEP,
                theme.defaultSkin(),
            )
        }
    }

    @Test
    fun solidSkinsAreTheElevenColorsWithFreshFirst() {
        assertEquals(11, SOLID_SKINS.size)
        assertEquals(SkinOption.FRESH, SOLID_SKINS.first())
        assertFalse(SkinOption.KEEP in SOLID_SKINS)
        assertEquals(30, SOLID_SKINS.size + SceneTheme.entries.size)
    }

    @Test
    fun logoCycleWalksSolidsThenThemesAndWraps() {
        assertEquals(
            null to SOLID_SKINS[1],
            nextSkinChoice(null, SOLID_SKINS.first()),
        )
        val firstTheme = SceneTheme.entries.first()
        assertEquals(
            firstTheme to firstTheme.defaultSkin(),
            nextSkinChoice(null, SOLID_SKINS.last()),
        )
        assertEquals(
            null to SOLID_SKINS.first(),
            nextSkinChoice(SceneTheme.entries.last(), SkinOption.PINE),
        )
    }
}
