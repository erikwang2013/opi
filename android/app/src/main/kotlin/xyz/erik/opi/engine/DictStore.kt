// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

import android.content.Context
import java.io.File
import java.io.IOException
import java.io.InputStream
import java.util.LinkedHashSet
import xyz.erik.opi.jni.OpiEngine

/**
 * 词库目录编排（2026-10-14，M3「词库管理」移动端半边；纯 JVM —— 注入 [DictApi]
 * 与目录即可测，类本体不碰 android.*，[Companion.from] 是唯一的生产接线点）。
 *
 * 用户词库住在 `filesDir/dicts` 下的 `*.tsv`，**目录就是用户词库的事实源**：引擎按文件名
 * （去扩展）给词库取名（ABI 契约），于是三件事都围绕这条命名约定 ——
 * 启动装载 = 目录里每个 `*.tsv` 都 [DictApi.importDict] 一遍；
 * 导入 = 流拷进目录 + 装载；删除 = 引擎与文件**两处同时做**（漏掉文件那一半，
 * 下次启动 [loadAll] 就把它装回来 —— 与「清除用户词库」必须同步删文件是同一课）。
 */
class DictStore(private val dir: File, private val api: DictApi) {

    /** 列表一项。`system=true` = 内置库（只读：没有开关也没有删除）。 */
    data class Entry(val name: String, val count: Int, val enabled: Boolean, val system: Boolean)

    /** 启动装载结果。[failed] 是失败文件的词库名（去扩展），顺序同目录序。 */
    data class LoadReport(val loaded: Int, val failed: List<String>)

    /** 导入结果。[Rejected.reason] 直接面向用户（设置页原样显示）。 */
    sealed interface ImportResult {
        /** [name] = 归一后的词库名（= 引擎里的名字，调用方拿它清停用名单 [withDisabled]）；
         * [count] = 引擎实际接收的词条数。 */
        data class Imported(val name: String, val count: Int) : ImportResult

        data class Rejected(val reason: String) : ImportResult
    }

    /** 目录里的词库文件（只认 `.tsv` 且是普通文件；按名字排序 —— 装载顺序确定、可复现）。 */
    fun files(): List<File> =
        (dir.listFiles() ?: emptyArray())
            .filter { it.isFile && it.name.endsWith(EXT) }
            .sortedBy { it.name }

    /** 词库列表（引擎为唯一事实源：系统库也在内）。坏 JSON → 空表，不崩（见 [parseList]）。 */
    fun list(): List<Entry> = parseList(api.dictList())

    /** 启用/停用（系统库由引擎拒绝，返回 false；设置页据此回滚开关）。 */
    fun setEnabled(name: String, on: Boolean): Boolean = api.setDictEnabled(name, on)

    /**
     * 删除词库：`removeDict` + **删文件**，两个都做 —— 漏掉文件那一半，下次启动
     * [loadAll] 会把它装回来（本仓「清除用户词库」必须同步删文件的先例）。
     *
     * 文件那一半**不看引擎脸色**：引擎拒绝（系统库 / 没装载过）时目录里若真有同名
     * tsv，那是「导入失败留下的坏文件」，删掉正是用户要的；系统库本来就不在本目录，
     * 删不到东西。`*.bak` 只在导入中途崩溃时残留，顺手清掉。
     */
    fun remove(name: String): Boolean {
        val ok = try {
            api.removeDict(name)
        } catch (e: Throwable) {
            // 死 .so 抛 Error 而非 Exception（同 EngineLoader 的 catch 理由）：文件那一半照做
            false
        }
        File(dir, "$name$EXT").delete()
        File(dir, "$name$EXT.bak").delete()
        return ok
    }

    /**
     * 内置词库**首启种子**（2026-10-16，康熙字典「全发」）：把随包的内置词库拷进词库
     * 目录**一次** —— 它随后就是一份普通导入词库（词库页可见、可停用、可删除）。
     *
     * **只种一次**（标记文件 `.seeded-<name>` 在则永不再种）：用户删过的不许复活 ——
     * 与 [remove] 的「文件那一半」语义**对偶**（那边删文件防「下次启动装回来」，
     * 这边不能反过来把它塞回去）。目录里已有同名文件（升级用户/手导过）⇒ 认账、
     * 不覆盖、打标记。源读不到（资产缺）⇒ 不种**也不打标记**（下次启动还有机会）。
     * 返回值 = 本次调用是否真的种下（供调用方记日志）。
     */
    fun seedBundled(name: String, open: () -> InputStream?): Boolean {
        val marker = File(dir, ".seeded-$name")
        if (marker.exists()) return false
        val target = File(dir, "$name$EXT")
        if (!dir.isDirectory && !dir.mkdirs()) return false
        if (target.exists()) {
            marker.createNewFile()
            return false
        }
        val src = open() ?: return false
        return try {
            // 流拷（同 importTsv 的纪律：整份读进内存是 OOM 的配方）
            src.use { ins -> target.outputStream().use { ins.copyTo(it) } }
            marker.createNewFile()
            true
        } catch (e: IOException) {
            target.delete() // 半截文件不留（同 importTsv 的坏文件纪律）
            false
        }
    }

    /**
     * 启动装载：目录里每个 `*.tsv` 都 `importDict` 一遍（在 `EngineLoader.load` 之后调用）。
     * 单个失败**不中断其余**（坏文件只进同一条报告，调用方据此给一条可见提示），不挡启动。
     *
     * [disabled] 是宿主持有的停用名单（[parseDisabled] 解析后的集合）：**装载之后重放**
     * ——引擎里导入层是内存态，同名重导入会把启用位置回 true（`api/dict.rs` 裁决 2），
     * 不重放的话「用户停用一个词库」会在每次进程重启后静默复活。只对**本轮装成功**的
     * 名字重放（文件名在了才需要停用；名单里的陈旧条目引擎自己会拒，不去喂它）。
     */
    fun loadAll(disabled: Set<String> = emptySet()): LoadReport {
        val failed = mutableListOf<String>()
        val loadedNames = mutableListOf<String>()
        for (f in files()) {
            val n = try {
                api.importDict(f.absolutePath)
            } catch (e: Throwable) {
                -1 // 死 .so 也算「这一个失败」，别的文件继续装
            }
            if (n < 0) failed += (nameOf(f.name) ?: f.name) else loadedNames += nameOf(f.name) ?: f.name
        }
        for (name in loadedNames) {
            if (name in disabled) {
                try {
                    api.setDictEnabled(name, false)
                } catch (e: Throwable) {
                    // 死 .so：整轮装载已经在上面每个文件上记过失败，这里不再单独报
                }
            }
        }
        return LoadReport(loadedNames.size, failed)
    }

    /**
     * 导入一个用户选的 tsv：**流拷**进 [dir]（不整份进内存 —— SAF 回调在主线程上，
     * `readText()` 式的整份读取就是 OOM/ANR 的配方）→ `importDict(绝对路径)`。
     *
     * 失败语义对齐 [UserWordStore.importFrom]：不抛，交回 [ImportResult.Rejected]。
     * 同名替换先把旧文件挪成 `.bak`，拷贝/装载失败就还回去 —— 坏文件不留（留着下次
     * 启动会反复装载失败、报同一条错），旧文件不丢（用户一次手滑不该毁掉一份好词库）。
     */
    fun importTsv(displayName: String, open: () -> InputStream): ImportResult {
        val name = nameOf(displayName)
            ?: return ImportResult.Rejected("文件名不能用（$displayName）")
        if (!dir.isDirectory && !dir.mkdirs()) return ImportResult.Rejected("建不了词库目录")
        val target = File(dir, "$name$EXT")
        val backup = File(dir, "$name$EXT.bak")
        if (target.exists()) {
            backup.delete()
            if (!target.renameTo(backup)) return ImportResult.Rejected("旧词库文件挪不动（$name$EXT）")
        }
        // 超限统一走这条：删新文件、还原旧文件，再把原因交给设置页原样显示。
        fun reject(reason: String): ImportResult {
            target.delete()
            restore(backup, target)
            return ImportResult.Rejected(reason)
        }
        // 有界拷贝：超 8 MiB 立刻停，不把一份超大文件先落满磁盘再判失败。
        var tooBig = false
        val copied = try {
            open().use { input ->
                target.outputStream().use { out ->
                    val buf = ByteArray(64 * 1024)
                    var total = 0L
                    while (true) {
                        val n = input.read(buf)
                        if (n < 0) break
                        total += n
                        if (total > MAX_BYTES) {
                            tooBig = true
                            break
                        }
                        out.write(buf, 0, n)
                    }
                }
            }
            !tooBig
        } catch (e: Throwable) {
            false
        }
        if (!copied) {
            return reject(
                if (tooBig) "词库文件达到上限（${MAX_BYTES / 1024 / 1024} MiB）"
                else "无法读取所选文件（$displayName）",
            )
        }
        // 条目数先按非空行估上界（引擎还会按有效行精确复核）；再核启用数/总条目。
        val lines = try {
            target.useLines { seq -> seq.count { it.isNotBlank() } }
        } catch (e: Throwable) {
            return reject("无法读取已拷贝的词库（${e::class.simpleName}）")
        }
        if (lines > MAX_ENTRIES) return reject("词库条目达到上限（$MAX_ENTRIES 条）")
        val others = try {
            list().filter { !it.system && it.enabled && it.name != name }
        } catch (e: Throwable) {
            emptyList()
        }
        if (others.size + 1 > MAX_ENABLED_DICTS) {
            return reject("同时启用的导入词库达到上限（$MAX_ENABLED_DICTS 个）")
        }
        if (others.sumOf { it.count } + lines > MAX_TOTAL_ENTRIES) {
            return reject("导入词库总条目达到上限（$MAX_TOTAL_ENTRIES 条）")
        }
        val count = try {
            api.importDict(target.absolutePath)
        } catch (e: Throwable) {
            return reject("引擎不可用（${e::class.simpleName}）")
        }
        if (count < 0) {
            return reject("不是有效的词库 tsv（引擎拒收；也可能达到上限）")
        }
        backup.delete()
        return ImportResult.Imported(name, count)
    }

    private fun restore(backup: File, target: File) {
        if (backup.exists()) backup.renameTo(target)
    }

    companion object {
        /** 词库目录名（`filesDir/dicts`）。 */
        const val DIR_NAME = "dicts"

        // 导入上限（2026-10-24 用户要求；与 engine-core 的 `api/dict.rs` 同值，
        // 引擎仍会二次校验 —— 这里是给设置页的**可读提示**，不替代引擎门禁）。
        const val MAX_BYTES = 8L * 1024 * 1024
        const val MAX_ENTRIES = 200_000
        const val MAX_ENABLED_DICTS = 8
        const val MAX_TOTAL_ENTRIES = 500_000

        /** 目录只认这个扩展名（契约：`*.tsv`，区分大小写 —— 启动装载与列表用同一口径）。
         * 导入时一律另存为 `<名>.tsv`，所以用户选的文件本身叫什么扩展名都不影响。 */
        const val EXT = ".tsv"

        /**
         * 停用名单串（逗号分隔的词库名，存 `ImePreferences.disabledDicts`）→ 集合。
         * 容错：去空白、丢空段；**空串 = 空集 = 全部启用**（缺省行为，与桌面
         * `DisabledDicts` 同一份契约 —— 词库名含逗号表达不了，名字带逗号极罕见，
         * 真撞上到那天再定转义）。
         */
        fun parseDisabled(v: String): Set<String> =
            v.split(',').map { it.trim() }.filter { it.isNotEmpty() }.toCollection(LinkedHashSet())

        /** 名单串 + 一名词库的启用位 → 新名单串（启用 = 从名单移除；全启用 ⇒ 空串）。 */
        fun withDisabled(v: String, name: String, enabled: Boolean): String {
            val next = parseDisabled(v).toMutableSet()
            if (enabled) next.remove(name) else next.add(name)
            return next.joinToString(",")
        }

        /**
         * 生产实例（设置页与 IME 启动各建一个 —— 无状态，不必共享）：
         * `filesDir/dicts` + [OpiEngine] 的四个词库出口。
         *
         * 用适配对象而不是让 `OpiEngine` 直接实现 [DictApi]：`OpiEngine.kt` 是 JNI 三份
         * 声明面之一（`jni_contract` 门禁钉着），给它加 `override` 就把那张表的改动
         * 绑到本接口上 —— JNI 注册表「整表连带」的坑不值得多一个这种耦合点。
         *
         * `dictList()` 声明可空（JNI 惯例）：null 折成 `[]`（引擎未装载的哨兵，同族约定）。
         */
        fun from(context: Context): DictStore = DictStore(
            dir = File(context.filesDir, DIR_NAME),
            api = object : DictApi {
                override fun importDict(path: String): Int = OpiEngine.importDict(path)
                override fun dictList(): String = OpiEngine.dictList() ?: "[]"
                override fun setDictEnabled(name: String, on: Boolean): Boolean =
                    OpiEngine.setDictEnabled(name, on)

                override fun removeDict(name: String): Boolean = OpiEngine.removeDict(name)
            },
        )

        /**
         * 文件名/路径 → 词库名（契约：名字 = 文件名去扩展）。`db/my-dict.tsv` → `my-dict`。
         * 取 basename 顺带把 `../x.tsv` 这类路径穿越压回本目录；剩下的不是能安全落盘的
         * 名字就 null（空、`.`、`..`、带分隔符）—— **显示名是别的应用给的，是信任边界**。
         */
        fun nameOf(raw: String): String? {
            val base = File(raw).name.substringBeforeLast('.', File(raw).name).trim()
            return base.takeIf {
                it.isNotEmpty() && it != "." && it != ".." &&
                    !it.contains('/') && !it.contains('\\')
            }
        }

        /**
         * `dictList()` JSON → 列表模型（serde 输出 `[{"name","count","enabled","system"}]`，
         * **键按字典序**，解析不得依赖顺序）。容错口径：坏 JSON / 空串 / 半截字节 → 空表；
         * 条目缺字段 → 取缺省（count=0、开关注 false）；**无名条目跳过**（列表点不动、
         * 开关没有 key，塞空串只是把坏数据往下游推）。
         *
         * 手写正则而非 org.json：JVM 测试没有 android.jar 运行时（同 `SymbolCatalog.parseBlocks`
         * 的理由与写法）。
         */
        fun parseList(json: String): List<Entry> {
            val out = mutableListOf<Entry>()
            for (m in objRe.findAll(json)) {
                val s = m.value
                val name = (nameRe.find(s)?.groupValues?.getOrNull(1) ?: "").replace("\\\"", "\"")
                if (name.isEmpty()) continue
                out += Entry(
                    name = name,
                    count = countRe.find(s)?.groupValues?.getOrNull(1)?.toIntOrNull() ?: 0,
                    enabled = enabledRe.find(s)?.groupValues?.getOrNull(1) == "true",
                    system = systemRe.find(s)?.groupValues?.getOrNull(1) == "true",
                )
            }
            return out
        }
    }
}

/**
 * 词库管理 JNI 面（OpiEngine 的词库四出口；JVM 测试注入假实现）。
 *
 * ⚠️ **不入 [OpiEngineApi]**（同 SymbolApi 的定位）：词库管理归设置页与启动编排，
 * 不进 IME 状态机；单独成面也让 `OpiEngine.kt` 的声明（JNI 注册表三份声明面之一）
 * 不必为本接口加 `override` —— 少一个跨面耦合点。
 */
interface DictApi {
    /** 装载一个 tsv（名字 = 文件名去扩展）。返回条目数；失败 -1（既有状态不变）。 */
    fun importDict(path: String): Int

    /** 词库列表 JSON `[{name,count,enabled,system}]`（系统库也在内）。 */
    fun dictList(): String

    /** 启用/停用；返回是否成功（系统库拒绝 → false）。 */
    fun setDictEnabled(name: String, on: Boolean): Boolean

    /** 卸载词库；返回是否成功。⚠️ 只动引擎内存，**不动文件**（文件那一半归 [DictStore.remove]）。 */
    fun removeDict(name: String): Boolean
}

// 固定 schema 专用解析（只消费引擎 serde 输出）。
// ⚠️ Android 用 ICU 正则：末尾 `}` 会被当量化符解析抛 PatternSyntaxException（JVM 测试抓不到）
// → 必须写成 `\}`（同 SymbolCatalog 的注释）。
private val objRe = Regex("""\{[^{}]*\}""")
private val nameRe = Regex(""""name":"((?:[^"\\]|\\.)*)"""")
private val countRe = Regex(""""count":(\d+)""")
private val enabledRe = Regex(""""enabled":(true|false)""")
private val systemRe = Regex(""""system":(true|false)""")
