// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! opi-clip 事件/状态/控制面快照的纯逻辑门禁（无 FFI、无剪贴板、无网络）。

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

private const val ID = "0123456789abcdef0123456789abcdef"
private const val EID = "fedcba9876543210fedcba9876543210"

class ClipProtocolTest {

    @Test
    fun `parses every event shape the core emits`() {
        val discovered = """{"type":"peer_discovered","peer":{"instance_id":"$ID",
            "name":"书桌","address":"192.168.1.5:4000","fingerprint":"$EID",
            "capabilities":["text"],"trusted":true,"connected":true}}"""
        assertEquals(
            ClipEvent.PeerDiscovered(ClipPeer(ID, "书桌", "192.168.1.5:4000", true, true)),
            parseClipEvent(discovered),
        )

        assertEquals(
            ClipEvent.PeerOffline(ID),
            parseClipEvent("""{"type":"peer_offline","instance_id":"$ID"}"""),
        )
        assertEquals(
            ClipEvent.PairingPrompt(ID, "123456", true),
            parseClipEvent(
                """{"type":"pairing_prompt","instance_id":"$ID","sas":"123456","incoming":true}""",
            ),
        )
        // 成功时 reason 是 JSON null —— 这条专门盯解析器的 null 支持。
        assertEquals(
            ClipEvent.PairingFinished(ID, true, null),
            parseClipEvent(
                """{"type":"pairing_finished","instance_id":"$ID","trusted":true,"reason":null}""",
            ),
        )
        assertEquals(
            ClipEvent.PairingFinished(ID, false, "rejected"),
            parseClipEvent(
                """{"type":"pairing_finished","instance_id":"$ID","trusted":false,
                    "reason":"rejected"}""",
            ),
        )
        assertEquals(
            ClipEvent.ClipboardReceived(ID, "a\nb\"c", EID),
            parseClipEvent(
                """{"type":"clipboard_received","instance_id":"$ID","text":"a\nb\"c",
                    "version":{"wall_ms":1,"counter":2,"instance_id":"$ID",
                    "event_id":"$EID"}}""",
            ),
        )
        assertEquals(
            ClipEvent.SendFailed(ID),
            parseClipEvent("""{"type":"send_failed","instance_id":"$ID","event_id":"$EID"}"""),
        )
        assertEquals(
            ClipEvent.Error("boom"),
            parseClipEvent("""{"type":"error","message":"boom"}"""),
        )
    }

    @Test
    fun `bad or unknown events are dropped`() {
        assertNull(parseClipEvent(""))
        assertNull(parseClipEvent("not json"))
        assertNull(parseClipEvent("""{"type":"future_thing"}"""))
        assertNull(parseClipEvent("""{"type":"peer_offline"}"""))
        assertNull(parseClipEvent("""{"type":"clipboard_received","instance_id":"$ID"}"""))
        // 解析器现在认识 null（不把整份载荷判死）。
        assertEquals(JVal.JNull, parseLine("""{"a":null}""")?.get("a"))
    }

    @Test
    fun `peers array parses and malformed entries are skipped`() {
        val peers = parseClipPeers(
            """[{"instance_id":"$ID","name":"A","address":"1.2.3.4:5",
                "trusted":false,"connected":true},{"name":"bad"}]""",
        )
        assertEquals(listOf(ClipPeer(ID, "A", "1.2.3.4:5", false, true)), peers)
        assertTrue(parseClipPeers("[]").isEmpty())
        assertTrue(parseClipPeers("not-json").isEmpty())
        assertTrue(parseClipPeers("").isEmpty())
    }

    @Test
    fun `reducer tracks pending and source but never stores text`() {
        var ui = ClipUiState().applyEvent(
            ClipEvent.PeerDiscovered(ClipPeer(ID, "书桌", "10.0.0.2:1", true, true)),
        )
        ui = ui.applyEvent(ClipEvent.PairingPrompt(ID, "123456", true))
        assertEquals(ClipPending(ID, "123456", true), ui.pending[ID])

        ui = ui.applyEvent(ClipEvent.PairingFinished(ID, false, "rejected"))
        assertNull(ui.pending[ID])
        assertEquals("与设备 书桌 的配对未完成：rejected", ui.lastError)

        val received = ui.applyEvent(ClipEvent.ClipboardReceived(ID, "TOP-SECRET", EID))
        assertEquals("书桌", received.lastSource)
        assertFalse(received.toString().contains("TOP-SECRET"))
        // 同一 event_id 重复投递：状态不变（UI 不会重复提示/重复写剪贴板）。
        assertSame(received, received.applyEvent(ClipEvent.ClipboardReceived(ID, "OTHER", EID)))

        val second = received.applyEvent(ClipEvent.ClipboardReceived(ID, "OTHER", EID + "a"))
        assertEquals("书桌", second.lastSource)
        assertFalse(second.toString().contains("OTHER"))

        val failed = second.applyEvent(ClipEvent.SendFailed(ID))
        assertEquals("发送到 书桌 失败", failed.lastError)
        val longError = failed.applyEvent(ClipEvent.Error("x".repeat(400)))
        assertEquals(300, longError.lastError?.length)
        assertNull(failed.applyEvent(ClipEvent.PairingFinished(ID, true, null)).lastError)
    }

    @Test
    fun `status snapshot round trips and has no clipboard text field`() {
        val st = ClipStatus(
            enabled = true,
            running = true,
            mode = ClipMode.MANUAL,
            deviceName = "书桌 \"A\"",
            peers = listOf(ClipPeer(ID, "书桌", "10.0.0.2:1", true, true)),
            pending = listOf(ClipPending(ID, "654321", false)),
            lastSource = "书桌",
            lastError = null,
            ffiError = null,
        )
        val wire = encodeClipStatus(st)
        assertEquals(st, parseClipStatus(wire))
        assertTrue(wire.contains("\"last_error\":null"))
        assertNull(parseClipStatus("not json"))
        assertNull(parseClipStatus("""{"ok":false}"""))
    }

    @Test
    fun `mode degrades to manual only on wayland`() {
        assertEquals(ClipMode.POLL, clipModeFor(windows = true, sessionType = "wayland", waylandDisplay = null))
        assertEquals(ClipMode.POLL, clipModeFor(windows = false, sessionType = "x11", waylandDisplay = null))
        assertEquals(ClipMode.POLL, clipModeFor(windows = false, sessionType = null, waylandDisplay = ""))
        assertEquals(ClipMode.MANUAL, clipModeFor(windows = false, sessionType = "wayland", waylandDisplay = null))
        assertEquals(ClipMode.MANUAL, clipModeFor(windows = false, sessionType = "Wayland", waylandDisplay = null))
        assertEquals(ClipMode.MANUAL, clipModeFor(windows = false, sessionType = null, waylandDisplay = "wayland-0"))
    }

    @Test
    fun `device name is non-empty and capped at 64 utf8 bytes`() {
        assertEquals("opi-desktop", clipDeviceName(null))
        assertEquals("opi-desktop", clipDeviceName("   "))
        assertEquals("a".repeat(64), clipDeviceName("a".repeat(100)))
        val cjk = clipDeviceName("桌".repeat(40))
        assertEquals(21, cjk.length)
        assertTrue(cjk.toByteArray(Charsets.UTF_8).size <= 64)
        assertEquals(cjk, String(cjk.toByteArray(Charsets.UTF_8), Charsets.UTF_8))
    }

    @Test
    fun `config json escapes paths and carries the kernel fields`() {
        val json = clipConfigJson("C:\\Users\\甲\\opi", "书桌", tcpPort = 0, discoveryPort = 45987)
        val m = parseLine(json) ?: error("配置 JSON 解析失败：$json")
        assertEquals("C:\\Users\\甲\\opi", (m["data_dir"] as JVal.JStr).v)
        assertEquals("书桌", (m["device_name"] as JVal.JStr).v)
        assertEquals(0L, (m["tcp_port"] as JVal.JNum).v)
        assertEquals(45987L, (m["discovery_port"] as JVal.JNum).v)
    }
}
