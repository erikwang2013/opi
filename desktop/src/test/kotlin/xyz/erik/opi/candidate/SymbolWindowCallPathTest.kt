// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **状态栏点 Ω 整进程崩的回归网**（P0，2026-10-13）。
//!
//! 崩的形态：`SymbolWindow` 有两条调用路 —— 设置窗那条在 `Window{}` 内容里
//! （`SettingsWindow.kt`），状态栏那条在 `application{}` 顶层（`StatusBar.kt` 的
//! `SymbolWindow(...)` 调用，**没有 Window 祖先**）。而 `SymbolWindow` 的窗前段落里读了
//! `LocalFocusManager.current`（`staticCompositionLocalOf`，**无默认值**）⇒ 组合即抛
//! `IllegalStateException: CompositionLocal LocalFocusManager not present` ⇒ 从
//! application 的组合里冒出去 ⇒ **整个状态栏进程退出**（用户看到的：点 Ω，条没了）。
//! 2026-10-13 :0 真机复现的日志里那两帧正是这条链：
//! `SymbolsPageKt.SymbolWindow(SymbolsPage.kt:566)` ← `StatusBarKt.StatusBarApp(StatusBar.kt:283)`，
//! 末尾 `Exception in thread "main" ... not present`，进程退出码 1。
//!
//! ⚠️ **为什么这条网是「按行序」而不是「离屏渲染」**（一代探针的教训，别再走一遍）：
//! 第一版在 `ImageComposeScene` 里组合 `SymbolWindow`，期望它抛缺 CompositionLocal ——
//! 实测**证伪**，坏树上它也绿：离屏场景自己 provide 了 `LocalFocusManager`（不抛），而
//! 同一个组合里 `Window(...)` 先抛 `IllegalStateException: Method should be called from
//! AWT event dispatch thread`（离屏场景里没有真窗）⇒ 窗前那条读**永远轮不到被观察**。
//! **离屏场景不是 `application{}` 作用域的忠实模型**：它多给了 Window 级 locals、
//! 又给不了真窗。
//!
//! ⚠️ 这条网是**静态**的：它按行序读源码，不跑组合。「窗真开起来 + 进程不死」由 :0 真
//! 桌面那一轮证（点 Ω → 符号页出现 + 进程在），两者缺一不可。

package xyz.erik.opi.candidate

import org.junit.Assert.assertTrue
import org.junit.Test
import java.nio.file.Files
import java.nio.file.Paths

class SymbolWindowCallPathTest {

    /**
     * 只有 **Window 内容**才 provide 的 locals：`application{}` 顶层（无 Window 祖先）读
     * 它们就是上面那个崩。名单宁缺毋滥 —— 加一个本来就有默认值的进来会把好代码判红。
     */
    private val windowOnlyLocals = listOf("LocalFocusManager", "LocalWindowInfo")

    /**
     * 窗前**做重活**是同一类错的另一种代价形态（2026-10-13 排查：真机点 Ω **24.6s** 才
     * 见到窗）。字形过滤要按码位问字体，本机 fontconfig 单次实测 ~2.5ms、符号页全表 5742
     * 条 ⇒ 一遍 ~7s（冷启动更久）。`Window(` 之前的活都会推迟「窗存在」这件事 ——
     * 用户看到的是「点了没反应」，与崩掉在观感上一样坏。
     */
    private val heavyBeforeWindow = listOf("SymbolCategories.filter(", "EmojiData.filter(", "GlyphCoverage.")

    /**
     * 纯函数式判据：某文件里 `Window(` 调用行**之前**的每一行。
     *
     * 识别口径（有意从简）：调用行 = 去缩进后以 `Window(` 开头（注释里提 Window 不算）。
     */
    private fun preWindow(lines: List<String>): List<IndexedValue<String>> {
        val windowAt = lines.indexOfFirst { it.trimStart().startsWith("Window(") }
        assertTrue("文件里找不到 `Window(` 调用行 —— 结构变了，来修这条门禁", windowAt >= 0)
        return lines.take(windowAt).withIndex().toList()
    }

    /**
     * 窗前读 window-only local 的每一行。误报（注释里写全了 `.current`）会响，漏报
     * （读到它却不写成 `.current`）需要绕一层 helper，那时这条门禁的说明会告诉你怎么改。
     */
    private fun preWindowReads(lines: List<String>): List<String> =
        preWindow(lines)
            .filter { (_, l) -> windowOnlyLocals.any { l.contains("$it.current") } }
            .map { (i, l) -> "${i + 1}: ${l.trim()}" }

    @Test
    fun `symbols page reads window-only locals inside the Window content`() {
        val p = Paths.get("src/main/kotlin/xyz/erik/opi/candidate/SymbolsPage.kt")
        assertTrue("SymbolsPage.kt 不在了（搬家了？）：${p.toAbsolutePath()}", Files.exists(p))
        val bad = preWindowReads(Files.readAllLines(p))
        assertTrue(
            "SymbolsPage.kt 的窗前段落读了只有 Window 内容里才有的 CompositionLocal：$bad ⇒ " +
                "状态栏那条调用路（application{} 顶层，无 Window 祖先）点 Ω 即整进程崩（2026-10-13 真机复现）",
            bad.isEmpty(),
        )
    }

    /** 窗前做重活同样推迟「窗存在」：真机实测 24.6s 才见窗（用户观感 = 点了没反应）。 */
    @Test
    fun `symbols page does no glyph filtering before its Window exists`() {
        val p = Paths.get("src/main/kotlin/xyz/erik/opi/candidate/SymbolsPage.kt")
        assertTrue("SymbolsPage.kt 不在了（搬家了？）：${p.toAbsolutePath()}", Files.exists(p))
        val bad = preWindow(Files.readAllLines(p))
            .filter { (_, l) -> heavyBeforeWindow.any { l.contains(it) } }
            .map { (i, l) -> "${i + 1}: ${l.trim()}" }
        assertTrue(
            "SymbolsPage.kt 在 `Window(` 之前跑了字形过滤：$bad ⇒ 窗要等这几千次字体查询跑完" +
                "才建得出来（2026-10-13 真机实测：点 Ω 到窗出现 24.6s）",
            bad.isEmpty(),
        )
    }

    /** 条本体同一条规矩：`StatusBarApp` 的窗前段落也不许读（它就在 `application{}` 顶层）。 */
    @Test
    fun `status bar app reads window-only locals inside its Window content`() {
        val p = Paths.get("src/main/kotlin/xyz/erik/opi/candidate/StatusBar.kt")
        assertTrue("StatusBar.kt 不在了（搬家了？）：${p.toAbsolutePath()}", Files.exists(p))
        val bad = preWindowReads(Files.readAllLines(p))
        assertTrue("StatusBar.kt 的窗前段落读了 window-only CompositionLocal：$bad", bad.isEmpty())
    }
}
