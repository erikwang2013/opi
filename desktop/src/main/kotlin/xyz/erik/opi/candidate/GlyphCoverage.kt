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

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.jetbrains.skia.FontMgr
import org.jetbrains.skia.FontStyle
import java.util.concurrent.ConcurrentHashMap

/**
 * 把「按 [key] 算一次」的重活挪到**后台线程**：先给 [empty]、算完再换成结果。
 *
 * 为什么必须有它（2026-10-13 实测）：把过滤写在 `Window(` 前、或写在窗内容里，**一样慢** ——
 * 窗要等内容那一趟组合跑完才显示。于是用户点 Ω 后 15-25s 眼前什么都没有（真机实测 24.6s）。
 * 挪到协程里之后：窗口立刻出来，网格等过滤落地（页面本来就配了空态文案）。
 * 过滤本身还是那么贵 —— 但**不再挡在「窗存在」这条路上**。
 */
@Composable
internal fun <T> deferredFilter(empty: T, key: Any?, compute: () -> T): T {
    var value by remember(key) { mutableStateOf(empty) }
    LaunchedEffect(key) { value = withContext(Dispatchers.Default) { compute() } }
    return value
}

/**
 * 字形覆盖判定。幂等（同一条文本问几次答案一样）⇒ 调用方可放心缓存结果。
 *
 * ⚠️ 这个问题**很贵**：本机 fontconfig 单码位实测 ~2.5ms（2501 次裸调 6.1s），而符号页
 * 全表 5742 条 ⇒ 一遍过滤 ~7s（冷启动更久）。所以：
 *  · 结果按码位缓存（[probe]）——同一进程里第二次开页几乎免费；
 *  · 过滤必须走 [deferredFilter]（后台协程）。留在组合里 —— 无论在 `Window(` 之前还是
 *    挪进窗内容 —— 都会把「窗显示出来」推到过滤跑完之后（真机点 Ω 实测 24.6s 才见窗，
 *    挪进内容只降到 15.6s）；`SymbolWindowCallPathTest` 钉着「窗前不许有过滤调用」。
 */
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
    /** 码位 → 答案。字体在一次进程里不会中途换 ⇒ 缓存永不失效；重复探测只是重算一遍（幂等）。 */
    private val cache = ConcurrentHashMap<Int, Boolean>()

    private fun probe(codePoint: Int): Boolean = cache.getOrPut(codePoint) {
        FontMgr.default.matchFamilyStyleCharacter(null, FontStyle.NORMAL, null, codePoint) != null
    }
}
