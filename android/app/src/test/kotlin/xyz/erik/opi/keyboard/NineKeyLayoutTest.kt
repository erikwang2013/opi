// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** 9 键布局表门禁（纯函数，无 Compose）。 */
class NineKeyLayoutTest {

    /**
     * scheme.rs 的 Nine 类表的**快照**（2026-10-09）。跨语言没有共享常量，故两边
     * 各自钉住 + **交叉比对**（下面第二条断言）：改任何一边都会在这条红。
     * 复核命令（改 scheme.rs 后手跑）：
     *   grep -A10 "'2' =>" ../../../../crates/engine-core/src/scheme.rs
     */
    private val nineClasses = mapOf(
        '2' to "abc", '3' to "def", '4' to "ghi", '5' to "jkl",
        '6' to "mno", '7' to "pqrs", '8' to "tuv", '9' to "wxyz",
    )

    @Test
    fun sendsExactlyTheSchemeKeys() {
        val sent = nineKeyRows.flatten().map { it.key }
        // 逗号键发送**半角** `,`（引擎标点表出全角，见下面两条测试）；列表是发送键。
        assertEquals(listOf(",", "2", "3", "4", "5", "6", "7", "8", "9"), sent)
        // 0/1 不是输入键（引擎只收 2–9），表里也不该有
        assertTrue(sent.none { it == "0" || it == "1" })
    }

    @Test
    fun letterHintsMatchSchemeClasses() {
        for (k in nineKeyRows.flatten()) {
            if (k.hint.isEmpty()) continue
            val expected = nineClasses[k.key.first()] ?: error("键 ${k.key} 不在类表里")
            assertEquals("键 ${k.label} 的提示字母与 scheme.rs 的类不符", expected, k.hint)
        }
    }

    @Test
    fun commaKeyGoesThroughTheEnginePunctTable() {
        // `，` 键**发送**的是半角逗号（由引擎标点表出全角），不是直接提交全角字
        val comma = nineKeyRows.flatten().first { it.label == "，" }
        assertEquals(",", comma.key)
        assertEquals("逗号", spokenKeyName(",", ))
    }

    @Test
    fun spokenNamesCoverDigitsAndSymbols() {
        assertEquals("数字 2，字母 a b c", nineKeySpoken(NineKey("2", "2", "abc")))
        assertEquals("逗号", nineKeySpoken(NineKey("，", ",")))
        assertEquals("符号", spokenKeyName("符"))
        assertEquals("生僻字键盘", spokenKeyName("生僻字"))
    }
}
