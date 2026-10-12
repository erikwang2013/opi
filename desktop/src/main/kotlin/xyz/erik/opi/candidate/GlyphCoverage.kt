// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「本机字体画不画得出这个字形」—— 符号页/表情页**渲染前**的过滤判据（2026-10-13）。
//!
//! 与 Android `SymbolCatalog.canRenderGlyph` **逐条同构**（那边 `Paint.hasGlyph`，
//! 这边 Skia）：多码位序列一律放行、只有单码位才问字体、空串判不可渲染。
//!
//! 为什么必须问**渲染用的那条字体链**：候选窗是 Compose Desktop（Skia/skiko）画的，
//! 字形取自 `FontMgr.default` 的系统回退链；`java.awt.Font`（AWT）走的是另一套
//! fontconfig 映射 —— 拿 AWT 问，答的是「AWT 画不画得出」，不是「这个窗画不画得出」。
//! 两个页面都没有自定义 FontFamily（无 `FontFamily(...)` 覆写），所以渲染与这里的探针同源。
//!
//! 来由（与 Android 同一次报障）：数据里有 U+1FAEB「cracking face」这类新码位，
//! 本机字体没覆盖 ⇒ 网格里画出一个空心矩形。数据本身没错（新字体会显示），
//! 错的只是把它硬画出来。

package xyz.erik.opi.candidate

import org.jetbrains.skia.FontMgr
import org.jetbrains.skia.FontStyle

/** 字形覆盖判定。无状态、幂等（同一条文本问几次答案一样）⇒ 调用方可放心缓存结果。 */
internal object GlyphCoverage {
    /** @return true = 可以画（本机有字形，或多码位序列交给连字/回退处理）。 */
    fun canRender(text: String): Boolean {
        // 与 Android 的 `text.codePointCount(0, text.length) > 1 || paint.hasGlyph(text)` 同构：
        // **只有恰好一个码位**才去问字体。多码位问不出可信答案 —— 旗帜（🇨🇳 = 两个区域
        // 指示符）、家庭（ZWJ 串）、键帽这类整串连字在字体里的存在性与单字符无关，
        // 问出来的「没有」会把本来能显示的整类藏掉，比留一个空心框更坏。
        if (text.codePointCount(0, text.length) != 1) return text.isNotEmpty()
        return probe(text.codePointAt(0))
    }

    /**
     * 单个码位在本机字体链里有没有字形。
     * `family = null` = 不问特定家族，用系统默认回退链（**就是渲染时那条**）；
     * `bcp47 = null` = 不给语言提示 —— 给了会挑到「按语言优选」的字体，
     * 与「这台机器默认画不画得出」不是同一个问题。
     */
    private fun probe(codePoint: Int): Boolean =
        FontMgr.default.matchFamilyStyleCharacter(null, FontStyle.NORMAL, null, codePoint) != null
}
