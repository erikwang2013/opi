// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.asr

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** 语音状态机 / 权限门 / 结果校验的纯 JVM 门禁（Android 侧只负责喂事件）。 */
class AsrStateTest {

    @Test
    fun lifecycleReachesListeningTranscribingAndIdle() {
        var s = AsrUiState()
        s = reduce(s, AsrEvent.Start)
        assertEquals(AsrPhase.PREPARING, s.phase)
        s = reduce(s, AsrEvent.Ready)
        assertEquals(AsrPhase.LISTENING, s.phase)
        s = reduce(s, AsrEvent.Stop)
        assertEquals(AsrPhase.TRANSCRIBING, s.phase)
        s = reduce(s, AsrEvent.Final(accepted = true))
        assertEquals(AsrPhase.IDLE, s.phase)
    }

    @Test
    fun stopOnlyLeavesPreparingOrListening() {
        assertEquals(AsrPhase.IDLE, reduce(AsrUiState(), AsrEvent.Stop).phase)
        val error = AsrUiState(AsrPhase.ERROR, "x")
        assertEquals(AsrPhase.ERROR, reduce(error, AsrEvent.Stop).phase)
    }

    @Test
    fun levelOnlyMovesWhileListening() {
        var s = reduce(AsrUiState(), AsrEvent.Level(-10f))
        assertEquals(-60f, s.levelDb, 0.01f) // PREPARING 不响
        s = reduce(reduce(s, AsrEvent.Start), AsrEvent.Ready)
        s = reduce(s, AsrEvent.Level(-18.5f))
        assertEquals(-18.5f, s.levelDb, 0.01f)
        s = reduce(s, AsrEvent.Stop)
        s = reduce(s, AsrEvent.Level(-3f))
        assertEquals(-18.5f, s.levelDb, 0.01f) // 识别中音量条已隐藏
    }

    @Test
    fun failureMissingDeniedAndCancelCarryTheRightPhase() {
        val fail = reduce(AsrUiState(), AsrEvent.Fail("麦克风读取失败"))
        assertEquals(AsrPhase.ERROR, fail.phase)
        assertEquals("麦克风读取失败", fail.message)

        val missing = reduce(AsrUiState(), AsrEvent.Missing(whisper = true))
        assertEquals(AsrPhase.MODEL_MISSING, missing.phase)
        assertTrue(missing.whisper)

        val denied = reduce(AsrUiState(), AsrEvent.Denied)
        assertEquals(AsrPhase.PERMISSION_DENIED, denied.phase)
        assertTrue(denied.message.orEmpty().contains("麦克风"))

        assertEquals(AsrPhase.IDLE, reduce(fail, AsrEvent.Cancel).phase)
    }

    @Test
    fun gateChecksLibraryThenPermissionThenModel() {
        assertEquals(AsrGate.UNSUPPORTED, asrGate(false, false, false))
        assertEquals(AsrGate.NEED_PERMISSION, asrGate(true, false, false))
        assertEquals(AsrGate.MODEL_MISSING, asrGate(true, true, false))
        assertEquals(AsrGate.READY, asrGate(true, true, true))
    }

    @Test
    fun commitTextTrimsAndRejectsBlankResults() {
        assertEquals("你好", asrCommitText("  你好 \n"))
        assertEquals(null, asrCommitText(""))
        assertEquals(null, asrCommitText("   "))
        assertEquals(null, asrCommitText(null))
    }

    @Test
    fun levelFractionIsClamped() {
        assertEquals(0f, asrLevelFraction(-120f), 0.001f)
        assertEquals(0f, asrLevelFraction(-60f), 0.001f)
        assertEquals(1f, asrLevelFraction(0f), 0.001f)
        assertEquals(1f, asrLevelFraction(12f), 0.001f)
        assertEquals(0.5f, asrLevelFraction(-30f), 0.001f)
    }

    @Test
    fun generationDropsLateResults() {
        val gen = AsrGeneration()
        val first = gen.begin()
        assertTrue(gen.isCurrent(first))
        val second = gen.begin()
        assertFalse("旧会话的 final 不能写进新编辑器", gen.isCurrent(first))
        assertTrue(gen.isCurrent(second))
        gen.invalidate()
        assertFalse(gen.isCurrent(second))
    }

    @Test
    fun modelCatalogHasPinnedSizesAndHashes() {
        assertEquals(2, AsrModels.packs.size)
        for (pack in AsrModels.packs) {
            assertTrue("${pack.id} 体积应大于 100MB", pack.bytes > 100L * 1024 * 1024)
            for (file in pack.files) {
                assertTrue("${file.path} 不是 https", file.url.startsWith("https://"))
                assertTrue("${file.path} 大小未固定", file.bytes > 0)
                assertTrue(
                    "${file.path} SHA-256 长度不对",
                    Regex("[0-9a-f]{64}").matches(file.sha256),
                )
            }
        }
        assertTrue(AsrModels.VAD_SHA256.matches(Regex("[0-9a-f]{64}")))
    }
}
