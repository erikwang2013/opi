// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! Windows 控制面的**纯协议层**门禁（ControlPlane.kt）：请求字节、应答解析、broker
//! 校验/转发判定。本机（Linux）跑不了 JNA 管道，但这一层是两端唯一共用的契约 ——
//! 字段名抄错、布尔漏认、白名单放行未知动作，都会在这里红，而不是在真机上表现为
//! 「点了没反应」。TSF 侧的同一份语义在 `crates/tsf-opi/src/ctl_tests.rs` 钉。

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test

class ControlPlaneTest {

    private val stateLine = "lang=zh;trad=off;punct=on;fw=on;im=opi;mode=pinyin;enpro=off"

    // ---------- 应答解析 ----------

    @Test
    fun `state reply carries the tsf state line verbatim`() {
        val r = parseControlReply(
            """{"id":1,"ok":true,"state":"$stateLine","type":"ctl_result"}""",
        )!!
        assertTrue(r.ok)
        assertEquals(stateLine, r.state)
        assertNull(r.token)
        assertNull(r.texts)
        assertNull(r.error)
    }

    @Test
    fun `action reply exposes the mode token`() {
        val r = parseControlReply("""{"id":2,"ok":true,"token":"wubi","type":"ctl_result"}""")!!
        assertTrue(r.ok)
        assertEquals("wubi", r.token)
        assertNull(r.state)
    }

    @Test
    fun `search reply exposes texts in order`() {
        val r = parseControlReply("""{"id":3,"ok":true,"texts":["😄","🙂"],"type":"ctl_result"}""")!!
        assertEquals(listOf("😄", "🙂"), r.texts)
    }

    @Test
    fun `error reply keeps the message`() {
        val r = parseControlReply(
            """{"error":"中文标点配置保存失败：denied","id":4,"ok":false,"type":"ctl_result"}""",
        )!!
        assertFalse(r.ok)
        assertEquals("中文标点配置保存失败：denied", r.error)
        assertNull(r.texts)
    }

    @Test
    fun `broker failure label is the same shape as a tsf reply`() {
        val why = "输入法未连接或应答超时"
        val r = parseControlReply(controlErrorLabel(why))!!
        assertFalse(r.ok)
        assertEquals(why, r.error)
    }

    @Test
    fun `malformed replies are rejected instead of degraded`() {
        // 坏载荷必须 null（调用方转成可见失败），不能当成「成功但没数据」。
        assertNull(parseControlReply(null))
        assertNull(parseControlReply(""))
        assertNull(parseControlReply("not json"))
        assertNull("别的消息类型不是控制应答", parseControlReply("""{"ok":true,"type":"show"}"""))
        assertNull("ok 必须是布尔而不是字符串", parseControlReply("""{"ok":"true","type":"ctl_result"}"""))
        assertNull("缺 ok", parseControlReply("""{"state":"$stateLine","type":"ctl_result"}"""))
        assertNull(
            "texts 里混进非字符串 = 坏载荷",
            parseControlReply("""{"ok":true,"texts":["a",1],"type":"ctl_result"}"""),
        )
    }

    // ---------- 请求字节 ----------

    @Test
    fun `request builders are the broker contract`() {
        assertEquals("""{"op":"state"}""", controlStateRequest())
        assertEquals("""{"action":"punct","op":"action"}""", controlActionRequest("punct"))
        assertEquals("""{"op":"search","query":"smile"}""", controlSearchRequest("smile"))
        assertEquals("""{"op":"commit","text":"😄"}""", controlCommitRequest("😄"))
        // 符号里有 `"`、`\`、换行：不转义会让 broker 整行解析失败（点了没反应）。
        assertEquals(
            """{"op":"commit","text":"\"\\\n"}""",
            controlCommitRequest("\"\\\n"),
        )
        // 发出去的每一行 broker 都得能读（契约两端同一份 JSON 口径）。
        for (line in listOf(
            controlStateRequest(),
            controlActionRequest("mode"),
            controlSearchRequest("smile"),
            controlCommitRequest("\"\\\n"),
        )) {
            assertTrue("broker 该认自己 client 的请求：$line", controlDispatch(line) is ControlDispatch.Forward)
        }
    }

    // ---------- broker 判定 ----------

    private fun forward(line: String): ControlDispatch.Forward {
        val d = controlDispatch(line)
        if (d !is ControlDispatch.Forward) fail("该转发却拒绝了：$line → $d")
        return d as ControlDispatch.Forward
    }

    private fun rejected(line: String, why: String): ControlDispatch.Reject {
        val d = controlDispatch(line)
        if (d !is ControlDispatch.Reject) fail("该拒绝却转发了：$line → $d")
        val r = d as ControlDispatch.Reject
        val parsed = parseControlReply(r.reply)!!
        assertFalse("拒绝应答必须是 ok=false：${r.reply}", parsed.ok)
        assertTrue("拒绝原因该提到「$why」（实际：${parsed.error}）", parsed.error!!.contains(why))
        return r
    }

    @Test
    fun `state and every allowed action are forwarded`() {
        assertEquals("\"op\":\"state\"", forward("""{"op":"state"}""").opFields)
        for (action in listOf("lang", "trad", "punct", "fullwidth", "englishpro", "mode")) {
            assertEquals(
                "动作名/字段名抄错就会让 TSF 报未知动作：$action",
                "\"action\":\"$action\",\"op\":\"action\"",
                forward("""{"action":"$action","op":"action"}""").opFields,
            )
        }
    }

    @Test
    fun `unknown or malformed actions are rejected at the broker`() {
        // 白名单在服务端：本机任意进程都能连这条管道（信任边界见 ControlPlane.kt 头）。
        rejected("""{"action":"symbols","op":"action"}""", "未知动作")
        rejected("""{"action":"insert","op":"action"}""", "未知动作")
        rejected("""{"op":"action"}""", "缺 action")
        rejected("""{"action":1,"op":"action"}""", "缺 action")
    }

    @Test
    fun `search bounds are enforced at the broker`() {
        assertEquals(
            "\"op\":\"search\",\"query\":\"smile\"",
            forward("""{"op":"search","query":"smile"}""").opFields,
        )
        assertEquals(
            "256 字节是上限，该放行",
            "\"op\":\"search\",\"query\":\"${"a".repeat(256)}\"",
            forward("""{"op":"search","query":"${"a".repeat(256)}"}""").opFields,
        )
        // 上限按**字节**不是字符（TSF 侧 ctl.rs 的 MAX_QUERY 就是字节）：85 个汉字
        // = 255 字节放行；86 个 = 258 字节必须在这里拒绝，而不是让 TSF 静默丢弃。
        forward("""{"op":"search","query":"${"你".repeat(85)}"}""")
        rejected("""{"op":"search","query":""}""", "搜索词")
        rejected("""{"op":"search","query":"${"a".repeat(257)}"}""", "搜索词")
        rejected("""{"op":"search","query":"${"你".repeat(86)}"}""", "搜索词")
        rejected("""{"op":"search"}""", "缺 query")
    }

    @Test
    fun `commit bounds are enforced at the broker`() {
        assertEquals(
            "\"op\":\"commit\",\"text\":\"你好\"",
            forward("""{"op":"commit","text":"你好"}""").opFields,
        )
        assertEquals(
            "1024 字符是上限，该放行",
            "\"op\":\"commit\",\"text\":\"${"a".repeat(1024)}\"",
            forward("""{"op":"commit","text":"${"a".repeat(1024)}"}""").opFields,
        )
        rejected("""{"op":"commit","text":""}""", "文本")
        rejected("""{"op":"commit","text":"${"a".repeat(1025)}"}""", "文本")
        rejected("""{"op":"commit"}""", "缺 text")
    }

    @Test
    fun `non control payloads are rejected`() {
        rejected("""{"op":"klass"}""", "未知控制操作")
        rejected("""{"op":1}""", "未知控制操作")
        rejected("[]", "不是 JSON 对象")
        rejected("not json", "不是 JSON 对象")
    }

    // ---------- 按钮 → 动作名 ----------

    @Test
    fun `status buttons map to the frozen control action names`() {
        assertEquals("trad", statusActionOp(StatusBarButton.TRAD))
        assertEquals("punct", statusActionOp(StatusBarButton.PUNCT))
        assertEquals("fullwidth", statusActionOp(StatusBarButton.FULLWIDTH))
        assertEquals("lang", statusActionOp(StatusBarButton.LANG))
        assertEquals("englishpro", statusActionOp(StatusBarButton.ENPRO))
        // 这些按钮走别的路：MODE 走 controlMode（要读新 token），其余是进程内开窗。
        for (b in listOf(
            StatusBarButton.MODE, StatusBarButton.SYMBOLS, StatusBarButton.VK,
            StatusBarButton.LOGO, StatusBarButton.SETTINGS,
        )) {
            assertNull("$b 不该走「动作名」这条路", statusActionOp(b))
        }
    }
}
