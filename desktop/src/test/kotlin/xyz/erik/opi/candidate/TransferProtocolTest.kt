// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 跨设备传输的纯逻辑门禁：开关/标签、文件 JSON、路径扫描与上限、事件解析、
//! 控制面快照、自动剪贴板只对已信任设备。无 FFI、无 AWT、无网络。

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths

private const val TID = "0123456789abcdef0123456789abcdef"
private const val PID = "fedcba9876543210fedcba9876543210"

class TransferProtocolTest {

    @Test
    fun `transfer is off by default and the tab is renamed`() {
        assertFalse("跨设备传输必须默认关闭", OpSettings().clipEnabled)
        assertEquals("跨设备传输", SettingsTab.CLIP.label)
    }

    @Test
    fun `file JSON round trips with optional name and mime`() {
        val json = encodeTransferFiles(
            listOf(
                TransferPick("/tmp/a.txt"),
                TransferPick("/tmp/b.bin", "展示名.bin", "application/octet-stream"),
            ),
        )
        assertEquals(
            listOf(
                TransferPick("/tmp/a.txt"),
                TransferPick("/tmp/b.bin", "展示名.bin", "application/octet-stream"),
            ),
            parseTransferFiles(json),
        )
    }

    @Test
    fun `file JSON rejects relative, empty and oversized manifests`() {
        assertNull(parseTransferFiles(""))
        assertNull(parseTransferFiles("[]"))
        assertNull(parseTransferFiles("""[{"path":"relative.txt"}]"""))
        assertNull(parseTransferFiles("""{"path":"/tmp/a"}"""))
        assertNull(parseTransferFiles("""[{"path":""}]"""))
        assertNull(parseTransferFiles("""[{"name":"no-path"}]"""))
        assertNull(parseTransferFiles("""[{"path":"/tmp/a","name":""}]"""))
        assertNull(
            parseTransferFiles("""[{"path":"/tmp/a","name":"${"x".repeat(257)}"}]"""),
        )
        val tooMany = (0..TRANSFER_MAX_ENTRIES).joinToString(",", "[", "]") {
            """{"path":"/tmp/$it"}"""
        }
        assertNull(parseTransferFiles(tooMany))
    }

    @Test
    fun `scan counts directories and files and rejects bad roots`() {
        val root = Files.createTempDirectory("opi-transfer-scan")
        try {
            Files.createDirectories(root.resolve("sub"))
            Files.write(root.resolve("a.txt"), byteArrayOf(1, 2, 3))
            Files.write(root.resolve("sub/b.bin"), byteArrayOf(4, 5, 6, 7))

            val prep = prepareTransferPaths(listOf(root.toString()))
            assertTrue(prep is TransferPrep.Ready)
            prep as TransferPrep.Ready
            assertEquals("目录 + 子目录 + 2 个文件", 4, prep.entries)
            assertEquals(7L, prep.bytes)
            assertEquals(listOf(TransferPick(root.toString())), parseTransferFiles(prep.json))

            assertEquals(
                "重复路径",
                "选择了重复路径",
                (prepareTransferPaths(listOf(root.toString(), root.toString()))
                    as TransferPrep.Rejected).reason,
            )
            assertTrue(prepareTransferPaths(emptyList()) is TransferPrep.Rejected)
            assertTrue(prepareTransferPaths(listOf("relative/x")) is TransferPrep.Rejected)
        } finally {
            root.toFile().deleteRecursively()
        }
    }

    @Test
    fun `explicit symlink roots are rejected`() {
        val root = Files.createTempDirectory("opi-transfer-link")
        try {
            val target = root.resolve("real.txt")
            Files.write(target, byteArrayOf(1))
            val link = root.resolve("link.txt")
            try {
                Files.createSymbolicLink(link, target)
            } catch (_: UnsupportedOperationException) {
                return // 文件系统不支持符号链接：由内核兜底，不在这里失败
            }
            val prep = prepareTransferPaths(listOf(link.toString()))
            assertTrue(prep is TransferPrep.Rejected)
            assertTrue((prep as TransferPrep.Rejected).reason.contains("符号链接"))
        } finally {
            root.toFile().deleteRecursively()
        }
    }

    @Test
    fun `limits mirror the kernel boundaries`() {
        assertNull(transferLimitError(TRANSFER_MAX_ENTRIES, 0, 0))
        assertTrue(transferLimitError(TRANSFER_MAX_ENTRIES + 1, 0, 0)!!.contains("128"))
        assertNull(transferLimitError(1, TRANSFER_MAX_TOTAL_BYTES, TRANSFER_MAX_FILE_BYTES))
        assertTrue(
            transferLimitError(1, 0, TRANSFER_MAX_FILE_BYTES + 1)!!.contains("2 GiB"),
        )
        assertTrue(
            transferLimitError(1, TRANSFER_MAX_TOTAL_BYTES + 1, 1)!!.contains("8 GiB"),
        )
        assertEquals(128, TRANSFER_MAX_ENTRIES)
        assertEquals(2L * 1024 * 1024 * 1024, TRANSFER_MAX_FILE_BYTES)
        assertEquals(8L * 1024 * 1024 * 1024, TRANSFER_MAX_TOTAL_BYTES)
    }

    @Test
    fun `picker selection stores the whole path, not its name components`() {
        // 2026-11-04 用户报障：Path 实现 Iterable<Path>，`set + path` 会命中
        // plus(Iterable) 重载，把 /tmp/.../notes.txt 拆成 tmp、目录、文件名
        // 三个元素（选中计数虚增、发送时被绝对路径校验拒绝）。必须整体增删。
        val file = Paths.get("/tmp/opi-picker-test/notes.txt")
        val picked = togglePicked(emptySet(), file)
        assertEquals(setOf(file), picked)
        assertEquals(emptySet<Path>(), togglePicked(picked, file))
    }

    @Test
    fun `transfer byte formatting uses binary units without an extra step`() {
        // 2026-11-04 用户报障：实际 M 的文件显示成 G —— 旧实现 unit 从 0 起，
        // 65 MiB 被多除一次 1024 画成 65.0 GiB。单位必须从 KiB 起、只除到 <1024。
        assertEquals("0 B", formatTransferBytes(0))
        assertEquals("1.0 KiB", formatTransferBytes(1024))
        assertEquals("65.0 MiB", formatTransferBytes(65L * 1024 * 1024))
        assertEquals("1.5 GiB", formatTransferBytes((1.5 * 1024 * 1024 * 1024).toLong()))
    }

    @Test
    fun `default save dir follows the platform convention`() {
        assertEquals(
            Paths.get("/home/e", "Downloads", "opi"),
            defaultTransferSaveDir(windows = false, userHome = "/home/e", userProfile = null),
        )
        val win = defaultTransferSaveDir(
            windows = true,
            userHome = "ignored",
            userProfile = "C:\\Users\\e",
        )
        assertTrue(win.endsWith(Paths.get("Downloads", "opi")))
        assertEquals(
            "Windows 缺 USERPROFILE 时回落 user.home",
            defaultTransferSaveDir(windows = true, userHome = "D:\\home", userProfile = null),
            defaultTransferSaveDir(windows = true, userHome = "D:\\home", userProfile = ""),
        )
    }

    @Test
    fun `parses every transfer event shape`() {
        assertEquals(
            ClipEvent.Transfer(TransferEvent.Offer(PID, TID, 2, 4096, outgoing = false)),
            parseClipEvent(
                """{"type":"transfer_offer","instance_id":"$PID","transfer_id":"$TID",
                    "files":[{"relative_path":"a"},{"relative_path":"b"}],
                    "total_bytes":4096,"outgoing":false}""",
            ),
        )
        assertEquals(
            ClipEvent.Transfer(TransferEvent.Progress(PID, TID, 1024, 4096, true)),
            parseClipEvent(
                """{"type":"transfer_progress","instance_id":"$PID","transfer_id":"$TID",
                    "transferred_bytes":1024,"total_bytes":4096,"outgoing":true}""",
            ),
        )
        assertEquals(
            ClipEvent.Transfer(TransferEvent.Completed(PID, TID, 1, 7, false, "/tmp/out")),
            parseClipEvent(
                """{"type":"transfer_completed","instance_id":"$PID","transfer_id":"$TID",
                    "files":[{"relative_path":"a"}],"total_bytes":7,"outgoing":false,
                    "saved_dir":"/tmp/out"}""",
            ),
        )
        assertEquals(
            ClipEvent.Transfer(TransferEvent.Failed(PID, TID, "disk full", true)),
            parseClipEvent(
                """{"type":"transfer_failed","instance_id":"$PID","transfer_id":"$TID",
                    "reason":"disk full","outgoing":true}""",
            ),
        )
        assertEquals(
            ClipEvent.Transfer(TransferEvent.Canceled(PID, TID, false)),
            parseClipEvent(
                """{"type":"transfer_canceled","instance_id":"$PID","transfer_id":"$TID",
                    "outgoing":false}""",
            ),
        )
        assertNull(parseClipEvent("""{"type":"transfer_mystery","instance_id":"$PID"}"""))
        assertNull(
            parseClipEvent("""{"type":"transfer_offer","instance_id":"$PID"}"""),
        )
    }

    @Test
    fun `reducer keeps terminal states and tracks progress`() {
        var state = TransferUiState()
        state = state.apply(TransferEvent.Offer(PID, TID, 2, 4096, outgoing = false))
        assertEquals(1, state.items.size)
        assertEquals(TransferState.WAITING, state.items[0].state)

        state = state.apply(TransferEvent.Progress(PID, TID, 2048, 4096, outgoing = false))
        assertEquals(TransferState.ACTIVE, state.items[0].state)
        assertEquals(2048L, state.items[0].transferredBytes)

        state = state.apply(TransferEvent.Completed(PID, TID, 2, 4096, false, "/tmp/out"))
        assertEquals(TransferState.DONE, state.items[0].state)
        assertEquals("/tmp/out", state.items[0].savedDir)
        assertEquals(4096L, state.items[0].transferredBytes)

        val done = state
        state = state.apply(TransferEvent.Progress(PID, TID, 1, 4096, false))
        assertEquals("完成后的迟到进度不得回退状态", done, state)
        assertEquals("终态不得被 mark 复活", done, done.mark(TID, TransferState.ACTIVE))

        state = state.apply(TransferEvent.Offer(PID, "aaaa", 1, 1, outgoing = true))
        state = state.apply(TransferEvent.Failed(PID, "aaaa", "peer closed", true))
        assertEquals(TransferState.FAILED, state.items.last().state)
        state = state.apply(TransferEvent.Canceled(PID, "aaaa", true))
        assertEquals("失败已是终态，取消不得覆盖", TransferState.FAILED, state.items.last().state)
    }

    @Test
    fun `status snapshot carries transfers`() {
        val transfers = listOf(
            TransferUi(TID, PID, false, 3, 100, 40, TransferState.ACTIVE),
            TransferUi("bbbb", PID, true, 1, 9, 9, TransferState.DONE),
        )
        val status = ClipStatus(
            enabled = true,
            running = true,
            mode = ClipMode.POLL,
            deviceName = "desk",
            peers = emptyList(),
            pending = emptyList(),
            lastSource = null,
            lastError = null,
            ffiError = null,
            transfers = transfers,
        )
        assertEquals(status, parseClipStatus(encodeClipStatus(status)))
        assertEquals("无 transfers 字段的旧宿主 = 空表", emptyList<TransferUi>(), parseTransferStatuses(null))
    }

    @Test
    fun `auto clipboard publishes only when enabled and a trusted peer exists`() {
        val sent = ArrayList<String>()
        var enabled = false
        var trusted = 0
        val gate = AutoClipboardGate(
            enabled = { enabled },
            trustedPeerCount = { trusted },
            publish = { sent += it },
        )
        gate.onLocalText("off")
        assertTrue(sent.isEmpty())

        enabled = true
        gate.onLocalText("no-peer")
        assertTrue("没有已信任设备时不得广播", sent.isEmpty())

        trusted = 1
        gate.onLocalText("hello")
        gate.onLocalText("")
        gate.onLocalText(null)
        assertEquals(listOf("hello"), sent)

        enabled = false
        gate.onLocalText("after-disable")
        assertEquals(listOf("hello"), sent)
    }

    @Test
    fun `only trusted and connected peers can receive files`() {
        val peers = listOf(
            ClipPeer("1", "online", "", trusted = true, connected = true),
            ClipPeer("2", "offline", "", trusted = true, connected = false),
            ClipPeer("3", "unpaired", "", trusted = false, connected = true),
        )
        assertEquals(listOf("1"), sendablePeers(peers).map { it.instanceId })
    }
}
