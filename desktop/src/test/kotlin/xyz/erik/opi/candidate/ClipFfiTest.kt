// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! JNA 装载门禁：候选路径、显式覆盖、库缺失时返回 Unavailable（不抛），
//! 以及本机真库存在时的 OpiString 返回/释放（CI 没编 Rust 就跳过）。

package xyz.erik.opi.candidate

import java.io.File
import java.nio.file.Files
import java.nio.file.Paths
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test

class ClipFfiTest {

    @Test
    fun `explicit OPI_FFI_LIB is the only candidate`() {
        val file = "/opt/opi/libopi_ffi.so"
        assertEquals(
            listOf(File(file).absolutePath),
            ffiCandidates(file, "/opt/opi/bin/opi", windows = false, userDir = "/work/desktop"),
        )
    }

    @Test
    fun `jpackage linux layout is covered`() {
        val c = ffiCandidates(null, "/opt/opi/bin/opi", windows = false, userDir = "/work/desktop")
        assertTrue(c.contains("/opt/opi/lib/app/libopi_ffi.so"))
        assertTrue(c.contains("/opt/opi/bin/libopi_ffi.so"))
        assertTrue(c.contains("/work/target/release/libopi_ffi.so"))
        assertEquals("opi_ffi", c.last())
    }

    @Test
    fun `windows candidates use the dll name`() {
        val c = ffiCandidates(null, "/apps/opi/opi.exe", windows = true, userDir = "/work/desktop")
        assertTrue(c.any { it.endsWith("/app/opi_ffi.dll") })
        assertTrue(c.none { it.endsWith(".so") })
    }

    @Test
    fun `missing library becomes Unavailable without throwing`() {
        val missing = "/nonexistent-opi-ffi/libopi_ffi.so"
        val loaded = loadClipFfi(
            envLib = missing,
            appPath = null,
            windows = false,
            userDir = "/tmp",
        )
        assertTrue("库缺失必须折成 Unavailable，不能抛给设置页", loaded is FfiLoad.Unavailable)
        assertTrue((loaded as FfiLoad.Unavailable).reason.contains(missing))
    }

    @Test
    fun `real built library returns and frees OpiString`() {
        val name = if (isWindows()) "opi_ffi.dll" else "libopi_ffi.so"
        val lib = Paths.get("..", "target", "release", name).toAbsolutePath().normalize().toFile()
        assumeTrue("本机没有 target/release/$name（CI 未编 Rust），跳过真库门禁", lib.isFile)

        val loaded = loadClipFfi(
            envLib = lib.absolutePath,
            appPath = null,
            windows = isWindows(),
            userDir = System.getProperty("user.dir") ?: ".",
        )
        assertTrue("真库应当能装载：$loaded", loaded is FfiLoad.Loaded)
        val ffi = (loaded as FfiLoad.Loaded).ffi
        // 结构体按值返回 + UTF-16 + 恰好一次 free 全在这一行里。
        assertEquals("", ffi.opi_clip_last_error().takeText(ffi))
    }

    @Test
    fun `real built library round trips non-empty status and destroys the handle`() {
        val name = if (isWindows()) "opi_ffi.dll" else "libopi_ffi.so"
        val lib = Paths.get("..", "target", "release", name).toAbsolutePath().normalize().toFile()
        assumeTrue("本机没有 target/release/$name（CI 未编 Rust），跳过真库门禁", lib.isFile)

        val loaded = loadClipFfi(
            envLib = lib.absolutePath,
            appPath = null,
            windows = isWindows(),
            userDir = System.getProperty("user.dir") ?: ".",
        )
        assertTrue("真库应当能装载：$loaded", loaded is FfiLoad.Loaded)
        val ffi = (loaded as FfiLoad.Loaded).ffi
        val dir = Files.createTempDirectory("opi-clip-ffi-test").toFile()
        try {
            // create 只解析配置、不绑端口（见 cabi_clip.rs）；status 此时给非空 JSON。
            val cfg = Utf16Arg(clipConfigJson(dir.absolutePath, "ffi-test"))
            val handle = ffi.opi_clip_create(cfg.memory, cfg.len)
            assertNotNull("create 应当返回句柄", handle)
            try {
                val status = ffi.opi_clip_status(handle).takeText(ffi)
                val m = parseLine(status) ?: error("状态应是非空 JSON：$status")
                assertEquals(false, (m["running"] as JVal.JBool).v)
                assertEquals("ffi-test", (m["device_name"] as JVal.JStr).v)
            } finally {
                ffi.opi_clip_destroy(handle)
            }
        } finally {
            dir.deleteRecursively()
        }
    }
}
