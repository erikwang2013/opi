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
            """{"type":"pairing_prompt","incoming":true,"instance_id":"deadbeef","sas":"123456"}""",
        )
        assertEquals(
            ClipEvent.PairingPrompt("deadbeef", "123456", incoming = true),
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
