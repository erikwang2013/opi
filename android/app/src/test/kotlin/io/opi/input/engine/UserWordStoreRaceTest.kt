// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package io.opi.input.engine

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File
import java.util.Collections
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executor
import java.util.concurrent.TimeUnit

/**
 * **跨实例**的数据安全：IME 与设置页各持一个 [UserWordStore]（同文件、同进程）。
 *
 * 单实例行为见 [UserWordStoreTest]；这里只放「两个实例同时在写同一个文件」才会暴露的问题
 * —— 这类缺陷在单实例测试里**结构上不可能**出现，所以必须单独一个类。
 */
class UserWordStoreRaceTest : UserWordStoreTestBase() {

    /**
     * 审计实测的数据丢失（v1.3.0）：toast 报「已导入 N 条」、引擎内存也对，**文件却是旧的**。
     *
     * ```
     * T0  IME 防抖导出 OLD（任务已进 io 队列，尚未 rename）
     * T1  设置页导入导出 NEW
     *     落盘后 = OLD      ← 旧快照胜出，导入凭空消失（下次 load() 读回旧文件）
     * ```
     *
     * 两个实例各带独立执行器 ⇒ rename 顺序不受任何约束。这里显式把两个队列交回测试线程
     * 执行，并**按实测顺序**先跑设置页那次（新）、后跑 IME 那次（旧）—— 旧的那次必须
     * 自己作废，而不是靠「它恰好排在新写盘前面」。
     *
     * ⚠️ 这个用例同时钉住了 [UserWordStore.invalidate] 的**调用顺序**：世代号是「全部作废」
     * 而非「作废更旧的」，落在 `saveSoon()` 之后会把导入自己那次写盘也拦掉 —— 那时本用例
     * 断言的文件根本不会出现，照样红。
     */
    @Test
    fun staleInFlightWriteCannotOverwriteAnImport() {
        val f = target()
        val ime = FakeEngine().apply { export = """{"OLD":1}""" }
        val settings = FakeEngine().apply { export = """{"NEW_with_import":1}""" }
        val imeQueue = mutableListOf<Runnable>()
        val settingsQueue = mutableListOf<Runnable>()
        val debIme = FakeDebouncer()
        val sIme = store(f, ime, debIme, io = Executor { imeQueue += it })
        val sSettings = store(f, settings, io = Executor { settingsQueue += it })

        sIme.scheduleSave()
        debIme.fire() // T0：OLD 导出，任务入队（尚未落盘）
        val r = sSettings.importFrom { """{"version":1,"words":[]}""" } // T1：NEW 导出，任务入队

        assertEquals(UserWordStore.ImportResult.Imported(1), r)
        assertEquals(1, imeQueue.size)
        assertEquals(1, settingsQueue.size)

        settingsQueue.removeAt(0).run() // 新写盘先落地
        imeQueue.removeAt(0).run() // 迟到的旧 rename —— 不许胜出

        assertEquals("""{"NEW_with_import":1}""", f.readText())
    }

    /**
     * 上一条的结构性保证：生产（不注入 io）时两个实例必须落**在同一条**队列上，
     * 于是「rename 顺序 = 入队顺序 = 调用方线程上的导出顺序」。
     *
     * 观测点选「父目录为 null」的失败路径：它是**唯一**在 io 线程上执行的回调，借它直接
     * 读到落盘线程的身份。同一个单线程执行器 ⇒ `Thread` 是同一个对象；每实例一个执行器
     * ⇒ 两个不同的 `Thread`（确定性红，不依赖时序）。
     */
    @Test
    fun twoInstancesSaveOnTheSameThread() {
        val threads = Collections.synchronizedList(mutableListOf<Thread>())
        val done = CountDownLatch(2)

        fun instance(name: String): Pair<UserWordStore, FakeDebouncer> {
            val deb = FakeDebouncer()
            // 刻意**不传 io**：走生产默认值，这才是被考察的那条路径
            val s = UserWordStore(
                file = File("$name.json"), // parentFile == null → 不进文件系统，只借道上报
                importJson = { 0 },
                exportJson = { "{}" },
                debouncer = deb,
                onWriteFailure = {
                    threads += Thread.currentThread()
                    done.countDown()
                },
            )
            return s to deb
        }

        val (a, debA) = instance("a")
        val (b, debB) = instance("b")
        a.scheduleSave()
        debA.fire()
        b.scheduleSave()
        debB.fire()

        assertTrue("落盘没跑完", done.await(5, TimeUnit.SECONDS))
        assertEquals(2, threads.size)
        assertSame("两个实例必须在同一条落盘队列上", threads[0], threads[1])
    }

    // ---- 崩溃残骸回收：唯一 tmp 名之后，「下次保存覆盖 tmp」不再成立 ----

    /**
     * 进程被 kill（不是正常退出）会留下 `*.tmp`，且没有任何人回收 —— 每崩一次永久多一个。
     * 启动时扫一遍。
     */
    @Test
    fun loadSweepsCrashResidue() {
        val f = target().apply { writeText("{}") }
        val corpse = File(tmp.root, "${UserWordStore.FILE_NAME}.1234$TMP").apply {
            writeText("half-written")
            setLastModified(System.currentTimeMillis() - 10 * UserWordStore.RESIDUE_MIN_AGE_MS)
        }

        store(f, FakeEngine()).load()

        assertFalse("上次进程留下的残骸必须被回收：${corpse.name}", corpse.exists())
    }

    /**
     * 但**正在被写的** tmp 不能删：设置页的导入写盘随时可能在飞（两个实例同进程），
     * 删掉它等于那次导入静默丢失 —— 比留个残骸严重得多。
     */
    @Test
    fun loadKeepsTmpThatIsStillBeingWritten() {
        val f = target().apply { writeText("{}") }
        val live = File(tmp.root, "${UserWordStore.FILE_NAME}.9999$TMP").apply { writeText("half") }

        store(f, FakeEngine()).load()

        assertTrue("刚建出来的 tmp 可能是别的实例正在写的", live.exists())
    }

    private companion object {
        const val TMP = ".tmp"
    }
}
