// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.clip

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test

/** 宿主侧文件清单/限额（纯 JVM）：JSON 字段、路径/MIME、128 条目/2 GiB/8 GiB 前置拒绝。 */
class TransferFilesTest {

    private fun entry(
        path: String,
        size: Long = 1,
        mime: String = "text/plain",
        isDir: Boolean = false,
    ) = TransferEntry(path, size, mime, isDir)

    @Test
    fun filesJsonMatchesTheKernelSchema() {
        val json = transferFilesJson(
            listOf(
                TransferSpec("/cache/a.txt", "a.txt", "text/plain"),
                TransferSpec("/cache/b.bin"),
            ),
        )
        assertEquals(
            """[{"path":"/cache/a.txt","name":"a.txt","mime":"text/plain"},""" +
                """{"path":"/cache/b.bin"}]""",
            json,
        )
    }

    @Test
    fun filesJsonEscapesQuotesAndBackslashes() {
        val json = transferFilesJson(listOf(TransferSpec("""/tmp/a"b\c""", """x"y""", "text/plain")))
        assertEquals(
            """[{"path":"/tmp/a\"b\\c","name":"x\"y","mime":"text/plain"}]""",
            json,
        )
    }

    @Test
    fun limitAcceptsNormalBatch() {
        assertNull(transferLimitError(listOf(entry("a.txt"), entry("dir", isDir = true))))
    }

    @Test
    fun rejectsEmptyAndTooManyEntries() {
        assertNotNull(transferLimitError(emptyList()))
        val many = List(MAX_TRANSFER_FILES + 1) { entry("f$it.txt") }
        assertEquals(
            "条目超过 $MAX_TRANSFER_FILES 个（当前 ${MAX_TRANSFER_FILES + 1} 个），请减少文件",
            transferLimitError(many),
        )
    }

    @Test
    fun rejectsOversizedSingleFile() {
        val result = transferLimitError(listOf(entry("huge.bin", MAX_TRANSFER_FILE_BYTES + 1)))
        assertEquals("单个文件超过 2 GiB：huge.bin", result)
    }

    @Test
    fun rejectsOversizedTotal() {
        val entries = List(5) { entry("f$it.bin", MAX_TRANSFER_FILE_BYTES) }
        assertEquals("总大小超过 8 GiB，请分批发送", transferLimitError(entries))
    }

    @Test
    fun rejectsPathTooLongAndDuplicates() {
        val long = "a".repeat(MAX_TRANSFER_PATH_BYTES + 1)
        assertNotNull(transferLimitError(listOf(entry(long))))
        assertEquals(
            "存在重名路径：a.txt",
            transferLimitError(listOf(entry("a.txt"), entry("a.txt"))),
        )
    }

    @Test
    fun safeNamesStripSeparatorsAndFallBack() {
        assertEquals("a.txt", safeTransferName("/storage/emulated/0/a.txt"))
        assertEquals("a_b.txt", safeTransferName("a:b.txt"))
        assertEquals("c.txt", safeTransferName("a\\c.txt"))
        assertEquals("文件", safeTransferName("..."))
    }

    @Test
    fun duplicateNamesKeepTheirExtension() {
        val used = mutableSetOf("a.txt")
        assertEquals("a (2).txt", uniqueTransferName("a.txt", used))
        assertEquals("a (3).txt", uniqueTransferName("a.txt", used))
    }

    @Test
    fun mimeFallsBackByExtension() {
        assertEquals("image/jpeg", mimeForName("photo.JPG"))
        assertEquals("application/pdf", mimeForName("doc.pdf"))
        assertEquals("application/octet-stream", mimeForName("data.unknown"))
    }

    @Test
    fun byteFormattingIsCompact() {
        assertEquals("512 B", formatTransferBytes(512))
        assertEquals("1.0 KiB", formatTransferBytes(1024))
        assertEquals("2.0 MiB", formatTransferBytes(2L * 1024 * 1024))
    }
}
