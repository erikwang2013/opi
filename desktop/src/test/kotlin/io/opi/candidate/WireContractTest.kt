// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 线协议**字段契约**：`PipeServer.handleLine` ↔ `crates/tsf-opi/src/candidate_io.rs`
//! 的 `CandidateClient::show`/`hide`/`position`。
//!
//! 纯 JVM：handleLine 只碰 CandidateModel 与 Protocol.kt 的解析器，不碰 kernel32
//! （named pipe 只能在 Windows 验收，见 PipeServer.kt `connect` 的「未实测」段）。
//!
//! 为什么断言要落在**这一层**：ProtocolDepthTest 只证明「这行能不能解析」，证明不了
//! 「字段名有没有抄错」—— `page_count` 拼成 `pageCount` 只是那个字段静默丢失，
//! 窗口照常显示、不报错。所以下面每条合法输入都是 serde_json **1.0.151
//! `json!{}.to_string()` 的真实字节**（本机实测：键按字典序、中文不转义），不是手写的。

package io.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class WireContractTest {

    /** 喂若干行 → 返回被更新的模型。PipeServer 构造不碰 kernel32，Linux 上可跑。 */
    private fun feed(vararg lines: String): CandidateModel {
        val model = CandidateModel()
        val server = PipeServer(model)
        lines.forEach(server::handleLine)
        return model
    }

    /**
     * 发端 `show` 的实测字节（键 = BTreeMap 字典序，非 ASCII 原样 UTF-8）。
     * 对照 `candidate_io.rs` 的 `CandidateClient::show`。
     */
    private val realShow =
        "{\"buffer\":\"ni\",\"candidates\":[\"你\",\"尼\"],\"mode\":\"pinyin\"," +
            "\"page\":1,\"page_count\":3,\"type\":\"show\"}"

    @Test
    fun showMapsEveryField() {
        val m = feed(realShow)
        assertTrue("show 必须让窗口可见", m.visible)
        assertEquals("ni", m.buffer)
        assertEquals(listOf("你", "尼"), m.candidates)
        assertEquals(3, m.pageCount)
        assertEquals("pinyin", m.mode)
    }

    /**
     * `page` 在线协议上是 **1 起**（发端 `CandidateSink::on_composition_changed` 已经
     * 做过 `page + 1`；引擎侧 `TsfLogic::page()` 是 0 起）。收端**不得**再加一次 ——
     * 加了会让「第 3 页」显示成「第 4 页」，且只在第 2 页起才看得出来。
     */
    @Test
    fun pageIsOneBasedOnTheWire() {
        assertEquals(1, feed(realShow).page)
        assertEquals(3, feed(realShow.replace("\"page\":1", "\"page\":3")).page)
    }

    /**
     * 键序不可依赖：serde_json 未开 `preserve_order` 时 `Map` 是 BTreeMap，
     * 发出来的是**字典序**（实测 `select` 是 `{"index":0,"type":"select"}`，index 在前）。
     * 收端一律按名取，故两种键序必须映射出同一个模型。
     */
    @Test
    fun keyOrderDoesNotMatter() {
        val alphabetical = feed(realShow)
        // 同一份数据、按键插入序排（发端 json!{} 的书写顺序），仅键序不同
        val insertionOrder = feed(
            "{\"type\":\"show\",\"buffer\":\"ni\",\"candidates\":[\"你\",\"尼\"]," +
                "\"page\":1,\"page_count\":3,\"mode\":\"pinyin\"}",
        )
        assertEquals(insertionOrder.buffer, alphabetical.buffer)
        assertEquals(insertionOrder.candidates, alphabetical.candidates)
        assertEquals(insertionOrder.page, alphabetical.page)
        assertEquals(insertionOrder.pageCount, alphabetical.pageCount)
        assertEquals(insertionOrder.mode, alphabetical.mode)
        assertEquals(insertionOrder.visible, alphabetical.visible)
    }

    @Test
    fun hideClearsVisibility() {
        val m = feed(realShow, "{\"type\":\"hide\"}")
        assertFalse("hide 必须让窗口不可见", m.visible)
    }

    @Test
    fun positionMovesWindow() {
        // 单位 = AWT 逻辑像素，收端直接当 .dp 用（见 Main.kt 头注释；两侧约定一致）。
        val m = feed(realShow, "{\"type\":\"position\",\"x\":120,\"y\":340}")
        assertEquals(120, m.x)
        assertEquals(340, m.y)
    }

    /**
     * 模式字符串必须与 `candidate_io.rs` 的 `mode_str` 逐条对齐。
     * `else -> "拼音"` 是**载荷分支**：Rust 侧有意把 `Mode::Traditional` 也编码成
     * `"pinyin"`（见 `mode_str` 注释），改掉它两端不会报错，只会静默显示错标签。
     */
    @Test
    fun modeStringsMatchModeStr() {
        assertEquals("拼音", modeLabel("pinyin")) // ← 同时是 Mode::Traditional 的落点
        assertEquals("英文", modeLabel("english"))
        assertEquals("数字", modeLabel("number"))
        assertEquals("符号", modeLabel("symbol"))
        assertEquals("拼音", modeLabel("traditional")) // 发端目前不发；见 mode_str 注释
    }

    /** 坏行只丢弃，不许抛、不许污染已有状态（serveLoop 存活靠这条）。 */
    @Test
    fun malformedLinesAreIgnoredNotFatal() {
        val m = feed(realShow)
        // 非 JSON、类型错、字段缺失、空行
        feedInto(m, "not json at all", "{\"type\":123}", "{\"type\":\"show\"}", "", "[1,2,3]")
        assertEquals("坏行不得冲掉 buffer", "ni", m.buffer)
        assertEquals(listOf("你", "尼"), m.candidates)
    }

    /** `\uXXXX` 转义同样要能解析（serde_json 默认不转义，但解析器的转义分支要活着）。 */
    @Test
    fun escapedNonAsciiAlsoParses() {
        val m = feed(realShow.replace("你", "\\u4f60"))
        assertEquals(listOf("你", "尼"), m.candidates)
    }

    /** 复用同一个 server，喂若干行（坏行场景要保持模型已被 show 初始化过）。 */
    private fun feedInto(model: CandidateModel, vararg lines: String) {
        val server = PipeServer(model)
        lines.forEach(server::handleLine)
    }
}
