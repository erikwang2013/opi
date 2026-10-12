// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 搜索的**纯函数**门禁（2026-10-13）：`opi-ctl search` 的应答怎么翻译成显示态
//! （[searchResult] / [searchView]）。界面只画这些函数的产物，所以纪律钉在这里：
//! **坏载荷不许画成「没有匹配」**（那是最容易混过去的假绿）、**Pending 不是失败**、
//! 表情页的命中要按表情集筛、两页都要过字形覆盖。

package xyz.erik.opi.candidate

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SymbolSearchTest {

    private val emoji = setOf("😀", "🐶")
    private val allPass: (String) -> Boolean = { true }

    @Test
    fun `hits keep the engine order verbatim`() {
        val v = searchView(
            SymbolSearch.Hits(listOf("♥", "、", "😀")),
            emojiTab = false,
            emojiSet = emoji,
            canRender = allPass,
        )
        assertEquals(listOf("♥", "、", "😀"), (v as SearchView.Hits).texts)
    }

    @Test
    fun `emoji tab keeps only emoji hits`() {
        // 引擎的符号搜索不过滤表情（命中的是全表），表情页只该显示表情里的命中。
        val v = searchView(
            SymbolSearch.Hits(listOf("♥", "😀", "dog", "🐶")),
            emojiTab = true,
            emojiSet = emoji,
            canRender = allPass,
        )
        assertEquals(listOf("😀", "🐶"), (v as SearchView.Hits).texts)
    }

    @Test
    fun `unrenderable hits are dropped on both tabs`() {
        // 搜索路径不豁免字形覆盖：搜出来的东西也不该是空心框。
        for (emojiTab in listOf(false, true)) {
            val v = searchView(
                SymbolSearch.Hits(listOf("😀", "♡")),
                emojiTab = emojiTab,
                emojiSet = emoji,
                canRender = { it != "♡" },
            )
            assertEquals("emojiTab=$emojiTab", listOf("😀"), (v as SearchView.Hits).texts)
        }
    }

    @Test
    fun `failure is reported verbatim and never as an empty result`() {
        val why = "「opi-ctl search」退出码 3：通道不通"
        val v = searchView(SymbolSearch.Failed(why), emojiTab = false, emojiSet = emoji, canRender = allPass)
        // 关键：它是 Failed 而不是 None —— 坏载荷画成「没有匹配」是这个功能最容易撒的谎。
        assertEquals(why, (v as SearchView.Failed).why)
    }

    @Test
    fun `pending and no-hit are distinct states`() {
        assertTrue(searchView(null, false, emoji, allPass) === SearchView.Pending)
        assertTrue(searchView(SymbolSearch.Hits(emptyList()), false, emoji, allPass) === SearchView.None)
        // 命中全被筛掉也算「没有匹配」（不是失败：引擎确实回了话）。
        assertTrue(
            searchView(SymbolSearch.Hits(listOf("♥")), emojiTab = true, emojiSet = emoji, canRender = allPass)
                === SearchView.None,
        )
    }

    @Test
    fun `searchResult maps exit code zero plus payload to hits`() {
        val v = searchResult(0, "[\"♥\",\"、\"]")
        assertEquals(listOf("♥", "、"), (v as SymbolSearch.Hits).texts)
        // 空数组是**合法**载荷（引擎说没搜到），不是坏载荷。
        assertEquals(emptyList<String>(), (searchResult(0, "[]") as SymbolSearch.Hits).texts)
    }

    @Test
    fun `searchResult treats a broken payload as failure not as no-hit`() {
        // "ok" 是**版本对不上**的指纹：旧插件（没有 Search 方法）或把 search 接回 Trigger
        // 那种接线错，回的就是它。桌面若把它当结果画，网格里会出现 o 和 k 两个字。
        for (bad in listOf("ok", "[", "[\"a\",\"b\"", "{}", "", "[1,2]", "[\"a\"] 尾随垃圾")) {
            val v = searchResult(0, bad)
            assertTrue("载荷 `$bad` 该判失败，实得 ${v::class.simpleName}", v is SymbolSearch.Failed)
            // 原始输出要带进原因里（用户/开发者才有线索）。
            assertTrue("原因里该带原始输出：${(v as SymbolSearch.Failed).why}", v.why.contains("看不懂"))
        }
    }

    @Test
    fun `searchResult carries the exit code and the original reason line`() {
        val f = searchResult(4, "opi-ctl: 符号搜索入口未接线（引擎侧未落地），本次查询无效")
            as SymbolSearch.Failed
        assertTrue("退出码要报出来：${f.why}", f.why.contains("退出码 4"))
        assertTrue("原因行要原样带出来：${f.why}", f.why.contains("未接线"))
        // 空关键词那条（2）走同一条路 —— 它在线协议里也可能从服务端回来（客户端本地也拦一次）。
        assertTrue(searchResult(2, "err:2:搜索关键字为空") is SymbolSearch.Failed)
    }
}
