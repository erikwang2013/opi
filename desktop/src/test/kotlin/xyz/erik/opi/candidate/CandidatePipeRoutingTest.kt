// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 2026-10-25 热升级隔离：PRIMARY（opi-v2，新版 DLL）与 LEGACY（opi，升级前
//! 已打开的应用）共用同一个候选模型时的取舍。纯 JVM，不碰 kernel32。

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class CandidatePipeRoutingTest {

    private val showNi =
        "{\"buffer\":\"ni\",\"candidates\":[\"你\"],\"mode\":\"pinyin\"," +
            "\"page\":1,\"page_count\":1,\"type\":\"show\"}"
    private val showHao =
        "{\"buffer\":\"hao\",\"candidates\":[\"好\"],\"mode\":\"pinyin\"," +
            "\"page\":1,\"page_count\":1,\"type\":\"show\"}"

    private fun primary(model: CandidateModel) =
        PipeServer(model, PRIMARY_PIPE_NAME, CandidateSource.PRIMARY)

    private fun legacy(model: CandidateModel) =
        PipeServer(model, LEGACY_PIPE_NAME, CandidateSource.LEGACY)

    @Test
    fun newAndOldClientsUseDifferentPipes() {
        assertEquals("\\\\.\\pipe\\opi-v2", PRIMARY_PIPE_NAME)
        assertEquals("\\\\.\\pipe\\opi", LEGACY_PIPE_NAME)
        assertTrue("旧管道名必须保持原样，旧 DLL 才连得上", LEGACY_PIPE_NAME.endsWith("\\opi"))
    }

    @Test
    fun primaryAlwaysTakesOverFromLegacy() {
        assertTrue(acceptsShow(CandidateSource.LEGACY, CandidateSource.PRIMARY))
        assertTrue(acceptsShow(CandidateSource.NONE, CandidateSource.PRIMARY))
        assertTrue(acceptsShow(CandidateSource.PRIMARY, CandidateSource.PRIMARY))
    }

    @Test
    fun legacyCannotOverwritePrimary() {
        assertFalse(acceptsShow(CandidateSource.PRIMARY, CandidateSource.LEGACY))
        assertTrue(acceptsShow(CandidateSource.LEGACY, CandidateSource.LEGACY))
        assertTrue(acceptsShow(CandidateSource.NONE, CandidateSource.LEGACY))
    }

    @Test
    fun legacyDisconnectDoesNotHidePrimaryWindow() {
        assertFalse(hidesModel(CandidateSource.PRIMARY, CandidateSource.LEGACY))
        assertTrue(hidesModel(CandidateSource.PRIMARY, CandidateSource.PRIMARY))
        assertTrue(hidesModel(CandidateSource.LEGACY, CandidateSource.LEGACY))
        // 新应用只激活、没显示过就失焦：不能藏掉旧应用正显示的候选。
        assertFalse(hidesModel(CandidateSource.LEGACY, CandidateSource.PRIMARY))
    }

    @Test
    fun primaryShowWinsOverAStaleLegacyShow() {
        val model = CandidateModel()
        primary(model).handleLine(showNi)
        legacy(model).handleLine(showHao)
        assertEquals("ni", model.buffer)
        assertEquals(listOf("你"), model.candidates)
        assertEquals(CandidateSource.PRIMARY, model.source)
    }

    @Test
    fun legacyStillWorksWhenNoNewClientIsConnected() {
        val model = CandidateModel()
        legacy(model).handleLine(showHao)
        assertEquals("hao", model.buffer)
        assertEquals(CandidateSource.LEGACY, model.source)
        assertTrue(model.visible)
    }

    @Test
    fun stateChangedOnlyWakesTheStatusBarAndKeepsTheWindow() {
        val model = CandidateModel()
        var signals = 0
        val server = PipeServer(
            model, PRIMARY_PIPE_NAME, CandidateSource.PRIMARY,
            onClientChanged = { signals++ },
        )
        server.handleLine("""{"type":"state_changed"}""")
        assertEquals("一次状态变化只发一次唤醒", 1, signals)
        assertFalse("状态变化不应该让窗口出现/消失", model.visible)
        assertEquals(CandidateSource.NONE, model.source)
        // 即使调用方给了回调，LEGACY 也不许唤醒状态栏（旧 DLL 状态不可信）。
        val legacyServer = PipeServer(
            model, LEGACY_PIPE_NAME, CandidateSource.LEGACY,
            onClientChanged = { signals++ },
        )
        legacyServer.handleLine("""{"type":"state_changed"}""")
        assertEquals("legacy 没有状态栏订阅，不该再唤醒", 1, signals)
    }

    @Test
    fun legacyPositionCannotMoveThePrimaryWindow() {
        val model = CandidateModel()
        primary(model).handleLine(showNi)
        model.x = 200
        model.y = 320
        legacy(model).handleLine("{\"type\":\"position\",\"x\":900,\"y\":100}")
        assertEquals(200, model.x)
        assertEquals(320, model.y)
        // 没有 PRIMARY 时，legacy 自己的 caret 仍然生效（旧应用照常贴光标）。
        val oldOnly = CandidateModel()
        legacy(oldOnly).handleLine(showHao)
        legacy(oldOnly).handleLine("{\"type\":\"position\",\"x\":900,\"y\":100}")
        assertEquals(900, oldOnly.x)
        assertEquals(100, oldOnly.y)
    }
}
