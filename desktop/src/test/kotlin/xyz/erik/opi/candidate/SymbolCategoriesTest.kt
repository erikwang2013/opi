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
    fun `page size is sized for the desktop window`() {
        // 12 列 × 6 行。本用例原名 `page size matches android`（两端同值 8×4=32）——
        // 2026-10-12 用户裁决「符号按钮点击后要是个**大的**页面」后桌面放大到 72，
        // **两端有意不同值**：页大小跟屏走（手机 32、桌面 72）。
        assertEquals(72, SymbolCategories.PAGE_SIZE)
        assertEquals(12, SymbolCategories.COLUMNS)
        assertEquals(0, SymbolCategories.PAGE_SIZE % SymbolCategories.COLUMNS)
    }

    @Test
    fun `data file candidates per platform`() {
        // Windows 顺位：① %LOCALAPPDATA%\opi\（与 settings.ini 同目录）→ ② exe 同目录
        // （windows.yml 打包时装 tsv 的地方）；Linux = /usr/share/opi/（fcitx5-glue 的
        // CMake install 在装）。顺位/路径错 = 静默显示「没有类目数据」——不报任何错。
        val exe = Paths.get("/app")
        val win = symbolDataCandidates(windows = true, localAppData = "/la", exeDir = exe)
        assertEquals(2, win.size)
        assertEquals(Paths.get("/la", "opi", "symbol_categories.tsv"), win[0])
        assertEquals(Paths.get("/app", "symbol_categories.tsv"), win[1])
        // LOCALAPPDATA 缺省（空串/未设）⇒ 退回当前目录，而不是抛
        assertEquals(Paths.get(".", "opi", "symbol_categories.tsv"), symbolDataCandidates(true, null, exe)[0])
        assertEquals(Paths.get(".", "opi", "symbol_categories.tsv"), symbolDataCandidates(true, "", exe)[0])
        // 非 jpackage 启动（IDE/命令行）⇒ 没有 exe 同目录这一顺位，而不是拼出个野路径
        assertEquals(listOf(Paths.get("/la", "opi", "symbol_categories.tsv")), symbolDataCandidates(true, "/la", null))
        assertEquals(
            listOf(Paths.get("/usr/share/opi/symbol_categories.tsv")),
            symbolDataCandidates(windows = false, localAppData = "/la", exeDir = exe),
        )
    }

    @Test
    fun `loader takes the first candidate that yields data`() {
        // 兜底链的行为：① 缺/坏（解析空）⇒ 用 ②，而不是报「没有类目数据」把好数据藏起来；
        // ①② 都好 ⇒ ① 优先；全不行 ⇒ 空表。坏候选落下一顺位与 dict_path.rs 的 load_dict 同语义。
        val dir = Files.createTempDirectory("opi-sym")
        val a = dir.resolve("a.tsv")
        val b = dir.resolve("b.tsv")
        Files.writeString(b, "1\t英文\t! ?\n")
        assertEquals(listOf("英文"), loadSymbolCategories(listOf(a, b)).map { it.label }) // ① 缺
        Files.writeString(a, "1\t常用\t， 。\n")
        assertEquals(listOf("常用"), loadSymbolCategories(listOf(a, b)).map { it.label }) // ① 优先
        Files.writeString(a, "不是TSV\n")
        assertEquals(listOf("英文"), loadSymbolCategories(listOf(a, b)).map { it.label }) // ① 坏 ⇒ ②
        assertTrue(loadSymbolCategories(listOf(dir.resolve("x.tsv"), dir.resolve("y.tsv"))).isEmpty())
        dir.toFile().deleteRecursively()
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
        assertTrue(
            "22 类里应有超过一页的类目（翻页才活）",
            cats.any { it.symbols.size > SymbolCategories.PAGE_SIZE },
        )
        assertEquals("常用", cats[0].label)
        assertEquals("拼音", cats[12].label) // 序号 13：字段 2 是名称、行序即序号序
    }
}
