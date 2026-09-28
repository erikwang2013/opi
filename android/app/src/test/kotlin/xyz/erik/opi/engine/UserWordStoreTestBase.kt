// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

import xyz.erik.opi.ime.Debouncer
import org.junit.Rule
import org.junit.rules.TemporaryFolder
import java.io.File
import java.util.concurrent.Executor

/**
 * [UserWordStore] 多个测试类共用的夹具：真实临时目录 + 假防抖 / 假引擎 / 假执行器。
 *
 * 抽出来**只是因为行数**（一个文件装不下），不是抽象 —— 复制三份会让假引擎各自漂移，
 * 而假引擎的语义（负数 = 引擎拒收、`Error` 而非 `Exception` = 死 .so）正是这些测试的
 * 判据本身。
 */
abstract class UserWordStoreTestBase {

    @get:Rule
    val tmp = TemporaryFolder()

    /** 假防抖：不排线程、不 sleep，手动 fire 才执行（对齐 ImeStateTest 的做法）。 */
    class FakeDebouncer : Debouncer {
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
    class FakeEngine {
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
    val directIo = Executor { it.run() }

    fun store(
        file: File,
        engine: FakeEngine = FakeEngine(),
        debouncer: FakeDebouncer = FakeDebouncer(),
        io: Executor = directIo,
        onWriteFailure: (String) -> Unit = {},
    ) = UserWordStore(file, engine::importJson, engine::exportJson, debouncer, io, onWriteFailure)

    fun target(): File = File(tmp.root, UserWordStore.FILE_NAME)

    /**
     * 落盘残骸（名字里带 `.tmp` 的东西）。**扫的是 `tmp.root`**，因为 tmp 建在
     * `file.parentFile`（目标**旁边**），不是目标本身。
     *
     * ⚠️ 这里踩过：rename 失败场景里目标的身份是个**目录**（`tmp.root/opi_user_words.json`），
     * 一度写成扫那个目标目录 —— tmp 根本不在里面，断言静默变成空断言（把它挪对之前，
     * 删除收尸逻辑的变异**照样全绿**）。
     *
     * tmp 名唯一之后，「不存在恰好叫 `FILE_NAME.tmp` 的东西」不再是不变式 —— 真正要守的是
     * 「不留残骸」。这个判据**更强**：改名后的残留它照样抓得到，而旧判据对它失明。
     */
    fun residue(): List<String> =
        (tmp.root.listFiles() ?: emptyArray()).map { it.name }.filter { it.contains(".tmp") }
}
