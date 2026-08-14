package io.opi.input.jni

import android.content.Context
import android.util.Log
import java.io.File
import java.io.IOException

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

    /** size 校验重拷规则（对齐 flutter：不存在或 size 不一致即重拷）。 */
    fun needsCopy(assetSize: Long, existingSize: Long?): Boolean =
        existingSize == null || existingSize != assetSize

    /**
     * 编排（纯 JVM，无 android.*）：先查资产 size（命中已缓存副本则跳过 1.7MB 字节读取）→
     * 需重拷时 readAsset+write → load(path)。
     * 任一步 I/O 失败（资产缺失/损坏/写盘失败）→ load(null) 回退内置 35 词词库，返回 false
     * （不抛异常，避免 IME 进程在 onCreateInputView 崩溃）；load 失败同样回退。
     */
    fun loadAsset(fileOps: FileOps, api: LoadApi, targetPath: String): Boolean {
        try {
            val assetSize = fileOps.assetLength()
            if (assetSize == null || needsCopy(assetSize, fileOps.existingSize())) {
                fileOps.write(fileOps.readAsset())
            }
        } catch (e: IOException) {
            api.load(null) // I/O 失败 → 回退内置词库（与 flutter catch → loadFallback 一致）
            return false
        } catch (e: Exception) {
            api.load(null) // 兜底：任何异常都不得让 IME 崩溃
            return false
        }
        val ok = api.load(targetPath)
        if (!ok) api.load(null) // 回退内置词库（与 flutter catch → loadFallback 一致）
        return ok
    }

    /** 生产入口：assets/luna.opid → filesDir/luna.opid → OpiEngine。 */
    fun load(context: Context): Boolean {
        val target = File(context.filesDir, FILE_NAME)
        val result = loadAsset(
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
        if (result) Log.i(TAG, "luna loaded (${target.length()} bytes)")
        else Log.w(TAG, "luna load failed, engine on builtin fallback dict")
        return result
    }
}
