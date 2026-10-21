// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「候选字号」门禁（2026-10-14）：三档映射、classicui.conf 的**只动 Font** 合并、
//! 写入（含建目录/建文件）、落点推导、以及 `CandidateFont` 键在设置 ini 的三处往返。
//! 盯的是三处静默失效：
//! ① 路径错 = 写了个 fcitx5 不读的文件（推导与 addon 配置共用一处，这里钉「同目录」）；
//! ② merge 写成整文件重写 = 把用户的 Theme/注释抹掉（用「逐字保留」钉住，且夹具特意
//!   放了含 "Font" 子串的 `MenuFont` —— 键判定写成 contains 就会误伤）；
//! ③ 档位 ↔ pango 描述漂 = 界面上点「中」实际写成别的字号（映射表是唯一一份）。

package xyz.erik.opi.candidate

import java.nio.file.Files
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class ClassicUiFontTest {

    /** Linux 落点的期望串是 `/` 分隔；`Paths.get` 按**运行平台**拼路径，Windows 主机
     *  上跑这条时 `toString()` 会带 `\`。比较前统一成 `/` —— 判的是推导，不是分隔符。 */
    private fun posix(p: java.nio.file.Path): String = p.toString().replace('\\', '/')

    @Test
    fun `path follows xdg then home, same derivation as the addon config`() {
        assertEquals(
            "/x/cfg/fcitx5/conf/classicui.conf",
            posix(classicUiPathFor(xdgConfigHome = "/x/cfg", home = "/home/u")),
        )
        // XDG_CONFIG_HOME 缺省 ⇒ $HOME/.config
        assertEquals(
            "/home/u/.config/fcitx5/conf/classicui.conf",
            posix(classicUiPathFor(xdgConfigHome = null, home = "/home/u")),
        )
        // 空串等同未设置（与 settingsPathFor 同一口径）
        assertEquals(
            "/home/u/.config/fcitx5/conf/classicui.conf",
            posix(classicUiPathFor(xdgConfigHome = "", home = "/home/u")),
        )
        // 与 addon 配置**同目录**（两处共用 fcitxConfDir；这条防以后有人各写一份推导）
        assertEquals(
            settingsPathFor(false, "/x/cfg", "/home/u", null).parent,
            classicUiPathFor("/x/cfg", "/home/u").parent,
        )
    }

    @Test
    fun `merge replaces only the font line and keeps everything else verbatim`() {
        // 夹具 = 真机 classicui.conf 的形状（Theme 三行）+ 手写注释 + 空行 + MenuFont
        val before = """
            |Theme=opi-paper
            |# 手写注释：别删
            |DarkTheme=opi-paper
            |
            |UseDarkTheme=True
            |Font=Sans 10
            |MenuFont=Sans Bold 14
            |""".trimMargin()
        val after = mergeClassicUiFont(before, "Sans 14")
        assertEquals(
            """
            |Theme=opi-paper
            |# 手写注释：别删
            |DarkTheme=opi-paper
            |
            |UseDarkTheme=True
            |Font=Sans 14
            |MenuFont=Sans Bold 14
            |""".trimMargin(),
            after,
        )
        // 反向正控：含 "Font" 子串的邻居键不许被本函数碰（键判定必须是等于，不是包含）
        assertTrue("MenuFont 被误伤了：\n$after", after.contains("MenuFont=Sans Bold 14"))
    }

    @Test
    fun `merge inserts when missing and never mistakes a comment for the key`() {
        val before = "Theme=opi-paper\n# Font=Sans 9 这是注释，不是键\n"
        assertEquals(
            "Theme=opi-paper\n# Font=Sans 9 这是注释，不是键\nFont=Sans 12\n",
            mergeClassicUiFont(before, "Sans 12"),
        )
    }

    @Test
    fun `inline comment and spacing after the key are recognised as the font line`() {
        assertEquals("Font=Sans 12\n", mergeClassicUiFont("Font = Sans 10 # 手调的\n", "Sans 12"))
    }

    @Test
    fun `duplicate font lines collapse to one`() {
        // 两条同键在 fcitx5 里读哪条是实现细节 —— 留两条是给我们自己埋的坑（见实现注释）
        assertEquals(
            "Font=Sans 12\nTheme=x\n",
            mergeClassicUiFont("Font=Sans 10\nTheme=x\nFont=Sans 9\n", "Sans 12"),
        )
    }

    @Test
    fun `merging twice changes nothing more (no blank line growth)`() {
        // 文件以 \n 结尾时按行切会多出一个空段；把它当独立行拼回去 = 每保存一次末尾多一行
        // 空行（首版就是这么写的，真机 opi_fcitx5.conf 末尾也躺着这样一行）。幂等是判据。
        val once = mergeClassicUiFont("Theme=x\nFont=Sans 10\n", "Sans 14")
        assertEquals(once, mergeClassicUiFont(once, "Sans 14"))
        assertEquals("Theme=x\nFont=Sans 14\n", once)
    }

    @Test
    fun `write creates the file (and its directory) then only touches the font line`() {
        val dir = Files.createTempDirectory("opi-classicui-test")
        try {
            val p = dir.resolve("conf/classicui.conf") // 目录也不存在：要连目录一起建
            assertEquals("Font=Sans 12\n", writeClassicUiFont(p, "中"))
            assertEquals("Font=Sans 12\n", Files.readString(p))
            // 第二次换档：只有 Font 行变，别的行（含手写注释）原样
            Files.writeString(p, "Font=Sans 12\n# 注释\nTheme=opi-paper\n")
            writeClassicUiFont(p, "大")
            assertEquals("Font=Sans 14\n# 注释\nTheme=opi-paper\n", Files.readString(p))
        } finally {
            dir.toFile().deleteRecursively()
        }
    }

    @Test
    fun `three choices map to pango descriptions and unknown falls back to the middle`() {
        assertEquals("Sans 10", classicUiFontDesc("小"))
        assertEquals("Sans 12", classicUiFontDesc("中"))
        assertEquals("Sans 14", classicUiFontDesc("大"))
        assertEquals("认不得的档位名按中档，不抛", "Sans 12", classicUiFontDesc("特大"))
        assertEquals(listOf("小", "中", "大"), CLASSIC_UI_FONT_NAMES)
    }

    @Test
    fun `candidate font round trips through the settings ini and falls back to the middle`() {
        val s = OpSettings(candidateFont = "大")
        assertEquals(s, OpSettings.fromIni(s.toIni()))
        assertTrue("toIni 缺键行：\n${OpSettings().toIni()}", OpSettings().toIni().contains("CandidateFont=中\n"))
        assertEquals("中", OpSettings.fromIni("").candidateFont) // 缺键 → 中
        assertEquals("中", OpSettings.fromIni("CandidateFont=特大\n").candidateFont) // 认不得 → 中
        assertEquals("小", OpSettings.fromIni("CandidateFont=小\n").candidateFont)
        // 三处里的第三处（mergeIni）：本窗管理这个键 —— 改回默认值、别的行不动
        val merged = mergeIni("CandidateFont=大\nDisplayName=我的\n", OpSettings())
        assertTrue("merge 没把键改回去：\n$merged", merged.contains("CandidateFont=中\n"))
        assertTrue("merge 抹了无关行：\n$merged", merged.contains("DisplayName=我的"))
    }
}
