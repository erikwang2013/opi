// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.runtime.snapshots.Snapshot
import androidx.compose.runtime.snapshots.SnapshotStateList
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** 假符号引擎：块内容 + 搜索结果均由测试注入；emoji 与否走独立的 `emojiSymbols()`。 */
class FakeSymbolApi : SymbolApi {
    var blocksJson = ""
    var blocksCalls = 0
    val blockContents = mutableMapOf<Short, List<String>>()

    var searchResults: List<String>? = null
    val searchCalls = mutableListOf<String>()

    /** `emojiSymbols()` 的返回。null 模拟 JNI 回 null（引擎未就绪 / FFI 抖一下）。 */
    var emojiResults: List<String>? = null
    var emojiCalls = 0

    override fun searchSymbols(keyword: String): Array<String>? {
        searchCalls += keyword
        return searchResults?.toTypedArray()
    }

    override fun emojiSymbols(): Array<String>? {
        emojiCalls++
        return emojiResults?.toTypedArray()
    }

    /** `emojiGroups()` 的返回（分类条）。null 模拟 JNI 回 null。 */
    var groupsJson: String? = null
    var groupsCalls = 0

    override fun emojiGroups(): String? {
        groupsCalls++
        return groupsJson
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

    /**
     * **判据只能来自引擎的 emoji 标志。**
     *
     * 本用例的原版叫 `emojiFiltersByNonBmpCodePoint`，断言 `♥` 不是 emoji —— 它把
     * 「含代理对（非 BMP）」这个**复制品判据**钉成了正确行为，于是在引擎换成 UTS#51
     * `Emoji` 属性后仍然全绿（绿的原因是 SymbolCatalog 当时也在用那份复制品）。
     * 现在两条都落在真实差集的两端：
     *   - `♥` U+2665 是 **BMP** 字符，却是真 emoji（旧启发式漏掉 164 条这一类）；
     *   - `◰` U+1F780 在**补充平面**，却是几何图形不是 emoji（旧启发式放行 265 条）。
     * 宿主侧任何重算（代理对 / 区间表 / 名字匹配）都会在这两条上同时判反。
     */
    @Test
    fun emojiFollowsEngineFlagNotCodePointShape() {
        val api = FakeSymbolApi().apply {
            searchResults = listOf("。", "😄", "♥", "◰")
            emojiResults = listOf("😄", "♥")
        }
        val catalog = SymbolCatalog(api)
        assertEquals(listOf("😄", "♥"), catalog.emoji)
        assertTrue("BMP 真 emoji 被漏判", catalog.isEmoji("♥"))
        assertFalse("非 BMP 的非 emoji 被放行", catalog.isEmoji("◰"))
        assertFalse("普通符号不得算 emoji", catalog.isEmoji("。"))
    }

    /** emoji 集是**一次取回、按集合查**；每条符号各打一次 FFI 会让面板滚动时卡顿。 */
    @Test
    fun emojiSetIsQueriedOnceForManyChecks() {
        val api = FakeSymbolApi().apply {
            searchResults = listOf("😄", "♥")
            emojiResults = listOf("😄", "♥")
        }
        val catalog = SymbolCatalog(api)
        catalog.emoji
        catalog.emoji
        catalog.isEmoji("♥")
        assertEquals(1, api.emojiCalls)
    }

    /**
     * 2026-10-30：`emoji` 与 `emojiGroups` 曾各拉一次 `emojiSymbols()`、各跑一遍逐条
     * `canRender`（689 条 Paint.hasGlyph 首帧探两遍）。原始表与存活统计必须一次取回、
     * 两边共用 —— 否则分类条一出现就再付一次整表探测。
     */
    @Test
    fun emojiAndGroupsShareOneFetchAndOneGlyphProbePass() {
        val api = FakeSymbolApi().apply {
            emojiResults = listOf("😀", "😄", "🐶", "🐱")
            groupsJson = """[{"name":"笑脸","start":0,"count":2},{"name":"动物","start":2,"count":2}]"""
        }
        val probes = mutableListOf<String>()
        val catalog = SymbolCatalog(api, canRender = { probes += it; true })
        assertEquals(listOf("😀", "😄", "🐶", "🐱"), catalog.emoji)
        assertEquals(
            listOf(
                SymbolCatalog.EmojiGroup("笑脸", 0, 2),
                SymbolCatalog.EmojiGroup("动物", 2, 2),
            ),
            catalog.emojiGroups,
        )
        assertEquals("emoji 与 emojiGroups 各拉了一次 emojiSymbols()", 1, api.emojiCalls)
        assertEquals("每条只该探测一次字形", listOf("😀", "😄", "🐶", "🐱"), probes)
        assertTrue(catalog.isEmoji("🐶"))
        assertFalse(catalog.isEmoji("🫫"))
        assertEquals("isEmoji 不得再触发取表", 1, api.emojiCalls)

        // 反序（先分类条再网格）同样共用：emojiGroups 重算下标时不得再探一遍。
        val api2 = FakeSymbolApi().apply {
            emojiResults = listOf("😀", "😄", "🐶", "🐱")
            groupsJson = """[{"name":"笑脸","start":0,"count":2},{"name":"动物","start":2,"count":2}]"""
        }
        val probes2 = mutableListOf<String>()
        val catalog2 = SymbolCatalog(api2, canRender = { probes2 += it; true })
        catalog2.emojiGroups
        catalog2.emoji
        assertEquals(1, api2.emojiCalls)
        assertEquals(4, probes2.size)
    }

    /** `emojiSymbols()` 回 null（JNI 常态）不得抛，只当「没拿到」。 */
    @Test
    fun nullEmojiArrayIsTreatedAsNotReady() {
        val api = FakeSymbolApi().apply { searchResults = listOf("😄"); emojiResults = null }
        val catalog = SymbolCatalog(api)
        assertTrue(catalog.emoji.isEmpty())
        assertFalse(catalog.isEmoji("😄"))
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

    /**
     * 同上，`all` 也不能缓存空；而 `emoji` 与它**是两条独立出口** ——
     * 2026-10-09 换源后表情页直接用引擎的浏览全量，「全部」还没就绪时它照样能给。
     * 旧写法 `all.filter(isEmoji)` 会让表情页跟着 `all` 一起空（本用例原名
     * `...EmojiFollows`，钉的就是那份耦合 —— 换源的收益之一就是把它解开）。
     */
    @Test
    fun emptyAllIsNotCachedAndEmojiIsIndependent() {
        val api = FakeSymbolApi().apply {
            searchResults = emptyList()
            emojiResults = listOf("😄")
        }
        val catalog = SymbolCatalog(api)
        assertTrue(catalog.all.isEmpty())
        assertEquals("表情页不再依赖 all（引擎全量独立可用）", listOf("😄"), catalog.emoji)

        api.searchResults = listOf("。", "😄")
        assertEquals(listOf("。", "😄"), catalog.all)
        assertEquals(listOf("😄"), catalog.emoji)
    }

    /**
     * 同上，**emoji 集自己也不能缓存空**。它与 `all` 是两条独立出口（引擎里也是），
     * `all` 拿到而 emoji 没拿到的窗口真实存在 —— 若把空集写进 `_emoji`，
     * 「表情」页会一直空到重启进程，而「全部」页看着一切正常。
     */
    @Test
    fun emptyEmojiSetIsNotCachedSoItRecovers() {
        val api = FakeSymbolApi().apply {
            searchResults = listOf("。", "😄")
            emojiResults = emptyList() // 首次：引擎未就绪 → 空
        }
        val catalog = SymbolCatalog(api)
        assertTrue("首次本来就该是空", catalog.emoji.isEmpty())

        val firstEmojiCalls = api.emojiCalls
        api.emojiResults = listOf("😄") // 引擎就绪
        assertEquals("空结果被缓存了 → 表情页永远空到重启", listOf("😄"), catalog.emoji)
        assertTrue("应当重新查了一次", api.emojiCalls > firstEmojiCalls)
    }

    // ---- 表情分类条（2026-10-12 用户裁决「中间是表情分类」）----

    /** 引擎 serde 的固定形状：`[{"name","start","count"}]`。 */
    private val groupsJson = """[{"name":"笑脸","start":0,"count":3},{"name":"人物","start":3,"count":2}]"""

    @Test
    fun emojiGroupsParseAndCache() {
        val api = FakeSymbolApi().apply { groupsJson = this@SymbolCatalogTest.groupsJson }
        val catalog = SymbolCatalog(api)
        assertEquals(
            listOf(
                SymbolCatalog.EmojiGroup("笑脸", 0, 3),
                SymbolCatalog.EmojiGroup("人物", 3, 2),
            ),
            catalog.emojiGroups,
        )
        val calls = api.groupsCalls
        catalog.emojiGroups
        assertEquals("拿到后应缓存，不重复打 JNI", calls, api.groupsCalls)
    }

    @Test
    fun unrenderableGlyphsAreFilteredAndGroupStartsFollow() {
        // 2026-10-12 用户报障「有个表情显示是长方形框」= U+1FAEB（Unicode 17.0），
        // 设备系统字体没这个字形。面板层按 canRender 摘掉它；**分组下标必须跟着重算**
        // —— 点分类条按 start/count 切片，不重算就切错组。
        // 夹具：5 条表情，第 1 条（下标 1）渲染不出；组 = [0,3) 笑脸 / [3,5) 人物。
        val api = FakeSymbolApi().apply {
            emojiResults = listOf("😀", "🫫", "😄", "🐶", "🐱")
            groupsJson = "[{\"name\":\"笑脸\",\"start\":0,\"count\":3},{\"name\":\"人物\",\"start\":3,\"count\":2}]"
        }
        val catalog = SymbolCatalog(api, canRender = { it != "🫫" })
        assertEquals(listOf("😀", "😄", "🐶", "🐱"), catalog.emoji)
        // 笑脸组：原 [0,3) → 摘掉 1 条 ⇒ start=0 / count=2；人物组：原 [3,5) → 前面活了 2 条 ⇒ start=2 / count=2
        assertEquals(
            listOf(
                SymbolCatalog.EmojiGroup("笑脸", 0, 2),
                SymbolCatalog.EmojiGroup("人物", 2, 2),
            ),
            catalog.emojiGroups,
        )
        // 全部渲染不出 ⇒ 空列表（页面显示空态，不是画一排空心框）
        val none = SymbolCatalog(api, canRender = { false })
        assertTrue(none.emoji.isEmpty())
    }

    @Test
    fun emptyEmojiGroupsIsNotCachedSoItRecovers() {
        val api = FakeSymbolApi().apply { groupsJson = null } // 未就绪
        val catalog = SymbolCatalog(api)
        assertTrue("首次本来就该是空", catalog.emojiGroups.isEmpty())
        api.groupsJson = """[{"name":"笑脸","start":0,"count":3963}]"""
        assertEquals(
            "空结果被缓存 → 分类条永远不出现",
            listOf(SymbolCatalog.EmojiGroup("笑脸", 0, 3963)),
            catalog.emojiGroups,
        )
    }
}
