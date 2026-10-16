// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「词库」页的门禁（纯 JVM，无 UI、无 JNA；全部跑在临时目录里的**真文件**上）。
//!
//! ⚠️ 夹具列序 = **`词<TAB>拼音<TAB>频次`**（与 `engine-data::dict_text::parse_report` 的
//! `cols[0]=word` 同源；**lead 任务书那句写反了，别照抄**）。2 列也行（缺频次按 1000）。
//!
//! 盯的是四条链路各自的静默失效形态：
//! · 目录推导 —— 错一个字 = 导入的词库 IME 永远读不到（和 settingsPathFor 同一份理由）；
//! · 列表/计数 —— 「列了个不能用的」与「数了个不对的数」都不会报错；
//! · 导入 —— 覆盖 = 静默毁用户数据（故断言「绝不覆盖」）；自我拷贝 = 每导一次多一份；
//! · 停用名单 —— 空 = 全启用的方向搞反 = 一次保存停掉所有词库。

package xyz.erik.opi.candidate

import java.io.RandomAccessFile
import java.nio.file.Files
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class DictsTest {

    @get:Rule
    val folder = TemporaryFolder()

    // ---------- 目录推导（纯函数，两平台）----------

    @Test
    fun `dicts dir follows the platform contract`() {
        // Linux：$XDG_DATA_HOME/opi/dicts；缺省/空串 ⇒ $HOME/.local/share/opi/dicts
        // （空串等同未设置 —— 与 settingsPathFor、Rust 侧 data_dir 同口径）。
        assertEquals(
            "/x/data/opi/dicts",
            dictsDirFor(windows = false, xdgDataHome = "/x/data", home = "/home/u", localAppData = null).toString(),
        )
        assertEquals(
            "/home/u/.local/share/opi/dicts",
            dictsDirFor(windows = false, xdgDataHome = null, home = "/home/u", localAppData = null).toString(),
        )
        assertEquals(
            "/home/u/.local/share/opi/dicts",
            dictsDirFor(windows = false, xdgDataHome = "", home = "/home/u", localAppData = null).toString(),
        )
        // Windows：断言**结构**（基目录 + opi/dicts 两段），不是反斜杠字面串 ——
        // Paths.get 按运行平台的分隔符拼路径，在 Linux 上跑这条时反斜杠串永不出现
        // （与 SettingsTest 的 windows path 用例同一条教训）。
        val w = dictsDirFor(
            windows = true, xdgDataHome = null, home = null,
            localAppData = "C:\\Users\\u\\AppData\\Local",
        )
        assertEquals("dicts", w.fileName.toString())
        assertEquals("opi", w.parent.fileName.toString())
        assertEquals("C:\\Users\\u\\AppData\\Local", w.parent.parent.toString())
    }

    // ---------- 列表 + 计数 ----------

    @Test
    fun `list shows only tsv regular files and counts valid three column lines`() {
        val dir = folder.newFolder().toPath()
        Files.writeString(dir.resolve("good1.tsv"), "你好\tnihao\t100\n世界\tshijie\t90\n吧\tba\t5\n")
        Files.writeString(dir.resolve("good2.tsv"), "啊\ta\t1\n")
        // 坏行三种（1 列 / 4 列 / 空行）都不计数；文件**照列**、数字如实 —— 沉默的 0
        // 与「假装都在」都是假信息。CRLF 的行也是合法行（Windows 导入的词库）。
        // ⚠️ 口径（lead 契约「只列 2 个 tsv、坏行计数如实、非 tsv 不列」的落地）：坏行
        // **不让文件消失** —— 半坏的词库照列、条数只算合法行（隐藏它 = 用户「导入完
        // 什么也没发生」，正是本仓在防的静默失败）；非 tsv 与目录不列。
        Files.writeString(dir.resolve("bad.tsv"), "好\tok\t1\n没有TAB\n四\t列\t在\t这\n\n好\thao\t2\n")
        Files.writeString(dir.resolve("crlf.tsv"), "你好\tnihao\t1\r\n")
        // 2 列（缺频次）= 解析器按 1000 照收 ⇒ 计数也得算（`2..=3` 同口径）；这条要是
        // 退回「恰好 2 个 TAB」，纯 2 列词库就会显示「0 条」而 IME 明明加载 —— 静默错数。
        Files.writeString(dir.resolve("two-col.tsv"), "世界\tshijie\n你好\tnihao\t100\n")
        Files.writeString(dir.resolve("note.txt"), "这不是词库\n")
        Files.createDirectory(dir.resolve("dir.tsv")) // 名字像但不是普通文件 ⇒ 不列

        val entries = listDicts(dir)
        assertEquals(listOf("bad", "crlf", "good1", "good2", "two-col"), entries.map { it.name })
        assertEquals(DictCount.Lines(2), entries.first { it.name == "bad" }.count)
        assertEquals(DictCount.Lines(1), entries.first { it.name == "crlf" }.count)
        assertEquals(DictCount.Lines(3), entries.first { it.name == "good1" }.count)
        assertEquals(DictCount.Lines(1), entries.first { it.name == "good2" }.count)
        assertEquals(DictCount.Lines(2), entries.first { it.name == "two-col" }.count)
        assertEquals(Files.size(dir.resolve("good1.tsv")), entries.first { it.name == "good1" }.bytes)
        // 目录还没有（没导入过）/空目录 ⇒ 空列表，不是错误。
        assertEquals(emptyList<DictEntry>(), listDicts(dir.resolve("nope")))
        assertEquals(emptyList<DictEntry>(), listDicts(folder.newFolder().toPath()))
    }

    @Test
    fun `too large files are reported without counting`() {
        // 稀疏文件：逻辑大小 200MB+、占盘 0（读它才是浪费）。上限是**不数**的判据。
        val big = folder.newFolder().toPath().resolve("big.tsv")
        RandomAccessFile(big.toFile(), "rw").use { it.setLength(DICT_COUNT_MAX_BYTES + 1) }
        assertEquals(DictCount.TooLarge, countDictLines(big))
    }

    // ---------- 导入（复制；重名自动加后缀，绝不覆盖）----------

    @Test
    fun `import copies and never overwrites`() {
        val src = folder.newFolder().toPath()
        val dst = folder.newFolder().toPath()

        Files.writeString(src.resolve("foo.tsv"), "A\n")
        val first = importDict(src.resolve("foo.tsv"), dst)
        assertEquals(dst.resolve("foo.tsv"), first)
        assertEquals("A\n", Files.readString(dst.resolve("foo.tsv")))

        // 源就在词库目录里（同一个文件）⇒ 不复制、原样认账（否则导一次多一份自我拷贝）。
        assertEquals(first, importDict(first, dst))
        assertEquals(listOf("foo.tsv"), namesIn(dst))

        // 另一个同名、不同内容 ⇒ 加后缀，已有那份**一个字节不动**。
        Files.writeString(src.resolve("foo.tsv"), "B\n")
        assertEquals(dst.resolve("foo-2.tsv"), importDict(src.resolve("foo.tsv"), dst))
        assertEquals("B\n", Files.readString(dst.resolve("foo-2.tsv")))
        assertEquals("A\n", Files.readString(dst.resolve("foo.tsv")))

        Files.writeString(src.resolve("foo.tsv"), "C\n")
        assertEquals(dst.resolve("foo-3.tsv"), importDict(src.resolve("foo.tsv"), dst))
        assertEquals(listOf("foo-2.tsv", "foo-3.tsv", "foo.tsv"), namesIn(dst))

        // 目标目录不存在 ⇒ 连同目录一起创建（首次导入的常态）。
        val fresh = dst.resolve("sub").resolve("deeper")
        assertEquals(fresh.resolve("foo.tsv"), importDict(src.resolve("foo.tsv"), fresh))
    }

    @Test
    fun `import rejects non tsv and missing sources`() {
        val src = folder.newFolder().toPath()
        val dst = folder.newFolder().toPath()
        Files.writeString(src.resolve("x.txt"), "不是词库\n")
        assertThrows(IllegalArgumentException::class.java) { importDict(src.resolve("x.txt"), dst) }
        assertThrows(IllegalArgumentException::class.java) { importDict(src.resolve("nope.tsv"), dst) }
        assertEquals(emptyList<String>(), namesIn(dst)) // 拒绝了就什么都没落
    }

    // ---------- 删除 ----------

    @Test
    fun `delete removes the file`() {
        val dir = folder.newFolder().toPath()
        val p = dir.resolve("gone.tsv")
        Files.writeString(p, "b\ta\t1\n")
        assertTrue(deleteDict(dir, "gone"))
        assertFalse("删完文件还得在 = 假删除", Files.exists(p))
        // 再删 = false（只有一个「已经没有了」的如实回执，UI 据此换措辞）。
        assertFalse(deleteDict(dir, "gone"))
    }

    // ---------- 停用名单（ini 三处往返 + 回落）----------

    @Test
    fun `disabled list round trips through all three ini sites`() {
        val s = OpSettings(disabledDicts = "foo,bar-2")
        // 1) toIni 写得出
        assertTrue(s.toIni(), s.toIni().contains("DisabledDicts=foo,bar-2\n"))
        // 2) fromIni 读得回（整份往返，不是单键）
        assertEquals(s, OpSettings.fromIni(s.toIni()))
        // 3) mergeIni：管理键（旧行被顶掉）+ 未知键逐字保留（既有纪律不许被本轮破坏）
        val merged = mergeIni("# 手写\nDisplayName=OPI\nDisabledDicts=old\n", s)
        assertTrue(merged, merged.contains("DisabledDicts=foo,bar-2\n"))
        assertFalse(merged, merged.contains("DisabledDicts=old\n"))
        assertTrue(merged, merged.contains("DisplayName=OPI\n"))
        assertTrue(merged, merged.contains("# 手写\n"))
        // 缺键/空值 = 空 = **全部启用**（方向搞反 = 老配置升级后一次保存停掉所有词库）。
        assertEquals("", OpSettings.fromIni("").disabledDicts)
        assertEquals("", OpSettings.fromIni("DisabledDicts=\n").disabledDicts)
        assertTrue(OpSettings().dictEnabled("anything"))
        // 名字里没有枚举可钉 —— 认不得的名字（没有对应文件）只是无害的死名字。
        assertEquals(s, OpSettings.fromIni("DisabledDicts=foo,bar-2\n"))
    }

    @Test
    fun `toggle writes and reads the disable list`() {
        val s0 = OpSettings()
        assertTrue(s0.dictEnabled("foo"))
        val off = s0.withDictEnabled("foo", false)!!
        assertEquals("foo", off.disabledDicts)
        assertFalse(off.dictEnabled("foo"))
        assertTrue("停一个不许殃及别人", off.dictEnabled("bar"))
        val off2 = off.withDictEnabled("bar", false)!!
        assertEquals("foo,bar", off2.disabledDicts)
        assertEquals("bar", off2.withDictEnabled("foo", true)!!.disabledDicts)
        // 一个都不停用 ⇒ 空串（仍是「全启用」的表达，别再写个空段）。
        assertEquals("", off2.withDictEnabled("bar", true)!!.withDictEnabled("foo", true)!!.disabledDicts)
        // 容错解析：空白 / 空段 / 重复（与 fuzzyPairSet 同调）。
        assertEquals(setOf("a", "b"), parseDisabledDicts(" a , ,b ,a"))
        assertEquals(emptySet<String>(), parseDisabledDicts(""))
        // 生效集是解析后的集合 —— 手改的名单（带空白）也认。
        assertFalse(OpSettings(disabledDicts = " foo ").dictEnabled("foo"))
    }

    @Test
    fun `toggle refuses names the disable list cannot express`() {
        // （lead 2026-10-14 裁决：拒绝 + 告知 > 静默失效）名单是逗号分隔的文本：名字含
        // 逗号写进去会被切成两个名字（拨了开关没反应 + 殃及同名的别人）；首尾空白会被
        // 解析 trim 掉（写进去也永远不命中）。两种都**拒绝此翻转** —— 返回 null，
        // 调用方据此不改文件 + 状态行明说（本函数保证「不是没表达，就是没写」）。
        val s = OpSettings()
        assertNull("含逗号的名字必须被拒绝，而不是写成两个名字", s.withDictEnabled("a,b", false))
        assertNull("首尾带空白同样表达不了（trim 掉就永远不命中）", s.withDictEnabled(" foo ", false))
        assertNull("空名字也表达不了", s.withDictEnabled("", false))
        // 拒绝**不殃及已有名单**：good 的停用状态一个字节不改（null = 什么都没打算写）。
        val withOne = OpSettings(disabledDicts = "good")
        assertNull(withOne.withDictEnabled("a,b", false))
        assertEquals("good", withOne.disabledDicts)
        // 正常名字照旧 —— 拒绝只落在表达不了的那些上。
        assertEquals("good,a", withOne.withDictEnabled("a", false)!!.disabledDicts)
    }

    private fun namesIn(dir: java.nio.file.Path): List<String> =
        Files.newDirectoryStream(dir).use { ds -> ds.map { it.fileName.toString() }.sorted() }

    // ---- 内置词库**首启种子**（2026-10-16，康熙字典「全发」）----
    // 与 Android `DictStore.seedBundled` / fcitx5 `opiSeedBundledDict` 语义一字不差：
    // 种一次（标记门控）、用户删过的不复活、已有同名文件认账不覆盖、源缺不种也不打标记。

    private fun seedStream(s: String = "丂\tkao\t4000\n") =
        java.io.ByteArrayInputStream(s.toByteArray())

    @Test
    fun `seedBundledDict copies once and marks seeded`() {
        val dir = Files.createTempDirectory("opi-seed")
        var opens = 0
        val open = { opens++; seedStream() as java.io.InputStream }
        assertTrue("首次应种下", seedBundledDict("kangxi", dir, open))
        assertEquals("丂\tkao\t4000\n", Files.readString(dir.resolve("kangxi.tsv")))
        assertEquals(1, opens)
        assertFalse("第二次不种（标记在）", seedBundledDict("kangxi", dir, open))
        assertEquals("源不重复读", 1, opens)
    }

    @Test
    fun `seedBundledDict never resurrects a deleted dict`() {
        val dir = Files.createTempDirectory("opi-seed")
        assertTrue(seedBundledDict("kangxi", dir) { seedStream() })
        Files.delete(dir.resolve("kangxi.tsv")) // 用户在词库页删了
        assertFalse("删过的不许复活", seedBundledDict("kangxi", dir) { seedStream() })
        assertFalse(Files.exists(dir.resolve("kangxi.tsv")))
    }

    @Test
    fun `seedBundledDict keeps an existing file untouched`() {
        val dir = Files.createTempDirectory("opi-seed")
        Files.writeString(dir.resolve("kangxi.tsv"), "用户自己的\n")
        var opens = 0
        assertFalse("已有同名文件：认账、不覆盖", seedBundledDict("kangxi", dir) { opens++; seedStream() })
        assertEquals("用户文件原样", "用户自己的\n", Files.readString(dir.resolve("kangxi.tsv")))
        assertEquals("不读源", 0, opens)
        assertFalse("且已打标记：下次也不种", seedBundledDict("kangxi", dir) { opens++; seedStream() })
        assertEquals(0, opens)
    }

    @Test
    fun `seedBundledDict missing source is a noop without marking`() {
        val dir = Files.createTempDirectory("opi-seed")
        assertFalse("源缺（资源没打进包）：不种", seedBundledDict("kangxi", dir) { null })
        assertFalse("标记不许建 —— 下次还有机会", Files.exists(dir.resolve(".seeded-kangxi")))
        assertTrue("源回来了就种得下", seedBundledDict("kangxi", dir) { seedStream() })
        assertTrue(Files.isRegularFile(dir.resolve("kangxi.tsv")))
    }
}
