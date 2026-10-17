// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.jni

import android.content.Context
import android.util.Log
import java.io.File
import java.io.IOException

import xyz.erik.opi.settings.ImePreferences

/**
 * luna.opid 资产加载编排（对齐 flutter engine_controller.dart _loadLuna）：
 * - 资产解压到 filesDir；不存在或 size 不一致即重拷（app 升级后 asset 更新而旧文件残留）
 * - OpiEngine.load(path) 失败 → load(null) 回退内置 35 词词库（Rust install 语义）
 *
 * 纯 JVM 可测：needsCopy/loadAsset 不依赖 android.*，注入 FileOps/LoadApi 即可
 * （JVM 单测中 android.util.Log 为 stub，会抛 "not mocked"——日志只留在生产入口）。
 */
object EngineLoader {

    const val ASSET_NAME = "luna.opid"
    const val FILE_NAME = "luna.opid"
    const val ASSET_NAME_TRAD = "trad.opid"
    const val FILE_NAME_TRAD = "trad.opid"
    /** 笔画词典（2026-10-09，笔画键盘 / 生僻字键盘）。5.4MB —— 同样走 size 校验重拷。 */
    const val ASSET_NAME_STROKE = "stroke.opid"

    /** 五笔码表两张（2026-10-14，M4）：按用户选的方案装**一张**（见 `load` 的尾段）。 */
    const val ASSET_NAME_WUBI86 = "wubi86.opid"
    const val ASSET_NAME_WUBI98 = "wubi98.opid"
    const val FILE_NAME_STROKE = "stroke.opid"
    /** 五笔码表的落地名（两张，按方案二选一；见 [ASSET_NAME_WUBI86] 段）。 */
    const val FILE_NAME_WUBI86 = "wubi86.opid"
    const val FILE_NAME_WUBI98 = "wubi98.opid"
    private const val TAG = "EngineLoader"

    /** 文件操作抽象（JVM 测试注入假实现）。 */
    interface FileOps {
        /**
         * 资产大小（字节）；无法获取（如资产在 APK 中为 gzip 压缩、openFd 不可用）
         * 返回 null → 视为需要重拷，走 readAsset 完整读取。缺失/损坏不在此抛错。
         */
        fun assetLength(): Long?

        /** 读资产字节。资产缺失/损坏抛 IOException。 */
        @Throws(IOException::class)
        fun readAsset(): ByteArray

        /** 目标文件当前大小；不存在返回 null。 */
        fun existingSize(): Long?

        /** 覆盖写目标文件。 */
        fun write(bytes: ByteArray)
    }

    /** 引擎加载抽象（JVM 测试注入假实现；生产为 OpiEngine）。 */
    fun interface LoadApi {
        /** 加载 path；null/空串 → 内置回退词库；坏路径 → false。 */
        fun load(path: String?): Boolean
    }

    /** 繁体词典加载抽象（trad 是可选增强：失败不回退内置，不触碰已装的 luna）。 */
    fun interface LoadTradApi {
        fun loadTrad(path: String): Boolean
    }

    /** 笔画词典加载抽象（同上，可选增强：失败 = 笔画方案回退普通路径）。 */
    /** 五笔装载口（`OpiEngine.loadWubi`）—— 与 LoadStrokeApi 同一条形状（可注入假实现）。 */
    fun interface LoadWubiApi {
        fun loadWubi(path: String): Boolean
    }

    fun interface LoadStrokeApi {
        fun loadStroke(path: String): Boolean
    }

    /** size 校验重拷规则（对齐 flutter：不存在或 size 不一致即重拷）。 */
    fun needsCopy(assetSize: Long, existingSize: Long?): Boolean =
        existingSize == null || existingSize != assetSize

    /**
     * 编排（纯 JVM，无 android.*）：先查资产 size（命中已缓存副本则跳过 1.7MB 字节读取）→
     * 需重拷时 readAsset+write → load(path)。
     * 任一步 I/O 失败（资产缺失/损坏/写盘失败）→ load(null) 回退内置 35 词词库，返回 false
     * （不抛异常，避免 IME 进程在 onCreateInputView 崩溃）；load 失败同样回退。
     *
     * **边界（别把这里当作「so 坏了也不崩」的保证）**：本函数只覆盖**词库**层面的失败。
     * so 缺失/ABI 不匹配是另一个失败域，且发生得更早 —— `OpiEngine` 是 object，
     * `OpiImeService.kt:28` 的 `EngineController()` 默认参数在 Service 构造时就触碰它
     * （`System.loadLibrary` 在类初始化里），`SettingsScreen` 首次组合也读 `OpiEngine`，
     * 两者都在 EngineLoader 之前。要让那种情形也降级，得在 `OpiEngine` 的类初始化处加守卫。
     */
    fun loadAsset(fileOps: FileOps, api: LoadApi, targetPath: String): Boolean {
        try {
            val assetSize = fileOps.assetLength()
            if (assetSize == null || needsCopy(assetSize, fileOps.existingSize())) {
                fileOps.write(fileOps.readAsset())
            }
        } catch (e: IOException) {
            fallback(api) // I/O 失败 → 回退内置词库（与 flutter catch → loadFallback 一致）
            return false
        } catch (e: Exception) {
            fallback(api) // 兜底：任何异常都不得让 IME 崩溃
            return false
        }
        // 捕 Throwable 是有意的：OpiEngine 是 object，init 里 System.loadLibrary("opi_ffi")
        // 失败抛的是 UnsatisfiedLinkError（Error 而非 Exception）——so 缺失/ABI 不匹配时
        // catch(Exception) 拦不住，设置页会在启动时直接崩。
        val ok = try {
            api.load(targetPath)
        } catch (e: Throwable) {
            false
        }
        if (!ok) fallback(api)
        return ok
    }

    /**
     * 回退装载内置 35 词词库。**必须自己护 throwable**：死 .so 时 `OpiEngine` 的类初始化
     * 已失败，每次访问都抛 `NoClassDefFoundError`/`UnsatisfiedLinkError`（都是 `Error`），
     * 在调用点外面套 `catch (Exception)` 拦不住 —— 崩溃只会从「装载失败」推后到「回退失败」。
     * 集中在这一处，三个调用点（两个 I/O catch + load 失败）都不会漏。
     */
    private fun fallback(api: LoadApi) {
        try {
            api.load(null)
        } catch (e: Throwable) {
            // so 自身不可用：无路可退，交由调用方按 result=false 处理
        }
    }

    /**
     * 繁体资产编排：与 loadAsset 相同的 size 校验重拷；失败只返回 false 并保留主词典
     * （spec 2026-08-15 错误处理：trad.opid 加载失败 → 繁体模式回退查简体库，logcat 告警）。
     */
    fun loadTradAsset(fileOps: FileOps, api: LoadTradApi, targetPath: String): Boolean {
        try {
            val assetSize = fileOps.assetLength()
            if (assetSize == null || needsCopy(assetSize, fileOps.existingSize())) {
                fileOps.write(fileOps.readAsset())
            }
        } catch (e: Exception) {
            return false
        }
        // 同 loadAsset 的 Throwable 理由：生产入口 load(context) 紧接 luna 之后调本函数，
        // 这里漏掉 Error 会在同一进程里照样崩（trad 是可选增强，失败只降级不崩）。
        return try {
            api.loadTrad(targetPath)
        } catch (e: Throwable) {
            false
        }
    }

    /**
     * 笔画资产编排：与 loadTradAsset 逐行同构（size 校验重拷 + 失败只降级）——
     * stroke.opid 是留给「笔画键盘 / 生僻字键盘」的可选增强，加载失败只让那两个方案
     * 回退普通路径（logcat 告警），不碰 luna/trad。
     */
    fun loadStrokeAsset(fileOps: FileOps, api: LoadStrokeApi, targetPath: String): Boolean {
        try {
            val assetSize = fileOps.assetLength()
            if (assetSize == null || needsCopy(assetSize, fileOps.existingSize())) {
                fileOps.write(fileOps.readAsset())
            }
        } catch (e: Exception) {
            return false
        }
        return try {
            api.loadStroke(targetPath)
        } catch (e: Throwable) {
            false
        }
    }

    /**
     * 五笔码表资产编排：与 loadStrokeAsset 逐行同构（size 校验重拷 + 失败只降级）——
     * 码表是留给「五笔」方案的可选增强，装载失败只让那个方案回退普通路径（logcat 告警），
     * 不碰 luna/trad/stroke。
     */
    fun loadWubiAsset(fileOps: FileOps, api: LoadWubiApi, targetPath: String): Boolean {
        try {
            val assetSize = fileOps.assetLength()
            if (assetSize == null || needsCopy(assetSize, fileOps.existingSize())) {
                fileOps.write(fileOps.readAsset())
            }
        } catch (e: Exception) {
            return false
        }
        return try {
            api.loadWubi(targetPath)
        } catch (e: Throwable) {
            false
        }
    }

    /**
     * luna 是否已在本进程装好（以及当时的结果）。
     *
     * **不能重复 `OpiEngine.load`**：设置页与 IME 服务同进程、共用 Rust 单例，
     * `install` 会连**输入方案 / 模式 / 开关 / trad/stroke/wubi 挂载**一起重置。
     * 用户报障「打开输入法 app，再到其他 app 输入无反应」＝设置页（launcher）
     * 把 IME 正在用的 9 键/14 键方案重置成默认全键盘，返回后按键不出候选。
     * 因此 luna 只装一次；trad/stroke/wubi 仍每次重挂（它们不重置主引擎）。
     */
    internal class LunaLoadGate {
        private var done = false
        private var result = false

        /** 成功一次后固定返回 true 且不再调用 [block]；失败不置位，下次重试。 */
        @Synchronized
        fun run(block: () -> Boolean): Boolean {
            if (done) return result
            val r = block()
            if (r) {
                done = true
                result = true
            }
            return r
        }
    }

    private val lunaGate = LunaLoadGate()

    /** 生产入口：assets/luna.opid → filesDir/luna.opid → OpiEngine。 */
    @Synchronized
    fun load(context: Context): Boolean {
        val target = File(context.filesDir, FILE_NAME)
        // 已装好 ⇒ 跳过 install（保留方案/模式/开关）；失败（回退内置）不置位，
        // 下次调用继续尝试，与旧的「每次视图创建都重试」语义一致。
        val result = lunaGate.run {
            loadAsset(
                fileOps = object : FileOps {
                    override fun assetLength(): Long? = try {
                        context.assets.openFd(ASSET_NAME).use { it.length }
                    } catch (e: IOException) {
                        null // gzip 压缩资产 openFd 不可用 → 视为需要重拷，走完整读取
                    }

                    override fun readAsset(): ByteArray =
                        context.assets.open(ASSET_NAME).use { it.readBytes() }

                    override fun existingSize(): Long? =
                        if (target.exists()) target.length() else null

                    override fun write(bytes: ByteArray) {
                        target.writeBytes(bytes)
                    }
                },
                api = LoadApi { OpiEngine.load(it) },
                targetPath = target.absolutePath,
            )
        }
        if (result) Log.i(TAG, "luna loaded (${target.length()} bytes)")
        else Log.w(TAG, "luna load failed, engine on builtin fallback dict")

        // trad 可选增强：失败仅告警，不影响 luna 主词典（OpiEngine.loadTrad 严格加载）
        val tradTarget = File(context.filesDir, FILE_NAME_TRAD)
        val tradOk = loadTradAsset(
            fileOps = object : FileOps {
                override fun assetLength(): Long? = try {
                    context.assets.openFd(ASSET_NAME_TRAD).use { it.length }
                } catch (e: IOException) {
                    null
                }

                override fun readAsset(): ByteArray =
                    context.assets.open(ASSET_NAME_TRAD).use { it.readBytes() }

                override fun existingSize(): Long? =
                    if (tradTarget.exists()) tradTarget.length() else null

                override fun write(bytes: ByteArray) {
                    tradTarget.writeBytes(bytes)
                }
            },
            api = LoadTradApi { OpiEngine.loadTrad(it) },
            targetPath = tradTarget.absolutePath,
        )
        if (tradOk) Log.i(TAG, "trad loaded (${tradTarget.length()} bytes)")
        else Log.w(TAG, "trad load failed, Traditional mode falls back to simplified dict")

        // stroke 可选增强（2026-10-09）：失败仅告警，笔画方案回退普通路径
        val strokeTarget = File(context.filesDir, FILE_NAME_STROKE)
        val strokeOk = loadStrokeAsset(
            fileOps = object : FileOps {
                override fun assetLength(): Long? = try {
                    context.assets.openFd(ASSET_NAME_STROKE).use { it.length }
                } catch (e: IOException) {
                    null
                }

                override fun readAsset(): ByteArray =
                    context.assets.open(ASSET_NAME_STROKE).use { it.readBytes() }

                override fun existingSize(): Long? =
                    if (strokeTarget.exists()) strokeTarget.length() else null

                override fun write(bytes: ByteArray) {
                    strokeTarget.writeBytes(bytes)
                }
            },
            api = LoadStrokeApi { OpiEngine.loadStroke(it) },
            targetPath = strokeTarget.absolutePath,
        )
        if (strokeOk) Log.i(TAG, "stroke loaded (${strokeTarget.length()} bytes)")
        else Log.w(TAG, "stroke load failed, Stroke scheme falls back to the plain path")

        // 五笔码表（2026-10-14，M4 第二波）：**按方案装一张**（86/98 二选一 —— 两表码
        // 空间重叠、权重不可比，同时进引擎就是两套体系打架，见 desktop 的 Wubi.kt 与
        // fcitx5 的 opi_wubi.h 同一条裁决）。方案读自偏好（缺省 86）。失败仅告警。
        val plan = ImePreferences.from(context).wubiPlan
        val wubiAsset = if (plan == "98") ASSET_NAME_WUBI98 else ASSET_NAME_WUBI86
        val wubiFile = if (plan == "98") FILE_NAME_WUBI98 else FILE_NAME_WUBI86
        val wubiTarget = File(context.filesDir, wubiFile)
        val wubiOk = loadWubiAsset(
            fileOps = object : FileOps {
                override fun assetLength(): Long? = try {
                    context.assets.openFd(wubiAsset).use { it.length }
                } catch (e: IOException) {
                    null
                }

                override fun readAsset(): ByteArray =
                    context.assets.open(wubiAsset).use { it.readBytes() }

                override fun existingSize(): Long? =
                    if (wubiTarget.exists()) wubiTarget.length() else null

                override fun write(bytes: ByteArray) {
                    wubiTarget.writeBytes(bytes)
                }
            },
            api = LoadWubiApi { OpiEngine.loadWubi(it) },
            targetPath = wubiTarget.absolutePath,
        )
        if (wubiOk) Log.i(TAG, "wubi($plan) loaded (${wubiTarget.length()} bytes)")
        else Log.w(TAG, "wubi($plan) load failed, Wubi scheme falls back to the plain path")

        // 五键（风格/混输进入方式/个数/回车/自动上屏）也在这一刻推给引擎 —— 引擎刚建好，
        // 正是「配置 → 引擎」的落点（与 fcitx5 的 applyOpiSettings、TSF 的 apply_settings
        // 同一条）。方案不在这里（它已经在上面决定了装哪张表）。
        // 值域/名字 → 整数码的换算在 WubiNames.kt（锁步门禁比对 engine_wubi.rs）。
        ImePreferences.from(context).applyWubiSettings()
        return result
    }
}
