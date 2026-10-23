// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 自定义短语编解码纯 JVM 测试（无 Android）：JSON 规范化/导入形状、TSV/CSV/等号/空白
 * 五种导入、非法输入整份拒收、合并计数、分页边界。
 */
class CustomPhraseCodecTest {

    private fun ok(raw: String): CustomPhraseDecode.Ok {
        val d = CustomPhraseCodec.decode(raw)
        assertTrue("应解析成功，实际：$d", d is CustomPhraseDecode.Ok)
        return d as CustomPhraseDecode.Ok
    }

    private fun invalid(raw: String): String {
        val d = CustomPhraseCodec.decode(raw)
        assertTrue("应拒收，实际：$d", d is CustomPhraseDecode.Invalid)
        return (d as CustomPhraseDecode.Invalid).reason
    }

    // ---------- JSON ----------

    @Test
    fun canonicalJsonHasContractShape() {
        assertEquals(
            """{"version":1,"phrases":[{"trigger":"dz","text":"地址"}]}""",
            CustomPhraseCodec.encodeCanonical(listOf(CustomPhrase("dz", "地址"))),
        )
    }

    @Test
    fun canonicalJsonRoundTripsWithEscapes() {
        val phrases = listOf(
            CustomPhrase("dz", "地址"),
            CustomPhrase("hi", "say \"hi\"\nline2\\end"),
        )
        assertEquals(phrases, ok(CustomPhraseCodec.encodeCanonical(phrases)).phrases)
    }

    @Test
    fun hostStateJsonCarriesEnabled() {
        val json = CustomPhraseCodec.encodeState(false, listOf(CustomPhrase("dz", "地址")))
        val d = ok(json)
        assertEquals(false, d.enabled)
        assertEquals(listOf(CustomPhrase("dz", "地址")), d.phrases)
        // 规范化 JSON 里没有 enabled ⇒ null（引擎合同没有这个字段）
        assertEquals(null, ok(CustomPhraseCodec.encodeCanonical(listOf(CustomPhrase("dz", "地址")))).enabled)
    }

    @Test
    fun jsonArrayAcceptsObjectsAndPairsAndNormalizesCase() {
        val d = ok("""[{"trigger":"DZ","text":"地址"},["AB","Alpha"]]""")
        assertEquals(listOf(CustomPhrase("dz", "地址"), CustomPhrase("ab", "Alpha")), d.phrases)
    }

    @Test
    fun jsonWithoutVersionIsToleratedForHandWrittenFiles() {
        val d = ok("""{"phrases":[{"trigger":"dz","text":"地址"}]}""")
        assertEquals(1, d.phrases.size)
    }

    @Test
    fun emptyJsonArrayIsValidButEmptyFileIsNot() {
        // 宿主文件「关掉开关 + 删光短语」就长这样：enabled 必须能读回来
        val d = ok("""{"version":1,"enabled":false,"phrases":[]}""")
        assertEquals(false, d.enabled)
        assertEquals(emptyList<CustomPhrase>(), d.phrases)
        assertEquals(emptyList<CustomPhrase>(), ok("[]").phrases)
    }

    @Test
    fun jsonVersionMismatchRejected() {
        assertTrue(invalid("""{"version":2,"phrases":[]}""").contains("版本"))
    }

    @Test
    fun malformedJsonRejected() {
        assertTrue(invalid("""{"version":1,""").contains("JSON"))
        assertTrue(invalid("""{"version":1 "phrases":[]}""").contains("JSON"))
    }

    @Test
    fun deeplyNestedJsonRejectedWithoutStackOverflow() {
        // 递归下降必须自己封顶：否则恶意/损坏文件是 Error（栈溢出），catch 不住
        val raw = "[".repeat(1000) + "]".repeat(1000)
        assertTrue(invalid(raw).contains("嵌套"))
    }

    @Test
    fun missingPhrasesRejected() {
        assertTrue(invalid("""{"version":1}""").contains("phrases"))
    }

    @Test
    fun nonLetterOrEmptyTriggerRejected() {
        assertTrue(invalid("""{"phrases":[{"trigger":"d1","text":"x"}]}""").contains("英文字母"))
        assertTrue(invalid("""{"phrases":[{"trigger":"","text":"x"}]}""").contains("不能为空"))
    }

    @Test
    fun emptyTextRejected() {
        assertTrue(invalid("""{"phrases":[{"trigger":"dz","text":"  "}]}""").contains("短语不能为空"))
    }

    @Test
    fun triggerLengthLimitMatchesEngine() {
        val max = "a".repeat(CustomPhraseCodec.MAX_TRIGGER_LEN)
        assertEquals(1, ok("""[{"trigger":"$max","text":"x"}]""").phrases.size)
        val over = "a".repeat(CustomPhraseCodec.MAX_TRIGGER_LEN + 1)
        assertTrue(invalid("""[{"trigger":"$over","text":"x"}]""").contains("过长"))
    }

    @Test
    fun phraseCountLimitMatchesEngine() {
        // 3 个小写字母 = 17576 个不重复 trigger，够覆盖 2000 上限
        val lines = (0 until CustomPhraseCodec.MAX_PHRASES + 1).joinToString("\n") { i ->
            "${('a' + i / 676)}${('a' + (i / 26) % 26)}${('a' + i % 26)}\tv"
        }
        // 断言只钉「越限被拒」，不钉面向用户的措辞（提示文案会被产品润色）
        assertTrue(invalid(lines).contains("上限"))
    }

    @Test
    fun badEntryShapeRejectedWholeFile() {
        // 第一条合法、第二条非法 ⇒ 整份拒收（不许部分导入）
        assertTrue(
            invalid("""[{"trigger":"dz","text":"地址"},{"trigger":"d1","text":"x"}]""")
                .contains("第 2 条"),
        )
    }

    // ---------- 行式 ----------

    @Test
    fun tsvParsesAndLowercases() {
        val d = ok("dz\t地址\nAB\t你好\n")
        assertEquals(listOf(CustomPhrase("dz", "地址"), CustomPhrase("ab", "你好")), d.phrases)
    }

    @Test
    fun csvHandlesQuotedFieldsAndDoubledQuotes() {
        val d = ok("dz,\"hello, world\"\n\"ab\",\"say \"\"hi\"\"\"\n")
        assertEquals(
            listOf(CustomPhrase("dz", "hello, world"), CustomPhrase("ab", "say \"hi\"")),
            d.phrases,
        )
    }

    @Test
    fun csvMustHaveTwoColumns() {
        assertTrue(invalid("dz,地址,多余").contains("两列"))
    }

    @Test
    fun unclosedQuoteRejected() {
        assertTrue(invalid("dz,\"未闭合").contains("引号"))
    }

    @Test
    fun equalsAndWhitespaceLinesParse() {
        val d = ok("dz=地址\nab 你好\n")
        assertEquals(listOf(CustomPhrase("dz", "地址"), CustomPhrase("ab", "你好")), d.phrases)
    }

    @Test
    fun commaAfterEqualsBelongsToText() {
        val d = ok("dz=hello, world")
        assertEquals(listOf(CustomPhrase("dz", "hello, world")), d.phrases)
    }

    @Test
    fun lineWithoutSeparatorRejected() {
        assertTrue(invalid("dz地址").contains("分隔符"))
    }

    @Test
    fun blankLinesSkippedButEmptyImportRejected() {
        assertTrue(invalid("  \n\n").contains("空的"))
        assertEquals(1, ok("\n\ndz=地址\n\n").phrases.size)
    }

    @Test
    fun bomIsIgnored() {
        assertEquals(listOf(CustomPhrase("dz", "地址")), ok("\uFEFFdz=地址").phrases)
    }

    @Test
    fun duplicateTriggerLaterWins() {
        val d = ok("dz\tfirst\ndz\tsecond")
        assertEquals(listOf(CustomPhrase("dz", "second")), d.phrases)
    }

    // ---------- 合并 ----------

    @Test
    fun mergeCountsAddedUpdatedAndSkipsIdentical() {
        val existing = listOf(CustomPhrase("dz", "地址"), CustomPhrase("ab", "Alpha"))
        val imported = listOf(
            CustomPhrase("dz", "新地址"), // 更新
            CustomPhrase("cd", "新条目"), // 新增
            CustomPhrase("ab", "Alpha"), // 一字不差 ⇒ 跳过
        )
        val m = CustomPhraseCodec.merge(existing, imported)
        assertEquals(1, m.added)
        assertEquals(1, m.updated)
        assertEquals(
            listOf(CustomPhrase("dz", "新地址"), CustomPhrase("ab", "Alpha"), CustomPhrase("cd", "新条目")),
            m.phrases,
        )
    }

    @Test
    fun mergeTreatsTriggerCaseInsensitively() {
        val m = CustomPhraseCodec.merge(listOf(CustomPhrase("dz", "地址")), listOf(CustomPhrase("DZ", "地址2")))
        assertEquals(0, m.added)
        assertEquals(1, m.updated)
        assertEquals(listOf(CustomPhrase("dz", "地址2")), m.phrases)
    }

    @Test
    fun mergeLaterEntryInSameImportWins() {
        val m = CustomPhraseCodec.merge(
            emptyList(),
            listOf(CustomPhrase("dz", "一"), CustomPhrase("dz", "二")),
        )
        assertEquals(1, m.added)
        assertEquals(0, m.updated)
        assertEquals(listOf(CustomPhrase("dz", "二")), m.phrases)
    }

    // ---------- 分页 ----------

    @Test
    fun pageCountBoundaries() {
        assertEquals(1, CustomPhraseCodec.pageCount(0))
        assertEquals(1, CustomPhraseCodec.pageCount(1))
        assertEquals(1, CustomPhraseCodec.pageCount(10))
        assertEquals(2, CustomPhraseCodec.pageCount(11))
        assertEquals(3, CustomPhraseCodec.pageCount(21))
        assertEquals(3, CustomPhraseCodec.pageCount(30))
    }

    @Test
    fun clampPageKeepsInsideRange() {
        assertEquals(0, CustomPhraseCodec.clampPage(-3, 25))
        assertEquals(2, CustomPhraseCodec.clampPage(9, 25))
        assertEquals(0, CustomPhraseCodec.clampPage(5, 0))
    }

    @Test
    fun pageSliceTakesTenPerPageAndClamps() {
        val all = (1..25).map { CustomPhrase("k$it", "v$it") }
        assertEquals(10, CustomPhraseCodec.pageSlice(all, 0).size)
        assertEquals("k11", CustomPhraseCodec.pageSlice(all, 1).first().trigger)
        assertEquals(5, CustomPhraseCodec.pageSlice(all, 2).size)
        assertEquals("k21", CustomPhraseCodec.pageSlice(all, 2).first().trigger)
        // 越界页按最后一页取（删除条目后页码可能越界）
        assertEquals("k21", CustomPhraseCodec.pageSlice(all, 99).first().trigger)
    }

    @Test
    fun encodeTsvEndsWithNewlineAndEmptyStaysEmpty() {
        assertEquals("", CustomPhraseCodec.encodeTsv(emptyList()))
        assertEquals("dz\t地址\n", CustomPhraseCodec.encodeTsv(listOf(CustomPhrase("dz", "地址"))))
    }
}
