// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 皮肤的**契约门禁**：显示名与桌面/配置面同名、id 是持久化契约、九款色板必须真的不同。
 * ⚠️ 三端（本文件 ⇄ 桌面 Settings.kt ⇄ fcitx5 opi_config.h/主题目录）**同集合同顺序**
 * 的跨文件回归钉在 desktop 的 SettingsTest（那边一次读得到全部四份源文件）。
 */
class SkinOptionTest {

    @Test
    fun tenSkinsShareTheDesktopNames() {
        // 与桌面设置窗（desktop/.../Settings.kt 的 SkinOption）和 fcitx5 配置面
        // （opi_config.h 的 OpiSkin i18n 名）**逐字同名** —— 三处漂开就是三套词汇表。
        assertEquals(
            listOf("不改动", "浅色", "深色", "宣纸", "朱砂", "清爽", "湛蓝", "柠檬", "晚霞", "星光"),
            SkinOption.entries.map { it.label },
        )
    }

    @Test
    fun idsAreThePersistenceContract() {
        // id 落进 SharedPreferences：改 id = 老用户的皮肤偏好静默失效回默认。
        assertEquals(
            listOf("follow", "light", "dark", "paper", "cinnabar", "fresh", "azure", "lemon", "sunset", "starlight"),
            SkinOption.entries.map { it.id },
        )
        assertEquals(SkinOption.PAPER, SkinOption.fromId("paper"))
        assertEquals(SkinOption.STARLIGHT, SkinOption.fromId("starlight"))
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
        assertEquals(FreshPalette, SkinOption.FRESH.palette)
        assertEquals(AzurePalette, SkinOption.AZURE.palette)
        assertEquals(LemonPalette, SkinOption.LEMON.palette)
        assertEquals(SunsetPalette, SkinOption.SUNSET.palette)
        assertEquals(StarlightPalette, SkinOption.STARLIGHT.palette)
    }

    @Test
    fun skinsActuallyDiffer() {
        // 复制粘贴一个色板 = 换肤肉眼无变化，而这种错**不报任何错**（本仓的老毛病）。
        // 九款 = 除「不改动」（与浅色同板是**设计**，见 paletteMappingFollowsThePriorityDoc）。
        val nine = SkinOption.entries.drop(1)
        assertEquals("九款的键面底色必须两两不同", 9, nine.map { it.palette.keyBg }.toSet().size)
        assertEquals("九款的键面字色必须两两不同", 9, nine.map { it.palette.keyFg }.toSet().size)
        // 强调色：星光/暗光都是深底，但强调色必须不同（星光金色 vs 暗光雾蓝）——
        // 「新皮肤与既有款看不出区别」正是这轮要防的形态。
        assertTrue(
            "星光与暗光的强调色必须不同",
            SkinOption.STARLIGHT.palette.accent != SkinOption.DARK.palette.accent,
        )
        assertTrue(
            "星光与暗光的键面底色必须不同",
            SkinOption.STARLIGHT.palette.keyBg != SkinOption.DARK.palette.keyBg,
        )
    }
}
