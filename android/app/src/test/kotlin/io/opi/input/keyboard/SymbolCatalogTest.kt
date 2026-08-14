package io.opi.input.keyboard

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** 假符号引擎：块内容 + 搜索结果均由测试注入（JNI 回文本数组，无 emoji 标记）。 */
class FakeSymbolApi : SymbolApi {
    var blocksJson = ""
    var blocksCalls = 0
    val blockContents = mutableMapOf<Short, List<String>>()
    val searchCalls = mutableListOf<String>()
    var searchResults: List<String>? = null

    override fun searchSymbols(keyword: String): Array<String>? {
        searchCalls += keyword
        return searchResults?.toTypedArray()
    }

    override fun symbolBlocks(): String {
        blocksCalls++
        return blocksJson
    }

    override fun symbolsInBlock(id: Short): Array<String>? = blockContents[id]?.toTypedArray()
}

class SymbolCatalogTest {
    private val twoBlocksJson =
        """[{"id":1,"start":12288,"end":12351,"name":"CJK 符号","common":true},""" +
            """{"id":4,"start":128512,"end":128591,"name":"表情符号","common":true}]"""

    @Test
    fun commonUnionsBlocksDedupedByFirstOccurrence() {
        val api = FakeSymbolApi().apply {
            blocksJson = twoBlocksJson
            blockContents[1] = listOf("、", "。", "、") // 块内重复
            blockContents[4] = listOf("😄")
        }
        val catalog = SymbolCatalog(api)
        assertEquals(listOf("、", "。", "😄"), catalog.common)
    }

    @Test
    fun commonCachesBlockQuery() {
        val api = FakeSymbolApi().apply {
            blocksJson = twoBlocksJson
            blockContents[1] = listOf("。")
        }
        val catalog = SymbolCatalog(api)
        catalog.common
        catalog.common
        assertEquals(1, api.blocksCalls)
    }

    @Test
    fun allUsesEmptyKeywordSearch() {
        val api = FakeSymbolApi().apply { searchResults = listOf("。", "😄") }
        val catalog = SymbolCatalog(api)
        assertEquals(listOf("。", "😄"), catalog.all)
        assertEquals(listOf(""), api.searchCalls)
    }

    @Test
    fun emojiFiltersByNonBmpCodePoint() {
        val api = FakeSymbolApi().apply { searchResults = listOf("。", "😄", "♥") }
        val catalog = SymbolCatalog(api)
        assertEquals(listOf("😄"), catalog.emoji)
    }

    @Test
    fun searchRoutesKeywordAndBlankFallsBackToAll() {
        val api = FakeSymbolApi().apply { searchResults = listOf("。") }
        val catalog = SymbolCatalog(api)
        assertEquals(listOf("。"), catalog.search("ju"))
        assertEquals(listOf("。"), catalog.search("   "))
        assertEquals(listOf("ju", ""), api.searchCalls)
    }

    @Test
    fun recentsInsertFrontDedupeAndCapAt50() {
        val catalog = SymbolCatalog(FakeSymbolApi())
        repeat(60) { catalog.recordRecent("s$it") }
        assertEquals(50, catalog.recents.size)
        assertEquals("s59", catalog.recents.first())
        catalog.recordRecent("s59") // 去重置顶
        assertEquals(50, catalog.recents.size)
        assertEquals("s59", catalog.recents.first())
    }

    @Test
    fun parseBlocksReadsSerdeSchema() {
        val blocks = SymbolCatalog.parseBlocks(
            """[{"id":1,"start":12288,"end":12351,"name":"CJK 符号","common":true},""" +
                """{"id":5,"start":13312,"end":19903,"name":"CJK 扩展 A","common":false}]"""
        )
        assertEquals(2, blocks.size)
        assertEquals(1.toShort(), blocks[0].id)
        assertEquals("CJK 符号", blocks[0].name)
        assertEquals(12288, blocks[0].start)
        assertEquals(12351, blocks[0].end)
        assertTrue(blocks[0].common)
        assertEquals(5.toShort(), blocks[1].id)
        assertFalse(blocks[1].common)
    }

    @Test
    fun parseBlocksToleratesEmptyAndGarbage() {
        assertTrue(SymbolCatalog.parseBlocks("").isEmpty())
        assertTrue(SymbolCatalog.parseBlocks("not json").isEmpty())
    }
}
