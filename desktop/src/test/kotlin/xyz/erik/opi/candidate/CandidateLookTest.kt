// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 候选窗「外观」设置的门禁（2026-10-22）：设置里的候选字号三档 → 字号缩放。
//! 皮肤跟随由 `CandidateFontGateTest` 的渲染判据钉住；这里只测纯函数 ——
//! 拆分只为让两个文件都留在 500 行门禁内。

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class CandidateLookTest {

    @Test
    fun `candidate font setting scales the table and keeps the user floor`() {
        val design = PANEL_DESIGN_SIZE
        // 中档 = 既有行为（1.0）；认不得的名字与 OpSettings.fromIni 同口径回落中档。
        assertEquals(1f, candidateFontScale("中"), 0.001f)
        assertEquals(1f, candidateFontScale("认不得"), 0.001f)
        assertEquals(10f / 12f, candidateFontScale("小"), 0.001f)
        assertEquals(14f / 12f, candidateFontScale("大"), 0.001f)
        // 与 pango 三档同比：小 10pt / 中 12pt / 大 14pt。
        assertEquals(
            CandidateFont.Candidate.value * 10f / 12f,
            fontSizeFor(CandidateFont.Candidate, design, candidateFontScale("小")).value,
            0.01f,
        )
        assertEquals(
            CandidateFont.Candidate.value * 14f / 12f,
            fontSizeFor(CandidateFont.Candidate, design, candidateFontScale("大")).value,
            0.01f,
        )
        // 「小」只缩得动大字号：序号/页码不得跌破用户线（12px）。
        for (base in listOf(CandidateFont.Index, CandidateFont.Page)) {
            assertTrue(
                "「小」档把 ${base.value} 缩到用户线以下",
                fontSizeFor(base, design, candidateFontScale("小")).value >=
                    CandidateFont.MIN.value,
            )
        }
        // 正控：基准字号本身仍在用户线上（否则上面的下限断言恒真）。
        assertTrue(CandidateFont.Candidate.value >= 12f)
    }
}
