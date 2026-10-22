// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 候选窗「字号下限 + 按窗口放大 + 不溢出」门禁（2026-10-14）。用户原话「候选词有点小了
//! 至少12px」；经 lead 转述确认**本窗也在内**（「浮在状态栏上方那个」）⇒ 本轮抬档 + 门禁。
//!
//! 四组判据，缺一组就不成门禁：
//!
//! 1. **下限与覆盖**：反射枚举 `CandidateFont` 暴露的**每个** getter，逐项断言 ≥ MIN，且与
//!    门禁名单**集合相等**（新增字号漏进名单会红、名单里有不存在的名字也红）；再把本轮放大
//!    的落点（Candidate ≥20、Buffer ≥18）单独钉住 —— 谁想降回 17 要先过这条。判据本体有
//!    正控（断言它对 8.sp 判假），免得恒真放行。
//! 2. **按窗口取较大值**：`fontSizeFor` 是纯函数 —— 设计尺寸 = 基准；只加宽或只加高**都不放大**
//!    （另一边没余量）；两边等比放大则同比放大且封顶 [MAX_FONT_SCALE]；窗口变小也不低于基准。
//! 3. **接线**：面板真画出来的候选墨迹高度 = 按 `fontSizeFor(Candidate, 该窗)` 单画的墨迹高度
//!    （同布局换字前后逐像素差）。**设计尺寸与 2× 窗两档都验** —— 只读常量表的门禁会绕过被测
//!    代码（面板里写死字面量它照样绿），只验设计尺寸则「按窗口放大」那段没被接上也不知道。
//! 4. **不溢出**：真窗尺寸下，现实页形（8 条 × 1/2/4/6 字 + 最长输入串的读数行）≤ 窗高。
//!    真窗尺寸**现读 Main.kt**（候选窗那处 `val windowState = rememberWindowState(...)` ——
//!    不是「第一个 `rememberWindowState`」，那是设置窗），不手抄第二份；Main.kt 改了尺寸，
//!    「design window」那条会红、夹具跟着走。⚠️ 量高度用的画布**比窗高**：拿真窗高当画布时
//!    溢出会被裁在窗底、底色恰好铺满 ⇒ 恒「不溢出」（第一版就是这洞，见判据 4 注释）。
//!    更宽的页形（8 条 × 8/10 字）只**印**不断言 —— 引擎到不了那么宽，真要支持得先加大窗高。
//!
//! 5. **放大档也不溢出**：2× 窗（字号吃到 1.5× 上限、候选 30.sp）下同一批页形 ≤ 336px ——
//!    封顶值与「封顶后仍放得下」都由实测背书（不是只写一个常数）。
//!
//! ⚠️ ubuntu CI 没有 CJK 字体（2026-10-13 教训：硬写「中」会假红）：夹具一律走
//! `discoveredCoveredCodePoints` 发现式取字，缺字机器退到希腊/ASCII（夹具字会印出来）。
//! 判据 1、2 与字体无关；判据 3 两侧同字同号。
//! 判据 4 **不是**「CI 上只弱不假红」：缺 CJK 字体时回退字体的**行盒更高**（DejaVu 级
//! ~1.4× vs CJK ~1.25×），同一页在 CI 上反而更高 —— 20.sp 初版就是这样在 CI 边沿
//! （本机 165 / CI 字体 175 > 当时的窗高 168）。**2026-10-22 起候选窗是固定两行
//! 横条**（拼音一行、候选一行，高 76dp、宽按两行内容实测），判据 4 相应改成「现实
//! 页形高度恒等于条高」并用横向溢出的页形钉住翻页控件不被挤出窗；判据 6 钉「拼音
//! 墨迹在候选墨迹上方」。本机逐字复现这个环境：
//!   `printf '<fontconfig><dir>/usr/share/fonts/truetype/dejavu</dir></fontconfig>' > /tmp/fc-nocjk/fonts.conf`（再加 <cachedir>）
//!   前提自证：`FONTCONFIG_FILE=… fc-list ':charset=4e2d' | wc -l` 应为 0
//!   跑：`FONTCONFIG_FILE=… ./gradlew test --tests "…CandidateFontGateTest"`
//! 两个环境都绿才算这道门禁过（字号那一侧的高度余量就是按这个口径留的，见 CandidateWindow.kt
//! 的「留白换字号」）。
//!
//! 量的是**离屏栅格**（ImageComposeScene → PNG + 逐像素），证明「这么画会得到这些像素」，
//! **不证明**真窗（TSF named pipe 驱动）里长这样 —— 那要 Windows 机。

package xyz.erik.opi.candidate

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.jetbrains.skia.EncodedImageFormat
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.awt.image.BufferedImage
import java.io.File
import java.nio.file.Files
import java.nio.file.Paths
import javax.imageio.ImageIO
import kotlin.math.abs

/**
 * 门禁名单：成员名 ↔ 值。成员被**删** ⇒ 这里编译不过；成员被**加** ⇒ 下面的反射那步抓红。
 * 名单是「人写的第二份」，所以必须靠反射把「漏写」这条堵上（不然它会静默少测）。
 */
private val FONT_TABLE = listOf(
    "MIN" to CandidateFont.MIN,
    "Buffer" to CandidateFont.Buffer,
    "Mode" to CandidateFont.Mode,
    "Empty" to CandidateFont.Empty,
    "Candidate" to CandidateFont.Candidate,
    "Index" to CandidateFont.Index,
    "Page" to CandidateFont.Page,
    "Arrow" to CandidateFont.Arrow,
)

/**
 * 本轮「桌面候选窗也放大」的落点（lead 2026-10-14 转述的用户确认）。
 * 与 [CandidateFont.MIN]（用户线 12px）**分开**：MIN 管「不许比用户线小」，这里管「不许悄悄
 * 降回放大前」。想降回去，得先删这条并说明为什么。
 */
private val RAISED_FLOOR = listOf("Candidate" to 20.sp, "Buffer" to 18.sp)

/** 判据本体（单独成函数是为了对它下正控）：字号是否 ≥ 用户线。 */
private fun atFloor(v: TextUnit): Boolean = v >= CandidateFont.MIN

class CandidateFontGateTest {

    private val outDir = File("/tmp/opi-render-probe")

    /** 哨兵底色：面板自己的底色（#2D2D2D）绝不等于它，「没画到的地方」才认得出。 */
    private val sentinel = Color(0xFFFF00FF)

    /**
     * 真窗尺寸 —— **现读 Main.kt**（不手抄第二份；改了 Main.kt 这里跟着变）。
     *
     * 锚点是 `val windowState = rememberWindowState(...)`（候选窗那处**赋值**）：Main.kt 里另有一个
     * 800×560（第 120 行 `if` 分支里的**设置**窗，它 inline 传给 `state =`）。别改成「找第一个
     * `rememberWindowState`」—— 那正好命中设置窗（本门禁第一版就这么翻过车），字号会按 800×560
     * 基准算，门禁自己先假红。
     */
    /**
     * 夹具窗 = 横条设计尺寸（720×48）。**不是**运行期真实宽度：真实宽度按每页内容动态
     * 计算（`rememberCandidateBarSize`），没有固定值；这里固定尺寸只为量字号/行数。
     * 设计尺寸与 Main.kt 的接线由下面的 `main drives window and panel from one size` 钉住。
     */
    private val realWindow: DpSize get() = PANEL_DESIGN_SIZE

    /** 仓库根下的源文件（相对 desktop/ 模块目录，与 SettingsLockstepTest 同一取法）。 */
    private fun repoFile(rel: String): String {
        val p = Paths.get("../$rel")
        assertTrue("源文件不在了（搬家了？）：$rel ⇒ ${p.toAbsolutePath()}", Files.exists(p))
        return Files.readString(p)
    }

    // ---------- 判据 1：下限与覆盖 ----------

    @Test
    fun `every candidate font size is at or above the floor, and covered by the gate list`() {
        assertTrue("门禁名单空了 —— 下面全是空转", FONT_TABLE.size >= 8)
        // 直接问目标对象暴露了哪些 getter（值类属性可能带 `-hash` 后缀，切掉；`$` 合成名滤掉）。
        val declared = CandidateFont::class.java.declaredMethods
            .filter { it.name.startsWith("get") }
            .map { it.name.removePrefix("get").substringBefore('-') }
            .filter { it.matches(Regex("[A-Za-z][A-Za-z0-9]*")) }
            .toSet()
        val listed = FONT_TABLE.map { it.first }.toSet()
        println("CandidateFont 反射到：${declared.sorted()}；门禁名单：${listed.sorted()}")
        assertTrue("一个成员都没反射到（Kotlin 代码生成变了？）—— 这条会空转变绿", declared.isNotEmpty())
        // 两边**集合相等**（不是「declared ⊆ listed」）：少了成员 = 将来加个 8.sp 字号静默漏检，
        // 多了成员 = 名单在测一个不存在的东西（滤名写错），两种都得红。
        assertEquals(
            "门禁名单与 CandidateFont 的成员对不上 —— 对象有而名单没覆盖：${declared - listed}" +
                "（新增字号请同步加进 FONT_TABLE）；名单有而对象没有：${listed - declared}（反射滤名写错了？）",
            listed.sorted(),
            declared.sorted(),
        )
        for ((name, v) in FONT_TABLE) {
            assertTrue("$name 不是 sp 量纲（$v）", v.isSp)
            assertTrue("$name = $v 低于下限 ${CandidateFont.MIN}（用户线：至少 12px）", atFloor(v))
        }
        val byName = FONT_TABLE.toMap()
        for ((name, floor) in RAISED_FLOOR) {
            val v = byName.getValue(name)
            assertTrue(
                "$name = $v 低于本轮放大落点 $floor（2026-10-14：桌面候选窗要放大）——要降回去请先说明",
                v >= floor,
            )
        }
    }

    /** 正控：判据对 8.sp 必须判假 —— 否则「全绿」只说明它恒定放行。 */
    @Test
    fun `the floor predicate can fail`() {
        assertFalse("判据对 8.sp 也放行 ⇒ 它恒真，上面那条测不出任何东西", atFloor(8.sp))
        assertTrue(atFloor(CandidateFont.MIN))
    }

    /**
     * 尺寸**只有一个来源**：Main.kt 必须用 `rememberCandidateBarSize(model)` 的返回值同时
     * 驱动窗口（rememberWindowState）与面板（`windowSize = barSize`）。此前窗口写死一份、
     * 面板另算一份，必然漂移（门禁上一次就是这么抓到 320 vs 真窗的）。
     */
    @Test
    fun `main drives window and panel from one measured size`() {
        val src = repoFile("desktop/src/main/kotlin/xyz/erik/opi/candidate/Main.kt")
        assertTrue(
            "Main.kt 没把 rememberCandidateBarSize 的结果传给面板（windowSize = barSize）",
            src.contains("rememberCandidateBarSize(model, fontScale)") &&
                src.contains("windowSize = barSize"),
        )
        // 同一份尺寸还必须真的驱动**窗口**：只留 `windowSize = barSize` 的话，把
        // rememberWindowState 改回写死尺寸/删掉 LaunchedEffect(barSize)，窗口与面板
        // 仍会漂移而测试照绿（review 2026-10-22）。
        assertTrue(
            "Main.kt 的窗口尺寸没接 rememberCandidateBarSize（width/height = barSize）",
            src.contains("width = barSize.width") && src.contains("height = barSize.height"),
        )
        assertTrue(
            "Main.kt 没在横条尺寸变化后同步窗口（LaunchedEffect(barSize)）",
            src.contains("LaunchedEffect(barSize) { windowState.size = barSize }"),
        )
        // 固定两行：设计高度就是运行期高度（宽度才按内容变）。
        assertEquals("横条设计高度固定 76dp（拼音行 + 候选行）", 76, PANEL_DESIGN_SIZE.height.value.toInt())
        assertEquals("横条设计宽度基准 720dp", 720, PANEL_DESIGN_SIZE.width.value.toInt())
    }

    // ---------- 判据 2：按窗口取较大值 ----------

    @Test
    fun `font size scales with the window and never shrinks`() {
        val base = CandidateFont.Candidate
        val design = PANEL_DESIGN_SIZE
        fun at(size: DpSize) = fontSizeFor(base, size).value
        assertEquals("设计尺寸（真窗）= 基准，不该有缩放", base.value, at(design), 0.01f)
        assertEquals(
            "只加宽：字不能变大（行高没余量，字大了反而溢出）",
            base.value, at(DpSize(design.width * 2, design.height)), 0.01f,
        )
        assertEquals(
            "只加高：同理（行宽没余量）",
            base.value, at(DpSize(design.width, design.height * 2)), 0.01f,
        )
        assertEquals(
            "两边等比 2× ⇒ 同比放大（封顶 $MAX_FONT_SCALE）",
            base.value * MAX_FONT_SCALE, at(DpSize(design.width * 2, design.height * 2)), 0.01f,
        )
        assertEquals(
            "再大也封顶（4× 仍 $MAX_FONT_SCALE）",
            base.value * MAX_FONT_SCALE, at(DpSize(design.width * 4, design.height * 4)), 0.01f,
        )
        assertEquals(
            "窗比设计尺寸小 ⇒ 不缩（字号只升不降，下限就是表里的基准）",
            base.value, at(DpSize(design.width / 2, design.height / 2)), 0.01f,
        )
    }

    // ---------- 判据 3：接线（面板真的在用那张表，并且真的随窗放大）----------

    @Test
    fun `the panel draws candidates at the table size, and follows a grown window`() {
        assertWired(realWindow)
        assertWired(DpSize(PANEL_DESIGN_SIZE.width * 2, PANEL_DESIGN_SIZE.height * 2))
    }

    private fun assertWired(window: DpSize) {
        val tag = "${window.width.value.toInt()}x${window.height.value.toInt()}"
        val ch = discoveredCoveredCodePoints(1).first()
        // 同一模型只把候选文字从「空格」换成那个字：两图的差异行 = 那个字的墨迹。
        val panelInk = diffRows(
            panel("w$tag-glyph.png", model(listOf(ch)), window),
            panel("w$tag-space.png", model(listOf(" ")), window),
        )
        // 参照：同字号单画同一个字，量法完全相同（颜色一致，只量「哪几行像素变了」）。
        val size = fontSizeFor(CandidateFont.Candidate, window)
        val refInk = diffRows(
            bare("w$tag-ref-glyph.png", ch, size),
            bare("w$tag-ref-space.png", " ", size),
        )
        assertTrue("[$tag] 候选字在面板里一个像素都没画出来（面板没渲染？）", panelInk > 0)
        assertTrue("[$tag] 参照字一个像素都没画出来（本机连发现式兜底字都缺？）", refInk > 0)
        assertTrue(
            "[$tag] 面板里候选墨迹 ${panelInk}px ≠ 按 ${size} 单画的 ${refInk}px" +
                " —— 面板没在用那张字号表，或没按窗口尺寸放大",
            abs(panelInk - refInk) <= 1,
        )
    }

    // ---------- 判据 4：不溢出（横条 = 固定两行，高度恒定） ----------

    @Test
    fun `real-world pages keep the fixed two-row height`() {
        val ch = discoveredCoveredCodePoints(1).first()
        // 印出本机**实际用到的**夹具字：CI 上（无 CJK 字体）这行会与本地不同，红的时候
        // 一眼看出是不是字体环境变了 —— 也是「发现式取字真的生效了」的正控。
        println("夹具字 = 「$ch」U+%04X（本机字体覆盖决定；CI 上非 CJK）".format(ch.first().code))
        val h = realWindow.height.value.toInt()
        // 画布**故意比条高**（240）：面板根节点按 windowSize.height 定高，横条若偷偷折成
        // 多行，requiredHeightPx 会大于条高，当场红。宽度用真窗宽（候选溢出由横向滚动接住，
        // 不参与高度）。
        val tall = DpSize(realWindow.width, 240.dp)
        for (n in listOf(1, 2, 4, 6)) {
            val need = requiredHeightPx(panel("c-8x$n.png", model(List(8) { ch.repeat(n) }), realWindow, tall))
            println("8 条 × $n 字 → 需要 ${need}px（条高 $h）")
            assertEquals("8 条 × $n 字把横条撑高了（需要 ${need}px）—— 高度必须恒定", h, need)
        }
        // 读数行也拉满一次：24 字母缓冲（2026-10-14 实测里最长的输入串是无空格长拼音）。
        val need = requiredHeightPx(
            panel("c-longbuf.png", model(List(8) { ch.repeat(4) }).apply {
                buffer = "guojiazhongdianyanfajihua"
            }, realWindow, tall),
        )
        println("24 字母缓冲 + 8 条 × 4 字 → 需要 ${need}px（条高 $h）")
        assertEquals("长缓冲把横条撑高了 —— 拼音行也固定一行", h, need)
    }

    /**
     * 横向溢出时**翻页控件必须还在右端**：候选区是 weight(1f) + horizontalScroll，长词挤
     * 不掉它。这条是横条版的正控 —— 换回不带 weight 的 Row 时，8 条 × 60 字会把翻页区推到
     * 窗外（右端 90px 一点墨迹都没有），当场红。
     */
    @Test
    fun `the page control stays visible when candidates overflow`() {
        val ch = discoveredCoveredCodePoints(1).first()
        val img = panel("bar-overflow.png", model(List(8) { ch.repeat(60) }), realWindow)
        assertTrue(
            "候选溢出时右侧翻页控件被挤出窗（应固定在右端）",
            hasInkInColumns(img, img.width - 90, img.width),
        )
    }

    /**
     * 放大档的高度预算：封顶值 = [MAX_FONT_SCALE]（1.5×）；2× 窗（1440×96）下字号吃到上限、
     * 候选 30.sp，横条仍是固定两行高度。判据与 `real-world pages` 相同，阈值换成放大后的条高。
     * 横向溢出/翻页控件不被挤走由 `the page control stays visible` 钉住。
     */
    @Test
    fun `the cap keeps a grown window at its fixed height`() {
        val ch = discoveredCoveredCodePoints(1).first()
        val grown = DpSize(PANEL_DESIGN_SIZE.width * 2, PANEL_DESIGN_SIZE.height * 2)
        // 前提：2× 窗确实吃到上限 —— 否则下面量的是别档，数字失去意义（改 MAX_FONT_SCALE 时先看这里）。
        assertEquals(
            "2× 窗没吃到上限 ⇒ 下面对不上号（MAX_FONT_SCALE 改了？）",
            MAX_FONT_SCALE,
            fontSizeFor(CandidateFont.Candidate, grown).value / CandidateFont.Candidate.value,
            0.01f,
        )
        val h = grown.height.value.toInt()
        val tall = DpSize(grown.width, 320.dp)
        for (n in listOf(1, 2, 4, 6)) {
            val need = requiredHeightPx(panel("big-8x$n.png", model(List(8) { ch.repeat(n) }), grown, tall))
            println("放大档（1.5×，候选 ${fontSizeFor(CandidateFont.Candidate, grown)}）8 条 × $n 字 → 需要 ${need}px（2× 条高 $h）")
            assertEquals("放大档 8 条 × $n 字把横条撑高了", h, need)
        }
        val need = requiredHeightPx(
            panel("big-longbuf.png", model(List(8) { ch.repeat(4) }).apply {
                buffer = "guojiazhongdianyanfajihua"
            }, grown, tall),
        )
        println("放大档 24 字母缓冲 + 8 条 × 4 字 → 需要 ${need}px（2× 条高 $h）")
        assertEquals("放大档长缓冲把横条撑高了", h, need)
    }

    /** 纯函数：横条宽度必须给翻页控件留位，并按上下限收口（超长候选由候选区滚动接住）。 */
    @Test
    fun `bar width accounts for the page control and clamps to the bounds`() {
        fun w(items: List<Float>, empty: Float = 300f, page: Float = 100f): Float =
            candidateBarWidthDp(40f, 24f, items, empty, page, BAR_MIN_WIDTH, BAR_MAX_WIDTH)
        assertEquals(
            "页控件宽度必须整段计入（不能从候选区挤）",
            100f,
            w(emptyList(), page = 200f) - w(emptyList(), page = 100f),
            0.01f,
        )
        assertEquals("空态内容与「一条同宽候选」等价", w(emptyList(), empty = 300f), w(listOf(300f)), 0.01f)
        assertEquals("极长页：窗宽封顶（超出由候选区横向滚动）", BAR_MAX_WIDTH, w(List(8) { 500f }), 0.01f)
        assertEquals("极短页：不小于下限（保持长条形态）", BAR_MIN_WIDTH, w(emptyList(), empty = 0f, page = 0f), 0.01f)
    }

    // ---------- 判据 6：拼音在候选上方（2026-10-22 用户裁决） ----------

    /**
     * 「候选词中的拼音要在候选词上方」——字面量：拼音墨迹的每一行都严格在候选墨迹
     * 之上。用两张只换文字的面板图做差分行定位（与字号接线同一套量法），不依赖任何
     * 坐标常量：布局改回单行时两组墨迹会落在同一行，当场红。
     */
    @Test
    fun `pinyin ink is drawn above the candidate ink`() {
        val bufferRows = diffRowIndices(
            panel("rows-buffer-a.png", model(listOf("A")).apply { buffer = "mmm" }, realWindow),
            panel("rows-buffer-b.png", model(listOf("A")).apply { buffer = "nnn" }, realWindow),
        )
        val candidateRows = diffRowIndices(
            panel("rows-cand-a.png", model(listOf("WWW")), realWindow),
            panel("rows-cand-b.png", model(listOf("MMM")), realWindow),
        )
        assertTrue("拼音行一个像素都没画出来", bufferRows.isNotEmpty())
        assertTrue("候选行一个像素都没画出来", candidateRows.isNotEmpty())
        println("拼音墨迹行 = $bufferRows；候选墨迹行 = $candidateRows")
        assertTrue(
            "拼音墨迹（$bufferRows）不在候选墨迹（$candidateRows）上方 —— 用户裁决「拼音要在候选词上方」",
            bufferRows.max() < candidateRows.min(),
        )
    }

    /** 空候选（无匹配/分页到头）时拼音行也必须在：两张只换缓冲的空候选面板做差，上方有墨迹即在场。 */
    @Test
    fun `pinyin row survives an empty candidate list`() {
        val rows = diffRowIndices(
            panel("empty-buffer-a.png", model(emptyList()).apply { buffer = "mmm" }, realWindow),
            panel("empty-buffer-b.png", model(emptyList()).apply { buffer = "nnn" }, realWindow))
        assertTrue("空候选时拼音行消失了", rows.isNotEmpty())
        assertTrue("拼音行跑到「无匹配」行下面去了（$rows）", rows.max() < realWindow.height.value.toInt() / 2)
    }

    // ---------- 判据 7：候选窗跟着皮肤走（2026-10-22 用户报障） ----------

    /**
     * 底色必须取 `LocalBarPalette`（Main.kt 按 `Skin=` provide），而不是面板里写死的
     * 深色。用浅色皮肤渲一帧，左上角像素（面板 padding，一定是底色）必须等于
     * `skinColors(LIGHT).bg` —— 改回写死常量这条立刻红。
     */
    @Test
    fun `the panel background follows the skin palette`() {
        val light = skinColors(SkinOption.LIGHT)
        val w = realWindow.width.value.toInt()
        val h = realWindow.height.value.toInt()
        val img = render("skin-light.png", w, h) {
            CompositionLocalProvider(LocalBarPalette provides light) {
                CandidatePanel(
                    model(listOf("A")),
                    onSelect = {},
                    onNext = {},
                    onPrev = {},
                    windowSize = realWindow,
                )
            }
        }
        assertEquals(
            "候选窗底色没跟皮肤走（应 = 浅色皮肤的 bg）—— 面板里又写死颜色了？",
            light.bg.toArgb(),
            img.getRGB(0, 0),
        )
    }

    // ---------- 渲染夹具 ----------

    private fun render(name: String, w: Int, h: Int, content: @Composable () -> Unit): BufferedImage {
        val scene = ImageComposeScene(width = w, height = h, density = Density(1f)) {
            Box(Modifier.fillMaxSize().background(sentinel)) { content() }
        }
        val bytes = try {
            scene.render()
            scene.render().encodeToData(EncodedImageFormat.PNG)?.bytes ?: error("PNG 编码失败（$name）")
        } finally {
            scene.close()
        }
        outDir.mkdirs()
        val f = File(outDir, name)
        f.writeBytes(bytes)
        return ImageIO.read(f) ?: error("读不出 $name")
    }

    /**
     * 画一版面板。[window] 只管**字号缩放**（传进 `CandidatePanel`）；[canvas] 是画布尺寸。
     * 缺省画布 = 窗（接线判据用它：那里不量高度）；不溢出判据传高画布，好让溢出可被量到。
     */
    private fun panel(
        name: String,
        model: CandidateModel,
        window: DpSize,
        canvas: DpSize = window,
    ): BufferedImage =
        render(name, canvas.width.value.toInt(), canvas.height.value.toInt()) {
            CandidatePanel(model, onSelect = {}, onNext = {}, onPrev = {}, windowSize = window)
        }

    /** 单画一行字（判据 3 的参照物）。 */
    private fun bare(name: String, text: String, size: TextUnit): BufferedImage =
        render(name, 240, 96) { Text(text, fontSize = size, color = Color.White) }

    private fun model(ss: List<String>): CandidateModel = CandidateModel().apply {
        buffer = "nihao"
        candidates = ss
        page = 1
        pageCount = 3
        mode = "pinyin"
    }

    /** 两张同布局图里**有像素不同**的行数 = 被换掉那个字的墨迹高度（空格 = 无墨迹）。 */
    private fun diffRows(a: BufferedImage, b: BufferedImage): Int = diffRowIndices(a, b).size

    /** 同 [diffRows]，但返回**行号**（判据 6 要比较两处墨迹的上下位置）。 */
    private fun diffRowIndices(a: BufferedImage, b: BufferedImage): List<Int> {
        assertEquals("两图尺寸不同，不可比", a.width to a.height, b.width to b.height)
        val rows = mutableListOf<Int>()
        for (y in 0 until a.height) {
            for (x in 0 until a.width) {
                if (a.getRGB(x, y) != b.getRGB(x, y)) {
                    rows += y
                    break
                }
            }
        }
        return rows
    }

    /** 面板底色画到第几行（列 x=0 上最后一个非哨兵像素）+1 = 这一版内容**所需高度**（px）。 */
    private fun requiredHeightPx(img: BufferedImage): Int {
        val s = sentinel.toArgb()
        for (y in img.height - 1 downTo 0) if (img.getRGB(0, y) != s) return y + 1
        return 0
    }

    /**
     * [x0, x1) 列里有没有「非底色」像素。左上角 (0,0) 是面板 padding，一定是底色 ——
     * 用它当底色样本，不抄第二份颜色常量（抄了就会随皮肤/配色漂）。
     */
    private fun hasInkInColumns(img: BufferedImage, x0: Int, x1: Int): Boolean {
        val bg = img.getRGB(0, 0)
        for (x in x0.coerceAtLeast(0) until x1.coerceAtMost(img.width)) {
            for (y in 0 until img.height) if (img.getRGB(x, y) != bg) return true
        }
        return false
    }
}
