// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.nio.file.Files
import java.nio.file.Paths

/**
 * 长按退格的**接线**门禁（任务 2026-10-23：「每一个移动端布局都要有」）。
 *
 * 行为本身在 [KeyRouterTest]（纯 JVM 假引擎）；这里只钉「布局层没漏接」——
 * 漏接一个键盘照样编译通过（参数有默认值），只能靠静态检查兜住。
 * 同仓先例：[ComposingPlacementTest] 的调用形状断言。
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

    /** 每个带 ⌫ 的键面：参数声明 + 键本身都挂上长按。 */
    @Test
    fun everyKeyboardExposesAndWiresBackspaceLongPress() {
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
            assertTrue(
                "$file 必须暴露 onBackspaceLongPress 参数（默认 null = 可选接线）",
                src.contains("onBackspaceLongPress: (() -> Unit)? = null"),
            )
            assertEquals(
                "$file 的每个 ⌫ 都必须挂 onLongPress = onBackspaceLongPress",
                keyCount,
                Regex("onLongPress = onBackspaceLongPress").findAll(src).count(),
            )
        }
    }

    /** ImeScreen 把路由器那条路接到每个键面（数字面板 + 9/14/笔画/26）。 */
    @Test
    fun imeScreenConnectsEveryTypingKeyboard() {
        val src = moduleFile("src/main/kotlin/xyz/erik/opi/ime/ImeScreen.kt")
        assertEquals(
            "数字面板 + 四个字母/笔画分支都要接 router::handleBackspaceLongPress",
            5,
            Regex("onBackspaceLongPress = router::handleBackspaceLongPress").findAll(src).count(),
        )
        // 搜索叠盘是搜索框文本，不是引擎组合：不许接清空引擎那条（否则长按可能
        // 顺着 deleteBackward 删到宿主正文）。
        val searchCall = src.indexOf("onBackspace = state::searchBackspace")
        assertTrue(searchCall >= 0)
        val searchBlock = src.substring(searchCall, (searchCall + 400).coerceAtMost(src.length))
        assertTrue(
            "搜索叠盘不许接组合清空路由",
            !searchBlock.contains("onBackspaceLongPress = router::handleBackspaceLongPress"),
        )
    }

    /** 平铺候选视图自己的 ⌫（不经键盘组件）也要挂长按。 */
    @Test
    fun tiledCandidateViewWiresLongPress() {
        val src = moduleFile("src/main/kotlin/xyz/erik/opi/ime/CandidateTiledView.kt")
        assertTrue(src.contains("router.handleBackspaceLongPress()"))
        assertTrue("长按后候选清空要自动收拢回键盘", src.contains("collapseIfNoCandidates"))
    }

    /** 路由行为形状：有组合走 clear，空组合退回单字删除（绝不「删光宿主」）。 */
    @Test
    fun routerLongPressShape() {
        val src = moduleFile("src/main/kotlin/xyz/erik/opi/ime/KeyRouter.kt")
        val start = src.indexOf("fun handleBackspaceLongPress()")
        assertTrue("KeyRouter 必须有长按入口", start >= 0)
        val body = src.substring(start, src.indexOf("\n    }", start))
        assertTrue("组合态必须走引擎 clear", body.contains("controller.clear()"))
        assertTrue("空组合退回与短按相同的单字删除", body.contains("deleteBackward()"))
    }
}
