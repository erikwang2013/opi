// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `libopi_asr` 的 C ABI 薄壳（JNA）。真源 = `crates/opi-asr/src/ffi.rs`。
//!
//! 与 `ClipFfi.kt` 的两点不同：
//! · 字符串是 **NUL 结尾的 UTF-8**（不是 UTF-16 结构体）⇒ 入参自己写 [cStringArg]，
//!   出参用 `Memory.getString(0, "UTF-8")`。不走 JNA 的 String 映射：桌面 JVM 的默认
//!   编码在中文 Windows 上不是 UTF-8，模型路径会静默变乱码。
//! · 主库没有 RPATH（`readelf -d` 实测 NEEDED `libsherpa-onnx-c-api.so` 却找不到），
//!   sherpa 的 c-api 又有 `$ORIGIN` ⇒ 先按绝对路径把同目录的依赖库 preload 进进程，
//!   再载主库。Windows 同理（先 onnxruntime，再 c-api）：已加载模块按 basename 命中。
//!
//! 句柄线程安全（ffi.rs 头注）；`destroy` 必须是该句柄的最后一次调用 —— 由
//! [SpeechHost] 的单线程循环保证。

package xyz.erik.opi.candidate

import com.sun.jna.Library
import com.sun.jna.Memory
import com.sun.jna.Native
import com.sun.jna.NativeLibrary
import com.sun.jna.Pointer
import java.io.File
import java.nio.file.Paths

/** 10 个 `opi_asr_*`。名字必须与导出符号逐字一致。 */
internal interface OpiAsrFfi : Library {
    fun opi_asr_create(configJson: Pointer?): Pointer?
    fun opi_asr_destroy(handle: Pointer?)
    fun opi_asr_load_model(handle: Pointer?): Int
    fun opi_asr_start(handle: Pointer?): Int
    fun opi_asr_feed_pcm(handle: Pointer?, data: ByteArray, len: Int): Int
    fun opi_asr_finish(handle: Pointer?): Int
    fun opi_asr_cancel(handle: Pointer?): Int
    fun opi_asr_poll_event(handle: Pointer?, buf: Pointer?, cap: Int): Int
    fun opi_asr_last_error(handle: Pointer?, buf: Pointer?, cap: Int): Int
    fun opi_asr_max_event_bytes(): Int
}

/** NUL 结尾的 UTF-8 入参。调用期间调用方必须持有强引用（Memory 被回收 = use-after-free）。 */
internal fun cStringArg(text: String): Memory {
    val bytes = text.toByteArray(Charsets.UTF_8)
    val memory = Memory(bytes.size.toLong() + 1L)
    memory.write(0L, bytes, 0, bytes.size)
    memory.setByte(bytes.size.toLong(), 0)
    return memory
}

/** 读取 C 写回的 UTF-8 串；`len` 是 `opi_asr_*` 的返回值（不含 NUL）。 */
internal fun OpiAsrFfi.readCString(memory: Memory, len: Int): String =
    if (len <= 0) "" else memory.getString(0L, "UTF-8")

/** [opi_asr_last_error] 的固定缓冲：模型错误串不会接近这个长度。 */
internal const val ASR_ERROR_BYTES = 4096

/**
 * 读句柄的 last_error（失败调用之后立刻读）。句柄为 null 时 Rust 返回 create 阶段的错误。
 */
internal fun OpiAsrFfi.lastError(handle: Pointer?): String {
    val buf = Memory(ASR_ERROR_BYTES.toLong())
    val n = opi_asr_last_error(handle, buf, ASR_ERROR_BYTES)
    return readCString(buf, n)
}

internal sealed interface AsrLoad {
    class Loaded(val ffi: OpiAsrFfi) : AsrLoad
    class Unavailable(val reason: String) : AsrLoad
}

/**
 * 动态库候选路径（纯函数，门禁见 AsrFfiTest）。与 [ffiCandidates] 同一条 jpackage
 * 布局：Windows `<app>\app\opi_asr.dll`、Linux `<app>/lib/app/libopi_asr.so`。
 * `OPI_ASR_LIB` 是显式覆盖（显式给了就不再回退，理由同 ClipFfi）。
 */
internal fun asrCandidates(
    envLib: String?,
    appPath: String?,
    windows: Boolean,
    userDir: String,
): List<String> {
    val name = if (windows) "opi_asr.dll" else "libopi_asr.so"
    val out = LinkedHashSet<String>()
    val override = envLib?.trim()?.takeIf { it.isNotEmpty() }
    if (override != null) {
        out += File(override).absolutePath
        return out.toList()
    }
    val launcher = appPath?.takeIf { it.isNotBlank() }
        ?.let { runCatching { Paths.get(it).toAbsolutePath().parent }.getOrNull() }
    val roots = listOfNotNull(launcher, launcher?.parent)
    for (root in roots) {
        for (sub in listOf("", "app", "lib", "lib/app")) {
            out += root.resolve(sub).resolve(name).toString()
        }
    }
    // 开发/测试：Gradle 测试的 CWD = desktop/，打包任务的 CWD = desktop/ 亦然。
    out += Paths.get(userDir).resolve("../target/release/$name").normalize().toString()
    out += Paths.get(userDir).resolve("target/release/$name").normalize().toString()
    out += "opi_asr"
    return out.toList()
}

/**
 * 主库同目录必须先加载的依赖（顺序 = 加载顺序）。
 * Linux 的 c-api 自带 `$ORIGIN`，onnxruntime 跟着它走；Windows 则需要先 onnxruntime，
 * 再 c-api —— 依赖按已加载模块的 basename 解析。
 */
internal fun asrDependencyNames(windows: Boolean): List<String> = if (windows) {
    // cxx 在 c-api 前面：c-api 可能以它为 NEEDED（Windows 不搜 DLL 自身目录）。
    listOf("onnxruntime.dll", "sherpa-onnx-cxx-api.dll", "sherpa-onnx-c-api.dll")
} else {
    listOf("libonnxruntime.so", "libsherpa-onnx-cxx-api.so", "libsherpa-onnx-c-api.so")
}

/** preload 同目录依赖；返回失败说明（空 = 全成功/不存在，交给主库报自己的错）。 */
internal fun preloadAsrDeps(mainLib: String, windows: Boolean): List<String> {
    val dir = File(mainLib).absoluteFile.parentFile ?: return emptyList()
    val errors = ArrayList<String>(2)
    for (name in asrDependencyNames(windows)) {
        val dep = File(dir, name)
        if (!dep.isFile) continue
        try {
            NativeLibrary.getInstance(dep.absolutePath)
        } catch (t: Throwable) {
            errors += "$name: ${t.javaClass.simpleName}: ${t.message}"
        }
    }
    return errors
}

/** 加载 [OpiAsrFfi]；失败折成 [AsrLoad.Unavailable]，绝不把异常抛给 UI。 */
internal fun loadAsrFfi(
    envLib: String? = System.getenv("OPI_ASR_LIB"),
    appPath: String? = System.getProperty("jpackage.app-path"),
    windows: Boolean = isWindows(),
    userDir: String = System.getProperty("user.dir") ?: ".",
): AsrLoad {
    val errors = ArrayList<String>(2)
    for (candidate in asrCandidates(envLib, appPath, windows, userDir)) {
        val isPath = candidate.contains('/') || candidate.contains('\\') || candidate.endsWith(".so") ||
            candidate.endsWith(".dll")
        if (isPath && !File(candidate).isFile) {
            errors += "$candidate: 文件不存在"
            continue
        }
        try {
            preloadAsrDeps(candidate, windows).forEach { errors += "$candidate 依赖 $it" }
            return AsrLoad.Loaded(Native.load(candidate, OpiAsrFfi::class.java))
        } catch (t: Throwable) {
            errors += "$candidate: ${t.javaClass.simpleName}: ${t.message}"
        }
    }
    return AsrLoad.Unavailable(
        "加载 opi_asr 动态库失败（语音识别不可用）。已尝试：\n" +
            errors.joinToString("\n") +
            "\n修复：安装包应把库放在应用目录；开发时先 " +
            "cargo build --release -p opi-asr，或用 OPI_ASR_LIB 指给产物。",
    )
}
