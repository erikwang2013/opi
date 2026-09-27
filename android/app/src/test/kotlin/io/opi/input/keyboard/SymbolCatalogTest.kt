// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package io.opi.input.keyboard

import androidx.compose.runtime.snapshots.Snapshot
import androidx.compose.runtime.snapshots.SnapshotStateList
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
    fun recentsAreSnapshotObservable() {
        // SymbolPanel 在组合中读 catalog.recents：recents 必须是 Compose 可观察状态，
        // 否则 recordRecent 写入不产生快照通知 → 「最近使用」那一行永远不出现。
        val catalog = SymbolCatalog(FakeSymbolApi())
        catalog.recordRecent("😄")

        val reads = mutableListOf<Any>()
        Snapshot.observe(readObserver = { reads += it }, writeObserver = null) { catalog.recents }

        assertTrue(
            "读 recents 必须在快照中登记读取（可观察状态），实际 reads=$reads",
            reads.any { it is SnapshotStateList<*> },
        )
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

    /** 只有 `common:true` 的块进「常用」—— 否则「常用」会变成全部符号、且以几何图形打头。 */
    @Test
    fun commonExcludesBlocksNotMarkedCommon() {
        val api = FakeSymbolApi().apply {
            blocksJson =
                """[{"id":1,"start":12288,"end":12351,"name":"CJK 符号","common":true},""" +
                    """{"id":2,"start":9632,"end":9727,"name":"几何图形","common":false}]"""
            blockContents[1] = listOf("、", "。")
            blockContents[2] = listOf("■", "□")
        }
        val catalog = SymbolCatalog(api)
        assertEquals(listOf("、", "。"), catalog.common)
    }

    /**
     * **空结果不得写进缓存。** `_common` 一旦被写成空列表就再也不会重算 ——
     * 「第一次访问时引擎没就绪 / FFI 抖一下」会让三个标签从此永远全空，
     * 且要重启进程才恢复。空 = 没拿到，不是「真的没有」。
     */
    @Test
    fun emptyResultIsNotCachedSoItRecovers() {
        val api = FakeSymbolApi().apply {
            blocksJson =
                """[{"id":1,"start":12288,"end":12351,"name":"CJK 符号","common":true}]"""
            blockContents[1] = emptyList() // 首次：引擎未就绪 → 空
        }
        val catalog = SymbolCatalog(api)
        assertTrue("首次本来就该是空", catalog.common.isEmpty())

        val firstBlocksCalls = api.blocksCalls
        api.blockContents[1] = listOf("、", "。") // 引擎就绪
        assertEquals("空结果被缓存了 → 永远恢复不了", listOf("、", "。"), catalog.common)
        assertTrue("应当重新查了一次", api.blocksCalls > firstBlocksCalls)
    }

    /** 同上，`all` 也不能缓存空；`emoji` 跟着恢复（它曾是 `by lazy`，只算一次）。 */
    @Test
    fun emptyAllIsNotCachedAndEmojiFollows() {
        val api = FakeSymbolApi().apply { searchResults = emptyList() }
        val catalog = SymbolCatalog(api)
        assertTrue(catalog.all.isEmpty())
        assertTrue(catalog.emoji.isEmpty())

        api.searchResults = listOf("。", "😄")
        assertEquals(listOf("。", "😄"), catalog.all)
        assertEquals(listOf("😄"), catalog.emoji)
    }
}
