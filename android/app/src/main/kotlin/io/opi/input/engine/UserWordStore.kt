// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package io.opi.input.engine

import io.opi.input.ime.Debouncer
import java.io.File
import java.io.FileOutputStream
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
    /**
     * 落盘失败上报（写 tmp / fsync / rename 任一失败）。本类必须保持纯 JVM
     * （测试里不能碰 android.util.Log），日志由宿主注入 —— 静默的落盘失败
     * 等于学习结果凭空消失，排障时无从下手。
     */
    private val onWriteFailure: (String) -> Unit = {},
) {
    companion object {
        /** filesDir 下的用户词文件名（与 luna.opid / trad.opid 同目录）。 */
        const val FILE_NAME = "opi_user_words.json"

        /** 落盘防抖窗口：连打选词（间隔 < 1.5s）合并成一次写盘。 */
        const val SAVE_DEBOUNCE_MS = 1500L

        private const val TMP_SUFFIX = ".tmp"

        @Volatile
        private var saveEpoch = 0

        /**
         * 作废所有在途落盘。设置页清词与 IME 是两个 UserWordStore 实例，
         * 只能靠进程级世代号拦住「导出已完成、io 线程尚未 rename」的旧 JSON。
         */
        fun invalidate() {
            saveEpoch++
        }

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
     * 用户在设置页**主动**导入（与 [load] 相反：失败必须交回调用方，不得静默）。
     *
     * 判定权全在引擎（Kotlin 侧不解析 JSON，只认负数为拒收），理由同 [load]：
     * 版本/条数上限/空词条这些规则只有一份实现，抄到 Kotlin 侧必然漂移。
     * ⚠️ JNI 把三种失败（JSON 非法 / 引擎未 load / panic）合并成同一个 `-1` 哨兵，
     * 本侧**无法区分**，所以文案只说「不是有效的用户词表」，别在这里编更细的原因。
     *
     * 成功后**立即落盘**（不走 [SAVE_DEBOUNCE_MS]）：导入是用户点的一次性动作，
     * 而防抖窗口里进程一死（点完就划掉任务）就等于没导入 —— 与设置页「清除」必须
     * 同步删文件同理，一次显式动作不该把结果挂在 1.5 秒后的回调上。
     *
     * [source] 只进读失败的文案（设置页有两条导入路径：文件 / 剪贴板）——
     * 剪贴板读不出来时说「无法读取所选文件」是骗人的。
     */
    fun importFrom(source: String = "所选文件", readText: () -> String): ImportResult {
        val json = try {
            readText()
        } catch (e: Throwable) {
            // 选了目录/URI 失效/被撤权/文件已删/剪贴板无文本：读不出来就得说读不出来
            return ImportResult.Rejected("无法读取$source（${e.message ?: e::class.simpleName}）")
        }
        val count = try {
            importJson(json)
        } catch (e: Throwable) {
            // 死 .so 抛的是 Error 而非 Exception（见 load 的注释）—— 这里要报给用户，不是静默
            return ImportResult.Rejected("引擎不可用（${e::class.simpleName}）")
        }
        if (count < 0) return ImportResult.Rejected("不是有效的用户词表 JSON")
        saveSoon()
        return ImportResult.Imported(count)
    }

    /** 导入结果。[Rejected.reason] 直接面向用户（设置页原样显示）。 */
    sealed interface ImportResult {
        /** [count] = 引擎实际接收的词条数（合法的空词表是 0，不是失败）。 */
        data class Imported(val count: Int) : ImportResult

        data class Rejected(val reason: String) : ImportResult
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
        val token = saveEpoch
        val json = try {
            exportJson()
        } catch (e: Throwable) {
            return
        }
        io.execute {
            if (token != saveEpoch) return@execute
            // 原子写：先写同目录 tmp、**fsync**、再 rename（POSIX 同目录 rename 是原子替换）。
            // 崩在写 tmp 时旧的用户词仍完好。
            //
            // fsync 不是可选项：只 writeText 的话数据停在页缓存里，而 rename 是元数据
            // 操作，可能先于数据落盘 —— 掉电后目标文件存在却是 0 字节。注释原先声称的
            // 「进程在任何时刻被杀都安全」对 kill -9 成立，对掉电不成立；补上 sync 才
            // 两头都成立。
            //
            // tmp 名**每次唯一**（createTempFile）：本类有**两个实例**（IME 与设置页），
            // 各带自己的单线程执行器 —— 「单线程保证两次落盘不会交叉写同一个 tmp」只对
            // **同一个实例**成立，固定名会让两个实例的 FileOutputStream 交错写同一路径。
            // 代价：老注释里「下次保存覆盖 tmp」不再成立 → 失败路径必须自己收尸（见下），
            // 否则每失败一次就在 filesDir 留一个残骸。
            val dir = file.parentFile
            if (dir == null) {
                // createTempFile(…, dir=null) 会**静默**把 tmp 落到系统临时目录，再 rename
                // 跨文件系统必然失败 —— 宁可在这里报出来
                onWriteFailure("no parent dir for ${file.name}")
                return@execute
            }
            val tmp = try {
                File.createTempFile(file.name + ".", TMP_SUFFIX, dir)
            } catch (e: Throwable) {
                // 目录不可写/父路径不是目录：连 tmp 都建不出来，同属「这次学习结果丢了」
                onWriteFailure("create tmp for ${file.name} failed: ${e.message}")
                return@execute
            }
            try {
                FileOutputStream(tmp).use { out ->
                    out.write(json.toByteArray(Charsets.UTF_8))
                    out.fd.sync()
                }
                // rename 失败（目录只读/目标被换成目录）只丢这一次学习结果，保留旧文件；
                // 但返回值必须看一眼 —— 丢掉它等于失败无声无息
                if (!tmp.renameTo(file)) {
                    onWriteFailure("rename ${tmp.name} -> ${file.name} failed")
                    cleanup(tmp)
                }
            } catch (e: Throwable) {
                // 磁盘满/目录不可写：不冒泡（会崩 IME），但要留痕
                onWriteFailure("write ${file.name} failed: ${e.message}")
                cleanup(tmp)
            }
        }
    }

    /** 失败收尸。唯一名之后「下次保存覆盖 tmp」不再成立，不清就是长期堆积的残骸。 */
    private fun cleanup(tmp: File) {
        if (tmp.exists() && !tmp.delete()) onWriteFailure("tmp ${tmp.name} left behind")
    }
}
