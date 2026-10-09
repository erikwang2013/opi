// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 符号页的**数据层**门禁：TSV 解析、页大小、数据文件落点（+ 真资产双向钉住）。
//!
//! 解析口径与 Android 的 `keyboard/SymbolCategories.kt` 逐行镜像（见 SymbolsPage.kt
//! 的类注释）：两端读同一份 TSV，解析漂了就是「同一个符号页在两端排得不一样」。

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.nio.file.Files
import java.nio.file.Paths

class SymbolCategoriesTest {

    @Test
    fun `parse splits symbols by space and keeps tokens whole`() {
        val cats = SymbolCategories.parse(
            "1\t常用\t， 。 、\n" +
                "3\t英文\t! \" # \\\n",
        )
        assertEquals(2, cats.size)
        assertEquals(SymbolCategories.Category(1, "常用", listOf("，", "。", "、")), cats[0])
        // `"` 与 `\` 是**真的符号**（英文类目整行都是）：解析必须原样留着，
        // 上屏那一跳才需要转义（见 WireContractTest 的 commitText 那几条）。
        assertEquals(listOf("!", "\"", "#", "\\"), cats[1].symbols)
    }

    @Test
    fun `multi character tokens stay in one piece`() {
        // 组合序列/表情（网络类目）必须整块显示；逐码点切会把代理对拆坏、
        // 把组合字符拆成裸基字 —— 屏幕上看着像乱码，且只在某些类目上出现。
        val cats = SymbolCategories.parse("14\t网络\t╮(╯▽╰)╭ o͘ 😀\n")
        assertEquals(listOf("╮(╯▽╰)╭", "o͘", "😀"), cats[0].symbols)
    }

    @Test
    fun `bad lines are skipped not fatal`() {
        // 文件可能被替换/截断 ⇒ 降级成少几个类目，而不是打不开页面。
        val cats = SymbolCategories.parse(
            "1\t常用\t，。\n" + // 好行
                "只有两列\t没符号\n" + // 列不够
                "x\t序号不是数字\t，\n" + // 序号坏
                "4\t\t，\n" + // 名称为空
                "\t名在前\t，\n" + // 序号空
                "5\t空符号\t \n" + // 符号为空
                "\n", // 空行
        )
        assertEquals(1, cats.size)
        assertEquals("常用", cats[0].label)
    }

    @Test
    fun `page size matches android`() {
        // 8 列 × 4 行。与 Android 的 SymbolCategories.PAGE_SIZE 同值 —— 页大小不同
        // 会让同一类目在两端的第 2 页内容不一样（用户看着像「翻页错位」）。
        assertEquals(32, SymbolCategories.PAGE_SIZE)
        assertEquals(8, SymbolCategories.COLUMNS)
        assertEquals(0, SymbolCategories.PAGE_SIZE % SymbolCategories.COLUMNS)
    }

    @Test
    fun `data file location per platform`() {
        // Windows = %LOCALAPPDATA%\opi\symbol_categories.tsv（与 settings.ini 同目录，
        // 打包侧往这拷）；Linux = /usr/share/opi/（fcitx5 侧安装）。路径错 = 静默
        // 显示「没有类目数据」——不报任何错。
        val win = symbolDataPathFor(windows = true, localAppData = "/la")
        assertEquals("symbol_categories.tsv", win.fileName.toString())
        assertEquals("opi", win.parent.fileName.toString())
        assertEquals("/la", win.parent.parent.toString())
        // LOCALAPPDATA 缺省（空串/未设）⇒ 退回当前目录，而不是抛
        assertEquals(Paths.get(".", "opi", "symbol_categories.tsv"), symbolDataPathFor(true, null))
        assertEquals(Paths.get(".", "opi", "symbol_categories.tsv"), symbolDataPathFor(true, ""))
        assertEquals("/usr/share/opi/symbol_categories.tsv", symbolDataPathFor(false, "/la").toString())
    }

    /**
     * 真资产（`android/app/src/main/assets/symbol_categories.tsv`）：**两端共用同一份**，
     * 打包时分别拷进 Windows 的 `%LOCALAPPDATA%\opi\` 与 Linux 的 `/usr/share/opi/`。
     * 这里从 desktop 的测试工作目录（= desktop/）指过去。
     *
     * 资产不在 ⇒ **红**（不用 assume 跳过）：它搬家了 = 桌面侧的两条落点文档、打包
     * 拷贝、以及 `scripts/build-packages.sh` 都得跟着改，这条就是那个信号。
     */
    @Test
    fun `the shipped asset parses into the 22 categories`() {
        val f = Paths.get("../android/app/src/main/assets/symbol_categories.tsv")
        assertTrue("资产不在了（搬家了？桌面侧的落点/拷贝要同改）：${f.toAbsolutePath()}", Files.exists(f))
        val cats = SymbolCategories.parse(Files.readString(f))
        assertEquals("用户裁决的 22 类目", 22, cats.size)
        assertEquals((1..22).toList(), cats.map { it.index })
        assertTrue("每个类目都得有符号", cats.all { it.symbols.isNotEmpty() })
        // 每类都超过一页 ⇒ 翻页这条交互在真资产上真的会用到（否则就是死代码）
        assertTrue("22 类里应有超过 32 个符号的类目", cats.any { it.symbols.size > SymbolCategories.PAGE_SIZE })
        assertEquals("常用", cats[0].label)
        assertEquals("拼音", cats[12].label) // 序号 13：字段 2 是名称、行序即序号序
    }
}
