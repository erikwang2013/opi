// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 更新检测（`UpdateCheck.kt`）的纯函数门禁：版本比较、两个数据源的解析、失败语义、
//! 「关于」页文案。网络本身（`httpGet`）不测 —— 它只在真机上有意义；这里注入
//! `fetch`，把「拿到的字节 → 用户看到的结论」整条链路钉死。

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class UpdateCheckTest {

    // ---------- 版本比较 ----------

    @Test
    fun `version comparison is numeric, not lexical`() {
        assertTrue("1.20.2 < 1.20.3", compareVersions("1.20.2", "1.20.3") < 0)
        // 字符串比较会把 1.9.10 排到 1.9.9 前面 —— 老用户永远收不到提示。
        assertTrue("1.9.9 < 1.9.10", compareVersions("1.9.9", "1.9.10") < 0)
        assertTrue("1.20 < 1.20.1", compareVersions("1.20", "1.20.1") < 0)
        assertEquals("缺的段按 0", 0, compareVersions("1.20", "1.20.0"))
        assertEquals("v 前缀不算", 0, compareVersions("v1.20.2", "1.20.2"))
        assertTrue("2.0 > 1.99.99", compareVersions("2.0", "1.99.99") > 0)
    }

    // ---------- 数据源 1：version.json ----------

    @Test
    fun `version json parses the deployed shape`() {
        val p = parseVersionPoint(
            """{"download":"https://opi.erik.xyz/download","version":"1.20.3"}""",
        )
        assertEquals("1.20.3", p?.version)
        assertEquals("https://opi.erik.xyz/download", p?.download)
        // v 前缀、latest 别名、缺 download 都认。
        assertEquals("1.20.3", parseVersionPoint("""{"latest":"v1.20.3"}""")?.version)
        assertNull(parseVersionPoint("""{"latest":"v1.20.3"}""")?.download)
    }

    @Test
    fun `bad version json is rejected, not treated as up to date`() {
        assertNull(parseVersionPoint("not json"))
        assertNull(parseVersionPoint("""{"version":"newest"}"""))
        assertNull(parseVersionPoint("""{"version":""}"""))
        // download 必须是 https（防止站点被改出一个 http 明文下载口）。
        assertNull(
            parseVersionPoint("""{"download":"http://evil.example/x","version":"1.20.3"}""")
                ?.download,
        )
    }

    // ---------- 数据源 2：首页 JSON-LD ----------

    /** 线上 index.html 的 JSON-LD 片段（键序与真实一致）。 */
    private val realHead =
        """<script type="application/ld+json">{"@context":"https://schema.org","@graph":""" +
            """[{"@type":"SoftwareApplication","name":"小欧输入法","softwareVersion":"1.20.2",""" +
            """"downloadUrl":"https://github.com/erikwang2013/opi/releases/tag/v1.20.2"}]}</script>"""

    @Test
    fun `site index prefers the structured version`() {
        assertEquals("1.20.2", parseSiteVersion(realHead))
        // 下载文件名兜底（JSON-LD 被改坏/没更新时）。
        assertEquals(
            "1.21.0",
            parseSiteVersion("""<a href="download/opi-1.21.0-windows-x64-setup.exe">"""),
        )
        assertNull("没有任何版本痕迹 → null（调用方报失败，不猜）", parseSiteVersion("<html>"))
    }

    // ---------- 检测流程（注入 fetch，不碰网络） ----------

    @Test
    fun `check prefers version json and links the given download page`() {
        val seen = mutableListOf<String>()
        val result = checkForUpdate("1.20.2") { url ->
            seen += url
            if (url == UPDATE_VERSION_URL) """{"version":"1.20.3"}""" else null
        }
        assertEquals(UpdateCheck.Available("1.20.3", UPDATE_DOWNLOAD_URL), result)
        assertEquals("有了 version.json 就不再抓首页", listOf(UPDATE_VERSION_URL), seen)
    }

    @Test
    fun `check falls back to the index when version json is absent`() {
        val result = checkForUpdate("1.20.1") { url ->
            when (url) {
                UPDATE_VERSION_URL -> null
                UPDATE_INDEX_URL -> realHead
                else -> error("不该抓 $url")
            }
        }
        assertEquals(UpdateCheck.Available("1.20.2", UPDATE_DOWNLOAD_URL), result)
    }

    @Test
    fun `same version means up to date`() {
        val result = checkForUpdate(OPI_VERSION) { url ->
            if (url == UPDATE_VERSION_URL) """{"version":"$OPI_VERSION"}""" else null
        }
        assertEquals(UpdateCheck.UpToDate, result)
    }

    @Test
    fun `offline and unparseable sites are failures, never silent up-to-date`() {
        assertEquals(
            UpdateCheck.Failed("连不上更新服务器（网络不可用？）"),
            checkForUpdate("1.20.2") { null },
        )
        assertTrue(
            "站点没有版本信息必须是 Failed（静默当最新 = 更新功能假绿）",
            checkForUpdate("1.20.2") { "<html>maintenance</html>" } is UpdateCheck.Failed,
        )
    }

    // ---------- 「关于」页文案 ----------

    @Test
    fun `about label tells the user what happened`() {
        assertTrue(updateLabel(UpdateCheck.UpToDate, false).contains("已是最新版本"))
        assertTrue(
            updateLabel(UpdateCheck.Available("1.21.0", UPDATE_DOWNLOAD_URL), false)
                .contains("发现新版本 v1.21.0"),
        )
        assertTrue(
            updateLabel(UpdateCheck.Failed("断网"), false).contains("检测失败：断网"),
        )
        assertEquals("正在检测更新…", updateLabel(null, true))
        assertEquals("尚未检测", updateLabel(null, false))
        assertFalse(updateLabel(null, false).contains("最新"))
    }
}
