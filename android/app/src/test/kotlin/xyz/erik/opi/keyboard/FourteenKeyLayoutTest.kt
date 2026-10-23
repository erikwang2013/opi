// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** 14 键布局表门禁。发送键 = 字母对首字母（与 scheme.rs 的规范符号约定一致）。 */
class FourteenKeyLayoutTest {

    /** scheme.rs `Scheme::Fourteen.key_class` 的快照（2026-10-09，交叉比对用）。 */
    private val fourteenPairs = mapOf(
        'q' to "qw", 'e' to "er", 't' to "ty", 'u' to "ui", 'o' to "op",
        'a' to "as", 'd' to "df", 'g' to "gh", 'j' to "jk", 'l' to "l",
        'z' to "zx", 'c' to "cv", 'b' to "bn", 'm' to "m",
    )

    @Test
    fun fourteenKeysWithExactlyTwentySixLetters() {
        val keys = fourteenKeyRows.flatten()
        assertEquals(14, keys.size)
        val letters = keys.flatMap { it.label.toList() }
        assertEquals(26, letters.size)
        assertEquals(26, letters.toSet().size)
    }

    @Test
    fun labelLettersMatchSchemePairs() {
        for (k in fourteenKeyRows.flatten()) {
            val expected = fourteenPairs[k.key.first()] ?: error("键 ${k.key} 不在对表里")
            assertEquals("键 ${k.label} 与 scheme.rs 的字母对不符", expected, k.label)
            assertTrue("发送键必须是首字母", k.key.first() == k.label.first())
        }
    }

    /** 2026-10-24 参考图：前两行键面顶部数字提示 1–5 / 6–0（只显示，不发送）。 */
    @Test
    fun digitBadgesCoverTheFirstTwoRowsInOrder() {
        assertEquals(
            listOf("1", "2", "3", "4", "5", "6", "7", "8", "9", "0"),
            fourteenDigitBadges,
        )
        assertEquals(10, fourteenDigitBadges.size)
        assertEquals("前两行共 10 键", 10, fourteenKeyRows[0].size + fourteenKeyRows[1].size)
    }

    /** 第三行四枚字母键的符号提示：ZX(~) CV(!) BN(@) M(?)（参考图 bb.jpg）。 */
    @Test
    fun symbolHintsLiveOnTheThirdRowLetterKeys() {
        assertEquals(listOf("~", "!", "@", "?"), listOf("z", "c", "b", "m").map { fourteenSymbols[it] })
        val thirdRowKeys = fourteenKeyRows[2].map { it.key }.toSet()
        assertTrue("符号表的键必须都在第三行", fourteenSymbols.keys.all { it in thirdRowKeys })
        assertTrue("前两行键面没有符号提示", fourteenKeyRows[0].none { it.key in fourteenSymbols })
    }

    /**
     * 长按/上滑候选：`[大写对, 符号, 小写对]`。普通点击仍发协议字母
     * （14 键方案要 `q/e/t/...`，见 [FourteenKeyKeyboard] 的注释）。
     */
    @Test
    fun letterKeysOfferUppercaseSymbolLowercaseAlternatives() {
        val zx = fourteenKeyRows[2].first { it.key == "z" }
        val alts = fourteenAlternatives(zx)
        assertEquals(listOf("ZX", "~", "zx"), alts.map { it.label })
        assertEquals(listOf("Z", "~", "z"), alts.map { it.value })
        // 弹层默认选中符号那枚（键面上方就是符号）；普通点击仍是协议字母。
        assertEquals("符号是弹层默认项", "~", alts[FOURTEEN_DEFAULT_ALT_INDEX].label)
        // 前两行没有符号映射 ⇒ 无候选（普通点击 = 协议字母）。
        assertEquals(emptyList<FourteenAlt>(), fourteenAlternatives(fourteenKeyRows[0].first()))
    }

    /** 底行 2026-10-24：符 | 123 | , | 空格（最宽）| 。| 中英 | 换行。 */
    @Test
    fun bottomRowMatchesReferenceOrderAndWideSpace() {
        assertEquals(
            listOf("符", "123", ",", "空格", "。", "中英", "换行"),
            fourteenBottomKeys.map { it.label },
        )
        assertEquals(
            listOf(
                FourteenBottomAction.Symbol,
                FourteenBottomAction.Number,
                FourteenBottomAction.Comma,
                FourteenBottomAction.Space,
                FourteenBottomAction.Period,
                FourteenBottomAction.Mode,
                FourteenBottomAction.Enter,
            ),
            fourteenBottomKeys.map { it.action },
        )
        val space = fourteenBottomKeys.first { it.action == FourteenBottomAction.Space }
        assertTrue(
            "空格必须比其余键都宽（参考图是宽空格式）",
            fourteenBottomKeys.filter { it !== space }.all { it.weight < space.weight },
        )
    }
}
