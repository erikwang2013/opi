// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.settings

import org.junit.Assert.assertTrue
import org.junit.Test
import java.nio.file.Files
import java.nio.file.Paths

/**
 * **设置页可滚动**的静态门禁（2026-10-15，v1.9.2 装机核验时发现的真缺陷）。
 *
 * 症状：这一页到「拼音纠错」为止就**裁断**了，`简拼 / 拼音方式 / 词库 / 五笔 / 关于`
 * 全在视口之外，而且**滑不动** —— 1080×2400@420dpi 实测（视口 862dp）：
 * `adb shell input swipe` 与手搓 `motionevent` 序列两种手势都不动，uiautomator 语义树里
 * 也没有那些节点。成因：外层 `Column` 从建页起就没有 `verticalScroll`，`Scaffold` 的
 * content 槽不会替你滚 —— 内容比视口长就被裁掉。
 *
 * 为什么是静态（读源码）：这是**布局修饰符是否存在**，本仓 JVM 单测够不着
 * Compose（无 Robolectric、无 instrumentation）；同一条路数的先例见
 * `ime/ComposingPlacementTest`。它证明「代码仍然这么写」，**不**证明真机上滑得动 ——
 * 那一条的证据是装机实测（本文件头注记的那次）。
 *
 * ⚠️ 按**调用形状**取，不按行号（行号随任何编辑漂）。
 */
class SettingsScrollableTest {

    @Test
    fun outerColumnIsVerticallyScrollable() {
        val rel = "src/main/kotlin/xyz/erik/opi/settings/SettingsScreen.kt"
        val p = Paths.get(rel)
        assertTrue("源文件不在了（搬家了？）：$rel", Files.exists(p))
        // ⚠️ readAllBytes 而不是 Files.readString（Java 11 API，本模块单测编译级别不可见）。
        val src = String(Files.readAllBytes(p), Charsets.UTF_8)
        assertTrue(
            "外层 Column 上又没了 verticalScroll —— 设置页下半截会重新变成看不到也滑不动",
            src.contains(".verticalScroll(rememberScrollState())"),
        )
        assertTrue(
            "verticalScroll 与 rememberScrollState 都要 import（缺一个编译就红，这条是给改名让路）",
            src.contains("import androidx.compose.foundation.verticalScroll") &&
                src.contains("import androidx.compose.foundation.rememberScrollState"),
        )
    }
}
