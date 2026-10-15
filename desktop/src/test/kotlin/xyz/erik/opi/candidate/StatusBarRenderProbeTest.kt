// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 状态栏按钮**面**的像素门禁（2026-10-14，本批的主要价值）：用户这一轮的裁决是**视觉的**
//! （原话：「状态栏的中文、英文标点符号切换按钮：要句号在按钮中左上侧，逗号在右下侧，
//! 根据按钮显示最大，不要溢出；符号按钮中的符号也根据按钮显示最大，不要溢出；其他图标、
//! 文字等也在自己所在的按钮显示最大，不要溢出」），所以判据也只能是视觉的 —— 把
//! `StatusBarButtonFace` 离屏画进一张 **40×40 的画布、真按钮（30×30）居中**（与 `BarButton`
//! 里的摆法逐字一致），再逐像素读回来量墨迹框。
//!
//! ⚠️ **画布比按钮大一圈（四周各 5px）是这条门禁的关键**：溢出没有「留白」可以吸收 ——
//! 多出来的墨迹会落在按钮外那一圈里**被量到**。画布正好等于按钮的话，溢出会被画布裁掉，
//! 判据就退化成「裁掉了当然不溢出」的假绿。
//!
//! **2026-10-15（用户再裁决：「状态栏图标、文字在自己所在的按钮显示太大，缩小40％」）**：
//! 面内容整体 × `FACE_SCALE`（0.6）⇒ 面内容的**内盒** = 按钮 × 0.6、居中（18×18 落在
//! 30×30 里）。本文件两处跟着动，别只改一处：
//! · 「不溢出」量的仍是**按钮**（30×30）—— 那是用户说的边界，没因为缩小而改口径；
//! · 「左上/右下」这类**槽位比例**的判据改量**内盒**（[innerL]/[faceSide]）—— 缩放后
//!   字形只占按钮中间 60%，拿按钮的象限去量会把「逗号的尾巴」算进左下象限（那是量错了
//!   地方，不是画错了）；
//! · 字号/墨迹的**数字期望**按 `FACE_SCALE` 折算，逐条标了「缩放系数变了要同步这里」。
//!
//! · **不溢出** = 墨迹框 ⊂ 按钮矩形（30×30，用户的原话就是按钮）—— 边距 0，溢出 1px 可见；
//! · **左上/右下** = 「。」的墨迹在**内盒**的左上象限、「，」的在右下象限，且**另两个象限
//!   是空的**（布局上两枚字形占的是两个不相交的槽位，见 BarGlyphs.kt 的 `punctSlots`）；
//! · **按盒取最大** = 把**旧面**（2026-10-14 之前的写法 `Text(…, fontSize = 12.sp)`）
//!   在同一张画布上画一遍量墨迹，钉「新的 ≥ 2× 旧的」—— 基线是**当场量出来的**，
//!   不是记忆里的数字。⚠️ 2026-10-15 缩到 0.6 之后这条按同比例收紧（见用例里的注记）。
//!
//! ⚠️ **CI 的 ubuntu runner 没有 CJK 字体**（教训在 GlyphCoverageTest 的文件头：v1.7.19
//! 两条桌面测试因此红过）：所以这里没有一条断言建在「某个汉字画得出来」上。缺字形的用例
//! 走**发现式**分支（`GlyphCoverage.canRender` 现问本机字体，不问被测函数），断的仍是不
//! 变量；**画得出**这一侧由 ASCII（`.`, `,`, `en`, `26`）与四个自绘图标无条件钉死 ——
//! 它们在「装了字体的机器」上都画得出，所以「面什么都不画」这种坏法不会假绿。
//!
//! ⚠️ 它证的是「这些像素画得出来」，**不证**「窗开起来长这样」（同 SymbolRenderProbeTest
//! 的界：窗的位置/焦点/真机字体渲染都不在这里）。PNG 落在 `/tmp/opi-render-probe/`，
//! 人眼可直接看。

package xyz.erik.opi.candidate

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.jetbrains.skia.EncodedImageFormat
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.awt.image.BufferedImage
import java.io.File
import javax.imageio.ImageIO
import kotlin.math.abs
import kotlin.math.ceil

class StatusBarRenderProbeTest {

    private val outDir = File("/tmp/opi-render-probe")

    /** 按钮盒四周在画布上留的余量（px，密度 1f）：溢出会落在这一圈里被量到，不再被裁掉。 */
    private val probePad = 5
    private val canvas = BTN + 2 * probePad

    /**
     * 探针用的配色 = **纯白底 + 纯黑字**（不是真皮肤）：判据量的是**形状**，不是颜色，
     * 而最大对比让「有墨迹」的阈值（[INK_DIFF]）在任何字体粗细下都稳。真皮肤的对比在
     * `bar palette follows the skin` 那条纯函数用例里钉。
     */
    private val probeColors = SkinColors(
        bg = Color.White, fg = Color.Black, hover = Color.Black,
        line = Color.Black, accent = Color.Black,
    )

    // ---------- 像素读数 ----------

    /** 墨迹框（图像坐标，闭区间）。[empty] = 这块区域一个墨迹像素都没有。 */
    private data class Ink(val l: Int, val t: Int, val r: Int, val b: Int) {
        val w: Int get() = r - l + 1
        val h: Int get() = b - t + 1
        val empty: Boolean get() = r < l
        override fun toString(): String = if (empty) "空" else "[$l,$t..$r,$b] ${w}×$h"
    }

    /** 与纯白底差多少才算墨迹：40/255 ≈ 16% 覆盖 —— 抗锯齿的边缘像素也算（宁可多算）。 */
    private val inkDiff = 40

    private fun isInk(argb: Int): Boolean = maxOf(
        abs(((argb shr 16) and 0xFF) - 255),
        abs(((argb shr 8) and 0xFF) - 255),
        abs((argb and 0xFF) - 255),
    ) > inkDiff

    /** 读一块区域（行列闭区间）的墨迹框；没有墨迹 ⇒ [Ink.empty]。 */
    private fun inkIn(img: BufferedImage, cols: IntRange, rows: IntRange): Ink {
        var l = Int.MAX_VALUE
        var t = Int.MAX_VALUE
        var r = -1
        var b = -1
        for (y in rows) for (x in cols) {
            if (isInk(img.getRGB(x, y))) {
                if (x < l) l = x
                if (x > r) r = x
                if (y < t) t = y
                if (y > b) b = y
            }
        }
        return Ink(l, t, r, b)
    }

    /**
     * 一块**浮点**区域（面的比例坐标）换算成「整块落在里面」的像素行列：两侧各内缩 1px ——
     * 边界的抗锯齿像素不算数。方向是**让断言只弱不假红**：「这里不该有墨迹」的范围小一点，
     * 「这里有墨迹」的判据也小一点。
     */
    private fun inside(a: Float, b: Float): IntRange = (ceil(a).toInt() + 1)..(ceil(b).toInt() - 2)

    // 面的像素几何（密度 1f ⇒ dp == px）：[probePad] 宽的画布四周留白，按钮盒（30×30）居中。
    private val faceL = probePad
    private val faceR = faceL + BTN // 不含

    // 面内容的**内盒**（2026-10-15 起 = 按钮 × FACE_SCALE，居中）：槽位比例量的是这块。
    private val faceSide = BTN * FACE_SCALE // 18f
    private val innerL = faceL + (BTN - faceSide) / 2f // 11f（= 按钮左边 + 6）
    private val innerR = innerL + faceSide // 不含

    /** 面**内盒**的像素范围：行/列各 [0,1] → 整块落在这段里的行列。 */
    private fun cell(x0: Float, x1: Float, y0: Float, y1: Float) = inside(
        innerL + x0 * faceSide, innerL + x1 * faceSide,
    ) to inside(innerL + y0 * faceSide, innerL + y1 * faceSide)

    /**
     * 把一个 composable（面/旧面）画进画布中心（**与 BarButton 同一摆法**：30dp 的按钮盒
     * 居中），落一张 PNG 到 `/tmp/opi-render-probe/statusbar-*.png` 供人眼看，
     * 返回全画布的墨迹框与图。
     */
    private fun probe(name: String, content: @Composable () -> Unit): Pair<Ink, BufferedImage> {
        val scene = ImageComposeScene(width = canvas, height = canvas, density = Density(1f)) {
            CompositionLocalProvider(LocalBarPalette provides probeColors) {
                Box(
                    Modifier.fillMaxSize().background(probeColors.bg),
                    contentAlignment = Alignment.Center,
                ) { content() }
            }
        }
        val bytes = try {
            scene.render() // 首帧：布局 + 字体加载
            scene.render().encodeToData(EncodedImageFormat.PNG)?.bytes
                ?: throw AssertionError("PNG 编码失败")
        } finally {
            scene.close()
        }
        outDir.mkdirs()
        File(outDir, "statusbar-$name.png").writeBytes(bytes)
        val img = ImageIO.read(File(outDir, "statusbar-$name.png"))
            ?: throw AssertionError("读不出 statusbar-$name.png")
        return inkIn(img, 0..img.width - 1, 0..img.height - 1) to img
    }

    /** 一枚按钮的面（与状态栏里的调用逐字同参：盒 = 按钮本身）。 */
    @Composable
    private fun face(b: StatusBarButton, st: ImeState = ImeState(), mode: String = MODE_DEFAULT) {
        StatusBarButtonFace(b, st, mode, BTN.dp)
    }

    /** 旧面（2026-10-14 之前）：`Text(iconText, fontSize = 12.sp)` —— 只用来当**基线**量。 */
    @Composable
    private fun oldFace(text: String) {
        Text(text, fontSize = 12.sp, color = Color.Black, maxLines = 1)
    }

    // ---------- 用例 ----------

    /** 一个被测面：哪枚按钮 + 状态 + 档位。 */
    private data class FaceCase(
        val name: String,
        val b: StatusBarButton,
        val st: ImeState = ImeState(),
        val mode: String = MODE_DEFAULT,
    )

    @Test
    fun `no button face spills out of its button`() {
        // (c) 各按钮的文字/图标 bbox ⊂ 按钮矩形 = **用户的原话**（2026-10-14，见 StatusBar.kt
        // 的 BarButton）。⚠️ 2026-10-15 起这条**没改口径**：用户说的边界一直是**按钮**，
        // 缩小 40% 只把内容收进内盒（按钮 × FACE_SCALE），不把「不溢出」的边界跟着缩小。
        // 画图标的四枚也一并量（它们这一轮才从 17/13dp 改成按面取最大，几何也是新算的）；
        // 模式按钮三档各量一次（它的面由 token 驱动，不走 iconText）。
        val cases = listOf(
            FaceCase("punct-zh", StatusBarButton.PUNCT, ImeState(punct = true)),
            FaceCase("punct-en", StatusBarButton.PUNCT, ImeState(punct = false)),
            FaceCase("symbols", StatusBarButton.SYMBOLS),
            FaceCase("trad-fan", StatusBarButton.TRAD, ImeState(trad = true)),
            FaceCase("trad-jian", StatusBarButton.TRAD, ImeState(trad = false)),
            FaceCase("lang-en", StatusBarButton.LANG, ImeState(zh = false)),
            FaceCase("fullwidth-on", StatusBarButton.FULLWIDTH, ImeState(fullwidth = true)),
            FaceCase("fullwidth-off", StatusBarButton.FULLWIDTH, ImeState(fullwidth = false)),
            FaceCase("vk", StatusBarButton.VK),
            FaceCase("settings", StatusBarButton.SETTINGS),
            FaceCase("logo", StatusBarButton.LOGO),
            FaceCase("mode-pinyin", StatusBarButton.MODE, mode = "pinyin"),
            FaceCase("mode-en26", StatusBarButton.MODE, mode = "en26"),
            FaceCase("mode-wubi", StatusBarButton.MODE, mode = "wubi"),
        )
        val rows = mutableListOf<String>()
        for (c in cases) {
            val (ink, img) = probe(c.name) { face(c.b, c.st, c.mode) }
            rows += "${c.name} 墨迹 $ink（按钮盒 $faceL..${faceR - 1}，面内盒 ${innerL.toInt()}..${innerR.toInt() - 1}）"
            // 不溢出（用户原话）：所有用例都断 —— 缺字形 ⇒ 墨迹空 ⇒ 恒真，不是跳过。
            // 盒 = 按钮、**不给余量**：溢出 1px 就落在按钮外那一圈（画布四周留了 [probePad]px，
            // 所以溢出是真被量到的，不是被画布裁掉的）。四条溢出带分开量，报错时能看出往哪边漏。
            // ⚠️ 2026-10-15 起「内容 ⊆ 按钮」比「内容 ⊆ 内盒」**松**（内盒四周还有 6px 余量）
            // —— 这是有意的：用户要的边界就是按钮；内容缩过头由下面那条「≈ 按钮 × FACE_SCALE」
            // 与字号用例把住，不是靠这条。
            val spill = listOf(
                inkIn(img, 0..faceL - 1, 0..canvas - 1),
                inkIn(img, faceR..canvas - 1, 0..canvas - 1),
                inkIn(img, faceL..faceR - 1, 0..faceL - 1),
                inkIn(img, faceL..faceR - 1, faceR..canvas - 1),
            ).filter { !it.empty }
            assertTrue(
                "${c.name} 溢出按钮（墨迹 $ink 该落在 ${faceL..faceR - 1}；溢出的四带 $spill）",
                spill.isEmpty(),
            )
            // 正控：自绘图标 + 本机有字形的字（ASCII 在任何装了字体的机器上都有）必须真有
            // 墨迹 —— 否则上面两条「不溢出」是空转（什么都不画必然不溢出）。
            val icon = c.b == StatusBarButton.LOGO || c.b == StatusBarButton.VK ||
                c.b == StatusBarButton.FULLWIDTH || c.b == StatusBarButton.SETTINGS
            val drawn = if (icon) emptyList() else buttonSlots(c.b, c.st, c.mode).map { it.text }
            val expectInk = icon || drawn.isEmpty() || drawn.all(GlyphCoverage::canRender)
            if (expectInk) {
                assertTrue("${c.name} 该有墨迹（图标或本机有的字形）：墨迹 $ink 是空的", !ink.empty)
            } else {
                rows += "${c.name} 本机缺字形（${drawn.joinToString("/")}）：只断不溢出"
            }
        }
        println(
            "面墨迹（${canvas}×${canvas} 画布，按钮盒 $faceL..${faceR - 1}，" +
                "面内容内盒 ${innerL.toInt()}..${innerR.toInt() - 1} = 按钮 × $FACE_SCALE）：\n" +
                rows.joinToString("\n"),
        )
    }

    @Test
    fun `punct draws the period upper left and the comma lower right`() {
        // (b) 用户原话：「要句号在按钮中左上侧，逗号在右下侧」。两枚字形占两个**不相交**的
        // 象限槽（BarGlyphs 的 punctSlots：左上 + 右下），所以判据是四问：左上/右下**有**墨迹，
        // 右上/左下**没有** —— 后两问才是「位置对不对」的判据（前两问只证「画了东西」）。
        // ⚠️ 2026-10-15 起四个象限量的是**面内盒**（按钮 × FACE_SCALE，[cell] 已按它映射）：
        // 缩小后字形只占按钮中间 60%，拿按钮的象限去量会把逗号的尾巴算进左下象限。
        // 先量英文态（`.`,`,` 是 ASCII，任何装了字体的机器都有 ⇒ 无条件断言），
        // 再量中文态（「。」「，」，本机没有 CJK 字体就走发现式分支，见文件头）。
        for ((name, st) in listOf("punct-en" to ImeState(punct = false), "punct-zh" to ImeState(punct = true))) {
            val pair = punctSlots(st).map { it.text }
            val covered = pair.all(GlyphCoverage::canRender)
            val (ink, img) = probe(name) { face(StatusBarButton.PUNCT, st) }
            if (!covered) {
                // 本机画不出这两枚字形 ⇒ 面是空的（SymbolRenderProbeTest 实测：缺字形一像素
                // 都不画）。**不**把这个当「位置对」，只如实打出来。
                println("$name「${pair.joinToString("")}」本机缺字形（画出来 $ink）：象限判据不适用，只断不溢出（在上一条）")
                continue
            }
            assertTrue("$name 面该有墨迹（本机有字形）", !ink.empty)
            val (ulCols, ulRows) = cell(0f, 0.5f, 0f, 0.5f)
            val (lrCols, lrRows) = cell(0.5f, 1f, 0.5f, 1f)
            val (urCols, urRows) = cell(0.5f, 1f, 0f, 0.5f)
            val (llCols, llRows) = cell(0f, 0.5f, 0.5f, 1f)
            val ul = inkIn(img, ulCols, ulRows)
            val lr = inkIn(img, lrCols, lrRows)
            val ur = inkIn(img, urCols, urRows)
            val ll = inkIn(img, llCols, llRows)
            println("$name「${pair.joinToString("")}」左上=$ul 右下=$lr 右上=$ur 左下=$ll（面 $ink）")
            assertTrue("$name 左上象限该有「${pair[0]}」（用户：句号在左上）", !ul.empty)
            assertTrue("$name 右下象限该有「${pair[1]}」（用户：逗号在右下）", !lr.empty)
            assertTrue("$name 右上象限该是空的（墨迹 $ur）—— 有东西就是两枚没落进各自象限", ur.empty)
            assertTrue("$name 左下象限该是空的（墨迹 $ll）—— 同上", ll.empty)
        }
    }

    @Test
    fun `mode en26 hangs the corner badge at the bottom right`() {
        // 用户原话：「en图标，其中en右下角加26」。槽位是「en」占上部 + 「26」占右下
        // （BarGlyphs 的 modeSlots，两槽按 0.55 分界、**不相交**）。像素判据同标点那条
        // （三个格子量的都是**面内盒** = 按钮 × FACE_SCALE，[cell] 已按它映射）：
        // 上带与右下格**有**墨迹、左下格**没有** —— 最后一问才是「26 在右下角」的判据
        // （角标要是画到左下、或 en 画满整块面，左下格就有墨迹）。
        val slots = modeSlots("en26").map { it.text }
        assertEquals("en26 该是「en」+ 角标「26」两块", listOf("en", "26"), slots)
        val (ink, img) = probe("mode-en26") { face(StatusBarButton.MODE, mode = "en26") }
        assertTrue("en26 的两枚都是 ASCII（任何装了字体的机器都有）⇒ 面必须有墨迹", !ink.empty)
        val (topCols, topRows) = cell(0f, 1f, 0f, 0.55f)
        val (brCols, brRows) = cell(0.55f, 1f, 0.55f, 1f)
        val (blCols, blRows) = cell(0f, 0.55f, 0.55f, 1f)
        val top = inkIn(img, topCols, topRows)
        val br = inkIn(img, brCols, brRows)
        val bl = inkIn(img, blCols, blRows)
        println("en26：「en」带=$top 右下角=$br 左下格=$bl（面 $ink）")
        assertTrue("上带该有「en」", !top.empty)
        assertTrue("右下角该有角标「26」", !br.empty)
        assertTrue("左下格该是空的（墨迹 $bl）—— 有东西就是角标没落在右下角", bl.empty)
        // 无角标的两档：整块面给一个字（同一个 token 表驱动，别只测 en26）
        for (m in listOf("pinyin", "wubi")) {
            val (i2, _) = probe("mode-$m") { face(StatusBarButton.MODE, mode = m) }
            assertEquals("$m 该只有一个槽（无角标：${modeCorner(m)}）", 1, modeSlots(m).size)
            if (GlyphCoverage.canRender(modeFace(m))) {
                assertTrue("$m 面该有墨迹", !i2.empty)
            } else {
                println("$m「${modeFace(m)}」本机缺字形（画出来 $i2）：只断不溢出（在上一条）")
            }
        }
    }

    @Test
    fun `the new face is measurably bigger than the old 12sp text`() {
        // (d) 字号 **明显大于**旧 12.sp。基线**当场量**：把旧写法（`Text(…, 12.sp)`，2026-10-14
        // 之前的面）在同一张画布上画一遍。这样判据不依赖「我记得旧的有多大」——
        // 旧的大小若哪天变了，这条自己跟着动。
        // 另把**算出来的字号**（fitGlyphPx）打出来给人看：那是「按盒取最大」的直接读数。
        val (oldAscii, _) = probe("old-punct-en") { oldFace(punctIcon(ImeState(punct = false))) }
        val (newAscii, _) = probe("punct-en") { face(StatusBarButton.PUNCT, ImeState(punct = false)) }
        println("标点面：旧「.,」@12sp 墨迹 $oldAscii → 新「.」+「,」墨迹 $newAscii")
        // ⚠️ 2026-10-15：面内容乘了 FACE_SCALE ⇒ 这条「≥ 2× 旧的」同比例折算成
        // 「≥ 2 × FACE_SCALE × 旧的」（= 缩放后仍比旧的 12sp 面大 1.2 倍）。别改回不折算的
        // 2×：那是 2026-10-14 的口径，用户已经在那之上又裁决「缩小40％」了。
        assertTrue(
            "新的标点面该 ≥ 2 × $FACE_SCALE × 旧的（旧 $oldAscii / 新 $newAscii）——" +
                "「标点太小」正是用户 2026-10-14 报的毛病，2026-10-15 只在它之上整体缩 60%",
            newAscii.h >= 2 * FACE_SCALE * oldAscii.h && newAscii.w >= 2 * FACE_SCALE * oldAscii.w,
        )
        // 2026-10-15 裁决的**直接读数**（量的是真画出来的像素，不是重算一遍 fitGlyphPx）：
        // 标点面两枚各占内盒的一个象限、各按墨迹取满自己的格 ⇒ 合起来的墨迹高 ≈ 内盒高
        // = 按钮 × FACE_SCALE = 18px。这条**随 FACE_SCALE 自适应**（改常数它自己跟着走），
        // 它断的是「缩放真落在渲染里」—— 把 FACE_SCALE 的应用点删掉/挪到别处，这里立刻红。
        val expectH = (BTN * FACE_SCALE).toInt()
        assertTrue(
            "标点面墨迹高该 ≈ 按钮 × FACE_SCALE = ${expectH}px（实测 ${newAscii.h}）——" +
                "面内容没缩到 60%（或缩过头）都会在这儿露出来",
            abs(newAscii.h - expectH) <= 2,
        )
        // 算出来的字号（px，密度 1f）：每个槽位都按墨迹取最大。12.sp 的旧面 = 12px。
        // 名单里**没有角标「26」**：它占的是内盒右下角那一小块槽（0.45 × 内盒 = 8.1px 见方），
        // 字号被自己的槽顶住（缩放前 13.5px 槽实测 12.3px；× FACE_SCALE 后的值见下面打印）
        // —— 「≥ 2× 旧的 12px」这条规则只对**主字形**成立，角标按「塞进槽里且不溢出」判
        // （像素那条用例）。角标单独量、单独打印。
        // ⚠️ 量的是**面内盒**（按钮 × FACE_SCALE，2026-10-15 裁决）—— 拿按钮去量就不是
        // 产品里画的那一支了。缩放系数变了要同步这里（含下面的阈值 24 × FACE_SCALE）。
        fun fitIn(text: String, fracW: Float = 1f, fracH: Float = 1f) =
            fitGlyphPx(text, BTN * FACE_SCALE * fracW, BTN * FACE_SCALE * fracH)

        val sized = listOf(
            "punct-en「.」" to fitIn(".", fracW = 0.5f, fracH = 0.5f),
            "symbols「Ω」" to fitIn("Ω"),
            "trad「繁」" to fitIn("繁"),
            "lang「中」" to fitIn("中"),
            "mode「拼」" to fitIn("拼"),
            "mode-en「en」" to fitIn("en", fracH = 0.55f),
        )
        val badge = fitIn("26", fracW = 0.45f, fracH = 0.45f)
        // 「≥ 2× 旧的 12px」是 2026-10-14 立的（那轮把面从 12sp 顶到按盒取最大）；
        // 2026-10-15 用户裁决「缩小40％」⇒ 面内容整体 × FACE_SCALE，这条同比例收紧：
        // 阈值 = 24 × FACE_SCALE = 14.4px（= 缩放后仍比旧的 12px 大 1.2 倍）。
        // 缩放系数变了要同步这三处阈值。
        val minPx = 24f * FACE_SCALE
        println(
            "按盒取最大的字号（px，旧面 = 12.sp = 12px；盒 = 按钮 × $FACE_SCALE）：\n" +
                sized.joinToString("\n") { "  ${it.first} = ${"%.1f".format(it.second)}px" } +
                "\n  mode-26「26」（角标，槽 ${"%.1f".format(BTN * FACE_SCALE * 0.45f)}px） = " +
                "${"%.1f".format(badge)}px",
        )
        // ASCII 的两条无条件断（任何字体都有这两个字形）；汉字/Ω 走发现式（CI 没有 CJK 字体）
        assertTrue(
            "「.」的字号该 ≥ 24 × $FACE_SCALE = ${"%.1f".format(minPx)}px（实测 ${sized[0].second}）",
            sized[0].second >= minPx,
        )
        assertTrue(
            "「en」的字号该 ≥ 24 × $FACE_SCALE = ${"%.1f".format(minPx)}px（实测 ${sized[5].second}）",
            sized[5].second >= minPx,
        )
        for ((name, px) in sized) {
            val glyph = name.substringAfter('「').substringBefore('」')
            if (GlyphCoverage.canRender(glyph)) {
                assertTrue(
                    "$name 的字号该 ≥ 24 × $FACE_SCALE = ${"%.1f".format(minPx)}px" +
                        "（实测 ${"%.1f".format(px)}px）",
                    px >= minPx,
                )
            } else {
                println("  $name 本机缺字形（量到的是排版盒退路的 ${"%.1f".format(px)}px，不是墨迹）")
            }
        }
        // 角标：比旧面（12px）小是**设计**（用户要的是「右下角加 26」的小角标），但也不该
        // 缩到看不清 —— 6px 是「角落里还读得出两个数字」的下限（本机实测：缩放前 12.3px，
        // 2026-10-15 乘 FACE_SCALE 后见上面打印的值）。⚠️ 这条**不随 FACE_SCALE 折算**：
        // 它断的是「还看得见」这条产品下限，不是「按盒取最大」的比例。
        assertTrue("角标「26」的字号不该缩到 ${"%.1f".format(badge)}px（< 6px 就只是个墨点了）", badge >= 6f)
        // 汉字那条的**墨迹**对比（本机有 CJK 字体时才有意义）：旧面「。，」@12sp vs 新面。
        val pair = punctSlots(ImeState(punct = true)).map { it.text }
        if (pair.all(GlyphCoverage::canRender)) {
            val (oldZh, _) = probe("old-punct-zh") { oldFace(punctIcon(ImeState(punct = true))) }
            val (newZh, _) = probe("punct-zh") { face(StatusBarButton.PUNCT, ImeState(punct = true)) }
            println("中文标点：旧「。，」@12sp 墨迹 $oldZh → 新「。」+「，」墨迹 $newZh")
            // 同上面英文那条：2026-10-15 乘 FACE_SCALE 后按同比例折算（缩放系数变了要同步这里）。
            assertTrue(
                "新的中文标点面该 ≥ 2 × $FACE_SCALE × 旧的（旧 $oldZh / 新 $newZh）",
                newZh.h >= 2 * FACE_SCALE * oldZh.h,
            )
        }
    }
}
