// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 跨设备剪贴板开关的持久化门禁：默认关、往返、合并写不丢未知行。

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ClipSettingsTest {

    @Test
    fun `clip is off by default and legacy configs stay off`() {
        assertFalse("跨设备剪贴板必须默认关闭", OpSettings().clipEnabled)
        assertFalse(OpSettings.fromIni("").clipEnabled)
        assertFalse(OpSettings.fromIni("Learn=True\nChinesePunct=True\n").clipEnabled)
    }

    @Test
    fun `clip toggle round trips through ini`() {
        val on = OpSettings(clipEnabled = true)
        assertTrue(on.toIni().contains("ClipEnabled=True"))
        assertTrue(OpSettings.fromIni(on.toIni()).clipEnabled)

        val off = OpSettings(clipEnabled = false)
        assertTrue(off.toIni().contains("ClipEnabled=False"))
        assertFalse(OpSettings.fromIni(off.toIni()).clipEnabled)

        assertTrue(OpSettings.fromIni("ClipEnabled=true # 手写注释").clipEnabled)
    }

    @Test
    fun `merge updates clip without dropping unknown lines`() {
        val merged = mergeIni(
            "Learn=True\nClipEnabled=False\nCustom=1\n",
            OpSettings(clipEnabled = true),
        )
        assertTrue(merged.contains("ClipEnabled=True"))
        assertTrue("未知键必须原样保留", merged.contains("Custom=1"))
        assertEquals(
            "ClipEnabled 只能出现一行",
            1,
            Regex("(?m)^ClipEnabled=").findAll(merged).count(),
        )
    }
}
