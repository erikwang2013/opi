// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Test

class UiTextTest {
    @Test
    fun `english table loads and translates exact and prefixed labels`() {
        UiLang.init("en")
        assertEquals("Settings", UiLang.t("设置"))
        assertEquals("Downloaded · 8 MB", UiLang.t("已下载 · 8 MB"))
        assertEquals("中文", UiLang.t("中文"))
    }

    @Test
    fun `chinese mode leaves text untouched`() {
        UiLang.init("zh")
        assertEquals("设置", UiLang.t("设置"))
    }
}
