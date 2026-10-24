// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntRect
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.sp
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 26 键布局的纯数据门禁（2026-10-24 参考图 aa.jpg / cc.jpg）：
 * 行表、符号提示、弹层备选次序与默认项、弹层定位夹取。
 * 手势本身（长按/上滑/拖动选行）要真机或 Compose 仪器测试，本机 JVM 无 Robolectric 测不到。
 */
class Qwerty26LayoutTest {

    @Test
    fun englishHintsCoverEveryLetterKey() {
        assertEquals(26, qwertySymbolHints.size)
        assertEquals("`", qwertySymbolHints["q"])
        assertEquals("|", qwertySymbolHints["p"])
        assertEquals("…", qwertySymbolHints["t"])
        assertEquals("\"", qwertySymbolHints["y"])
        assertEquals("'", qwertySymbolHints["h"])
        assertEquals("-", qwertySymbolHints["c"])
        assertEquals("-", qwertySymbolHints["v"])
        assertEquals("/", qwertySymbolHints["m"])
    }

    @Test
    fun pinyinHintsSitOnlyOnTheAZRowsAndUseChineseQuotes() {
        assertEquals("“", pinyinSymbolHints["h"])
        assertEquals("”", pinyinSymbolHints["j"])
        // Q 行是数字角标，不挂符号提示（cc.jpg）。
        for (ch in "qwertyuiop") {
            assertNull("拼音 Q 行 $ch 不该有符号提示", pinyinSymbolHints[ch.toString()])
        }
        assertEquals(16, pinyinSymbolHints.size) // A 行 9 + Z 行 7
    }

    @Test
    fun pinyinDigitsRunOneToZeroAcrossTheQRow() {
        assertEquals(
            "1234567890",
            "qwertyuiop".map { pinyinDigitHints[it.toString()] }.joinToString(""),
        )
    }

    @Test
    fun alternativesAreUppercaseSymbolLowercaseWithSymbolSelectedByDefault() {
        val english = qwertyAlternatives("h", pinyin = false)
        assertEquals(listOf("H", "'", "h"), english.map { it.label })
        assertEquals(listOf("H", "'", "h"), english.map { it.value })
        val pinyin = qwertyAlternatives("h", pinyin = true)
        assertEquals(listOf("H", "“", "h"), pinyin.map { it.label })
        // 拼音 Q 行中间项是角标数字。
        assertEquals(listOf("Q", "1", "q"), qwertyAlternatives("q", pinyin = true).map { it.label })
        // 非字母（数字行）没有备选。
        assertTrue(qwertyAlternatives("1", pinyin = false).isEmpty())
        // 默认下标必须落在「符号/数字」这一项。
        assertEquals(1, QWERTY_ALTERNATIVE_DEFAULT)
        assertEquals("'", english[QWERTY_ALTERNATIVE_DEFAULT].value)
        assertEquals("1", qwertyAlternatives("q", pinyin = true)[QWERTY_ALTERNATIVE_DEFAULT].value)
    }

    @Test
    fun alternativeFontSizeFallsBackToNullForTheKeyFont() {
        assertNull(KeyAlternative("A", "A").fontSize)
        assertEquals(12.sp, KeyAlternative("A", "A", 12.sp).fontSize)
    }

    @Test
    fun englishBottomRowMatchesTheReferenceImage() {
        assertEquals(
            listOf("!?#", "pro", ",", "空格", ".", "中", "换行"),
            qwertyBottomKeys(english = true).map { it.label },
        )
    }

    @Test
    fun pinyinBottomRowMatchesTheReferenceImage() {
        assertEquals(
            listOf("符", "123", ",", "空格", "。", "英", "换行"),
            qwertyBottomKeys(english = false).map { it.label },
        )
    }

    @Test
    fun bottomRowModeKeyShowsTheTargetLanguage() {
        // 2026-10-25：模式键写**目标语言**（英文面「中」、中文面「英」），点一次直达。
        assertEquals("中", qwertyBottomKeys(english = true)[5].label)
        assertEquals("英", qwertyBottomKeys(english = false)[5].label)
        val space = qwertyBottomKeys(english = true)[3]
        assertEquals("空格", space.label)
        assertTrue("空格必须比其它键宽", space.weight > 1f)
    }

    @Test
    fun pinyinRowsHaveNoDigitRowAndKeepBackspaceOnTheShiftRow() {
        val rows = qwertyRows(showNumberRow = false)
        assertEquals(3, rows.size)
        assertEquals("⇧", rows[2].first())
        assertEquals("⌫", rows[2].last())
    }

    @Test
    fun popupOpensAboveTheKeyWhenThereIsRoom() {
        val pos = KeyPopupPositionProvider(IntSize(1080, 600)).calculatePosition(
            anchorBounds = IntRect(100, 400, 200, 500),
            windowSize = IntSize(1080, 600),
            layoutDirection = LayoutDirection.Ltr,
            popupContentSize = IntSize(100, 120),
        )
        // 上方：y = 键顶 - 间隙 - 弹层高；x = 键中心 - 弹层半宽。
        assertEquals(IntOffset(100, 400 - 4 - 120), pos)
    }

    @Test
    fun popupFlipsBelowAndStaysInsideTheKeyboardWindow() {
        val provider = KeyPopupPositionProvider(IntSize(1080, 600))
        // 首行键上方放不下 → 翻到键下方；左越界被夹回窗口内。
        val below = provider.calculatePosition(
            anchorBounds = IntRect(0, 0, 100, 100),
            windowSize = IntSize(1080, 600),
            layoutDirection = LayoutDirection.Ltr,
            popupContentSize = IntSize(100, 120),
        )
        assertEquals(IntOffset(8, 104), below)
        // 右侧越界 → 夹到右边距。
        val right = provider.calculatePosition(
            anchorBounds = IntRect(1000, 400, 1080, 500),
            windowSize = IntSize(1080, 600),
            layoutDirection = LayoutDirection.Ltr,
            popupContentSize = IntSize(100, 120),
        )
        assertEquals(1080 - 100 - 8, right.x)
        // 弹层翻到键下方后超出键盘窗底：Compose 给的整屏高（2400）放得下，
        // 但必须按键盘窗高（400）夹回窗内。
        val keyboard = KeyPopupPositionProvider(IntSize(1080, 400))
        val clamped = keyboard.calculatePosition(
            anchorBounds = IntRect(100, 300, 200, 380),
            windowSize = IntSize(1080, 2400),
            layoutDirection = LayoutDirection.Ltr,
            popupContentSize = IntSize(100, 300),
        )
        assertEquals(400 - 300 - 8, clamped.y)
        // 弹层比窗口还高时仍留在窗口内（上边距优先）。
        val tall = KeyPopupPositionProvider(IntSize(1080, 100)).calculatePosition(
            anchorBounds = IntRect(0, 0, 100, 100),
            windowSize = IntSize(1080, 100),
            layoutDirection = LayoutDirection.Ltr,
            popupContentSize = IntSize(100, 300),
        )
        assertEquals(8, tall.y)
    }

    @Test
    fun popupFallsBackToComposeWindowSizeWhenHostSizeIsUnknown() {
        val pos = KeyPopupPositionProvider(IntSize.Zero).calculatePosition(
            anchorBounds = IntRect(1000, 400, 1080, 500),
            windowSize = IntSize(1080, 600),
            layoutDirection = LayoutDirection.Ltr,
            popupContentSize = IntSize(100, 120),
        )
        assertEquals(1080 - 100 - 8, pos.x)
    }

    @Test
    fun wubiLabelsDifferOnlyOnTheLeadKeyAndSpace() {
        // ads.jpg：五笔只改第三行最左与空格键面，行表/其余键面全同拼音 26 键。
        assertEquals("⇧", qwertyLeadLabel(wubi = false))
        assertEquals("拼", qwertyLeadLabel(wubi = true))
        assertEquals("空格", qwertySpaceLabel(wubi = false))
        assertEquals("五笔", qwertySpaceLabel(wubi = true))
        val rows = qwertyRows(showNumberRow = false)
        assertEquals("⇧", rows[2].first())
        assertEquals("⌫", rows[2].last())
    }

    @Test
    fun wubiLeadKeyAndSpaceAreRenderedThroughTheHelpers() {
        val src = File("src/main/kotlin/xyz/erik/opi/keyboard/QwertyKeyboard.kt").readText()
        // 光有纯函数不算：键面必须真用它们，否则「拼/五笔」永远不上屏。
        assertTrue("第三行最左键面必须走 qwertyLeadLabel", src.contains("qwertyLeadLabel(wubi)"))
        assertTrue(
            "空格键面必须走 qwertySpaceLabel",
            src.contains("spaceLabel = qwertySpaceLabel(wubi)"),
        )
        assertTrue(
            "五笔态「拼」必须切拼音，而不是沿用 ⇧",
            src.contains("onTap = if (wubi) onSwitchToPinyin else onShift"),
        )
    }

    @Test
    fun wubiIsWiredToSwitchBackToPinyin26() {
        val src = File("src/main/kotlin/xyz/erik/opi/ime/ImeScreen.kt").readText()
        assertTrue("五笔盘必须标记 wubi", src.contains("wubi = state.inputMode == InputMode.WUBI"))
        assertTrue(
            "「拼」键必须切回拼音 26 键",
            src.contains("onSwitchToPinyin = { state.selectInputMode(InputMode.PINYIN26) }"),
        )
    }
}
