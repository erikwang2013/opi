// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「词库」页的**离屏渲染烟测**（2026-10-14，lead 追补）：本机无 X，真窗观感拿不到，
//! 但 `ImageComposeScene` 能把**真页**画进离屏画布（先例：StatusBarRenderProbeTest /
//! SymbolRenderProbeTest）。本文件只回答两问：
//! 1) 页在离屏画布上**渲得出、不抛**（Compose 的布局/状态类错误在这里才现形 ——
//!    纯 JVM 门禁 DictsTest 碰不到这类错），且墨迹是**纵向铺开**的（不是全叠在顶部 ——
//!    「渲染不崩」与「有布局」是两件事，见 [assertLaidOut]，首版探针就踩过）；
//! 2) 两种输入（空目录 / 2 个导入库 + 系统三行）下画布上**真有墨迹**，且两库态比
//!    同目录空态多出的墨迹 = 导入行真画了出来。
//!
//! ⚠️ 证据边界（同 StatusBarRenderProbeTest 的界，别读多）：证「渲得出来、画了东西」；
//! **不证**「窗开起来长这样」（窗位置/焦点/真机字体渲染都不在离屏场景里），也不证好不好
//! 看（间距/配色没有判据）。
//!
//! ⚠️ 墨迹断言的锚点**只用 ASCII**（系统三行 luna/trad/stroke + 导入提示里的目录路径 +
//! 夹具名 my-dict/zz-other）：CI 的 ubuntu runner 没有 CJK 字体（GlyphCoverageTest 文件头
//! 的教训，v1.7.19 因此红过），汉字标签在那种机器上一个像素都不画 —— 拿汉字当「页上有
//! 东西」的凭据就是赌字体。这里**不用 GlyphCoverage.canRender 当闸**：按它的规则
//! （「多码位一律放行」，见 GlyphCoverageTest），"luna"/"my-dict" 这类多字符 ASCII 恒真，
//! 闸是空的；真正的前提是「机器装了字体」，没字体的机器上本文件**红**（不静默绿）。
//!
//! ⚠️ 页上的目录读取走 `Dispatchers.IO`（本模块纪律：组合线程不碰文件 IO），是异步
//! 回来的：渲染循环用真实时间（60×10ms）等它落定。依据（1.11.1 源码，非猜测）：
//! `ImageComposeScene` 的效果上下文缺省是 `Dispatchers.Unconfined` —— IO 回来就地写状态
//! 标脏，下一次 `render()` 就重组到位。
//!
//! PNG 落在 `/tmp/opi-render-probe/dicts-*.png`，人眼可直接看（本机装了 CJK 字体，
//! 汉字也在图上）。

package xyz.erik.opi.candidate

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.ui.ImageComposeScene
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import java.awt.image.BufferedImage
import java.io.ByteArrayInputStream
import java.io.File
import java.nio.file.Files
import java.nio.file.Path
import javax.imageio.ImageIO
import kotlin.math.abs
import org.jetbrains.skia.EncodedImageFormat
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class DictsRenderProbeTest {

    @get:Rule
    val folder = TemporaryFolder()

    private val outDir = File("/tmp/opi-render-probe")

    /**
     * [assertLaidOut] 的下限（最下墨迹行，px）。本机实测：空态 219、两库态 323（两库多
     * 两行、页尾整体下移）；塌成「全叠在顶部」实测 49（变异轮把壳换成 Box；首版 PNG
     * 同形）。取 150：距两边各 ≥ 69px —— 只挡塌法，不量尺寸（正常漂移吸收得下）。
     */
    private val minInkBottom = 150

    /**
     * 离屏画一遍真页（620×700，密度 1f ⇒ dp == px；设置窗右内容区的量级），
     * 落 PNG 供人眼看，返回**稳定帧**（列表读完后的态）。
     *
     * 壳要照 SettingsWindow 摆：页自己**不铺底、也不起 Column** —— 底由壳铺
     * （`background(Paper)`），纵向排布也由壳给（`Column`）。⚠️ 首版探针把页直接塞进
     * `Box` ⇒ Box 把一页孩子**全部叠在同一处**（首版 PNG 人眼可见：整页挤在顶部 80px
     * 里、行与行互相盖住），断言却照样绿 —— 这也正是下面 [assertLaidOut] 那条
     * 「墨迹纵向铺开」判据的由来。
     */
    private fun probe(name: String, dir: Path): BufferedImage {
        val scene = ImageComposeScene(width = 620, height = 700, density = Density(1f)) {
            Box(Modifier.fillMaxSize().background(Paper)) {
                Column(Modifier.fillMaxSize().padding(horizontal = 22.dp, vertical = 6.dp)) {
                    DictsPage(OpSettings(), apply = {}, status = {}, dir = dir)
                }
            }
        }
        val png = try {
            scene.render() // 首帧：组合 + 布局 + 字体加载；LaunchedEffect 在这一帧把 IO 读取发出去
            var img = scene.render()
            // 列表读取在 Dispatchers.IO 上（异步）：真实时间等它回来 + 反复 render 把
            // 「读完了」那次重组画出来。两个小文件的读是微秒级，60×10ms 是量级余量。
            repeat(60) {
                img = scene.render()
                Thread.sleep(10)
            }
            img.encodeToData(EncodedImageFormat.PNG)?.bytes ?: throw AssertionError("PNG 编码失败")
        } finally {
            scene.close()
        }
        outDir.mkdirs()
        File(outDir, name).writeBytes(png)
        return ImageIO.read(ByteArrayInputStream(png)) ?: throw AssertionError("读不出 $name")
    }

    /** 与纸白底差 > 24 的像素数（阈值与取法同 SymbolRenderProbeTest 的 inkCells）。 */
    private fun inkCount(img: BufferedImage): Long {
        val bg = Paper.toArgb()
        val br = (bg shr 16) and 0xFF
        val bgG = (bg shr 8) and 0xFF
        val bb = bg and 0xFF
        var n = 0L
        for (y in 0 until img.height) {
            for (x in 0 until img.width) {
                val p = img.getRGB(x, y)
                val d = maxOf(
                    abs(((p shr 16) and 0xFF) - br),
                    abs(((p shr 8) and 0xFF) - bgG),
                    abs((p and 0xFF) - bb),
                )
                if (d > 24) n++
            }
        }
        return n
    }

    /** 与纸白底差 > 24 ⇒ 该像素算墨迹（同上阈值）。 */
    private fun isInk(argb: Int): Boolean {
        val bg = Paper.toArgb()
        return maxOf(
            abs(((argb shr 16) and 0xFF) - ((bg shr 16) and 0xFF)),
            abs(((argb shr 8) and 0xFF) - ((bg shr 8) and 0xFF)),
            abs((argb and 0xFF) - (bg and 0xFF)),
        ) > 24
    }

    /** 最下方有墨迹的行号（一行都没有 ⇒ -1）。 */
    private fun inkBottom(img: BufferedImage): Int {
        for (y in img.height - 1 downTo 0) {
            for (x in 0 until img.width) {
                if (isInk(img.getRGB(x, y))) return y
            }
        }
        return -1
    }

    /**
     * 粗判「页真排版了」：最下墨迹行要低于 [minInkBottom]（实测值见用例打印）。
     *
     * 不是布局判据（不量间距/位置/对齐），只挡一种塌法：**一页孩子叠在同一处**
     * （壳不给 Column / 孩子高度归零这类）—— 那时整页挤在顶部，而 `ink > 0` 照样绿，
     * 渲染烟测就白测了。首版探针实测撞到过：PNG 人眼可见整页在顶部 ~80px 内互相盖住
     * （成因是探针自己把页塞进 Box，见 [probe] 的注释）。
     */
    private fun assertLaidOut(img: BufferedImage, what: String) {
        val bottom = inkBottom(img)
        assertTrue(
            "$what：最下墨迹行 $bottom 该 > $minInkBottom —— 太低说明页的墨迹全挤在顶部" +
                "（孩子没被纵向排开）：渲染绿了但有内容没布局",
            bottom > minInkBottom,
        )
    }

    @Test
    fun `the page renders offscreen with no imported dicts`() {
        val dir = folder.newFolder("empty").toPath()
        val img = probe("dicts-empty.png", dir)
        val ink = inkCount(img)
        println("空目录：墨迹 $ink 像素、最下墨迹行 ${inkBottom(img)}（PNG /tmp/opi-render-probe/dicts-empty.png）")
        // 锚点全是 ASCII：系统三行名 luna/trad/stroke，加导入提示里的目录路径（临时目录
        // 路径无汉字）。有字体的机器上这几段必然画出来 ⇒ 零墨迹 = 布局塌了（真错误），
        // 而不是「本机缺字形」；没字体的机器上红（见文件头，不静默绿）。
        assertTrue("空目录页也该有墨迹（luna/trad/stroke 与目录路径都是 ASCII）：实测 $ink", ink > 0)
        assertLaidOut(img, "空目录页")
    }

    @Test
    fun `two imported dicts add ink over the same directory's empty state`() {
        // 同一个目录先空着渲一次、再放两个库渲一次：两次的提示文案**逐字相同**（目录
        // 路径就是同一个）⇒ 画布上的差只可能来自列表区（空态文案 vs 两行导入库）。
        val dir = folder.newFolder("dicts").toPath()
        val emptyImg = probe("dicts-empty-same-dir.png", dir)
        val emptyInk = inkCount(emptyImg)
        Files.writeString(dir.resolve("my-dict.tsv"), "你好\tnihao\t100\n世界\tshijie\t90\n吧\tba\t5\n")
        Files.writeString(dir.resolve("zz-other.tsv"), "啊\ta\t1\n")
        // 夹具自检：页面读到的就是这两个名字（列目录本身是 DictsTest 的门禁；这里只防
        // 「夹具写歪了 ⇒ 页面照渲染 ⇒ 断言在空转」）。
        assertEquals(listOf("my-dict", "zz-other"), listDicts(dir).map { it.name })
        val twoImg = probe("dicts-two.png", dir)
        val twoInk = inkCount(twoImg)
        println(
            "同一目录：空态 $emptyInk 像素（最下行 ${inkBottom(emptyImg)}）→ " +
                "两库 $twoInk 像素（最下行 ${inkBottom(twoImg)}）（PNG /tmp/opi-render-probe/dicts-two.png）",
        )
        assertTrue("两库页该有墨迹（my-dict / zz-other 是 ASCII）：实测 $twoInk", twoInk > 0)
        assertLaidOut(twoImg, "两库页")
        assertTrue(
            "两库态的墨迹必须多于同目录空态（$twoInk vs $emptyInk）—— 差额来自两行导入库" +
                "（ASCII 名字 + 条数·大小 + 删除框 + 开关）；少了就是列表行没画出来",
            twoInk > emptyInk,
        )
    }
}
