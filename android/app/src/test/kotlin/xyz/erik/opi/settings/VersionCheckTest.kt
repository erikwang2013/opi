// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 版本检测的**纯函数**契约（2026-10-22）：解析 `version.json` / 首页 JSON-LD、
 * 语义化数字比较。不联网 —— 网络出口 `fetchLatestVersion()` 只能在真机上验。
 */
class VersionCheckTest {

    @Test
    fun parsesVersionJson() {
        assertEquals(
            "1.22.0",
            parseVersionJson("""{"version":"1.22.0","download":"https://opi.erik.xyz/download"}"""),
        )
        assertEquals("1.22.0", parseVersionJson("""{ "version" : " 1.22.0 " }"""))
        assertNull("没有 version 字段", parseVersionJson("""{"download":"x"}"""))
        assertNull("空串", parseVersionJson(""))
    }

    @Test
    fun fallsBackToJsonLdSoftwareVersion() {
        val html = """
            <html><head>
            <script type="application/ld+json">
            {"@context":"https://schema.org","@type":"SoftwareApplication",
             "name":"opi","softwareVersion":"1.22.0"}
            </script></head></html>
        """.trimIndent()
        assertEquals("1.22.0", parseJsonLdVersion(html))
        assertNull("没有 softwareVersion", parseJsonLdVersion("<html></html>"))
    }

    @Test
    fun comparesVersionsNumericallySegmentBySegment() {
        // 字符串比较会把 1.21.10 判成更小 —— 这条是数字比较的判据。
        assertEquals(1, compareVersions("1.21.10", "1.21.2"))
        assertEquals(-1, compareVersions("1.21.2", "1.21.10"))
        assertEquals(0, compareVersions("1.22", "1.22.0"))
        assertEquals(0, compareVersions("v1.22.0", "1.22.0"))
        assertEquals(1, compareVersions("2.0", "1.99.99"))
    }

    @Test
    fun invalidVersionsNeverLookNewer() {
        assertNull(compareVersions("", "1.22.0"))
        assertNull(compareVersions("1.22.0-beta", "1.22.0"))
        assertNull(compareVersions("latest", "1.22.0"))
        assertFalse(isNewerVersion("latest", "1.22.0"))
        assertFalse(isNewerVersion("1.22.0", "1.22.0"))
        assertTrue(isNewerVersion("1.22.1", "1.22.0"))
    }
}
