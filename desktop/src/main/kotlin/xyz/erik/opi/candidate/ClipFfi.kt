// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! opi-clip 的 C ABI 薄壳（JNA）。导出面与 `macos/OpiFFI.h` 的 `opi_clip_*` 一一对应。
//!
//! 两个必须守住的约定（真源 `crates/opi-ffi/src/cabi_clip.rs` 头注）：
//! · **句柄不可跨线程**：`create` 记录创建线程，其余调用（含 destroy）必须在同一线程。
//!   本文件不做线程检查 —— 约束由 `ClipService` 的单线程循环保证；它故意没有加锁，
//!   加锁也救不了错线程调用（Rust 侧会直接拒绝）。
//! · **每个返回的 OpiString 恰好释放一次**：所有出口都走 [OpiStringStruct.takeText]，
//!   它在 finally 里调 `opi_ffi_free_string`。空串是 NULL 哨兵，free 对 NULL 安全。
//!
//! bool 映射：Rust 的 `bool` 是 **1 字节**，JNA 的 Java `boolean` 默认按 4 字节 `int`
//! 传/返（x86-64 上返回值高字节未必清零）⇒ 这里一律用 `Byte`，`!= 0` 判真。

package xyz.erik.opi.candidate

import com.sun.jna.Library
import com.sun.jna.Memory
import com.sun.jna.Native
import com.sun.jna.Pointer
import com.sun.jna.Structure
import com.sun.jna.Structure.FieldOrder
import java.io.File
import java.nio.file.Paths

/**
 * `cabi::OpiString`：UTF-16（ptr + len，非 NUL 结尾），Rust 分配。
 *
 * [Structure.ByValue] 不能省：`opi_clip_*` 按值返回它，`opi_ffi_free_string` 也按值接收。
 * JNA 的普通 `Structure` 默认按指针处理 —— 漏掉这个标记时 `{NULL,0}` 会被读成空指针，
 * 返回 null（真库门禁就是这么崩的）。
 */
@FieldOrder("ptr", "len")
class OpiStringStruct : Structure(), Structure.ByValue {
    @JvmField var ptr: Pointer? = null

    /** C `size_t`。桌面只支持 64 位（jpackage 的 x64 目标），Java long 即 64 位。 */
    @JvmField var len: Long = 0
}

/** 13 个 `opi_clip_*` + `opi_ffi_free_string`。名字必须与导出符号逐字一致。 */
interface OpiClipFfi : Library {
    fun opi_clip_create(configJson: Pointer?, len: Long): Pointer?
    fun opi_clip_destroy(clip: Pointer?)
    fun opi_clip_start(clip: Pointer?): Byte
    fun opi_clip_stop(clip: Pointer?)
    fun opi_clip_local_text(clip: Pointer?, text: Pointer?, len: Long): Int
    fun opi_clip_poll_event(clip: Pointer?, timeoutMs: Int): OpiStringStruct
    fun opi_clip_pair_start(clip: Pointer?, peerId: Pointer?, len: Long): Byte
    fun opi_clip_pair_confirm(clip: Pointer?, peerId: Pointer?, len: Long, accept: Byte): Byte
    fun opi_clip_pair_cancel(clip: Pointer?, peerId: Pointer?, len: Long): Byte
    fun opi_clip_peers(clip: Pointer?): OpiStringStruct
    fun opi_clip_forget_peer(clip: Pointer?, peerId: Pointer?, len: Long): Byte
    fun opi_clip_status(clip: Pointer?): OpiStringStruct
    fun opi_clip_last_error(): OpiStringStruct
    fun opi_ffi_free_string(s: OpiStringStruct)
}

/** `create` 的 UTF-16 入参缓冲；`Memory` 由 GC 释放，调用期间必须持有强引用。 */
internal class Utf16Arg(text: String) {
    val memory: Memory = Memory(text.length.toLong() * 2L)
    val len: Long = text.length.toLong()

    init {
        for (i in text.indices) memory.setShort(i * 2L, text[i].code.toShort())
    }
}

/** 事件 JSON 的硬上限再留一倍余量；超过它只可能是内存坏了，不解析。 */
private const val MAX_RETURNED_UTF16 = 4 * 1024 * 1024

/**
 * 读出并在 finally 里释放。`ptr == null` 视为空串（Rust 的 empty 哨兵），**仍然**
 * 调 free（对 NULL 安全）—— 漏掉这一步就是每次调用泄漏一个句柄。
 */
internal fun OpiStringStruct.takeText(ffi: OpiClipFfi): String {
    val text = try {
        val p = ptr
        val n = len
        when {
            p == null || n == 0L -> ""
            n < 0L || n > MAX_RETURNED_UTF16 -> ""
            else -> {
                val shorts = p.getShortArray(0, n.toInt())
                String(CharArray(shorts.size) { (shorts[it].toInt() and 0xFFFF).toChar() })
            }
        }
    } finally {
        ffi.opi_ffi_free_string(this)
    }
    return text
}

internal sealed interface FfiLoad {
    class Loaded(val ffi: OpiClipFfi) : FfiLoad
    class Unavailable(val reason: String) : FfiLoad
}

/**
 * 动态库候选路径（纯函数，门禁见 ClipFfiTest）。
 *
 * jpackage 把 `--input` 的文件放进 app 目录：Windows = `<app>\app\opi_ffi.dll`，
 * Linux = `<app>/lib/app/libopi_ffi.so`；启动器分别在 `<app>\opi.exe` 与
 * `<app>/bin/opi`。所以从启动器父目录再往上找一层，并试 app/lib 两个子目录。
 * `OPI_FFI_LIB` 是显式覆盖（CI 的预编译产物；**显式给了就不再悄悄回退**，
 * 否则设错了会静默伪装成「用了另一份库」）。
 */
internal fun ffiCandidates(
    envLib: String?,
    appPath: String?,
    windows: Boolean,
    userDir: String,
): List<String> {
    val name = if (windows) "opi_ffi.dll" else "libopi_ffi.so"
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
    // 开发/测试：仓库根的 target/release（Gradle 测试的 CWD = desktop/）。
    out += Paths.get(userDir).resolve("../target/release/$name").normalize().toString()
    out += Paths.get(userDir).resolve("target/release/$name").normalize().toString()
    // 系统搜索路径的最后一条（Windows 的 jna 会补 .dll，Linux 补 lib 前缀）。
    out += "opi_ffi"
    return out.toList()
}

/** 加载 [OpiClipFfi]；任何失败都折成 [FfiLoad.Unavailable]，绝不把异常抛给 UI。 */
internal fun loadClipFfi(
    envLib: String? = System.getenv("OPI_FFI_LIB"),
    appPath: String? = System.getProperty("jpackage.app-path"),
    windows: Boolean = isWindows(),
    userDir: String = System.getProperty("user.dir") ?: ".",
): FfiLoad {
    val errors = ArrayList<String>(2)
    for (candidate in ffiCandidates(envLib, appPath, windows, userDir)) {
        try {
            return FfiLoad.Loaded(Native.load(candidate, OpiClipFfi::class.java))
        } catch (t: Throwable) {
            errors += "$candidate: ${t.javaClass.simpleName}: ${t.message}"
        }
    }
    return FfiLoad.Unavailable(
        "加载 opi_ffi 动态库失败（剪贴板内核不可用）。已尝试：\n" +
            errors.joinToString("\n") +
            "\n修复：安装包应把库放在应用目录；开发时先 " +
            "cargo build --release -p opi_ffi，或用 OPI_FFI_LIB 指给产物。",
    )
}
