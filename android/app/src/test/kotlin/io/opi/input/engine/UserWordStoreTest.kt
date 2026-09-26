package io.opi.input.engine

import io.opi.input.ime.Debouncer
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assume
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File
import java.io.IOException
import java.util.concurrent.Executor

/**
 * 用户词落盘（学习持久化）纯 JVM 测试：真实临时目录 + 注入假防抖/假执行器/假引擎。
 *
 * 「能不能区分修复前后」：UserWordStore 本身就是被考察的行为，且每个用例都断言到
 * 「不实现持久化就不可能满足」的点 —— 防抖窗口内 0 次写盘、窗口结束 1 次写盘、
 * 落盘走 io 线程、原子写不留 tmp、各失败路径静默且旧文件完好。
 */
class UserWordStoreTest {

    @get:Rule
    val tmp = TemporaryFolder()

    /** 假防抖：不排线程、不 sleep，手动 fire 才执行（对齐 ImeStateTest 的做法）。 */
    private class FakeDebouncer : Debouncer {
        var scheduleCalls = 0
            private set
        var lastDelayMs = -1L
            private set
        private var current: (() -> Unit)? = null

        override fun schedule(delayMs: Long, action: () -> Unit): () -> Unit {
            scheduleCalls++
            lastDelayMs = delayMs
            current = action
            return { if (current === action) current = null }
        }

        /** 防抖窗口结束：执行已排队的回调。 */
        fun fire() {
            val a = current
            current = null
            a?.invoke()
        }
    }

    /** 假引擎：记录 import/export 调用；可注入返回值与异常（模拟引擎拒收/死 .so）。 */
    private class FakeEngine {
        val imported = mutableListOf<String>()
        var importResult = 1
        var export = """{"ni":3}"""
        var exportCalls = 0
            private set
        var throwOnImport: Throwable? = null
        var throwOnExport: Throwable? = null

        fun importJson(json: String): Int {
            imported += json
            throwOnImport?.let { throw it }
            return importResult
        }

        fun exportJson(): String {
            exportCalls++
            throwOnExport?.let { throw it }
            return export
        }
    }

    /** 直接执行的 io：测试里写盘同步完成（生产是后台单线程）。 */
    private val directIo = Executor { it.run() }

    private fun store(
        file: File,
        engine: FakeEngine = FakeEngine(),
        debouncer: FakeDebouncer = FakeDebouncer(),
        io: Executor = directIo,
    ) = UserWordStore(file, engine::importJson, engine::exportJson, debouncer, io)

    private fun target(): File = File(tmp.root, UserWordStore.FILE_NAME)

    // ---- 启动导入 ----

    @Test
    fun loadImportsExistingFile() {
        val f = target().apply { writeText("""{"ni":3}""") }
        val engine = FakeEngine()

        store(f, engine).load()

        assertEquals(listOf("""{"ni":3}"""), engine.imported)
    }

    @Test
    fun loadWithMissingFileIsSilent() {
        val engine = FakeEngine()

        store(target(), engine).load() // 文件不存在（首次安装）

        assertTrue(engine.imported.isEmpty())
    }

    @Test
    fun loadWithIllegalJsonIsSilentAndKeepsFile() {
        val f = target().apply { writeText("{ not json") }
        val engine = FakeEngine().apply { importResult = -1 } // 引擎拒收：负数

        store(f, engine).load()

        assertEquals(listOf("{ not json"), engine.imported) // 判定权在引擎，Kotlin 侧不解析 JSON
        assertTrue(f.exists()) // 不删不备份：留作人工排查，下次成功保存会原子覆盖它
    }

    @Test
    fun loadSwallowsDeadLibraryError() {
        // 死 .so：OpiEngine 是 object，类初始化失败后每次调用抛的是 Error 而非 Exception
        val f = target().apply { writeText("{}") }
        val engine = FakeEngine().apply { throwOnImport = UnsatisfiedLinkError("so missing") }

        store(f, engine).load() // 不抛 → 不崩 IME
    }

    @Test
    fun loadIsIdempotent() {
        val f = target().apply { writeText("{}") }
        val engine = FakeEngine()
        val s = store(f, engine)

        s.load()
        s.load() // onCreateInputView 可能重复走到

        assertEquals(1, engine.imported.size)
    }

    // ---- 防抖保存 ----

    @Test
    fun debounceCollapsesRapidChangesIntoOneWrite() {
        val f = target()
        val engine = FakeEngine().apply { export = """{"a":1}""" }
        val deb = FakeDebouncer()
        val s = store(f, engine, deb)

        repeat(5) { s.scheduleSave() } // 连打选词

        assertEquals(5, deb.scheduleCalls)
        assertEquals(0, engine.exportCalls) // 窗口内既不导出也不写盘
        assertFalse(f.exists())

        deb.fire()

        assertEquals(1, engine.exportCalls) // 5 次变更合并成 1 次落盘
        assertEquals("""{"a":1}""", f.readText())
    }

    @Test
    fun saveUsesDebounceWindowOfOneToTwoSeconds() {
        val deb = FakeDebouncer()

        store(target(), debouncer = deb).scheduleSave()

        assertEquals(UserWordStore.SAVE_DEBOUNCE_MS, deb.lastDelayMs)
        assertTrue(deb.lastDelayMs in 1000..2000)
    }

    @Test
    fun saveIsAtomicAndLeavesNoTmpBehind() {
        val f = target().apply { writeText("old") }
        val engine = FakeEngine().apply { export = "new" }
        val deb = FakeDebouncer()

        store(f, engine, deb).scheduleSave()
        deb.fire()

        assertEquals("new", f.readText())
        // tmp + rename：写完 tmp 即改名，目录里不该留下半截文件
        assertFalse(File(tmp.root, UserWordStore.FILE_NAME + ".tmp").exists())
    }

    @Test
    fun saveGoesThroughRenameSoReadOnlyTargetStillSucceeds() {
        // 「原子写」唯一可观测的推论：写的是新文件再 rename 覆盖 —— rename 只需要目录写权限，
        // 不需要目标文件写权限。直接 file.writeText() 的实现在这里必然失败。
        val f = target().apply { writeText("old") }
        f.setWritable(false)
        Assume.assumeFalse("当前用户无视文件权限（root），该断言无意义", f.canWrite())
        val engine = FakeEngine().apply { export = "new" }
        val deb = FakeDebouncer()

        try {
            store(f, engine, deb).scheduleSave()
            deb.fire() // 不抛

            assertEquals("new", f.readText()) // 只读目标也被整份替换
        } finally {
            f.setWritable(true)
        }
    }

    @Test
    fun writeGoesThroughIoExecutorNotCallerThread() {
        val f = target()
        val engine = FakeEngine().apply { export = "j" }
        val deb = FakeDebouncer()
        val queued = mutableListOf<Runnable>()

        store(f, engine, deb, io = Executor { queued += it }).scheduleSave()
        deb.fire()

        assertEquals(1, engine.exportCalls) // 导出在调用线程（与其它引擎调用同线程）
        assertEquals(1, queued.size)
        assertFalse(f.exists()) // 文件系统调用在 io 线程：还没写

        queued.removeAt(0).run()

        assertEquals("j", f.readText())
    }

    // ---- 失败路径：一律静默，绝不冒泡到输入路径 ----

    @Test
    fun exportFailureIsSilentAndKeepsPreviousFile() {
        val f = target().apply { writeText("old") }
        val engine = FakeEngine().apply { throwOnExport = IOException("engine gone") }
        val deb = FakeDebouncer()

        store(f, engine, deb).scheduleSave()
        deb.fire() // 不抛

        assertEquals("old", f.readText()) // 旧用户词完好，没有被半截数据覆盖
    }

    @Test
    fun writeFailureIsSilentAndKeepsPreviousFile() {
        // 目标目录不可写（父路径是普通文件）→ 写 tmp 必失败
        val f = target().apply { writeText("old") }
        val blocker = File(tmp.root, "blocker").apply { writeText("x") }
        val bad = File(blocker, UserWordStore.FILE_NAME)
        val deb = FakeDebouncer()

        store(bad, debouncer = deb).scheduleSave()
        deb.fire() // 不抛

        assertEquals("old", f.readText()) // 无关文件不受影响
        assertFalse(File(blocker, UserWordStore.FILE_NAME + ".tmp").exists())
    }

    @Test
    fun deadLibraryOnExportIsSilent() {
        // 死 .so 时 export 抛的是 Error（UnsatisfiedLinkError），而防抖回调跑在主线程：
        // 逃逸出去就是崩 IME，catch(Exception) 兜不住
        val f = target()
        val engine = FakeEngine().apply { throwOnExport = UnsatisfiedLinkError("so missing") }
        val deb = FakeDebouncer()

        store(f, engine, deb).scheduleSave()
        deb.fire() // 不抛

        assertFalse(f.exists())
        assertEquals(1, engine.exportCalls)
    }
}
