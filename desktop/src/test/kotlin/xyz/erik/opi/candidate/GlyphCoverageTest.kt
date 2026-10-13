// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 字形覆盖判据的门禁（2026-10-13）：**规则本身** —— 多码位一律放行、只有单码位问字体、
//! 空串不可渲染。规则逐条对齐 Android `SymbolCatalog.canRenderGlyph`（那边同样是
//! `codePointCount > 1 || hasGlyph`）：两端漂了就是「同一份数据在两端过滤得不一样」。
//!
//! ⚠️ 这一份测的是**规则**，不是「本机字体长什么样」：单码位有没有字形是字体决定的事实，
//! 换台机器/换个 Noto 版本答案就变。所以**正反两侧夹具都是发现式的**（[coveredCandidates]
//! 与 [uncoveredCandidates]）；探不到就红（不是跳过）：那说明前提没了，得换候选，
//! 而不是让一条测不出东西的用例静静变绿。
//!
//! ⚠️ 2026-10-13 的教训（CI 红）：正控原先把「中」写死 —— ubuntu runner 上没有覆盖 CJK
//! 的字体，`canRender("中")` 是 false，这条当场红。写死具体字符就是把「这台机器装没装
//! 中文字体」当成规则断言。夹具改成发现式之后，同一条规则在任何字体环境的机器上都成立。

package xyz.erik.opi.candidate

import org.jetbrains.skia.FontMgr
import org.jetbrains.skia.FontStyle
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 本机**可能缺字形**的候选码位：先真 emoji 码位（Unicode 17.0 新增，正是这次报障的来源），
 * 再私用区/未分配码位 —— 私用区按定义没有正经字体去覆盖，是「几乎必然缺」的兜底。
 * 顺序有意义：优先用真数据里出现的那种码位当夹具。
 *
 * 文件级（不是类成员）：字形判据门禁与离屏渲染探针共用同一个夹具取法。
 */
private val uncoveredCandidates = listOf(0x1FAEB, 0x1FAEC, 0x1FAEF, 0x10FFFD, 0xF0000, 0xE0001)

/**
 * 本机**必然有字形**的候选码位（顺序有意）：CJK → 日文假名 → 希腊文 → ASCII 字母/数字。
 * 中文输入法的主场机器几乎必然命中前者；只有西文的最小环境落到 `a` —— ASCII 是
 * 「装了字体的机器都盖得住」的兜底（CI 的 ubuntu runner 实测就落在这一档）。
 */
private val coveredCandidates = listOf(0x4E2D, 0x3042, 0x03A9, 0x61, 0x31)

/**
 * 字形覆盖的**独立取证**：与 `GlyphCoverage` 同一层的 Skia 原语，直接问。
 *
 * ⚠️ 发现夹具**不许走 `GlyphCoverage.canRender`**（那就成了自证：判据若恒定返回 true，
 * 会发现「候选全是可渲染的」并让断言放行）。这个原语是**证据**，canRender 是被测对象。
 */
private fun coverageOracle(codePoint: Int): Boolean =
    FontMgr.default.matchFamilyStyleCharacter(null, FontStyle.NORMAL, null, codePoint) != null

/** 探一个本机画不出的单码位；一个都探不到 ⇒ 夹具前提没了，红（不是跳过）。 */
internal fun discoveredUncoveredCodePoint(): String {
    for (cp in uncoveredCandidates) {
        if (!coverageOracle(cp)) return String(Character.toChars(cp))
    }
    throw AssertionError(
        "本机字体把候选码位（${uncoveredCandidates.joinToString { "U+%04X".format(it) }}）全盖住了：" +
            "这份测试的夹具前提没了 —— 换更生僻的候选或换测法，别让用例空转变绿",
    )
}

/** 探 [n] 个本机画得出、且互不相同的单码位；凑不够 ⇒ 夹具前提没了，红（不是跳过）。 */
internal fun discoveredCoveredCodePoints(n: Int): List<String> {
    val out = coveredCandidates.filter(::coverageOracle).map { String(Character.toChars(it)) }
    if (out.size < n) {
        throw AssertionError(
            "本机字体只盖住 ${out.size} 个候选码位（需要 $n 个）：这台机器连基本西文字形都缺，" +
                "夹具前提没了 —— 红，而不是让用例空转变绿",
        )
    }
    return out.take(n)
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
        // 夹具是**发现**的（本机有字形的某个码位），不是写死的「中」—— 写死就是拿
        // 「这台机器装没装中文字体」当规则断言（2026-10-13 CI 的 ubuntu runner 实测没有）。
        val good = discoveredCoveredCodePoints(1).single()
        assertTrue(
            "U+%04X 本机有字形，判据该说可渲染".format(good.codePointAt(0)),
            GlyphCoverage.canRender(good),
        )
    }

    @Test
    fun `the probe cache saves time without changing answers`() {
        // 2026-10-13 加缓存时补：缓存只许省时间、不许改答案。两道判据 —— 与**独立原语**
        // 对齐（coverageOracle 不经过 canRender），且**第二遍**（这一遍必走缓存）答案不变。
        // 单测共用一个进程，头一遍未必是冷探（别的用例可能先问过）—— 故判据不看「第几遍」，
        // 只看两遍都对：缓存写反/键写错（如按串长做键）都会在这一对正反夹具上露馅。
        val good = discoveredCoveredCodePoints(1).single()
        val bad = uncoveredCodePoint()
        for ((text, expected) in listOf(good to true, bad to false)) {
            assertEquals(
                "第一遍该是 $expected",
                expected,
                GlyphCoverage.canRender(text),
            )
            assertEquals(
                "第二遍（走缓存）与独立原语（coverageOracle）不一致 —— 缓存改变了答案",
                coverageOracle(text.codePointAt(0)),
                GlyphCoverage.canRender(text),
            )
        }
    }

    @Test
    fun `empty text is not renderable`() {
        // 空串没有码位可问（`codePointAt(0)` 会抛 IndexOutOfBounds）；Android 那边
        // `hasGlyph("")` 同样是 false —— 判「不可渲染」而不是抛，两条都对齐。
        assertFalse(GlyphCoverage.canRender(""))
    }
}
