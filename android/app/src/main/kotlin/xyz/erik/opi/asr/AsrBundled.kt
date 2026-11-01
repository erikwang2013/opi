// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.asr

import android.content.Context
import android.content.res.AssetManager
import java.io.File
import java.io.FileOutputStream
import java.io.IOException

/**
 * 完整包内置模型：模型放在 APK 的 `assets/asr/`，首次使用时解压到私有
 * `filesDir/asr/`（sherpa-onnx 按文件路径加载，asset 不是普通文件）。
 *
 * 普通包/基础包没有这些 asset，[ready] 恒为 false，走设置页下载。
 */
internal object AsrBundled {
    private const val ROOT = "asr"
    private const val VAD_ASSET = "$ROOT/vad/silero_vad.onnx"

    /** 安装包是否带了这套模型（只查 asset 是否存在，不读 239/358MB 内容）。 */
    fun ready(context: Context, pack: AsrModelPack): Boolean = runCatching {
        val am = context.assets
        fun exists(path: String): Boolean = am.open(path).use { true }
        pack.files.all { exists("$ROOT/${pack.dir}/${it.path}") } && exists(VAD_ASSET)
    }.getOrDefault(false)

    /** 解压模型 + VAD + manifest；在 ASR 工作线程调用，失败抛 IOException。 */
    fun install(context: Context, pack: AsrModelPack, onProgress: (Long, Long) -> Unit = { _, _ -> }) {
        val am = context.assets
        val total = pack.bytes + AsrModels.VAD_BYTES
        var done = 0L

        val vad = AsrModels.vadFile(context)
        if (!AsrModels.vadReady(context)) {
            done += copy(am, VAD_ASSET, vad, AsrModels.VAD_BYTES, done, total, onProgress)
        } else {
            done += AsrModels.VAD_BYTES
        }

        val dir = AsrModels.packDir(context, pack).apply { mkdirs() }
        for (f in pack.files) {
            val dest = File(dir, f.path)
            if (dest.length() == f.bytes) {
                done += f.bytes
                continue
            }
            done += copy(am, "$ROOT/${pack.dir}/${f.path}", dest, f.bytes, done, total, onProgress)
        }
        AsrModels.writeManifest(dir, pack)
        onProgress(total, total)
    }

    private fun copy(
        am: AssetManager,
        asset: String,
        dest: File,
        expected: Long,
        done: Long,
        total: Long,
        onProgress: (Long, Long) -> Unit,
    ): Long {
        dest.parentFile?.mkdirs()
        val part = File(dest.parentFile, dest.name + ".part")
        part.delete()
        var copied = 0L
        am.open(asset).use { input ->
            FileOutputStream(part).use { out ->
                val buf = ByteArray(1 shl 20)
                while (true) {
                    val n = input.read(buf)
                    if (n < 0) break
                    out.write(buf, 0, n)
                    copied += n
                    onProgress(done + copied, total)
                }
            }
        }
        if (copied != expected) {
            part.delete()
            throw IOException("内置模型 $asset 大小不符：$copied，应 $expected")
        }
        if (!part.renameTo(dest)) {
            part.copyTo(dest, overwrite = true)
            part.delete()
        }
        return expected
    }
}
