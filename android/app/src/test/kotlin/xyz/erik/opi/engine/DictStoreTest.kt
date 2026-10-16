// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.ByteArrayInputStream
import java.io.File
import java.io.IOException

/**
 * [DictStore] 纯 JVM 门禁（2026-10-14，M3「词库管理」移动端半边）。四条链路各一组：
 * 目录/命名归一、启动装载（N 个 tsv ⇒ N 次 importDict、坏文件不挡其余）、
 * 删除「引擎 + 文件两个都做」、导入（流拷 + 同名替换的失败回滚）。
 * 外加 `dictList()` JSON 容错 —— 引擎输出按字典序（serde 未开 preserve_order），
 * 解析不得依赖键序，坏输入不得崩。
 */
class DictStoreTest {

    @get:Rule
    val tmp = TemporaryFolder()

    /** 假引擎：记录四个出口的调用；拒收（-1 / false）与抛错（死 .so = Error）都可注入。 */
    private class FakeDictApi : DictApi {
        val imported = mutableListOf<String>()
        val removed = mutableListOf<String>()
        val enabledCalls = mutableListOf<Pair<String, Boolean>>()
        var importResult: (String) -> Int = { 5 }
        var removeOk = true
        var enableOk = true
        var listJson = "[]"
        var throwOnImport: Throwable? = null
        var throwOnRemove: Throwable? = null

        override fun importDict(path: String): Int {
            imported += path
            throwOnImport?.let { throw it }
            return importResult(path)
        }

        override fun dictList(): String = listJson

        override fun setDictEnabled(name: String, on: Boolean): Boolean {
            enabledCalls += name to on
            return enableOk
        }

        override fun removeDict(name: String): Boolean {
            removed += name
            throwOnRemove?.let { throw it }
            return removeOk
        }
    }

    private fun dir(): File = File(tmp.root, DictStore.DIR_NAME).also { it.mkdirs() }

    private fun store(api: FakeDictApi = FakeDictApi(), dir: File = dir()) = DictStore(dir, api)

    /** 导入残骸（`.bak`）：收尾判据 —— 失败回滚与成功收尾都不该留它。 */
    private fun residue(): List<String> =
        (dir().listFiles() ?: emptyArray()).map { it.name }.filter { it.endsWith(".bak") }

    // ---- 命名/目录归一 ----

    @Test
    fun nameStripsDirectoryAndExtension() {
        assertEquals("my-dict", DictStore.nameOf("db/my-dict.tsv"))
        assertEquals("my-dict", DictStore.nameOf("my-dict.tsv"))
        assertEquals("dict", DictStore.nameOf("dict.txt")) // 扩展名不限于 tsv：一律另存为 <名>.tsv
        assertEquals("noext", DictStore.nameOf("noext"))
        assertEquals("a.b", DictStore.nameOf("a.b.tsv")) // 只去最后一个扩展名
        assertEquals("空格", DictStore.nameOf(" 空格 .tsv")) // 去首尾空白
        assertEquals("x", DictStore.nameOf("../x.tsv")) // 路径穿越被 basename 压回本目录
    }

    @Test
    fun nameRejectsUnsafe() {
        assertNull(DictStore.nameOf(""))
        assertNull(DictStore.nameOf(".tsv"))
        assertNull(DictStore.nameOf("..tsv")) // 去扩展后剩 ".."，落盘就是往上一级走
        assertNull(DictStore.nameOf("a\\b.tsv"))
    }

    @Test
    fun filesAreTsvOnlySortedAndNotDirectories() {
        File(dir(), "b.tsv").writeText("x")
        File(dir(), "a.tsv").writeText("x")
        File(dir(), "note.txt").writeText("x")
        File(dir(), "sub.tsv").mkdirs() // 名为 .tsv 的目录不算词库
        File(dir(), "old.tsv.bak").writeText("x") // 导入残骸不列（也不装载）

        assertEquals(listOf("a.tsv", "b.tsv"), store().files().map { it.name })
        // 目录还不存在（第一次导入前）：空表，不是崩
        assertTrue(DictStore(File(tmp.root, "nope"), FakeDictApi()).files().isEmpty())
    }

    // ---- 启动装载 ----

    @Test
    fun loadAllImportsEachTsvOnceInOrder() {
        File(dir(), "b.tsv").writeText("x")
        File(dir(), "a.tsv").writeText("x")
        File(dir(), "note.txt").writeText("x")
        val api = FakeDictApi()

        val report = store(api).loadAll()

        assertEquals(2, report.loaded)
        assertTrue(report.failed.isEmpty())
        assertEquals(
            listOf(File(dir(), "a.tsv").absolutePath, File(dir(), "b.tsv").absolutePath),
            api.imported,
        )
    }

    @Test
    fun loadAllContinuesPastBadFileAndReportsOnce() {
        File(dir(), "good.tsv").writeText("x")
        File(dir(), "bad.tsv").writeText("x")
        File(dir(), "also-good.tsv").writeText("x")
        val api = FakeDictApi().also { it.importResult = { p -> if (p.endsWith("bad.tsv")) -1 else 7 } }

        val report = store(api).loadAll()

        assertEquals(2, report.loaded)
        assertEquals(listOf("bad"), report.failed) // 一条报告，不是每个文件一条
        assertEquals(3, api.imported.size) // 坏文件不挡其余：三个都调过
    }

    @Test
    fun loadAllSurvivesDeadEngine() {
        File(dir(), "a.tsv").writeText("x")
        val api = FakeDictApi().also { it.throwOnImport = NoClassDefFoundError("dead .so") }

        val report = store(api).loadAll()

        assertEquals(0, report.loaded)
        assertEquals(listOf("a"), report.failed) // 死 .so 落进「失败」格，不炸启动
    }

    @Test
    fun loadAllReplaysDisabledListAfterImport() {
        File(dir(), "a.tsv").writeText("x")
        File(dir(), "b.tsv").writeText("x")
        val api = FakeDictApi()

        val report = store(api).loadAll(disabled = setOf("b", "早就删掉的旧词库"))

        assertEquals(2, report.loaded)
        // 重放只打给「本轮装成功且在名单里」的名字；名单里的陈旧条目是引擎认不得的
        // 名字，不去喂它（同名重导入会把引擎里的启用位置回 true，不重放就静默复活）
        assertEquals(listOf("b" to false), api.enabledCalls)
    }

    @Test
    fun loadAllWithoutDisabledListDoesNotTouchSwitches() {
        File(dir(), "a.tsv").writeText("x")
        val api = FakeDictApi()
        store(api).loadAll()
        assertTrue(api.enabledCalls.isEmpty())
    }

    @Test
    fun disabledListParsesAndRoundTrips() {
        assertEquals(emptySet<String>(), DictStore.parseDisabled(""))
        assertEquals(setOf("b", "c"), DictStore.parseDisabled(" b ,, c ,")) // 去空白、丢空段

        assertEquals("b", DictStore.withDisabled("", "b", enabled = false))
        assertEquals("b,c", DictStore.withDisabled("b", "c", enabled = false))
        assertEquals("c", DictStore.withDisabled("b,c", "b", enabled = true))
        assertEquals("", DictStore.withDisabled("c", "c", enabled = true)) // 全启用 ⇒ 空串
    }

    // ---- 删除：引擎 + 文件两个都做 ----

    @Test
    fun removeDoesEngineAndFile() {
        val f = File(dir(), "my-dict.tsv").also { it.writeText("x") }
        val api = FakeDictApi()

        assertTrue(store(api).remove("my-dict"))

        assertEquals(listOf("my-dict"), api.removed)
        assertFalse(f.exists()) // 漏掉这一半，下次启动 loadAll 就把它装回来
    }

    @Test
    fun removeDeletesFileEvenWhenEngineRefuses() {
        val f = File(dir(), "my-dict.tsv").also { it.writeText("x") }
        val api = FakeDictApi().also { it.removeOk = false }

        assertFalse(store(api).remove("my-dict"))

        // 引擎拒绝也照删：目录里的同名文件是导入失败留下的坏文件，删掉正是用户要的
        assertFalse(f.exists())
    }

    @Test
    fun removeDeletesFileEvenWhenEngineDies() {
        val f = File(dir(), "my-dict.tsv").also { it.writeText("x") }
        val api = FakeDictApi().also { it.throwOnRemove = NoClassDefFoundError("dead .so") }

        assertFalse(store(api).remove("my-dict"))

        assertFalse(f.exists())
    }

    @Test
    fun removeClearsBakResidue() {
        File(dir(), "my-dict.tsv.bak").writeText("残骸")
        store().remove("my-dict")
        assertTrue(residue().isEmpty())
    }

    // ---- 导入 ----

    @Test
    fun importCopiesStreamAndImportsAbsolutePath() {
        val api = FakeDictApi()
        val content = "我\two\t100\n好\thao\n"

        val r = store(api).importTsv("db/my-dict.tsv") { ByteArrayInputStream(content.toByteArray()) }

        assertEquals(DictStore.ImportResult.Imported("my-dict", 5), r)
        assertEquals(content, File(dir(), "my-dict.tsv").readText()) // 落盘名 = 归一后的名字
        assertEquals(listOf(File(dir(), "my-dict.tsv").absolutePath), api.imported)
        assertTrue(residue().isEmpty())
    }

    @Test
    fun importRejectsUnsafeNameWithoutOpeningSource() {
        val api = FakeDictApi()

        val r = store(api).importTsv("..tsv") { error("不该被打开") }

        assertTrue(r is DictStore.ImportResult.Rejected)
        assertTrue(api.imported.isEmpty())
    }

    @Test
    fun importReadFailureRestoresPreviousFile() {
        File(dir(), "d.tsv").writeText("旧")
        val api = FakeDictApi()

        val r = store(api).importTsv("d.tsv") { throw IOException("打不开所选文件") }

        assertTrue(r is DictStore.ImportResult.Rejected)
        assertEquals("旧", File(dir(), "d.tsv").readText())
        assertTrue(api.imported.isEmpty())
        assertTrue(residue().isEmpty())
    }

    @Test
    fun importEngineRejectionRemovesBadCopyAndRestoresPrevious() {
        File(dir(), "d.tsv").writeText("旧")
        val api = FakeDictApi().also { it.importResult = { -1 } }

        val r = store(api).importTsv("d.tsv") { ByteArrayInputStream("坏".toByteArray()) }

        assertTrue(r is DictStore.ImportResult.Rejected)
        assertEquals("旧", File(dir(), "d.tsv").readText()) // 旧词库不因一次手滑消失
        assertTrue(residue().isEmpty())
    }

    @Test
    fun importEngineRejectionWithoutPreviousLeavesNothing() {
        val api = FakeDictApi().also { it.importResult = { -1 } }

        val r = store(api).importTsv("fresh.tsv") { ByteArrayInputStream("坏".toByteArray()) }

        assertTrue(r is DictStore.ImportResult.Rejected)
        // 坏文件不留：留着每次启动都会重装一次、报同一条错，而列表里看不到它
        assertEquals(emptyList<String>(), store(api).files().map { it.name })
        assertTrue(residue().isEmpty())
    }

    @Test
    fun importSuccessReplacesOldFile() {
        File(dir(), "d.tsv").writeText("旧")
        val api = FakeDictApi()

        val r = store(api).importTsv("d.tsv") { ByteArrayInputStream("新".toByteArray()) }

        assertEquals(DictStore.ImportResult.Imported("d", 5), r)
        assertEquals("新", File(dir(), "d.tsv").readText())
        assertEquals(listOf("d.tsv"), store(api).files().map { it.name })
        assertTrue(residue().isEmpty())
    }

    // ---- 启停 ----

    @Test
    fun setEnabledDelegatesAndReturnsEngineVerdict() {
        val api = FakeDictApi()
        val s = store(api)

        assertTrue(s.setEnabled("my-dict", false))
        assertEquals(listOf("my-dict" to false), api.enabledCalls)
        api.enableOk = false
        assertFalse(s.setEnabled("luna", true)) // 系统库：引擎拒绝，设置页据此回滚开关
    }

    // ---- dictList() JSON 容错 ----

    @Test
    fun listParsesEngineJson() {
        val api = FakeDictApi().also {
            // 引擎实际输出按字典序（serde 未开 preserve_order）—— 夹具就用真实键序
            it.listJson = """[{"count":95,"enabled":true,"name":"luna","system":true},""" +
                """{"count":7,"enabled":false,"name":"my-dict","system":false}]"""
        }

        assertEquals(
            listOf(
                DictStore.Entry("luna", 95, enabled = true, system = true),
                DictStore.Entry("my-dict", 7, enabled = false, system = false),
            ),
            store(api).list(),
        )
    }

    @Test
    fun listToleratesBadJsonAndMissingFields() {
        assertEquals(emptyList<DictStore.Entry>(), DictStore.parseList(""))
        assertEquals(emptyList<DictStore.Entry>(), DictStore.parseList("not json"))
        assertEquals(emptyList<DictStore.Entry>(), DictStore.parseList("""[{"count":3,"""))

        // 缺字段取缺省；无名条目跳过（列表点不动、开关没有 key）
        assertEquals(
            listOf(DictStore.Entry("ok", 0, enabled = false, system = false)),
            DictStore.parseList("""[{"name":"ok"},{"count":3},{"name":"","count":1}]"""),
        )
    }

    @Test
    fun listDecodesEscapedQuotesInName() {
        val json = """[{"name":"a\"b","count":1,"enabled":true,"system":false}]"""
        assertEquals("a\"b", DictStore.parseList(json).single().name)
    }

    // ---- 内置词库**首启种子**（2026-10-16，康熙字典「全发」）----
    // 随包的内置词库拷进词库目录**一次**，它随后就是一份普通导入词库
    //（词库页可见、可停用、可删除）。标记文件保证「只种一次」。

    private fun seedSrc(content: String = "丂\tkao\t4000\n") =
        ByteArrayInputStream(content.toByteArray()) as java.io.InputStream

    @Test
    fun seedBundledCopiesOnceAndMarksSeeded() {
        val dir = tmp.newFolder()
        val store = DictStore(dir, FakeDictApi())
        var opens = 0
        val open = { opens++; seedSrc() }
        assertTrue("首次应种下", store.seedBundled("kangxi", open))
        assertEquals("丂\tkao\t4000\n", File(dir, "kangxi.tsv").readText())
        assertEquals(1, opens)
        assertFalse("第二次不种（标记在）", store.seedBundled("kangxi", open))
        assertEquals("源不重复读", 1, opens)
    }

    @Test
    fun seedBundledNeverResurrectsADeletedDict() {
        val dir = tmp.newFolder()
        val store = DictStore(dir, FakeDictApi())
        assertTrue(store.seedBundled("kangxi") { seedSrc() })
        File(dir, "kangxi.tsv").delete() // 用户在词库页删了
        assertFalse("删过的不许复活（与 remove 的「文件那一半」语义对偶）", store.seedBundled("kangxi") { seedSrc() })
        assertFalse(File(dir, "kangxi.tsv").exists())
    }

    @Test
    fun seedBundledKeepsAnExistingUserFileUntouched() {
        val dir = tmp.newFolder()
        val store = DictStore(dir, FakeDictApi())
        File(dir, "kangxi.tsv").writeText("用户自己的\n")
        var opens = 0
        assertFalse("已有同名文件：认账、不覆盖", store.seedBundled("kangxi") { opens++; seedSrc() })
        assertEquals("用户文件原样", "用户自己的\n", File(dir, "kangxi.tsv").readText())
        assertEquals("不读源", 0, opens)
        assertFalse("且已打标记：升级用户场景下次也不种", store.seedBundled("kangxi") { opens++; seedSrc() })
        assertEquals(0, opens)
    }

    @Test
    fun seedBundledMissingSourceIsANoopWithoutMarking() {
        val dir = tmp.newFolder()
        val store = DictStore(dir, FakeDictApi())
        assertFalse("源缺（资产缺）：不种", store.seedBundled("kangxi") { null })
        assertFalse("标记不许建 —— 下次启动还有机会", File(dir, ".seeded-kangxi").exists())
        assertTrue("源回来了就种得下", store.seedBundled("kangxi") { seedSrc() })
        assertTrue(File(dir, "kangxi.tsv").isFile)
    }
}
