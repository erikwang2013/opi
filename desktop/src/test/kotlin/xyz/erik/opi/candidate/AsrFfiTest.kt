// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `libopi_asr` 加载层的门禁：候选路径、依赖 preload 顺序、缺库时折成可读错误。
//! 真库加载不在这里（CI 机器不一定编过 Rust）；缺库路径覆盖「JNA 找不到库」这条
//! 用户真会遇到的分支。

package xyz.erik.opi.candidate

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class AsrFfiTest {

    @Test
    fun `candidate paths cover jpackage layouts and the env override`() {
        val linux = asrCandidates(
            envLib = null, appPath = "/opt/opi/bin/opi-statusbar",
            windows = false, userDir = "/work/desktop",
        )
        // jpackage 的 app image：主启动器同目录、app/、lib/、lib/app/ 都要找。
        assertTrue(linux.toString(), linux.contains("/opt/opi/bin/libopi_asr.so"))
        assertTrue(linux.toString(), linux.contains("/opt/opi/app/libopi_asr.so"))
        assertTrue(linux.toString(), linux.contains("/opt/opi/lib/app/libopi_asr.so"))
        assertTrue(linux.toString(), linux.none { it.endsWith(".dll") })
        assertEquals("opi_asr", linux.last()) // 系统库名兜底

        val win = asrCandidates(
            envLib = null, appPath = "/opt/opi/bin/opi.exe",
            windows = true, userDir = "/work/desktop",
        )
        assertTrue(win.toString(), win.contains("/opt/opi/bin/opi_asr.dll"))
        assertTrue(win.toString(), win.contains("/opt/opi/app/opi_asr.dll"))
        assertTrue(win.toString(), win.none { it.endsWith(".so") })

        // 显式覆盖只产生它自己：给了 OPI_ASR_LIB 就不回退（与 ClipFfi 同口径）。
        assertEquals(
            listOf(File("/custom/libopi_asr.so").absolutePath),
            asrCandidates(" /custom/libopi_asr.so ", null, windows = false, userDir = "/work"),
        )
    }

    @Test
    fun `dependency preload order puts runtime before the apis`() {
        // Linux：onnxruntime → cxx → c-api（c-api 的 NEEDED 靠已加载模块满足）。
        // Windows 同序：DLL 按已加载模块的 basename 解析，不能先载 c-api。
        for (windows in listOf(false, true)) {
            val names = asrDependencyNames(windows)
            assertEquals(3, names.size)
            assertTrue(names[0], names[0].contains("onnxruntime"))
            assertTrue(names[1], names[1].contains("sherpa-onnx-cxx-api"))
            assertTrue(names[2], names[2].contains("sherpa-onnx-c-api"))
            assertTrue(names[0], names[0].endsWith(if (windows) ".dll" else ".so"))
        }
    }

    @Test
    fun `missing library is an unavailable reason instead of an exception`() {
        val missing = File(
            System.getProperty("java.io.tmpdir"),
            "opi-asr-missing-${System.nanoTime()}/libopi_asr.so",
        ).absolutePath
        val load = loadAsrFfi(
            envLib = missing, appPath = null, windows = false, userDir = "/work/desktop",
        )
        assertTrue("缺库该折成 Unavailable，不能把异常抛给 UI", load is AsrLoad.Unavailable)
        val reason = (load as AsrLoad.Unavailable).reason
        assertTrue(reason, reason.contains("文件不存在"))
        assertTrue(reason, reason.contains(missing))
        assertTrue(reason, reason.contains("OPI_ASR_LIB"))
    }

    @Test
    fun `preload of absent dependencies is a no-op`() {
        val dir = File(System.getProperty("java.io.tmpdir"), "opi-asr-nodeps-${System.nanoTime()}")
        assertTrue(dir.mkdir())
        try {
            assertEquals(
                emptyList<String>(),
                preloadAsrDeps(File(dir, "libopi_asr.so").absolutePath, windows = false),
            )
        } finally {
            dir.deleteRecursively()
        }
    }
}
