// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 语音模型目录、清单与下载（2026-11-02）。
//!
//! 模型**不进安装包**（SenseVoice 229 MB / Whisper 359 MB）：首次启用时由本文件下到
//! 用户私有目录。落点与 Rust 侧 `AsrConfig` 的缺省相对路径一一对应：
//! `models_dir/sensevoice`、`models_dir/whisper-small.en`、`models_dir/vad/silero_vad.onnx`。
//!
//! 每个文件都钉死 **size + SHA-256**（真值来自本机实测与上游元数据）。下载写 `.part`，
//! 边写边算哈希，校验通过才原子改名；模型目录里再写 `manifest.json`（schema =
//! `crates/opi-asr/src/manifest.rs`，`verify_models=true` 时 Rust 会再全量核一遍）。
//! 许可：SenseVoice = FunASR Model Open Source License v1.1（需保留署名），Whisper 权重
//! = MIT（OpenAI），Silero VAD = MIT，sherpa-onnx 运行时 = Apache-2.0。

package xyz.erik.opi.candidate

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.io.FileOutputStream
import java.net.HttpURLConnection
import java.net.URI
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.nio.file.StandardCopyOption
import java.nio.file.attribute.PosixFilePermissions
import java.security.MessageDigest
import kotlin.math.ceil

/** 语言 → 引擎的路由**只有这一处**：zh/auto = SenseVoice；en = Whisper。 */
internal enum class SpeechLanguage(val value: String, val label: String) {
    AUTO("auto", "自动"),
    ZH("zh", "中文"),
    EN("en", "English（高精度）"),
    ;

    val engine: SpeechEngine get() = if (this == EN) SpeechEngine.WHISPER else SpeechEngine.SENSEVOICE

    companion object {
        fun fromValue(v: String?): SpeechLanguage = entries.firstOrNull { it.value == v } ?: AUTO
    }
}

internal enum class SpeechEngine(val title: String, val detail: String) {
    SENSEVOICE("SenseVoice", "中文 / 中英混合短句，本地快速"),
    WHISPER("Whisper small.en", "英文高精度；适合长句，15 秒内短句建议用自动"),
}

/** 一个要下载的文件。`role` = Rust manifest 的绑定名（VAD 不写 manifest ⇒ null）。 */
internal data class ModelFileSpec(
    val name: String,
    val size: Long,
    val sha256: String,
    val role: String?,
    val source: ModelSource,
    val repo: String = "",
)

internal enum class ModelSource { HUGGING_FACE, GITHUB_RELEASE }

/** 模型本体（目录名 = Rust `AsrConfig` 的相对路径）。 */
internal data class SpeechModel(
    val engine: SpeechEngine,
    val dirName: String,
    val license: String,
    val files: List<ModelFileSpec>,
)

private const val SENSEVOICE_REPO = "csukuangfj/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17"
private const val WHISPER_REPO = "csukuangfj/sherpa-onnx-whisper-small.en"

internal val SENSEVOICE_MODEL = SpeechModel(
    engine = SpeechEngine.SENSEVOICE,
    dirName = "sensevoice",
    license = "FunASR Model Open Source License v1.1（分发需保留署名）",
    files = listOf(
        ModelFileSpec(
            "model.int8.onnx", 239_233_841L,
            "c71f0ce00bec95b07744e116345e33d8cbbe08cef896382cf907bf4b51a2cd51",
            "model", ModelSource.HUGGING_FACE, SENSEVOICE_REPO,
        ),
        ModelFileSpec(
            "tokens.txt", 315_894L,
            "f449eb28dc567533d7fa59be34e2abca8784f771850c78a47fb731a31429a1dc",
            "tokens", ModelSource.HUGGING_FACE, SENSEVOICE_REPO,
        ),
    ),
)

internal val WHISPER_MODEL = SpeechModel(
    engine = SpeechEngine.WHISPER,
    dirName = "whisper-small.en",
    license = "MIT（OpenAI Whisper 权重）",
    files = listOf(
        ModelFileSpec(
            "small.en-encoder.int8.onnx", 112_442_483L,
            "8bdac288f369aa94ee2194059238c465ed82ea9d47ee8fa4a8c0a891873e462f",
            "encoder", ModelSource.HUGGING_FACE, WHISPER_REPO,
        ),
        ModelFileSpec(
            "small.en-decoder.int8.onnx", 262_223_042L,
            "710ccf890e10f3faa15f51ec346081a2723c9f3adb6e4da81c6573a5a6f877fb",
            "decoder", ModelSource.HUGGING_FACE, WHISPER_REPO,
        ),
        ModelFileSpec(
            "small.en-tokens.txt", 835_554L,
            "306cd27f03c1a714eca7108e03d66b7dc042abe8c258b44c199a7ed9838dd930",
            "tokens", ModelSource.HUGGING_FACE, WHISPER_REPO,
        ),
    ),
)

/** Silero VAD：两个模型共用；Rust 直接按路径打开，不经过 manifest。 */
internal val VAD_FILE = ModelFileSpec(
    "silero_vad.onnx", 643_854L,
    "9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6",
    null, ModelSource.GITHUB_RELEASE,
)

internal fun SpeechEngine.model(): SpeechModel =
    if (this == SpeechEngine.WHISPER) WHISPER_MODEL else SENSEVOICE_MODEL

/** 该引擎跑起来需要的全部文件（模型本体 + VAD）。 */
internal fun SpeechEngine.requiredFiles(): List<ModelFileSpec> = model().files + VAD_FILE

internal fun SpeechEngine.totalBytes(): Long = requiredFiles().sumOf { it.size }

internal fun SpeechEngine.approxMb(): Int = ceil(totalBytes() / 1024.0 / 1024.0).toInt()

/** 私有模型根目录：Windows `%LOCALAPPDATA%\opi\models`；Linux `$XDG_DATA_HOME/opi/models`。 */
internal fun speechModelRoot(
    windows: Boolean,
    localAppData: String?,
    xdgDataHome: String?,
    home: String?,
): Path {
    if (windows) {
        val base = localAppData?.takeIf { it.isNotEmpty() } ?: "."
        return Paths.get(base, "opi", "models")
    }
    val base = xdgDataHome?.takeIf { it.isNotEmpty() }
        ?: Paths.get(home?.takeIf { it.isNotEmpty() } ?: ".", ".local", "share").toString()
    return Paths.get(base, "opi", "models")
}

internal fun speechModelRoot(): Path = speechModelRoot(
    windows = isWindows(),
    localAppData = System.getenv("LOCALAPPDATA"),
    xdgDataHome = System.getenv("XDG_DATA_HOME"),
    home = System.getProperty("user.home"),
)

internal fun SpeechEngine.dir(root: Path): Path = root.resolve(model().dirName)

internal fun vadPath(root: Path): Path = root.resolve("vad").resolve(VAD_FILE.name)

/**
 * 模型是否可加载（**不哈希**：UI 每次重组都调它，240 MB 的 SHA-256 不能放在这里；
 * Rust 的 `load_model` 会再全量校验，坏文件在那里报错）。清单缺失 = 未下载完。
 */
internal fun modelReady(engine: SpeechEngine, root: Path): Boolean {
    val dir = engine.dir(root)
    if (!Files.isRegularFile(dir.resolve("manifest.json"))) return false
    if (!engine.model().files.all { spec -> fileMatches(dir.resolve(spec.name), spec) }) return false
    return fileMatches(vadPath(root), VAD_FILE)
}

private fun fileMatches(path: Path, spec: ModelFileSpec): Boolean =
    runCatching { Files.isRegularFile(path) && Files.size(path) == spec.size }.getOrDefault(false)

/** 模型目录里那份 manifest（格式逐字对齐 `ModelManifest::from_json`）。 */
internal fun manifestJson(files: List<ModelFileSpec>): String =
    """{"version":1,"files":[""" + files.joinToString(",") { f ->
        """{"path":"${f.name}","size":${f.size},"sha256":"${f.sha256}","role":"${f.role}"}"""
    } + "]}"

/** 目录私有：模型是用户数据，不跟安装目录一起给全机读。Windows 上由 LocalAppData 保证。 */
internal fun ensurePrivateDir(dir: Path) {
    Files.createDirectories(dir)
    runCatching { Files.setPosixFilePermissions(dir, PosixFilePermissions.fromString("rwx------")) }
}

// ---------- 下载 ----------

internal enum class DownloadPhase { IDLE, RUNNING, DONE, ERROR }

internal data class ModelDownloadState(
    val phase: DownloadPhase = DownloadPhase.IDLE,
    val engine: SpeechEngine? = null,
    val done: Long = 0,
    val total: Long = 0,
    val message: String = "",
) {
    val fraction: Float get() = if (total <= 0L) 0f else (done.toFloat() / total).coerceIn(0f, 1f)
}

private class DownloadCancelled : Exception()

/**
 * 模型下载器：进程内单实例（设置页切走/切回不丢进度）。一次只下一个引擎 ——
 * 两个 200 MB 的连接并发只会互相抢带宽，也更难说清哪份进度是哪份。
 */
internal object SpeechModels {
    var state by mutableStateOf(ModelDownloadState())
        private set

    @Volatile private var cancelled = false
    private var worker: Thread? = null

    @Synchronized
    fun start(engine: SpeechEngine, root: Path) {
        if (state.phase == DownloadPhase.RUNNING) return
        cancelled = false
        state = ModelDownloadState(
            phase = DownloadPhase.RUNNING,
            engine = engine,
            total = engine.totalBytes(),
            message = "准备下载…",
        )
        worker = Thread({ runDownload(engine, root) }, "opi-speech-download").apply {
            isDaemon = true
            start()
        }
    }

    fun cancel() {
        cancelled = true
    }

    private fun runDownload(engine: SpeechEngine, root: Path) {
        try {
            ensurePrivateDir(root)
            var done = 0L
            val specs = engine.requiredFiles()
            for (spec in specs) {
                checkCancelled()
                val target = if (spec === VAD_FILE) vadPath(root) else engine.dir(root).resolve(spec.name)
                ensurePrivateDir(target.parent)
                state = state.copy(message = "检查 ${spec.name}…")
                if (fileMatches(target, spec) && sha256(target) == spec.sha256) {
                    done += spec.size
                    state = state.copy(done = done)
                    continue
                }
                Files.deleteIfExists(target)
                state = state.copy(message = "下载 ${spec.name}…")
                downloadVerified(spec, target) { fileBytes ->
                    state = state.copy(done = done + fileBytes, message = "下载 ${spec.name}…")
                }
                done += spec.size
                state = state.copy(done = done)
            }
            // 清单最后写：有它 = 这一组文件都验过了。Rust 的 load_model 以它为入口。
            val dir = engine.dir(root)
            writeTextAtomically(dir.resolve("manifest.json"), manifestJson(engine.model().files))
            state = state.copy(
                phase = DownloadPhase.DONE,
                done = state.total,
                message = "下载完成，SHA-256 校验通过",
            )
        } catch (_: DownloadCancelled) {
            state = state.copy(phase = DownloadPhase.IDLE, message = "已取消下载")
        } catch (t: Throwable) {
            state = state.copy(
                phase = DownloadPhase.ERROR,
                message = "下载失败：${t.message ?: t.javaClass.simpleName}",
            )
        }
    }

    private fun checkCancelled() {
        if (cancelled) throw DownloadCancelled()
    }

    private fun sha256(path: Path): String {
        val digest = MessageDigest.getInstance("SHA-256")
        Files.newInputStream(path).use { input ->
            val buf = ByteArray(1 shl 20)
            while (true) {
                checkCancelled()
                val n = input.read(buf)
                if (n <= 0) break
                digest.update(buf, 0, n)
            }
        }
        return digest.digest().joinToString("") { "%02x".format(it) }
    }

    /**
     * 逐个源试；全部失败才报错。写 `.part` 边写边算哈希，长度或 SHA 不对就删掉重来。
     * HTTPS-only（下载物还有 hash 兜底，这里挡的是明文降级）。
     */
    private fun downloadVerified(spec: ModelFileSpec, target: Path, onProgress: (Long) -> Unit) {
        val part = target.resolveSibling(target.fileName.toString() + ".part")
        val errors = ArrayList<String>(2)
        for (url in urls(spec)) {
            checkCancelled()
            try {
                if (URI(url).scheme != "https") error("只允许 HTTPS：$url")
                Files.deleteIfExists(part)
                val digest = MessageDigest.getInstance("SHA-256")
                var total = 0L
                var reported = 0L
                val conn = (URI(url).toURL().openConnection() as HttpURLConnection).apply {
                    requestMethod = "GET"
                    connectTimeout = 10_000
                    readTimeout = 30_000
                    instanceFollowRedirects = true
                    setRequestProperty("User-Agent", "opi-desktop/1.0")
                }
                try {
                    val code = conn.responseCode
                    if (code != HttpURLConnection.HTTP_OK) error("HTTP $code")
                    conn.inputStream.use { input ->
                        FileOutputStream(part.toFile()).use { out ->
                            val buf = ByteArray(64 * 1024)
                            while (true) {
                                checkCancelled()
                                val n = input.read(buf)
                                if (n < 0) break
                                out.write(buf, 0, n)
                                digest.update(buf, 0, n)
                                total += n
                                if (total - reported >= 1L shl 20) {
                                    reported = total
                                    onProgress(total)
                                }
                            }
                        }
                    }
                } finally {
                    conn.disconnect()
                }
                if (total != spec.size) error("大小不符：期望 ${spec.size}，收到 $total")
                val got = digest.digest().joinToString("") { "%02x".format(it) }
                if (!got.equals(spec.sha256, ignoreCase = true)) error("SHA-256 不符")
                try {
                    Files.move(part, target, StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE)
                } catch (_: Exception) {
                    Files.move(part, target, StandardCopyOption.REPLACE_EXISTING)
                }
                onProgress(total)
                return
            } catch (e: DownloadCancelled) {
                throw e
            } catch (t: Throwable) {
                runCatching { Files.deleteIfExists(part) }
                errors += "${t.message ?: t.javaClass.simpleName}（$url）"
            }
        }
        error(errors.joinToString("；"))
    }

    /** 官方源在前，镜像兜底（国内网络；两者内容同一份 SHA-256）。 */
    private fun urls(spec: ModelFileSpec): List<String> = when (spec.source) {
        ModelSource.HUGGING_FACE -> listOf(
            "https://huggingface.co/${spec.repo}/resolve/main/${spec.name}",
            "https://hf-mirror.com/${spec.repo}/resolve/main/${spec.name}",
        )
        ModelSource.GITHUB_RELEASE -> {
            val official =
                "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/${spec.name}"
            listOf(official, "https://gh-proxy.com/$official")
        }
    }
}
