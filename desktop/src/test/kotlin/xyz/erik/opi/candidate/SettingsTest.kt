// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「属性设置」的配置 IO 门禁（纯 JVM，无 UI、无 JNA）。
//!
//! 盯的是两处**静默失效**的地方：键名（三处逐字对应的契约）与落点路径
//! （「本窗写哪个文件」⇄「IME 侧读哪个文件」的接缝）。

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class SettingsTest {

    @Test
    fun `round trip keeps every switch`() {
        val s = OpSettings(learn = false, chinesePunct = true, fuzzy = false, jianpin = true)
        assertEquals(s, OpSettings.fromIni(s.toIni()))
    }

    @Test
    fun `defaults are all on`() {
        // 默认值 = 引擎既有行为。空文件/没配过 ⇒ 全开（不是全关！）。
        assertEquals(OpSettings(), OpSettings.fromIni(""))
        assertEquals(OpSettings(), OpSettings.fromIni("# 只有注释\n\n"))
    }

    @Test
    fun `key names are pinned`() {
        // 键名是**三处逐字对应**的契约（本文件 ⇄ opi_config.h ⇄ tsf settings.rs）。
        // 钉整行而不是 contains("Learn")：改名/改大小写/改空格都该在这里红。
        val ini = OpSettings().toIni()
        for (line in listOf("Learn=True", "ChinesePunct=True", "Fuzzy=True", "Jianpin=True")) {
            assertTrue("缺少键行：$line\n$ini", ini.contains("$line\n"))
        }
    }

    @Test
    fun `bool spelling is True or False`() {
        val ini = OpSettings(fuzzy = false).toIni()
        assertTrue(ini.contains("Fuzzy=False\n"))
        assertFalse(ini.contains("Fuzzy=true")) // 全小写不写出去（读侧认，但写侧统一）
    }

    @Test
    fun `merge keeps unknown keys and comments`() {
        // 2026-10-09 起保存走 mergeIni：fcitx5 侧新增的选项（ShiftToggle/Skin）与
        // 手写注释**不能被本窗抹掉** —— 此前是整文件重写，抹一次就是「设置窗
        // 保存后 Shift 切换静默失效」。
        val existing = """
            # 手写的注释
            Learn=True
            ShiftToggle=False
            Skin=浅色
            # 保底
        """.trimIndent() + "\n"
        val merged = mergeIni(existing, OpSettings(learn = false))
        // JUnit4 的两个参数是 (message, condition) —— 与 Kotlin 测试框架的习惯相反，
        // 写反了是**编译错**（Boolean 塞不进 String 位），实测抓过。
        assertTrue(merged, merged.contains("# 手写的注释\n"))
        assertTrue(merged, merged.contains("ShiftToggle=False\n"))
        assertTrue(merged, merged.contains("Skin=浅色\n"))
        assertTrue(merged, merged.contains("Learn=False\n"))
        assertFalse(merged, merged.contains("Learn=True"))
        // 没出现过的管理键要补齐（旧文件里缺键 = 用默认值写回去）
        assertTrue(merged, merged.contains("ChinesePunct=True\n"))
    }

    @Test
    fun `merge output round trips through the reader`() {
        // 行内注释、未知键、行内空格都掺上：合并结果必须仍被 fromIni 正确解析。
        val existing = "Learn=True\nFuzzy=False # 关掉\n未知键=whatever\n"
        val s = OpSettings(learn = false, chinesePunct = false, fuzzy = true, jianpin = true)
        val merged = mergeIni(existing, s)
        assertEquals(s, OpSettings.fromIni(merged))
        assertTrue(merged, merged.contains("未知键=whatever\n"))
    }

    @Test
    fun `tolerant reader`() {
        // 手改的配置：小写值、行内注释、未知键、多余空格 —— 都该认/该忽略，不该抛。
        val s = OpSettings.fromIni(
            """
            # 手改的
            Fuzzy = false   # 关模糊音
            Jianpin=OFF
            Learn=1
            未来才有的键=whatever
            """.trimIndent(),
        )
        assertEquals(false, s.fuzzy)
        assertEquals(false, s.jianpin)
        assertEquals(true, s.learn)
        assertEquals(true, s.chinesePunct) // 缺键 ⇒ 默认开
    }

    @Test
    fun `garbage value falls back to default`() {
        val s = OpSettings.fromIni("Fuzzy=maybe\nJianpin=\n")
        assertTrue(s.fuzzy)
        assertTrue(s.jianpin)
    }

    @Test
    fun `linux path is the fcitx5 addon config`() {
        // 对侧读侧 = opi_config.h 的 readAsIni(PkgConfig, "conf/opi_fcitx5.conf")。
        // 路径错 = 设置窗写了个 fcitx5 不读的文件（静默不生效），故钉死。
        assertEquals(
            "/x/cfg/fcitx5/conf/opi_fcitx5.conf",
            settingsPathFor(windows = false, xdgConfigHome = "/x/cfg", home = "/home/u", localAppData = null).toString(),
        )
        // XDG_CONFIG_HOME 缺省 ⇒ $HOME/.config
        assertEquals(
            "/home/u/.config/fcitx5/conf/opi_fcitx5.conf",
            settingsPathFor(windows = false, xdgConfigHome = null, home = "/home/u", localAppData = null).toString(),
        )
        // 空串等同未设置（与 Rust 侧 dict_path/data_dir 的 takeIf { isNotEmpty() } 同口径）
        assertEquals(
            "/home/u/.config/fcitx5/conf/opi_fcitx5.conf",
            settingsPathFor(windows = false, xdgConfigHome = "", home = "/home/u", localAppData = null).toString(),
        )
    }

    @Test
    fun `windows path is localappdata opi`() {
        // 对侧读侧 = crates/tsf-opi/src/settings.rs（与 dict_path.rs 的候选目录同一处）。
        //
        // ⚠️ 断言的是**结构**（基目录 + opi/settings.ini 两段），不是那个反斜杠字面串：
        // `Paths.get` 按**运行平台**的分隔符拼路径 —— 在 Linux 上跑这条测试时，
        // Windows 字面串永远不会出现（实测：首版就是这么红的，errorFileCount=1）。
        // 真正的 Windows 字面串只能在 Windows 上验；这里钉的是「拼了哪两段、基目录是谁」。
        val p = settingsPathFor(
            windows = true, xdgConfigHome = null, home = null,
            localAppData = "C:\\Users\\u\\AppData\\Local",
        )
        // 只用 Path 自己的 API（`takeLast` 那条扩展在 JDK11 目标下解析不到，实测红）
        assertEquals("settings.ini", p.fileName.toString())
        assertEquals("opi", p.parent.fileName.toString())
        assertEquals("C:\\Users\\u\\AppData\\Local", p.parent.parent.toString())
    }
}
