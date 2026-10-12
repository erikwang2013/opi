// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **离屏渲染探针**（2026-10-13）：把符号/表情页与搜索界面用 `ImageComposeScene` 真画成
//! PNG，再逐格数「有没有墨迹」—— 这一轮的两件事（字形过滤、搜索）都只能这样核：
//!
//! · 过滤：画不出的字形在网格里留下什么，是个**视觉事实**，只读代码证不了；这里前一张画
//!   原始表、后一张画过滤后的表，逐格数「有墨迹」的格子并逐像素比对。⚠️ 本机实测
//!   （2026-10-13，人眼看过 PNG）：**空白格，不是空心方框** —— 所以断言钉的是「过滤改变了
//!   像素 + 过滤后只剩画得出的」这类跟机器无关的不变量，不钉「方框变成了什么」。
//! · 搜索：Pending/没匹配/失败/命中四种态各画一张，四张图必须彼此不同。
//!
//! ⚠️ 它证的是「这些像素画得出来」，**不证**「窗开起来长这样」：窗口位置/焦点/真实键入
//! 都不在这里（那部分要上真桌面，见交付说明）。PNG 落在 `/tmp/opi-render-probe/`，
//! 人眼可直接看。

package xyz.erik.opi.candidate

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.unit.Density
import org.jetbrains.skia.EncodedImageFormat
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File
import javax.imageio.ImageIO
import kotlin.math.abs

class SymbolRenderProbeTest {

    private val outDir = File("/tmp/opi-render-probe")

    /** 离屏渲染成 PNG 落盘（`/tmp/opi-render-probe/<name>`）；内容底色 = 页面底色。 */
    private fun probePng(name: String, w: Int, h: Int, content: @Composable () -> Unit): File {
        val scene = ImageComposeScene(width = w, height = h, density = Density(1f)) {
            Box(Modifier.fillMaxSize().background(SymBg)) { content() }
        }
        val bytes = try {
            scene.render() // 首帧：布局 + 字体加载
            val img = scene.render() // 第二帧：确定态
            img.encodeToData(EncodedImageFormat.PNG)?.bytes ?: throw AssertionError("PNG 编码失败")
        } finally {
            scene.close()
        }
        outDir.mkdirs()
        return File(outDir, name).also { it.writeBytes(bytes) }
    }

    /**
     * 数「有墨迹」的格子数：格内任一像素与底色差 > 24 就算有墨迹（抗锯齿边缘、彩色 emoji
     * 都算）。格子按 `SymbolGridPage` 的排布算：12 列 × 6 行、外边距 (4, 2)。
     */
    private fun inkCells(png: File, cols: Int = SymbolCategories.COLUMNS, rows: Int = 6): Int {
        val img = ImageIO.read(png) ?: throw AssertionError("读不出 ${png.name}")
        val bg = SymBg.toArgb()
        val br = (bg shr 16) and 0xFF
        val bgg = (bg shr 8) and 0xFF
        val bb = bg and 0xFF
        val cw = (img.width - 8) / cols
        val ch = (img.height - 4) / rows
        var n = 0
        for (r in 0 until rows) {
            for (c in 0 until cols) {
                var ink = false
                row@ for (y in 2 + r * ch + 2 until 2 + (r + 1) * ch - 2) {
                    for (x in 4 + c * cw + 2 until 4 + (c + 1) * cw - 2) {
                        val p = img.getRGB(x, y)
                        val d = maxOf(
                            abs(((p shr 16) and 0xFF) - br),
                            abs(((p shr 8) and 0xFF) - bgg),
                            abs((p and 0xFF) - bb),
                        )
                        if (d > 24) {
                            ink = true
                            break@row
                        }
                    }
                }
                if (ink) n++
            }
        }
        return n
    }

    @Test
    fun `glyph filtering repacks the grid and drops what this machine cannot draw`() {
        val bad = discoveredUncoveredCodePoint()
        // 三条：本机画不出的那个 + 两个必然画得出的（ASCII/汉字，做对照）。
        val raw = listOf(bad, "a", "中")
        val shown = raw.filter(GlyphCoverage::canRender)
        assertEquals("判据该只摘掉那一条", listOf("a", "中"), shown)

        val before = probePng("grid-before.png", 820, 400) { SymbolGridPage(raw, 0) {} }
        val after = probePng("grid-after.png", 820, 400) { SymbolGridPage(shown, 0) {} }
        val nBefore = inkCells(before)
        val nAfter = inkCells(after)
        // ⚠️ 本机实测（离屏 Skia，2026-10-13）：缺字形的那个码位**一个像素都不画** ——
        // 第 0 格是**空白**，不是空心方框（人眼看过 grid-before.png）。真窗里是不是方框
        // 归真窗那一轮，别把这条读成「桌面上不会出现方框」。所以这里不钉「过滤前几格」，
        // 只钉两条跟机器无关的不变量：
        //  · 过滤后仍是那两条能画的（判据没多摘）；
        //  · 两张图**必须不同** —— 摘掉第 0 条会把后一条往左挪一格（真表那 25 条在这里
        //    的最小复现；过滤要是空转，两张图会逐字节相同）。
        assertEquals("过滤后该只剩 2 格（那一条整个不在表里了）", 2, nAfter)
        assertTrue("过滤不该多出墨迹（摘掉的是画不出的）", nAfter <= nBefore)
        assertTrue("两张图必须不同（不然过滤没落到像素上）", !before.readBytes().contentEquals(after.readBytes()))
    }

    @Test
    fun `the real emoji page drops exactly the entries this machine cannot draw`() {
        val raw = loadEmojiData()
        assertTrue("真表没读到", raw.entries.isNotEmpty())
        val filtered = EmojiData.filter(raw, GlyphCoverage::canRender)
        val dropped = raw.entries.size - filtered.entries.size
        println("真表：${raw.entries.size} 条 → 过滤后 ${filtered.entries.size} 条（摘掉 $dropped 条）")
        val a = probePng("emoji-page-raw.png", 820, 600) { SymbolGridPage(raw.entries, 0, emoji = true) {} }
        val b = probePng("emoji-page-filtered.png", 820, 600) {
            SymbolGridPage(filtered.entries, 0, emoji = true) {}
        }
        // 「真表里有没有画不出的条目」是字体决定的事实（换机器/换 Noto 版本就变）⇒ 不断言
        // 具体条数，只断言不变量：过滤后的每条都画得出、且是原表的子序列（保序）。
        assertTrue("过滤后还有画不出的（判据没生效）", filtered.entries.all(GlyphCoverage::canRender))
        assertEquals(raw.entries.filter(GlyphCoverage::canRender), filtered.entries)
        assertTrue("第一页得有东西（否则探针在空跑）", filtered.entries.size > SymbolCategories.PAGE_SIZE)
        // 第一页画的是「前 PAGE_SIZE 条」：只有这个窗口被摘/挪动了，两张图才会不同
        // （摘掉的几条若全在第二页，第一页逐字不变 ⇒ 两页该逐字节相同）。钉这个**等价
        // 关系**而不是「摘掉过就必然不同」——后者是机器相关的（本机 25 条里恰有 2 条落在
        // 第一页；换台字体覆盖不同的机器就不一定）。
        val same = a.readBytes().contentEquals(b.readBytes())
        val page0Same = raw.entries.take(SymbolCategories.PAGE_SIZE) ==
            filtered.entries.take(SymbolCategories.PAGE_SIZE)
        if (page0Same) {
            assertTrue("第一页窗口没变 ⇒ 两页该一模一样（过滤没画到它）", same)
        } else {
            assertTrue("第一页窗口变了 ⇒ 两页必须不同（过滤没落到像素上）", !same)
        }
        val nR = inkCells(a)
        val nF = inkCells(b)
        println("第一页墨迹格数：raw $nR → filtered $nF（raw 的空格 = 画不出的条目占着的格位）")
        assertTrue("过滤只该把条目往左挪/摘掉，不该凭空多出空白格", nF >= nR)
    }

    @Test
    fun `search pane draws each of its four states`() {
        val size = Modifier.fillMaxSize()
        val files = listOf(
            "search-pending.png" to probePng("search-pending.png", 724, 300) {
                SearchResultPane(SearchView.Pending, false, "没有匹配的符号", size) {}
            },
            "search-none.png" to probePng("search-none.png", 724, 300) {
                SearchResultPane(SearchView.None, false, "没有匹配的符号", size) {}
            },
            "search-failed.png" to probePng("search-failed.png", 724, 300) {
                SearchResultPane(
                    SearchView.Failed("「opi-ctl search」退出码 3：通道不通（fcitx5 没在跑？）"),
                    false,
                    "没有匹配的符号",
                    size,
                ) {}
            },
            "search-hits.png" to probePng("search-hits.png", 724, 300) {
                SearchResultPane(SearchView.Hits(listOf("♥", "、", "😀")), false, "没有匹配的符号", size) {}
            },
        )
        // 四种态必须画成四张**不同**的图：同一张重复落盘就是「状态没画出来」（最典型的
        // 是把 Pending 和「没匹配」都画成空白 —— 用户看不出在搜还是搜完了没结果）。
        val bytes = files.map { it.second.readBytes() }
        for (i in bytes.indices) for (j in i + 1 until bytes.size) {
            assertTrue(
                "${files[i].first} 与 ${files[j].first} 画出来一模一样（状态没区分开）",
                !bytes[i].contentEquals(bytes[j]),
            )
        }
        // 命中那张得真有网格：3 条命中 ⇒ 3 个格子有墨迹（分页器渲染的是第 0 页）。
        assertEquals("命中那张该有 3 格墨迹", 3, inkCells(files[3].second))
    }

    @Test
    fun `search field draws the query`() {
        val empty = probePng("search-field-empty.png", 220, 24) {
            SymbolSearchField("", { }, { }, Modifier.fillMaxSize())
        }
        val typed = probePng("search-field-typed.png", 220, 24) {
            SymbolSearchField("heart", { }, { }, Modifier.fillMaxSize())
        }
        assertTrue("占位提示该有墨迹", inkCells(empty, cols = 1, rows = 1) == 1)
        assertTrue("输入后该有墨迹", inkCells(typed, cols = 1, rows = 1) == 1)
        assertTrue("空/有内容必须画得不一样", !empty.readBytes().contentEquals(typed.readBytes()))
    }
}
