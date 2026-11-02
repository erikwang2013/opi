// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.asr

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import xyz.erik.opi.keyboard.MIC_SPACE_DESCRIPTION
import xyz.erik.opi.keyboard.keyClickAllowed
import xyz.erik.opi.keyboard.qwertyBottomKeys
import xyz.erik.opi.keyboard.qwertySpaceLabel
import xyz.erik.opi.keyboard.spokenKeyName
import java.nio.file.Files
import java.nio.file.Paths

/**
 * 接线门禁：参数有默认值，漏接一个键盘照样编译通过。
 * 行为在 [AsrStateTest]，这里只钉调用形状、六种空格键、长按抑制与生命周期停车。
 */
class AsrWiringTest {

    private fun moduleFile(rel: String): String {
        val p = Paths.get(rel)
        assertTrue("源文件不在了（搬家了？）：$rel", Files.exists(p))
        return String(Files.readAllBytes(p), Charsets.UTF_8)
    }

    private fun keyboardFile(name: String): String =
        moduleFile("src/main/kotlin/xyz/erik/opi/keyboard/$name")

    @Test
    fun everySpaceKeyShowsTheMicEntry() {
        val trueKeys = mapOf(
            "NineKeyKeyboard.kt" to 1,
            "FourteenKeyKeyboard.kt" to 1,
            "StrokeKeyboard.kt" to 1,
            "NumberPad.kt" to 1,
        )
        for ((file, count) in trueKeys) {
            assertEquals(
                "$file 的空格键必须接话筒",
                count,
                Regex("mic = true").findAll(keyboardFile(file)).count(),
            )
        }
        val tiled = moduleFile("src/main/kotlin/xyz/erik/opi/ime/CandidateTiledView.kt")
        assertEquals(1, Regex("mic = true").findAll(tiled).count())

        val qwerty = keyboardFile("QwertyKeyboard.kt")
        assertEquals(
            "26 键底行（中/英两种布局）都要接话筒",
            2,
            Regex("mic = micSpace").findAll(qwerty).count(),
        )
        assertTrue(
            "紧凑调用（搜索叠盘）默认不接话筒：只按 onSymbol 判定",
            qwerty.contains("micSpace: Boolean = onSymbol != null"),
        )
    }

    @Test
    fun searchOverlayKeepsSpaceOnly() {
        val src = moduleFile("src/main/kotlin/xyz/erik/opi/ime/ImeScreen.kt")
        val searchCall = src.indexOf("onSpace = state::searchSpace")
        assertTrue(searchCall >= 0)
        val block = src.substring(searchCall, (searchCall + 400).coerceAtMost(src.length))
        assertFalse("搜索叠盘的长按不能进录音", block.contains("micSpace"))
    }

    @Test
    fun keyButtonLongPressSuppressesTheSpaceClick() {
        val key = keyboardFile("KeyButton.kt")
        assertTrue("长按话筒必须有触觉反馈", key.contains("HapticFeedbackType.LongPress"))
        assertTrue("长按由独立观察器接管", key.contains("val micModifier"))
        assertTrue("抬手点击要查抑制标记", key.contains("keyClickAllowed(press.fired, altPress.fired)"))
        assertTrue("长按时不再走旧 onLongPress", key.contains("micEntry != null) null else onLongPress"))
        assertTrue("键面画自绘话筒", key.contains("OpiIcons.Mic"))
        assertTrue("读屏说明", key.contains("MIC_SPACE_DESCRIPTION"))

        assertTrue(keyClickAllowed(false, false))
        assertFalse("长按后不能再发空格", keyClickAllowed(true, false))
        assertFalse("备选弹层接管后不能再发", keyClickAllowed(false, true))
    }

    @Test
    fun spaceLabelAndAccessibilityStayCorrect() {
        assertTrue(qwertyBottomKeys(english = true).any { it.label == "空格" })
        assertTrue(qwertyBottomKeys(english = false).any { it.label == "空格" })
        assertEquals("空格", qwertySpaceLabel(wubi = false))
        assertEquals("空格", spokenKeyName("空格"))
        assertEquals("空格，长按语音输入", MIC_SPACE_DESCRIPTION)
    }

    @Test
    fun imeStopsRecordingOnEveryLifecycleEdgeAndCommitsThroughRetry() {
        val src = moduleFile("src/main/kotlin/xyz/erik/opi/OpiImeService.kt")
        assertTrue("结果必须走 commitWithRetry", src.contains("commit = { text ->"))
        assertTrue("提交前清掉拼音组合", src.contains("engineController.clear()"))
        assertTrue(src.contains("commitWithRetry(text)"))

        fun block(marker: String, span: Int = 600): String {
            val i = src.indexOf(marker)
            assertTrue("找不到 $marker", i >= 0)
            return src.substring(i, (i + span).coerceAtMost(src.length))
        }
        assertTrue(block("override fun onStartInput").contains("AsrRuntime.onEditorChanged()"))
        assertTrue(block("override fun onFinishInput()").contains("AsrRuntime.onEditorChanged()"))
        assertTrue(block("override fun onFinishInputView").contains("AsrRuntime.onEditorChanged()"))
        assertTrue(block("override fun onWindowHidden").contains("AsrRuntime.cancel()"))
        assertTrue(block("override fun onDestroy").contains("AsrRuntime.detach()"))
    }

    @Test
    fun warmRunsInBackgroundSilentlyAndOnlyForReadyModels() {
        val src = moduleFile("src/main/kotlin/xyz/erik/opi/asr/AsrRuntime.kt")
        val start = src.indexOf("fun warm()")
        val end = src.indexOf("fun start()", start)
        assertTrue("AsrRuntime 找不到 warm()", start >= 0 && end > start)
        val warm = src.substring(start, end)
        assertTrue("预热必须投到后台线程", warm.contains("worker.execute"))
        assertTrue("失败必须吞掉（点话筒时再报）", warm.contains("runCatching"))
        assertTrue("只预热已落地模型（完整包解压留给首次长按）", warm.contains("AsrModels.packReady"))
        assertTrue("同语言已装载即返回", warm.contains("loadedKey =="))
        assertTrue("静默装载：不给 UI 发事件", warm.contains("ensureModel(context, gen = null"))
        for (forbidden in listOf("asrGate(", "AsrPermissionActivity", "openRecorder", "micGranted(")) {
            assertFalse("warm 不能弹权限/录音：$forbidden", warm.contains(forbidden))
        }
    }

    @Test
    fun imeWarmsOnViewCreationEditorChangeAndWindowShow() {
        val svc = moduleFile("src/main/kotlin/xyz/erik/opi/OpiImeService.kt")
        val startInput = svc.substring(svc.indexOf("override fun onStartInput"), svc.indexOf("override fun onFinishInput"))
        assertTrue("onStartInput 要预热", startInput.contains("AsrRuntime.warm()"))
        val windowShown = svc.substring(svc.indexOf("override fun onWindowShown"), svc.indexOf("override fun onWindowHidden"))
        assertTrue("onWindowShown 要预热", windowShown.contains("AsrRuntime.warm()"))
        val host = moduleFile("src/main/kotlin/xyz/erik/opi/asr/AsrOverlay.kt")
        assertTrue("键盘视图创建时预热", host.contains("LaunchedEffect(Unit) { AsrRuntime.warm() }"))
    }

    @Test
    fun manifestDeclaresOnlyTheMicPermissionAndTransparentGate() {
        val manifest = moduleFile("src/main/AndroidManifest.xml")
        assertEquals(
            "RECORD_AUDIO 只声明一次",
            1,
            Regex("android.permission.RECORD_AUDIO").findAll(manifest).count(),
        )
        assertTrue(manifest.contains("android:name=\".asr.AsrPermissionActivity\""))
    }
}
