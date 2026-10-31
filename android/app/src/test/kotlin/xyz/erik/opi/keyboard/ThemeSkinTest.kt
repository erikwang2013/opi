// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

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
                "动态主题就是皮肤：${theme.id} 不能落到 FOLLOW（配色会脱离主题）",
                SkinOption.FOLLOW,
                theme.defaultSkin(),
            )
        }
    }

    @Test
    fun solidSkinsAreTheElevenColorsWithFreshFirst() {
        assertEquals(11, SOLID_SKINS.size)
        assertEquals(SkinOption.FRESH, SOLID_SKINS.first())
        assertFalse(SkinOption.FOLLOW in SOLID_SKINS)
        assertEquals(SKIN_COUNT, SOLID_SKINS.size + SceneTheme.entries.size)
    }

    private companion object {
        /** 11 纯色 + 19 动态；面板按这个总数展示。 */
        const val SKIN_COUNT = 30
    }
}
