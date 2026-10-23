// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

/**
 * 宿主落盘（filesDir 单 JSON 文件）纯 JVM 测试：往返、损坏容错、原子写失败上报、
 * 推引擎的 JSON 形状与启用态读回、死 .so（Error 而非 Exception）不抛。
 */
class CustomPhraseStoreTest {

    @get:Rule
    val tmp = TemporaryFolder()

    /** 假引擎：记录整表 JSON 与启用态；默认读回 = 最后一次推入的值。 */
    private class FakeEngine {
        val pushed = mutableListOf<String>()
        val enabledPushes = mutableListOf<Boolean>()
        var pushResult = 0
        var pushThrows: Throwable? = null

        /** 读回值；默认模拟「引擎收下了写入」。 */
        var readEnabledValue: Boolean? = null

        /** false = 写口假装收下、读回却是旧值（模拟引擎未装载/忽略写入）。 */
        var echoEnabled = true

        fun pushPhrases(json: String): Int {
            pushed += json
            pushThrows?.let { throw it }
            return pushResult
        }

        fun pushEnabled(on: Boolean) {
            enabledPushes += on
            if (echoEnabled) readEnabledValue = on
        }

        fun readEnabled(): Boolean = readEnabledValue ?: false
    }

    private fun store(file: File, engine: FakeEngine = FakeEngine()) = CustomPhraseStore(
        file = file,
        pushPhrases = engine::pushPhrases,
        pushEnabled = engine::pushEnabled,
        readEnabled = engine::readEnabled,
    )

    private fun target(): File = File(tmp.root, CustomPhraseStore.FILE_NAME)

    private fun targetEn(): File = File(tmp.root, CustomPhraseStore.FILE_NAME_EN)

    @Test
    fun missingFileLoadsDefaults() {
        val s = store(target()).load()
        assertEquals(true, s.enabled)
        assertEquals(emptyList<CustomPhrase>(), s.phrases)
    }

    @Test
    fun saveThenLoadRoundTrips() {
        val f = target()
        val state = CustomPhraseStore.State(false, listOf(CustomPhrase("dz", "地址")))
        assertNull(store(f).save(state))

        assertEquals(state, store(f).load())
        assertTrue(f.readText().contains("\"enabled\":false"))
        // 原子写不留残骸
        assertTrue(
            (tmp.root.listFiles() ?: emptyArray()).none { it.name.contains(".tmp") },
        )
    }

    @Test
    fun corruptFileLoadsDefaultsAndIsKept() {
        val f = target().apply { writeText("{ 坏掉的 json") }
        assertEquals(CustomPhraseStore.State(), store(f).load())
        assertTrue(f.exists()) // 不删：下次成功保存原子覆盖
    }

    @Test
    fun saveFailureIsReportedNotThrown() {
        val notDir = File(tmp.root, "not-a-dir").apply { writeText("") }
        val s = store(File(notDir, CustomPhraseStore.FILE_NAME))
        val error = s.save(CustomPhraseStore.State(true, listOf(CustomPhrase("dz", "地址"))))
        val failed = error != null
        assertTrue("写不进去必须交回可见原因，实际：$error", failed)
    }

    @Test
    fun pushSendsCanonicalJsonAndEnabledThenReadsBack() {
        val engine = FakeEngine()
        val state = CustomPhraseStore.State(false, listOf(CustomPhrase("dz", "地址")))
        assertNull(store(target(), engine).push(state))

        // 引擎只认规范化形状：没有 enabled 字段
        assertEquals(
            listOf("""{"version":1,"phrases":[{"trigger":"dz","text":"地址"}]}"""),
            engine.pushed,
        )
        assertEquals(listOf(false), engine.enabledPushes)
    }

    @Test
    fun pushRejectionIsVisibleAndSkipsEnabledWrite() {
        val engine = FakeEngine().apply { pushResult = -1 }
        val error = store(target(), engine).push(CustomPhraseStore.State(true, emptyList()))
        val rejected = error != null && error.contains("拒绝")
        assertTrue("实际：$error", rejected)
        assertTrue(engine.enabledPushes.isEmpty())
    }

    @Test
    fun enabledMismatchIsReported() {
        val engine = FakeEngine().apply {
            echoEnabled = false
            readEnabledValue = true // 推 false，却读回 true
        }
        val error = store(target(), engine).push(CustomPhraseStore.State(false, emptyList()))
        val mismatch = error != null && error.contains("启用")
        assertTrue("实际：$error", mismatch)
    }

    @Test
    fun deadLibraryErrorIsReportedNotThrown() {
        val engine = FakeEngine().apply { pushThrows = UnsatisfiedLinkError("so missing") }
        val error = store(target(), engine).push(CustomPhraseStore.State(true, emptyList()))
        val dead = error != null && error.contains("引擎不可用")
        assertTrue("实际：$error", dead)
    }

    @Test
    fun loadAndPushReplaysPersistedState() {
        val f = target()
        val state = CustomPhraseStore.State(true, listOf(CustomPhrase("ab", "Alpha")))
        store(f).save(state)

        val engine = FakeEngine()
        val loaded = store(f, engine).loadAndPush()

        assertEquals(state, loaded)
        assertEquals(
            listOf("""{"version":1,"phrases":[{"trigger":"ab","text":"Alpha"}]}"""),
            engine.pushed,
        )
        assertEquals(listOf(true), engine.enabledPushes)
    }

    @Test
    fun loadAndPushReportsFailureToCallbackWithoutThrowing() {
        val engine = FakeEngine().apply { pushResult = -1 }
        val errors = mutableListOf<String>()
        store(target(), engine).loadAndPush { errors += it }
        assertEquals(1, errors.size)
    }

    /** 英文套（2026-10-24）：独立文件，两套内容互不可见（中文套行为不变）。 */
    @Test
    fun englishAndChineseFilesAreIndependent() {
        assertNotEquals(
            "英文套必须用独立文件名，否则会覆盖中文套",
            CustomPhraseStore.FILE_NAME,
            CustomPhraseStore.FILE_NAME_EN,
        )
        val cnFile = target()
        val enFile = targetEn()
        val cnState = CustomPhraseStore.State(true, listOf(CustomPhrase("dz", "地址")))
        val enState = CustomPhraseStore.State(false, listOf(CustomPhrase("brb", "be right back")))
        assertNull(store(cnFile).save(cnState))
        assertNull(store(enFile).save(enState))

        assertEquals(cnState, store(cnFile).load())
        assertEquals(enState, store(enFile).load())
        assertTrue(cnFile.readText().contains("地址"))
        assertTrue(!cnFile.readText().contains("be right back"))
        assertTrue(enFile.readText().contains("be right back"))
        assertTrue(!enFile.readText().contains("地址"))
    }

    /** 英文套与中文套各自推自己的口：表、开关都不串。 */
    @Test
    fun englishAndChinesePushesUseSeparateEngines() {
        val cnEngine = FakeEngine()
        val enEngine = FakeEngine()
        val cnState = CustomPhraseStore.State(false, listOf(CustomPhrase("dz", "地址")))
        val enState = CustomPhraseStore.State(true, listOf(CustomPhrase("brb", "be right back")))

        assertNull(store(target(), cnEngine).push(cnState))
        assertNull(store(targetEn(), enEngine).push(enState))

        assertEquals(
            listOf("""{"version":1,"phrases":[{"trigger":"dz","text":"地址"}]}"""),
            cnEngine.pushed,
        )
        assertEquals(
            listOf("""{"version":1,"phrases":[{"trigger":"brb","text":"be right back"}]}"""),
            enEngine.pushed,
        )
        assertEquals(listOf(false), cnEngine.enabledPushes)
        assertEquals(listOf(true), enEngine.enabledPushes)
    }

    /** 英文文件坏掉的回退语义与中文同一条：默认值、不删文件。 */
    @Test
    fun corruptEnglishFileLoadsDefaultsAndIsKept() {
        val f = targetEn().apply { writeText("{ 坏掉的 json") }
        assertEquals(CustomPhraseStore.State(), store(f).load())
        assertTrue("不删：下次成功保存原子覆盖", f.exists())
    }
}
