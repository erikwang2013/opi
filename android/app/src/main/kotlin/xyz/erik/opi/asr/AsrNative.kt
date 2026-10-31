// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.asr

import android.util.Log

/**
 * `libopi_asr_jni.so` 的 JNI 薄层（见 `src/main/cpp/asr_jni.c`）。
 *
 * 库只在 arm64-v8a / x86_64 打包（见 app/build.gradle.kts 的 buildAsrNative）；
 * 32 位设备上加载失败是预期结果，[loaded] 为 false，话筒入口整个隐藏。
 */
internal object AsrNative {
    const val TAG = "OpiAsr"
    const val ERR_MODEL = -3
    const val ERR_BUSY = -5
    const val ERR_EVENT_TOO_SMALL = -8

    val loaded: Boolean = try {
        System.loadLibrary("opi_asr_jni")
        true
    } catch (t: Throwable) {
        Log.w(TAG, "libopi_asr_jni unavailable: ${t.message}")
        false
    }

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
