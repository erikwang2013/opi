// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package io.opi.input.engine

import io.opi.input.ime.Debouncer
import java.io.File
import java.util.concurrent.Executor
import java.util.concurrent.Executors

/**
 * 用户词（自造词 + 频次）落盘。引擎侧只有纯函数（`import_json` / `export_user_words`），
 * 文件读写与时机在 Kotlin 这一侧 —— Learner 是纯内存的，进程一被杀学过的词全忘，
 * 于是「最常用的那个词永远排在最后」。
 *
 * 不变式（同 EngineLoader）：**用户词文件坏了绝不崩输入法** —— 读取、导入、导出、
 * 写盘的任何失败都静默降级为「没有用户词」，不向调用方抛。
 *
 * 纯 JVM 可测：File 是 java.io；Debouncer/Executor 注入（测试用假防抖 + 直接执行器，
 * 无 Handler、无线程、无 sleep）。
 */
class UserWordStore(
    private val file: File,
    /** api.importUserWords：返回导入条数，负数 = JSON 非法（引擎侧保持既有状态不变）。 */
    private val importJson: (String) -> Int,
    /** api.exportUserWords：当前用户词 JSON。 */
    private val exportJson: () -> String,
    private val debouncer: Debouncer,
    /** 落盘线程。生产：单线程后台执行器（顺带保证两次落盘不会交叉写同一个 tmp）。 */
    private val io: Executor = newIoExecutor(),
) {
    companion object {
        /** filesDir 下的用户词文件名（与 luna.opid / trad.opid 同目录）。 */
        const val FILE_NAME = "opi_user_words.json"

        /** 落盘防抖窗口：连打选词（间隔 < 1.5s）合并成一次写盘。 */
        const val SAVE_DEBOUNCE_MS = 1500L

        private const val TMP_SUFFIX = ".tmp"

        /** 守护线程：不拖住测试 JVM 退出（Android 进程内无影响）。 */
        private fun newIoExecutor(): Executor =
            Executors.newSingleThreadExecutor { r ->
                Thread(r, "opi-user-words").apply { isDaemon = true }
            }
    }

    private var pending: (() -> Unit)? = null
    private var loaded = false

    /**
     * 启动导入（引擎装载后调用一次，主线程）。文件缺失/不可读/JSON 非法/引擎不可用
     * 一律静默降级。
     *
     * 同步而非异步：import 改的是引擎的 Learner 状态，必须与其它引擎调用同线程
     * （Rust 单例没有跨线程保证），且要赶在用户第一次选词之前生效；文件本身很小
     * （同一处的 EngineLoader 连 1.24MB 词库都在这个线程读）。
     *
     * 非法文件**不删除也不备份**：这份文件坏了就等于没学过 —— 没有可恢复的信息，
     * 删除/改名只是多一次写盘、多一个失败点；留着还能人工看一眼，而下一次成功保存
     * 会原子覆盖它，脏文件不会长期留存。
     */
    fun load() {
        if (loaded) return
        loaded = true
        val json = try {
            file.readText()
        } catch (e: Exception) {
            return // 文件缺失/不可读 → 没有用户词
        }
        try {
            importJson(json)
        } catch (e: Throwable) {
            // 死 .so：OpiEngine 是 object，System.loadLibrary 失败后每次调用抛的都是
            // UnsatisfiedLinkError / NoClassDefFoundError（Error 而非 Exception），
            // catch(Exception) 拦不住 —— 崩在这里就是「开机即崩」。
        }
    }

    /**
     * 学习状态变更（选词/删用户词）后调用：防抖合并，窗口结束后导出并在 io 线程落盘。
     *
     * 导出留在调用线程（引擎调用 + 小 JSON 序列化，与其它引擎调用同线程，不新增并发面），
     * 只有文件系统调用进 io 线程 —— 那才是慢且不可预测的部分（闪存抖动、磁盘满）。
     */
    fun scheduleSave() {
        pending?.invoke()
        pending = debouncer.schedule(SAVE_DEBOUNCE_MS) {
            pending = null
            saveSoon()
        }
    }

    private fun saveSoon() {
        // 导出失败不得从防抖回调里逃逸：那跑在主线程 Handler 上，抛出去就是崩 IME。
        val json = try {
            exportJson()
        } catch (e: Throwable) {
            return
        }
        io.execute {
            // 原子写：先写同目录 tmp 再 rename（POSIX 同目录 rename 是原子替换）。
            // 进程在任何时刻被杀，目标文件要么是旧内容要么是新内容，不会是写了一半的
            // JSON；崩在写 tmp 时旧的用户词仍完好（下次保存覆盖 tmp）。
            try {
                val tmp = File(file.parentFile, file.name + TMP_SUFFIX)
                tmp.writeText(json)
                // rename 失败（目录只读/被换成目录）只丢这一次学习结果，保留旧文件
                tmp.renameTo(file)
            } catch (e: Throwable) {
                // 磁盘满/目录不可写：静默。下一次变更会再触发一次保存。
                // ponytail: 不报错也没有日志，要排查落盘失败就在这里挂一个回调
            }
        }
    }
}
