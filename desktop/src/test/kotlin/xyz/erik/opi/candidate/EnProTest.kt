// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 英文专业键（en/pro）的门禁：生效范围（只在英文输入法）、老插件识别、常驻开关。
//! 2026-10-19 用户裁决：「这个键只控制是否匹配英文单词只在英文输入法有效，设置增加
//! 是否隐藏控制。」—— 同日**后一条裁决覆盖了位置**：键移到简繁键（TRAD）之后，
//! 并按设计图画白底圆角键帽（en 居中、pro 右下）。旧裁决的「放在标点符号键前面」
//! 作废，别按它改回枚举序（StatusBarTest 的 `button order is the display order` 钉住新顺序）。

package xyz.erik.opi.candidate

import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class EnProTest {

    @Test
    fun `enpro only runs in english mode`() {
        val en = ImeState(zh = false, englishProKnown = true)
        assertNull("英文输入法下应放行", enproBlockReason(en))
        val reason = enproBlockReason(en.copy(zh = true))
        assertNotNull("中文输入法下应在动作前挡住", reason)
        assertTrue("挡住时要说明原因（实际：$reason）", reason!!.contains("英文"))
    }

    @Test
    fun `old plugin without enpro state is told to restart`() {
        // 老插件（v1.15 之前加载的）state 行没有 enpro= 位 ⇒ 发动作只会收到
        // err:5「未知动作」。与其把裸错误弹给用户，不如直接提示重启 fcitx5。
        val reason = enproBlockReason(ImeState(zh = false))
        assertNotNull(reason)
        assertTrue("老插件该提示重启（实际：$reason）", reason!!.contains("重启"))
    }

    @Test
    fun `state parser records enpro support separately from the value`() {
        val supported = parseImeState("lang=en;trad=off;punct=on;fw=off;im=;mode=en26;enpro=on")
        assertTrue(supported.englishProKnown)
        assertTrue(supported.englishPro)
        assertFalse("缺键 = 老插件，不是「普通模式」", parseImeState("lang=en;trad=off;punct=on;fw=off;im=;mode=en26").englishProKnown)
        assertFalse(parseImeState("lang=zh;enpro=off").englishPro)
    }

    @Test
    fun `enpro has a hide switch and defaults on`() {
        assertTrue("默认常驻", StatusBarSettings().resident(StatusBarButton.ENPRO))
        val off = StatusBarSettings().with(StatusBarButton.ENPRO, false)
        assertFalse(off.enpro)
        assertFalse(StatusBarButton.ENPRO in visibleStatusButtons(off, windows = false))
        assertTrue(StatusBarButton.ENPRO in visibleStatusButtons(off.with(StatusBarButton.ENPRO, true), windows = false))
        // Windows 没有 opi-ctl englishpro ⇒ 不出开关也不上条（与其余 opi-ctl 动作同规）。
        assertFalse(statusButtonWired(StatusBarButton.ENPRO, windows = true))
        assertTrue(statusButtonWired(StatusBarButton.ENPRO, windows = false))
        assertFalse(StatusBarButton.ENPRO in visibleStatusButtons(StatusBarSettings(), windows = true))
    }
}
