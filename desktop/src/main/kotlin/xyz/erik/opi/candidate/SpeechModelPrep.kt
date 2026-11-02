// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 完整包内置模型的按需解压（从 SpeechModels.kt 拆出守 500 行）。
//!
//! 只解压当前引擎（+ VAD），边拷边报进度、可取消。旧实现一次拷两套模型约
//! 615 MB，且任一份资源缺失就抛异常杀死语音工作线程 —— 用户看到的是
//! 「一直正在加载模型」。失败/取消都折成 [ModelPrep]，不再向上抛。

package xyz.erik.opi.candidate

import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.StandardCopyOption

/** 完整包：模型作为 JAR 资源随包；首次录音时解压到用户模型目录。 */
internal fun bundledJar(): Boolean =
    SpeechModels::class.java.getResource("/asr/sensevoice/model.int8.onnx") != null

internal sealed interface ModelPrep {
    data object Ready : ModelPrep
    data object Cancelled : ModelPrep
    data class Failed(val message: String) : ModelPrep
}

private class PrepCancelled : Exception()

/**
 * 只解压**当前引擎**（+ VAD）的内置模型，并在拷贝过程中上报进度。
 *
 * 旧实现一次拷两套模型（约 615 MB）：完整包一启动就预热，语音工作线程被拷贝
 * 堵住，用户点话筒只看到「正在加载模型」；任一资源缺失还会抛异常把工作线程
 * 直接杀死。现在按需、可取消、异常折成 [ModelPrep.Failed]。
 */
internal fun ensureBundledModels(
    root: Path = speechModelRoot(),
    engine: SpeechEngine,
    onProgress: (Long, Long) -> Unit = { _, _ -> },
    isCancelled: () -> Boolean = { false },
): ModelPrep {
    if (!bundledJar() || modelReady(engine, root)) return ModelPrep.Ready
    val model = engine.model()
    val dir0 = engine.dir(root)
    val total = model.files.filterNot { fileMatches(dir0.resolve(it.name), it) }.sumOf { it.size } +
        if (fileMatches(vadPath(root), VAD_FILE)) 0L else VAD_FILE.size
    var done = 0L
    try {
        val dir = engine.dir(root)
        ensurePrivateDir(dir)
        for (spec in model.files) {
            val target = dir.resolve(spec.name)
            if (fileMatches(target, spec)) continue
            done = copyResource(
                "/asr/${model.dirName}/${spec.name}", target, spec.size, done, total, onProgress, isCancelled,
            )
        }
        // 完整包 JAR 里**没有** manifest.json（只有模型文件）；旧实现去 JAR 找它，
        // 抛异常杀死语音线程 —— 这正是「点话筒一直正在加载模型」的根因。
        val manifest = dir.resolve("manifest.json")
        if (!Files.isRegularFile(manifest)) {
            Files.write(manifest, manifestJson(model.files).toByteArray())
        }
        val vad = vadPath(root)
        if (!fileMatches(vad, VAD_FILE)) {
            ensurePrivateDir(vad.parent)
            done = copyResource(
                "/asr/vad/${VAD_FILE.name}", vad, VAD_FILE.size, done, total, onProgress, isCancelled,
            )
        }
        return if (modelReady(engine, root)) ModelPrep.Ready
        else ModelPrep.Failed("内置模型不完整：请重新安装完整包，或在设置 → 语音下载模型")
    } catch (_: PrepCancelled) {
        return ModelPrep.Cancelled
    } catch (t: Throwable) {
        return ModelPrep.Failed("准备内置模型失败：${t.message ?: t.javaClass.simpleName}")
    }
}

private fun copyResource(
    path: String,
    target: Path,
    expected: Long,
    baseDone: Long,
    total: Long,
    onProgress: (Long, Long) -> Unit,
    isCancelled: () -> Boolean,
): Long {
    val input = SpeechModels::class.java.getResourceAsStream(path)
        ?: error("完整包缺内置模型资源：$path")
    input.use { ins ->
        val part = target.resolveSibling(target.fileName.toString() + ".part")
        try {
            Files.newOutputStream(part).use { out ->
                val buf = ByteArray(1 shl 20)
                var copied = 0L
                while (true) {
                    if (isCancelled()) throw PrepCancelled()
                    val n = ins.read(buf)
                    if (n < 0) break
                    out.write(buf, 0, n)
                    copied += n
                    onProgress(baseDone + copied, total)
                }
            }
            if (expected >= 0 && Files.size(part) != expected) {
                error("内置模型大小不符：$path")
            }
            Files.move(part, target, StandardCopyOption.REPLACE_EXISTING)
            return baseDone + Files.size(target)
        } catch (t: Throwable) {
            Files.deleteIfExists(part)
            throw t
        }
    }
}
