// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.asr

import android.content.Context
import android.util.Log
import java.io.File

/**
 * `libopi_asr_jni.so` 的 JNI 薄层（见 `src/main/cpp/asr_jni.c`）。
 *
 * 完整包从 APK 的 nativeLibraryDir 加载；普通包/基础包没有原生库，下载到
 * `filesDir/asr/engine/<abi>/` 后按依赖顺序用绝对路径 `System.load`。
 * 32 位设备无 64 位库，[ensureLoaded] 返回 false，话筒入口整个隐藏。
 */
internal object AsrNative {
    const val TAG = "OpiAsr"
    const val ERR_MODEL = -3
    const val ERR_BUSY = -5
    const val ERR_EVENT_TOO_SMALL = -8

    @Volatile
    private var ready = false

    /** 已加载（完整包或已下载）。 */
    val loaded: Boolean get() = ready

    /**
     * 幂等加载。先试 APK 内置库，再试下载目录；失败时不抛异常，交给调用方
     * 显示「引擎未下载」。
     */
    @Synchronized
    fun ensureLoaded(context: Context): Boolean {
        if (ready) return true
        if (runCatching { System.loadLibrary("opi_asr_jni") }.isSuccess) {
            ready = true
            return true
        }
        val dir = AsrEngine.root(context) ?: return false
        if (!AsrEngine.ready(context)) return false
        return try {
            for (name in AsrEngine.LIBS) {
                System.load(File(dir, name).absolutePath)
            }
            ready = true
            true
        } catch (t: Throwable) {
            Log.w(TAG, "downloaded ASR engine load failed: ${t.message}")
            false
        }
    }

    /** 下载完成后重新尝试（此前可能已试过 APK 内置库）。 */
    fun reload(context: Context): Boolean = ensureLoaded(context)

    external fun create(configJson: String): Long
    external fun destroy(handle: Long)
    external fun loadModel(handle: Long): Int
    external fun start(handle: Long): Int
    external fun feedPcm(handle: Long, data: ByteArray): Int
    external fun finish(handle: Long): Int
    external fun cancel(handle: Long): Int
    external fun pollEvent(handle: Long, buf: ByteArray): Int
    external fun lastError(handle: Long, buf: ByteArray): Int
    external fun maxEventBytes(): Int
}
