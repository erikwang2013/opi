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
        // 主区域第一格是**分词键**（发送 `'`，2026-10-15 用户裁决）；其余是 2–9 输入键。
        assertEquals(listOf("'", "2", "3", "4", "5", "6", "7", "8", "9"), sent)
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
    fun segmentKeySendsTheBoundarySeparator() {
        // 主区域第一格 = 分词键（2026-10-15 用户裁决换回 2026-10-09 的形态；此前是
        // 面「，。?!」的多标点键）。发送的是 engine-core 的**音节边界分隔符** `'`
        // （`scheme.rs::matches_segmented`，`64'26` = ni|an），**不是标点** —— 缓冲空
        // 时那一路在引擎里是引号（`engine_punct.rs`），所以宿主必须拦住
        // （ImeScreen 的 onSegment 守卫）。字面量断言：这是协议值，别拿 SEGMENT_KEY 自证。
        val seg = nineKeyRows.first().first()
        assertEquals("键面", "分词", seg.label)  // JUnit4：消息在最前
        assertEquals("发送键（协议）", "'", seg.key)
        assertEquals("读屏名", "分词", nineKeySpoken(seg))
    }

    @Test
    fun spokenNamesCoverDigitsAndSymbols() {
        assertEquals("数字 2，字母 a b c", nineKeySpoken(NineKey("2", "2", "abc")))
        // 逗号只剩左列那枚（主区域是分词键了）
        assertEquals("逗号", nineKeySpoken(NineKey("，", ",")))
        assertEquals("分词", nineKeySpoken(NineKey("分词", SEGMENT_KEY)))
        assertEquals("符号", spokenKeyName("符"))
        assertEquals("生僻字键盘", spokenKeyName("生僻字"))
    }

    @Test
    fun stripPunctsSendEnginePunctKeys() {
        // 左列（空闲时）的标点：发送键必须落在 engine-core `punctuation.rs` 的
        // CHINESE_PUNCT 表里（`,.\? !` 都有条目）—— 上层不另造标点通路。
        assertEquals(
            "键面与顺序（主流输入法左列风格）",
            listOf("，", "。", "、", "？", "！"),
            nineKeyStripPuncts.map { it.label },
        )
        assertEquals(
            "发送键（引擎标点表的键）",
            listOf(",", ".", "\\", "?", "!"),
            nineKeyStripPuncts.map { it.key },
        )
    }
}
