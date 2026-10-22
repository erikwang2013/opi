// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 紧凑字号判据门禁（[compactKeys]）：常规竖屏不变、横屏变矮、单手变窄时各降一档。
 */
class KeyboardSizingTest {

    @Test
    fun regularPortraitPhoneKeepsFonts() {
        // 1080×2400@2.625：键区约 411dp 宽、220dp 高。
        assertFalse(compactKeys(widthDp = 411f, heightDp = 220f))
        assertFalse(compactKeys(widthDp = 320f, heightDp = 200f))
    }

    @Test
    fun landscapeIsCompact() {
        // 横屏键区约 411dp 宽、140dp 高。
        assertTrue(compactKeys(widthDp = 411f, heightDp = 140f))
    }

    @Test
    fun oneHandedNarrowIsCompact() {
        // 单手：320dp × 0.76 ≈ 243dp；390dp × 0.76 ≈ 296dp。
        assertTrue(compactKeys(widthDp = 243f, heightDp = 220f))
        assertTrue(compactKeys(widthDp = 296f, heightDp = 220f))
        // 430dp 单手 ≈ 327dp，仍有余量。
        assertFalse(compactKeys(widthDp = 327f, heightDp = 220f))
    }
}
