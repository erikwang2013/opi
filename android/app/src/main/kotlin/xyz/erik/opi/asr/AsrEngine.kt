// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.asr

import android.content.Context
import android.os.Build
import xyz.erik.opi.BuildConfig
import xyz.erik.opi.settings.OPI_VERSION_URL
import xyz.erik.opi.settings.httpGet
import xyz.erik.opi.settings.parseComponentJson
import java.io.File
import java.util.Locale

/**
 * 语音引擎原生库（2026-11-01 用户裁决）：只有完整包随 APK 提供；普通包/基础包
 * 在「设置 → 语音」按需下载。库来自本站稳定组件目录，逐个文件大小 + SHA-256
 * 由 `version.json` 的 `components.asr-engine-android-<abi>` 给出。
 */
internal object AsrEngine {
    /** 加载顺序 = 依赖顺序（c-api/主库按已加载模块的 basename 解析）。 */
    val LIBS = listOf(
        "libonnxruntime.so",
        "libsherpa-onnx-c-api.so",
        "libopi_asr.so",
        "libopi_asr_jni.so",
    )

    /** 下载前的展示值（压缩包约 8 MB）；真实大小以 version.json 为准。 */
    const val APPROX_BYTES = 8_000_000L

    fun abi(): String? =
        Build.SUPPORTED_ABIS.firstOrNull { it == "arm64-v8a" || it == "x86_64" }

    fun spec(): EngineSpec? {
        val abi = abi() ?: return null
        return EngineSpec(abi, "opi-asr-engine-android-$abi.zip")
    }

    fun root(context: Context): File? =
        spec()?.let { File(AsrModels.root(context), "engine/${it.abi}") }

    fun ready(context: Context): Boolean {
        val dir = root(context) ?: return false
        return LIBS.all { File(dir, it).length() > 0L }
    }

    /** 完整包装了引擎，不再需要下载。 */
    fun bundled(): Boolean = BuildConfig.ASR_ENGINE_BUNDLED

    fun componentKey(): String? = spec()?.let { "asr-engine-android-${it.abi}" }

    fun mb(bytes: Long): String = String.format(Locale.ROOT, "%.0f MB", bytes / 1048576.0)

    /**
     * 从官网清单取组件地址/大小/SHA-256。清单不可用时**不猜**地址 —— 没有校验值
     * 的下载宁可不让下。
     */
    fun remoteSpec(): Result<RemoteEngine> = runCatching {
        val key = componentKey() ?: error("当前设备 ABI 不支持语音引擎")
        val text = httpGet(OPI_VERSION_URL, 8_000)
        val pkg = parseComponentJson(text, key) ?: error("组件清单缺少 $key")
        val sha = pkg.sha256 ?: error("组件清单缺少 SHA-256")
        val size = pkg.size ?: error("组件清单缺少文件大小")
        val name = spec()?.zipName ?: error("当前设备 ABI 不支持语音引擎")
        val fallback = "https://github.com/erikwang2013/opi/releases/download/" +
            "v${BuildConfig.VERSION_NAME}/$name"
        RemoteEngine(pkg.url, fallback, size, sha)
    }
}

internal data class EngineSpec(val abi: String, val zipName: String)

internal data class RemoteEngine(
    val url: String,
    val fallback: String,
    val size: Long,
    val sha256: String,
)
