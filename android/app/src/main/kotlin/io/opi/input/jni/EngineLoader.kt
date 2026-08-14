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
     * 编排（纯 JVM，无 android.*）：读资产 → size 校验重拷 → load(path)。
     * load 失败（坏路径/损坏）→ load(null) 回退内置 35 词词库，返回 false。
     */
    fun loadAsset(fileOps: FileOps, api: LoadApi, targetPath: String): Boolean {
        val bytes = fileOps.readAsset()
        if (needsCopy(bytes.size.toLong(), fileOps.existingSize())) {
            fileOps.write(bytes)
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
