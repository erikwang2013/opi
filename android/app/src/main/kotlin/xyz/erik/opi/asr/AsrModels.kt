// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.asr

import android.content.Context
import android.os.Build
import java.io.File
import java.util.Locale

/**
 * 模型目录与固定校验值。模型**不进 APK**：下载到应用私有 `filesDir/asr`，
 * 下载时逐个文件校验 SHA-256 后再写 `manifest.json`（opi-asr 的加载器按它
 * 校验路径与大小；设备上不再重算 228/358MB 的哈希，见 AsrRuntime 的
 * `verify_models=false`）。
 *
 * 文件与哈希取自 opi-asr 真机基准使用的模型（crates/opi-asr/tests/real_wav.rs 同一套）。
 */
internal data class AsrModelFile(
    val path: String,
    val url: String,
    val bytes: Long,
    val sha256: String,
    val role: String,
)

internal data class AsrModelPack(
    val id: String,
    val dir: String,
    val title: String,
    val files: List<AsrModelFile>,
) {
    val bytes: Long get() = files.sumOf { it.bytes }
    val sha256: String get() = files.first().sha256
}

internal object AsrModels {
    const val SENSE_VOICE_ID = "sensevoice"
    const val WHISPER_ID = "whisper"
    const val VAD_BYTES = 643_854L
    const val VAD_SHA256 = "9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6"

    // 国内直连 huggingface.co 不通，用只读镜像；VAD 只在 GitHub Release 有原文件。
    private const val SV_BASE =
        "https://hf-mirror.com/csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17/resolve/main/"
    private const val WH_BASE =
        "https://hf-mirror.com/csukuangfj/sherpa-onnx-whisper-small.en/resolve/main/"
    private const val VAD_URL =
        "https://gh-proxy.com/https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/silero_vad.onnx"

    val senseVoice = AsrModelPack(
        id = SENSE_VOICE_ID,
        dir = "sensevoice",
        title = "SenseVoice（中英文，推荐）",
        files = listOf(
            AsrModelFile(
                "model.int8.onnx",
                SV_BASE + "model.int8.onnx",
                239_233_841,
                "c71f0ce00bec95b07744e116345e33d8cbbe08cef896382cf907bf4b51a2cd51",
                "model",
            ),
            AsrModelFile(
                "tokens.txt",
                SV_BASE + "tokens.txt",
                315_894,
                "f449eb28dc567533d7fa59be34e2abca8784f771850c78a47fb731a31429a1dc",
                "tokens",
            ),
        ),
    )

    val whisper = AsrModelPack(
        id = WHISPER_ID,
        dir = "whisper-small.en",
        title = "Whisper small.en（英文高精度）",
        files = listOf(
            AsrModelFile(
                "small.en-encoder.int8.onnx",
                WH_BASE + "small.en-encoder.int8.onnx",
                112_442_483,
                "8bdac288f369aa94ee2194059238c465ed82ea9d47ee8fa4a8c0a891873e462f",
                "encoder",
            ),
            AsrModelFile(
                "small.en-decoder.int8.onnx",
                WH_BASE + "small.en-decoder.int8.onnx",
                262_223_042,
                "710ccf890e10f3faa15f51ec346081a2723c9f3adb6e4da81c6573a5a6f877fb",
                "decoder",
            ),
            AsrModelFile(
                "small.en-tokens.txt",
                WH_BASE + "small.en-tokens.txt",
                835_554,
                "306cd27f03c1a714eca7108e03d66b7dc042abe8c258b44c199a7ed9838dd930",
                "tokens",
            ),
        ),
    )

    val packs = listOf(senseVoice, whisper)

    fun pack(id: String): AsrModelPack =
        packs.firstOrNull { it.id == id } ?: error("unknown ASR model pack: $id")

    fun root(context: Context): File = File(context.filesDir, "asr")

    fun packDir(context: Context, pack: AsrModelPack): File = File(root(context), pack.dir)

    fun vadFile(context: Context): File = File(root(context), "vad/silero_vad.onnx")

    fun vadReady(context: Context): Boolean = vadFile(context).length() == VAD_BYTES

    /**
     * 需要 VAD + 清单 + 每个文件的存在与大小。完整 SHA 在下载时校验；
     * 这里不做 239MB 哈希（主线程长按路径，不能卡 UI）。
     */
    fun packReady(context: Context, pack: AsrModelPack): Boolean {
        if (!vadReady(context)) return false
        val dir = packDir(context, pack)
        if (!File(dir, "manifest.json").isFile) return false
        return pack.files.all { File(dir, it.path).length() == it.bytes }
    }

    fun modelReady(context: Context, whisper: Boolean): Boolean =
        packReady(context, if (whisper) this.whisper else senseVoice)

    fun selected(context: Context, whisper: Boolean): AsrModelPack =
        if (whisper) this.whisper else senseVoice

    fun vadUrl(): String = VAD_URL

    /** 原生库只打包 64 位（armeabi-v7a 内存风险）：设置页据此隐藏无意义的下载入口。 */
    fun abiSupported(): Boolean =
        Build.SUPPORTED_ABIS.any { it == "arm64-v8a" || it == "x86_64" }

    fun mb(bytes: Long): String = String.format(Locale.ROOT, "%.0f MB", bytes / 1048576.0)
}
