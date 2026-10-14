// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.nio.file.Files
import java.nio.file.Paths

/**
 * **拼音去哪显示**的静态门禁（2026-10-15 用户裁决「移动端的拼音在候选词上方显示，
 * 不要在输入框里」—— 对 2026-10-10「组合串进字段」那条的改判）。
 *
 * 为什么是静态（读源码）而不是行为断言：这两条判据是**布局次序**与**某条调用不存在**，
 * 都在 Compose/服务层，本仓的 JVM 单测够不着（没有 Robolectric、没有 instrumentation）。
 * 静态门禁在本仓有先例（桌面 `SymbolWindowCallPathTest` 的窗前不许读 window-only locals）。
 * 它证明的是「代码仍然这么写」，**不**证明「真机上眼睛看到的是这样」。
 *
 * ⚠️ 断言按**调用形状**（`ComposingBar(controller)` / `CandidateBar(`）而不是行号 ——
 * 行号会随任何编辑漂（本仓文档引用同理，见 doc-citations 那条纪律）。
 */
class ComposingPlacementTest {

    private fun moduleFile(rel: String): String {
        val p = Paths.get(rel)
        assertTrue("源文件不在了（搬家了？）：$rel", Files.exists(p))
        // ⚠️ 用 readAllBytes + String(...) 而不是 Files.readString（Java 11 API，
        // 本模块的单测编译级别上不可见 —— 编译期就红，实测撞过）。
        return String(Files.readAllBytes(p), Charsets.UTF_8)
    }

    /** 拼音行必须在候选行**之前**出现（= 屏幕上方）。 */
    @Test
    fun composingBarSitsAboveTheCandidateBar() {
        val src = moduleFile("src/main/kotlin/xyz/erik/opi/ime/ImeScreen.kt")
        val composing = src.indexOf("ComposingBar(controller)")
        val candidates = src.indexOf("CandidateBar(")
        assertTrue(
            "两个调用都要在（缺一个门禁就失明了）：composing=$composing candidates=$candidates",
            composing >= 0 && candidates >= 0,
        )
        assertTrue("拼音行（$composing）必须在候选行（$candidates）之前", composing < candidates)
    }

    /** 拼音**不再**写进输入框：`setComposingText` 只允许以空串出现（收残留用）。 */
    @Test
    fun pinyinIsNoLongerPushedIntoTheInputField() {
        val src = moduleFile("src/main/kotlin/xyz/erik/opi/ComposingText.kt")
        assertTrue(
            "pushComposingText 里又出现了往字段写拼音的调用（裁决要求：不写）",
            src.contains("setComposingText(\"\""),
        )
        assertFalse(
            "出现 `setComposingText(text` 之类把缓冲写进字段的形态",
            Regex("""setComposingText\((?!"")\w""").containsMatchIn(src),
        )
    }
}
