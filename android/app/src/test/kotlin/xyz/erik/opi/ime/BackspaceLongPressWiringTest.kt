// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.nio.file.Files
import java.nio.file.Paths

/**
 * 长按退格的**接线**门禁（2026-10-23「按住连续删除」）。
 *
 * 行为本身在 [KeyRouterLongPressTest]（每拍一次普通退格）；这里只钉两件事：
 * 每个带 ⌫ 的键面都开了 `repeatOnLongPress`，以及没有任何地方再回到
 * 一次性 `clear()` 的旧长按入口。漏接一个键盘照样编译通过（参数有默认值），
 * 只能靠静态检查兜住。
 *
 * 断言按**调用形状**而非行号（行号会随任何编辑漂）。
 */
class BackspaceLongPressWiringTest {

    private fun moduleFile(rel: String): String {
        val p = Paths.get(rel)
        assertTrue("源文件不在了（搬家了？）：$rel", Files.exists(p))
        return String(Files.readAllBytes(p), Charsets.UTF_8)
    }

    private fun keyboardFile(name: String): String =
        moduleFile("src/main/kotlin/xyz/erik/opi/keyboard/$name")

    @Test
    fun everyKeyboardEnablesRepeatOnLongPressForBackspace() {
        // QwertyKeyboard 有两个 ⌫（英文 pro 底行 / 中文底行），其余各一个。
        val expectedKeys = mapOf(
            "QwertyKeyboard.kt" to 2,
            "NineKeyKeyboard.kt" to 1,
            "FourteenKeyKeyboard.kt" to 1,
            "StrokeKeyboard.kt" to 1,
            "NumberPad.kt" to 1,
        )
        for ((file, keyCount) in expectedKeys) {
            val src = keyboardFile(file)
            assertEquals(
                "$file 的每个 ⌫ 都必须开 repeatOnLongPress",
                keyCount,
                Regex("repeatOnLongPress = true").findAll(src).count(),
            )
            assertFalse(
                "$file 不能再挂旧的一次性清组合长按",
                src.contains("onBackspaceLongPress"),
            )
        }
    }

    @Test
    fun imeScreenNoLongerWiresClearAllLongPress() {
        val src = moduleFile("src/main/kotlin/xyz/erik/opi/ime/ImeScreen.kt")
        assertFalse(
            "长按重复由 KeyButton 自己驱动，ImeScreen 不应再有清组合路由",
            src.contains("handleBackspaceLongPress"),
        )
        // 搜索叠盘是搜索框文本，不是引擎组合：不许开连续删。
        val searchCall = src.indexOf("onBackspace = state::searchBackspace")
        assertTrue(searchCall >= 0)
        val searchBlock = src.substring(searchCall, (searchCall + 400).coerceAtMost(src.length))
        assertFalse(
            "搜索叠盘不许开连续删",
            searchBlock.contains("repeatOnLongPress"),
        )
    }

    @Test
    fun tiledCandidateViewUsesTheSameRepeatGesture() {
        val src = moduleFile("src/main/kotlin/xyz/erik/opi/ime/CandidateTiledView.kt")
        assertTrue(src.contains("repeatOnLongPress = true"))
        assertTrue("候选删空要自动收拢回键盘", src.contains("collapseIfNoCandidates"))
    }

    @Test
    fun keyButtonDrivesRepeatsAndRouterHasNoClearAllLongPress() {
        val key = keyboardFile("KeyButton.kt")
        assertTrue("长按阈值必须存在", key.contains("BACKSPACE_LONG_PRESS_MS"))
        assertTrue("重复间隔必须存在", key.contains("BACKSPACE_REPEAT_INTERVAL_MS"))
        assertTrue("必须有抬手观察器", key.contains("awaitReleaseOrCancel"))
        val router = moduleFile("src/main/kotlin/xyz/erik/opi/ime/KeyRouter.kt")
        assertFalse("旧的一次性清组合入口必须删掉", router.contains("handleBackspaceLongPress"))
        assertTrue("每拍仍是普通退格", router.contains("controller.backspace()"))
        assertTrue("组合空时退宿主一个字符", router.contains("deleteBackward()"))
    }
}
