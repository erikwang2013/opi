// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 状态栏按钮的悬停提示（2026-10-20）：文案唯一源 = `StatusBarButton` 的 label/hint，
//! [tooltipText] 只做拼接。单列一份是因为 StatusBarTest 已贴着 500 行门禁。

package xyz.erik.opi.candidate

import org.junit.Assert.assertTrue
import org.junit.Test

class StatusBarTooltipTest {

    @Test
    fun `every button has a non-empty hover tooltip containing its label`() {
        // 新增按钮漏写 hint 时，这条会在它身上红。
        for (b in StatusBarButton.entries) {
            val t = tooltipText(b)
            assertTrue("$b 的提示不能为空", t.isNotBlank())
            assertTrue("$b 的提示该含 label「${b.label}」", t.contains(b.label))
            assertTrue("$b 的提示该含 hint「${b.hint}」", t.contains(b.hint))
        }
        // LOGO 的提示要说明它兼顾品牌、拖动与换肤（用户可见的功能说明）。
        val logo = tooltipText(StatusBarButton.LOGO)
        assertTrue("LOGO 提示该说明拖动", logo.contains("拖动"))
        assertTrue("LOGO 提示该说明换肤", logo.contains("皮肤"))
    }
}
