// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Assume
import org.junit.Test
import java.io.File
import java.io.IOException
import java.util.concurrent.Executor

/**
 * 用户词落盘（学习持久化）纯 JVM 测试：真实临时目录 + 注入假防抖/假执行器/假引擎
 * （夹具见 [UserWordStoreTestBase]）。
 *
 * 「能不能区分修复前后」：UserWordStore 本身就是被考察的行为，且每个用例都断言到
 * 「不实现持久化就不可能满足」的点 —— 防抖窗口内 0 次写盘、窗口结束 1 次写盘、
 * 落盘走 io 线程、原子写不留 tmp、各失败路径静默且旧文件完好。
 *
 * 本类只放**单实例**行为；跨实例（IME ↔ 设置页）的数据安全见 [UserWordStoreRaceTest]，
 * 导入上限见 [UserWordStoreReadCapTest]。
 */
class UserWordStoreTest : UserWordStoreTestBase() {

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

    // ---- 设置页主动导入：与 load 相反，失败必须交回调用方（不许静默） ----

    @Test
    fun importFromAppliesFileAndPersistsImmediately() {
        val f = target()
        val engine = FakeEngine().apply { importResult = 2; export = """{"merged":1}""" }

        val r = store(f, engine).importFrom { """{"version":1,"words":[]}""" }

        assertEquals(UserWordStore.ImportResult.Imported(2), r)
        assertEquals(listOf("""{"version":1,"words":[]}"""), engine.imported) // 原文交给引擎，Kotlin 不解析
        // 立即落盘，且**不经过防抖**：FakeDebouncer 从没被 fire 过，走 scheduleSave 的实现
        // 在这里根本不会产生文件（导入是一次性动作，点完就被杀进程也不该丢）
        assertEquals("""{"merged":1}""", f.readText())
    }

    @Test
    fun rejectedImportReportsReasonAndChangesNothing() {
        val f = target().apply { writeText("""{"keep":1}""") }
        val engine = FakeEngine().apply { importResult = -1 } // 引擎拒收：负数

        val r = store(f, engine).importFrom { "{ not json" }

        assertTrue("拒收必须带原因给用户看：$r", r is UserWordStore.ImportResult.Rejected)
        assertTrue((r as UserWordStore.ImportResult.Rejected).reason.isNotEmpty())
        assertEquals(0, engine.exportCalls) // 拒收不得落盘
        assertEquals("""{"keep":1}""", f.readText()) // 既有用户词一字未动
    }

    @Test
    fun emptyFileIsRejectedByEngine() {
        val engine = FakeEngine().apply { importResult = -1 } // 空串 serde 必失败

        val r = store(target(), engine).importFrom { "" }

        assertTrue(r is UserWordStore.ImportResult.Rejected)
        assertEquals(listOf(""), engine.imported) // 空文件算不算有效也由引擎判，Kotlin 不猜
    }

    @Test
    fun emptyWordListIsSuccessWithZero() {
        val engine = FakeEngine().apply { importResult = 0 } // 合法空表：不是失败

        assertEquals(
            UserWordStore.ImportResult.Imported(0),
            store(target(), engine).importFrom { """{"version":1,"words":[]}""" },
        )
    }

    @Test
    fun unreadableFileIsRejectedWithoutTouchingEngine() {
        val engine = FakeEngine()

        val r = store(target(), engine).importFrom { throw IOException("Permission denied") }

        val reason = (r as UserWordStore.ImportResult.Rejected).reason
        assertTrue(reason, reason.contains("Permission denied")) // 读失败的原因要带到用户面前
        assertTrue(reason, reason.startsWith("无法读取所选文件")) // 默认来源：文件选择器
        assertTrue("读不出来就不该去碰引擎", engine.imported.isEmpty())
    }

    @Test
    fun readFailureNamesTheActualSource() {
        // 设置页有两条导入路径（文件 / 剪贴板）：剪贴板读不出来时说「无法读取所选文件」是骗人的
        val r = store(target(), FakeEngine()).importFrom("剪贴板") { throw IOException("没有文本") }

        val reason = (r as UserWordStore.ImportResult.Rejected).reason
        assertTrue(reason, reason.contains("剪贴板"))
    }

    @Test
    fun deadLibraryOnImportIsRejectedNotThrown() {
        // 死 .so 时抛的是 Error（UnsatisfiedLinkError），设置页不接住就是直接崩
        val engine = FakeEngine().apply { throwOnImport = UnsatisfiedLinkError("so missing") }

        val r = store(target(), engine).importFrom { "{}" } // 不抛

        assertTrue(r is UserWordStore.ImportResult.Rejected)
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
        // tmp + rename：写完 tmp 即改名，目录里不该留下半截文件（任何名字的都不留）
        assertEquals(emptyList<String>(), residue())
    }

    @Test
    fun twoInstancesNeverShareOneTmpName() {
        // 本类有**两个实例**（IME 与设置页），各带自己的单线程执行器 —— 「单线程保证两次
        // 落盘不会交叉写同一个 tmp」只对同一实例成立，固定 tmp 名会让两个实例的
        // FileOutputStream 交错写同一路径，把备好的用户词写坏。
        // 用 rename 必失败的目标（目录）逼出失败上报，上报文案里带着 tmp 名。
        val dir = File(tmp.root, UserWordStore.FILE_NAME).apply { mkdirs() }
        val reports = mutableListOf<String>()
        val debA = FakeDebouncer()
        val debB = FakeDebouncer()

        store(dir, debouncer = debA) { reports += it }.scheduleSave()
        debA.fire()
        store(dir, debouncer = debB) { reports += it }.scheduleSave()
        debB.fire()

        assertEquals(2, reports.size)
        // 两条文案只可能差 tmp 名（file.name 相同）⇒ 相同即两个实例撞了同一个 tmp
        assertNotEquals(reports[0], reports[1])
        assertEquals(emptyList<String>(), residue()) // 收尸也在：唯一名不再被下次保存覆盖
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
    fun invalidateDropsInFlightWrite() {
        val f = target()
        val engine = FakeEngine().apply { export = """{"keep":1}""" }
        val deb = FakeDebouncer()
        val queued = mutableListOf<Runnable>()

        store(f, engine, deb, io = Executor { queued += it }).scheduleSave()
        deb.fire()
        assertEquals(1, queued.size)
        assertFalse(f.exists())

        UserWordStore.invalidate()
        queued.removeAt(0).run()

        assertFalse("设置页清除后，在途写盘不得把旧词写回", f.exists())
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
        assertEquals(emptyList<String>(), residue()) // 连 tmp 都建不出来，更不该有残骸
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

    // ---- 失败必须被上报：落盘失败此前是完全静默的（无日志、无返回值） ----

    @Test
    fun writeFailureIsReported() {
        // 目标目录不可用（父路径是普通文件）→ 写 tmp 必失败
        val blocker = File(tmp.root, "blocker").apply { writeText("x") }
        val reports = mutableListOf<String>()
        val deb = FakeDebouncer()

        store(File(blocker, UserWordStore.FILE_NAME), debouncer = deb) { reports += it }
            .scheduleSave()
        deb.fire() // 不抛

        assertEquals(1, reports.size)
        assertTrue(reports[0], reports[0].isNotEmpty())
    }

    @Test
    fun renameFailureIsReported() {
        // 目标是**目录**：写 tmp 成功、rename 必失败（rename(2) 对目录目标返回 EISDIR）。
        // 原实现把 renameTo 的 Boolean 丢掉 —— 这一次学习结果凭空消失且毫无痕迹。
        val dir = File(tmp.root, UserWordStore.FILE_NAME).apply { mkdirs() }
        val reports = mutableListOf<String>()
        val deb = FakeDebouncer()

        store(dir, debouncer = deb) { reports += it }.scheduleSave()
        deb.fire() // 不抛

        assertTrue("目录目标应被保留", dir.isDirectory)
        assertEquals(1, reports.size) // rename 失败不再被吞掉
        assertTrue(reports[0].contains(UserWordStore.FILE_NAME))
        // 这条路径上 tmp **确实建出来了**（目录可写、只有 rename 失败）—— 唯一名不再被
        // 下一次保存覆盖，不显式删就是一个长期残骸。判据在目标**旁边**（`residue` 的注释）
        assertEquals(emptyList<String>(), residue())
    }

    @Test
    fun saveSuccessReportsNothing() {
        val reports = mutableListOf<String>()
        val deb = FakeDebouncer()

        store(target(), debouncer = deb) { reports += it }.scheduleSave()
        deb.fire()

        assertEquals(emptyList<String>(), reports) // 成功路径不产生噪音
    }
}
