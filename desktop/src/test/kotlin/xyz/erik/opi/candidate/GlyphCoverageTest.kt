// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 字形覆盖判据的门禁（2026-10-13）：**规则本身** —— 多码位一律放行、只有单码位问字体、
//! 空串不可渲染。规则逐条对齐 Android `SymbolCatalog.canRenderGlyph`（那边同样是
//! `codePointCount > 1 || hasGlyph`）：两端漂了就是「同一份数据在两端过滤得不一样」。
//!
//! ⚠️ 这一份测的是**规则**，不是「本机字体长什么样」：单码位有没有字形是字体决定的事实，
//! 换台机器/换个 Noto 版本答案就变。所以夹具用**发现式**取法 —— 先探出一个本机确实缺的
//! 码位，再拿它当夹具；一个都探不到就红（不是跳过）：那说明前提没了，得换候选，
//! 而不是让一条测不出东西的用例静静变绿。

package xyz.erik.opi.candidate

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 本机可能缺字形的候选码位：先真 emoji 码位（Unicode 17.0 新增，正是这次报障的来源），
 * 再私用区/未分配码位 —— 私用区按定义没有正经字体去覆盖，是「几乎必然缺」的兜底。
 * 顺序有意义：优先用真数据里出现的那种码位当夹具。
 *
 * 文件级（不是类成员）：字形判据门禁与离屏渲染探针共用同一个夹具取法。
 */
private val uncoveredCandidates = listOf(0x1FAEB, 0x1FAEC, 0x1FAEF, 0x10FFFD, 0xF0000, 0xE0001)

/** 探一个本机画不出的单码位；一个都探不到 ⇒ 夹具前提没了，红（不是跳过）。 */
internal fun discoveredUncoveredCodePoint(): String {
    for (cp in uncoveredCandidates) {
        val s = String(Character.toChars(cp))
        if (!GlyphCoverage.canRender(s)) return s
    }
    throw AssertionError(
        "本机字体把候选码位（${uncoveredCandidates.joinToString { "U+%04X".format(it) }}）全盖住了：" +
            "这份测试的夹具前提没了 —— 换更生僻的候选或换测法，别让用例空转变绿",
    )
}

class GlyphCoverageTest {

    /** 探一个本机画不出的单码位；一个都没有 ⇒ 夹具前提没了，红。 */
    private fun uncoveredCodePoint(): String = discoveredUncoveredCodePoint()

    @Test
    fun `single code point without a glyph is not renderable`() {
        val bad = uncoveredCodePoint()
        assertFalse(
            "U+%04X 本机没有字形，判据该说不可渲染".format(bad.codePointAt(0)),
            GlyphCoverage.canRender(bad),
        )
    }

    @Test
    fun `multi code point text always passes even when its parts have no glyph`() {
        val bad = uncoveredCodePoint()
        // 三类多码位序列各测一条 —— 它们正是 Android 注释里点名「问不得」的东西：
        // 变体选择符（键帽/emoji 变体）、ZWJ 串（家庭）、区域指示符对（旗帜）。
        // 判据若退化成「问整串」，这四条会全红。
        assertTrue("变体选择符序列不该被藏", GlyphCoverage.canRender(bad + "\uFE0F"))
        assertTrue("ZWJ 串不该被藏", GlyphCoverage.canRender(bad + "\u200D" + bad))
        assertTrue("旗帜（区域指示符对）不该被藏", GlyphCoverage.canRender("🇨🇳"))
        assertTrue("多字符 token（颜文字）不该被藏", GlyphCoverage.canRender("╮(╯▽╰)╭"))
    }

    @Test
    fun `single code point with a glyph is renderable`() {
        // ASCII 与汉字：这两档在本机与 CI 上都有字形（比「某个 emoji 在不在」稳得多）。
        assertTrue("ASCII", GlyphCoverage.canRender("a"))
        assertTrue("汉字", GlyphCoverage.canRender("中"))
    }

    @Test
    fun `empty text is not renderable`() {
        // 空串没有码位可问（`codePointAt(0)` 会抛 IndexOutOfBounds）；Android 那边
        // `hasGlyph("")` 同样是 false —— 判「不可渲染」而不是抛，两条都对齐。
        assertFalse(GlyphCoverage.canRender(""))
    }
}
