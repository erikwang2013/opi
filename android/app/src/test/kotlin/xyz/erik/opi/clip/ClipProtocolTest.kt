// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.clip

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** FFI JSON ↔ Kotlin 模型（纯 JVM）：事件分派、转义/多行文本、对端/状态投影、配置编码。 */
class ClipProtocolTest {

    @Test
    fun clipboardEventKeepsEscapesAndNewlines() {
        val text = "say \"hi\"\n第二行\t\\end 😀"
        val json = """{"type":"clipboard_received","instance_id":"ab12","text":${clipJsonString(text)},"version":{"wall_ms":1}}"""
        val event = parseClipEvent(json)
        assertTrue("应识别 clipboard_received，实际：$event", event is ClipEvent.ClipboardReceived)
        event as ClipEvent.ClipboardReceived
        assertEquals("ab12", event.instanceId)
        assertEquals(text, event.text)
    }

    @Test
    fun jsonUnicodeEscapesDecodeIncludingSurrogatePairs() {
        val event = parseClipEvent(
            """{"type":"clipboard_received","instance_id":"x","text":"\u4f60\u597d\uD83D\uDE00"}""",
        ) as ClipEvent.ClipboardReceived
        assertEquals("你好😀", event.text)
    }

    @Test
    fun pairingEventsDecode() {
        val prompt = parseClipEvent(
            """{"type":"pairing_prompt","incoming":true,"instance_id":"deadbeef","name":"书桌","sas":"123456"}""",
        )
        assertEquals(
            ClipEvent.PairingPrompt("deadbeef", "书桌", "123456", incoming = true),
            prompt,
        )
        val finished = parseClipEvent(
            """{"type":"pairing_finished","instance_id":"deadbeef","reason":null,"trusted":true}""",
        )
        assertEquals(
            ClipEvent.PairingFinished("deadbeef", trusted = true, reason = null),
            finished,
        )
    }

    @Test
    fun peersDecodeTrustAndConnectionFlags() {
        val peers = parseClipPeers(
            """[{"address":"192.168.1.7:45889","capabilities":1,"connected":true,""" +
                """"fingerprint":"0011223344556677","instance_id":"aa11","name":"书房","trusted":true},""" +
                """{"address":"192.168.1.8:45889","capabilities":1,"connected":false,""" +
                """"fingerprint":"8899aabbccddeeff","instance_id":"bb22","name":"","trusted":false}]""",
        )
        assertEquals(2, peers.size)
        assertEquals("书房", peers[0].name)
        assertTrue(peers[0].trusted)
        assertTrue(peers[0].connected)
        assertEquals("bb22", peers[1].instanceId)
        assertEquals(false, peers[1].trusted)
    }

    @Test
    fun statusAllowsNullIdentityBeforeStart() {
        val status = parseClipStatus(
            """{"device_name":"Pixel","discovery_port":45888,"fingerprint":null,"instance_id":null,"peers":0,"running":false,"tcp_port":0}""",
        )
        assertEquals(ClipStatus(false, null, null, "Pixel", 0), status)
    }

    @Test
    fun malformedAndExoticInputIsIgnoredNotThrown() {
        assertNull(parseClipEvent("not json"))
        assertNull(parseClipEvent("""{"type":"clipboard_received","instance_id":"x"}"""))
        assertNull(parseClipStatus("[]"))
        assertEquals(emptyList<ClipPeer>(), parseClipPeers("""{"not":"an array"}"""))
        assertEquals(ClipEvent.Unknown("future_event"), parseClipEvent("""{"type":"future_event"}"""))
    }

    @Test
    fun transferEventsDecodeAllFields() {
        val offer = parseClipEvent(
            """{"type":"transfer_offer","instance_id":"p1","transfer_id":"01af","files":[""" +
                """{"relative_path":"dir/a.txt","size":10,"mime":"text/plain","is_dir":false},""" +
                """{"relative_path":"dir","size":0,"mime":"","is_dir":true}],""" +
                """"total_bytes":10,"outgoing":false}""",
        ) as ClipEvent.TransferOffer
        assertEquals("p1", offer.instanceId)
        assertEquals("01af", offer.transferId)
        assertEquals(10L, offer.totalBytes)
        assertEquals(false, offer.outgoing)
        assertEquals(
            listOf(
                TransferFileInfo("dir/a.txt", 10, "text/plain", false),
                TransferFileInfo("dir", 0, "", true),
            ),
            offer.files,
        )

        val progress = parseClipEvent(
            """{"type":"transfer_progress","instance_id":"p1","transfer_id":"01af",""" +
                """"transferred_bytes":4,"total_bytes":10,"file_index":0,"file_count":2,"outgoing":true}""",
        ) as ClipEvent.TransferProgress
        assertEquals(4L, progress.transferredBytes)
        assertEquals(2, progress.fileCount)
        assertTrue(progress.outgoing)

        val completed = parseClipEvent(
            """{"type":"transfer_completed","instance_id":"p1","transfer_id":"01af",""" +
                """"files":[{"relative_path":"a","size":1,"mime":"x","is_dir":false}],""" +
                """"total_bytes":1,"outgoing":false,"saved_dir":"/sdcard/opi"}""",
        ) as ClipEvent.TransferCompleted
        assertEquals("/sdcard/opi", completed.savedDir)
        assertEquals(1L, completed.totalBytes)
        assertEquals(1, completed.files.size)

        val failed = parseClipEvent(
            """{"type":"transfer_failed","instance_id":"p1","transfer_id":"01af","reason":"disk full","outgoing":true}""",
        ) as ClipEvent.TransferFailed
        assertEquals("disk full", failed.reason)
        assertTrue(failed.outgoing)

        val canceled = parseClipEvent(
            """{"type":"transfer_canceled","instance_id":"p1","transfer_id":"01af","outgoing":false}""",
        ) as ClipEvent.TransferCanceled
        assertEquals("01af", canceled.transferId)
    }

    @Test
    fun autoClipboardOnlyTargetsTrustedPeers() {
        val untrusted = ClipPeer("u", "未配对", "", "", trusted = false, connected = true)
        val trustedOffline = ClipPeer("t1", "已配对离线", "", "", trusted = true, connected = false)
        val trustedOnline = ClipPeer("t2", "已配对在线", "", "", trusted = true, connected = true)
        assertEquals(
            listOf(trustedOffline, trustedOnline),
            trustedTargets(listOf(untrusted, trustedOffline, trustedOnline)),
        )
        assertEquals(false, hasTrustedTarget(listOf(untrusted)))
        assertEquals(true, hasTrustedTarget(listOf(untrusted, trustedOffline)))
    }

    @Test
    fun configJsonEscapesPathAndName() {
        val json = clipConfigJson("""/data/"x"\clip""", "书\"房")
        assertEquals(
            """{"data_dir":"/data/\"x\"\\clip","tcp_port":0,"discovery_port":45888,"device_name":"书\"房"}""",
            json,
        )
        // 回读一遍，确认是合法 JSON：serde 侧字段值应与原值一致。
        val root = ClipJson.parse(json) as Map<*, *>
        assertEquals("""/data/"x"\clip""", root["data_dir"])
        assertEquals("书\"房", root["device_name"])
        assertEquals(45888.0, root["discovery_port"])
    }
}
