// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

import xyz.erik.opi.ime.Debouncer
import java.io.File
import java.io.FileOutputStream
import java.io.Reader
import java.util.concurrent.Executor
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicInteger

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
    /**
     * 落盘线程。生产：**进程级共享**的单线程执行器（见 [sharedIo]）——
     * 共享才保证「rename 顺序 = 入队顺序 = 导出顺序」。测试注入直接执行器。
     */
    private val io: Executor = sharedIo,
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

        /**
         * 导入文本上限（字符）。**是 DoS 护栏，不是数据规则** —— 取值必须显著高于引擎
         * `MAX_IMPORT_WORDS = 100_000` 条所能产生的 JSON（导出形如 `{"text":"…","freq":N}`，
         * 约 25 字符/条 ⇒ 上限词表约 2.5M 字符），好让「条数超限」永远由**引擎**报
         * （它的错误信息更准确）。抄小了会把引擎本来收得下的词表挡在门外，
         * 而用户看到的只是「文件过大」。
         */
        const val MAX_IMPORT_CHARS = 16 * 1024 * 1024

        /**
         * [sweepResidue] 的年龄门槛：比这更新的 tmp 一律不删。
         *
         * 正在被写的 tmp 从建立到 rename 只有毫秒级，而崩溃残骸至少隔了一个进程生命周期
         * —— 门槛把两者分得很开，代价是残骸要多留一次启动。
         */
        const val RESIDUE_MIN_AGE_MS = 60_000L

        private const val TMP_SUFFIX = ".tmp"

        /**
         * 世代号（见 [invalidate]）。用原子量而不是 `@Volatile var` + `++`：后者是
         * 「读-改-写」，并发调用会丢增量。这里丢增量**目前**无害（单向闸：任何一次自增
         * 都作废它之前捕获的所有 token），但那是靠推理维持的性质，而重建它只要三行。
         */
        private val saveEpoch = AtomicInteger(0)

        /**
         * 作废所有在途落盘。设置页清词与 IME 是两个 UserWordStore 实例，
         * 只能靠进程级世代号拦住「导出已完成、io 线程尚未 rename」的旧 JSON。
         *
         * ⚠️ **必须落在要保住的那次** [scheduleSave] / [importFrom] **之前**。
         * 它是「**全部**作废」而不是「作废更旧的」：token 在导出那一刻捕获，所以本轮
         * 自增会把自己之后**同一次**调用的写盘也拦掉 —— 调用顺序反了，那次写盘凭空消失
         * 且没有任何上报。[UserWordStoreRaceTest] 里那条数据丢失用例正是这么钉的：
         * 顺序反了不是「拦不住旧的」，而是连新的都不落盘，两种错法都会红。
         */
        fun invalidate() {
            saveEpoch.incrementAndGet()
        }

        /**
         * 落盘线程：**进程级共享**的单线程执行器（守护线程，不拖住测试 JVM 退出；
         * Android 进程内无影响）。
         *
         * 必须共享，不能每实例一个：本类有**两个实例**（IME 与设置页），同一文件。
         * 各带一个执行器时两次 rename 互相竞争、顺序不受任何约束 —— 审计实测的
         * 「IME 的旧快照在设置页导入之后落盘、把导入整份盖掉」就是这么来的。共享一条
         * 队列后，入队序 = 调用方线程上的导出序 = rename 序，**最后导出的必然最后落盘**。
         * 顺带把「两次落盘不会交叉写同一个 tmp」从「同一实例内」升级成全类成立。
         * 代价是慢写盘会挡住另一次写盘 —— 这里只有几 KB 的 JSON，可以接受。
         *
         * ⚠️ 上面那条等式**隐含一个前提**：[saveSoon] 是「先 [exportJson]、后入队」两步，
         * 两步之间不是原子的，所以「入队序 = 导出序」只在**所有调用方都跑在同一线程**时
         * 成立。今天两处调用方（IME 的防抖回调、设置页的导入）都在主线程，成立。
         * **若将来有人在后台线程上调 [scheduleSave]，这条保证就断了**：两个线程可能
         * 一个先导出后入队、另一个反过来，旧快照又能最后落盘。那时要么把调用收回主线程，
         * 要么在 [saveEpoch] 上加一个「按导出序发号」的序列（见 [invalidate] 的说明）。
         */
        private val sharedIo: Executor =
            Executors.newSingleThreadExecutor { r ->
                Thread(r, "opi-user-words").apply { isDaemon = true }
            }

        /**
         * 有界读：最多读 [limit] 个字符，**超限返回 null**（由调用方转成用户可见的拒绝）。
         *
         * 为什么不让调用方 `readText()`：那样整份文件先变成 Kotlin String，上限只能在其后
         * 生效 —— 设置页的文件选择器回调跑在**主线程**上，「挑了个 2GB 文件」就是主线程
         * OOM/ANR，而引擎的条数上限根本来不及说话。这里累加着一超上限就返回，返回的
         * String 长度有界；从流里最多多读一个 [READ_CHUNK_CHARS] 块（那点内容当场丢弃），
         * 但整份文件从未被构造出来。
         *
         * 读失败**不在这里吞**：异常原样冒到 [importFrom] 的 catch（那里的文案带 source）。
         * 把读失败也变成 null 会让用户看到「文件过大」这种假话。
         */
        fun readImportText(reader: Reader, limit: Int = MAX_IMPORT_CHARS): String? {
            val sb = StringBuilder()
            val buf = CharArray(READ_CHUNK_CHARS)
            while (true) {
                val n = reader.read(buf)
                if (n < 0) return sb.toString()
                if (sb.length + n > limit) return null
                sb.append(buf, 0, n)
            }
        }

        /**
         * 同上，输入已经是 String 的那条路（剪贴板：`coerceToText()` 必然先整份物化）。
         *
         * 本函数挡不住 `coerceToText()` 自己那一步 —— 它只能挡住其后的 JSON 解析与引擎导入。
         * 仍值得有：超限时用户拿到的是可见的「过大」拒绝，而不是主线程上漫长无反馈的解析。
         */
        fun readImportText(text: String, limit: Int = MAX_IMPORT_CHARS): String? =
            if (text.length > limit) null else text

        private const val READ_CHUNK_CHARS = 8192
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
     *
     * 顺带做一次崩溃残骸清扫（[sweepResidue]）—— 进程被 kill 留下的 `*.tmp` 只有这里收。
     */
    fun load() {
        if (loaded) return
        loaded = true
        sweepResidue()
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
        // 顺序不能反（见 invalidate）：先作废掉本次导入**之前**导出的在途快照。
        // 少了这一步，IME 那条「已导出、尚未 rename」的旧 JSON 可能落在本文件的 rename
        // **之后**，把刚导入的词整份盖回去 —— 而 toast 已经说了「已导入 N 条」，
        // 引擎内存也确实是新的，只有文件是旧的（重启即「导入凭空消失」）。
        invalidate()
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
        val token = saveEpoch.get()
        val json = try {
            exportJson()
        } catch (e: Throwable) {
            return
        }
        io.execute {
            if (token != saveEpoch.get()) return@execute
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

    /**
     * 启动清扫：收掉上次进程被杀留下的残骸。
     *
     * 单次写盘的失败路径自己收尸（[cleanup]），但 `kill -9` / 系统杀后台 / 掉电不会 ——
     * tmp 名唯一之后「下次保存覆盖 tmp」不再成立，不收就是**每崩一次永久多一个文件**。
     * 挂在 [load] 上（每次装载扫一遍），清扫本身是幂等的。
     *
     * ⚠️ 只删**够旧**的（[RESIDUE_MIN_AGE_MS]）：本类有两个实例，设置页的导入写盘随时
     * 可能在飞，正在被写的 tmp 删掉 = 那次导入静默丢失 —— 比留个残骸严重得多。
     */
    private fun sweepResidue() {
        val dir = file.parentFile ?: return
        val files = dir.listFiles() ?: return
        val prefix = file.name + "."
        val cutoff = System.currentTimeMillis() - RESIDUE_MIN_AGE_MS
        for (f in files) {
            if (f.name.startsWith(prefix) && f.name.endsWith(TMP_SUFFIX) && f.lastModified() < cutoff) {
                if (!f.delete()) onWriteFailure("tmp ${f.name} left behind")
            }
        }
    }

    /** 失败收尸。唯一名之后「下次保存覆盖 tmp」不再成立，不清就是长期堆积的残骸。 */
    private fun cleanup(tmp: File) {
        if (tmp.exists() && !tmp.delete()) onWriteFailure("tmp ${tmp.name} left behind")
    }
}
