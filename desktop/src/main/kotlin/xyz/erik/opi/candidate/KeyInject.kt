// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 按键注入：把虚拟键盘的一次点击变成**一次真实的系统按键**。
//!
//! · Linux/X11 = XTEST（`XTestFakeKeyEvent`），经 JNA 直映系统库
//!   `libX11`/`libXtst`（不引新依赖，见 CLAUDE.md 的依赖纪律）。
//! · Windows = `SendInput`（jna-platform 的 User32）。
//! · **Wayland 原生不支持**：协议不允许客户端合成输入（这是设计，不是缺库、不是
//!   本版偷懒）；XWayland 下注入能到 X 应用，原生 Wayland 窗口收不到。如实不支持。
//!
//! 注入走系统输入队列 ⇒ **会照常流经用户当前的输入法**（中文态下打字母出候选、
//! 空格上屏）—— 这正是要的：虚拟键盘就该等价于按真键盘，本模块**不试图绕过** IME。
//!
//! 【取证状态】**别把「编译过」读成「能用」**：
//! · Linux 侧**已实测**：本机真连 X11、`libX11`/`libXtst` 真的装载成功（探针走的就是
//!   [openX11] 同一条代码），且 `XStringToKeysym`/`XKeysymToKeycode` 对**键表里每一个
//!   键**都解出了键码 —— 这是「键表里有没有错字」唯一的行为级证据
//!   （VirtualKeyboardTest 的探针，见 [x11KeycodeFor]）。
//! · Linux 侧**未实测**：`XTestFakeKeyEvent`/`XFlush` 那两行本身。本机是真有 X 会话的
//!   开发机，发一次按键就是往用户当时聚焦的窗口里打字 —— 这不是自动测试该有的副作用，
//!   要补这条证据请在自己的 Xvfb 上跑（xev 收键）。
//! · Windows 侧只过编译（本机没有 Windows），`injectWindows` **未经真机运行**。

package xyz.erik.opi.candidate

import com.sun.jna.Library
import com.sun.jna.Native
import com.sun.jna.Pointer
import com.sun.jna.platform.win32.User32
import com.sun.jna.platform.win32.WinDef
import com.sun.jna.platform.win32.WinUser

/**
 * 注入一次按键事件。返回 null = 已递交；非 null = **给用户看的**失败短语
 * （虚拟键盘把它显示在底部，不静默 —— 静默的注入失败查起来是玄学）。
 */
fun injectKey(key: VkKey, down: Boolean): String? =
    if (isWindows()) injectWindows(key, down) else injectX11(key, down)

// ---------- 自有表情：Ctrl+V ----------

private val VK_CTRL = VkKey("Ctrl", "Control_L", 0x11)
private val VK_V = VkKey("V", "v", 0x56)

/** 粘贴的事件序列（纯函数，JVM 测试钉键码与按下/抬起顺序）。 */
internal fun pasteKeySequence(): List<VkEvent> = listOf(
    VkEvent(VK_CTRL, true),
    VkEvent(VK_V, true),
    VkEvent(VK_V, false),
    VkEvent(VK_CTRL, false),
)

/**
 * 替用户按一次 Ctrl+V（自有表情：图片已在剪贴板里）。返回 null = 已递交；
 * 非 null = 给用户看的失败原因 —— 调用方据此提示「已复制，请手动粘贴」，不许报成功。
 *
 * Wayland 会话直接不注入：原生窗口收不到 XTEST，而 `DISPLAY` 往往只是 XWayland ——
 * 注入会「递交成功」却到不了目标，报「已粘贴」就是假成功。图已在剪贴板，手动一下即可。
 */
fun injectPaste(): String? {
    if (!isWindows() && !System.getenv("WAYLAND_DISPLAY").isNullOrEmpty()) {
        return "Wayland 会话下按键注入可能到不了目标窗口"
    }
    for ((i, ev) in pasteKeySequence().withIndex()) {
        val failure = injectKey(ev.key, down = ev.down)
        if (failure != null) {
            // 已经按下的修饰键必须抬起来，别把用户的 Ctrl 卡住（失败原因仍报首因）。
            if (i > 0) {
                injectKey(VK_CTRL, down = false)
                injectKey(VK_V, down = false)
            }
            return failure
        }
    }
    return null
}

// ---------- Linux/X11：XTEST ----------

/**
 * Xlib 里要用的函数（JNA 直映，用一个映一个，不多映）。
 *
 * `XKeysymToKeycode` 是**依赖当前键盘布局**的一步：同一 keysym 在 us/ru 布局下
 * 可能映不到（返回 NoSymbol = 0）—— 那种情况下如实报错，而不是注入另一个键。
 */
internal interface X11Api : Library {
    fun XOpenDisplay(name: String?): Pointer?
    fun XFlush(dpy: Pointer): Int
    fun XStringToKeysym(name: String): Long
    fun XKeysymToKeycode(dpy: Pointer, keysym: Long): Byte
}

/** XTEST 扩展：本模块只用到这一个导出。 */
internal interface XtstApi : Library {
    fun XTestFakeKeyEvent(dpy: Pointer, keycode: Int, isPress: Int, delay: Long): Int
}

/** X 会话状态：连上（复用 Display）或连不上（带一句给用户看的原因，缓存不重试）。 */
private sealed interface X11 {
    class Ready(val dpy: Pointer, val x11: X11Api, val xtst: XtstApi) : X11
    class Down(val reason: String) : X11
}

/**
 * 开一次 X 会话并**复用**（`lazy`：只试一次，失败也缓存 —— 每次点击重开一遍没意义）。
 *
 * 为什么复用 Display 而不是每次注入开关一次：`XCloseDisplay` 会丢掉**本连接**里还没
 * 被服务器处理的请求，而注入正是「发出去就不管」的异步请求 —— 开关式写法会随机丢
 * 事件（表现「点了没反应」，机器越快越容易复现）。代价是一条常驻连接，本进程本就常驻。
 *
 * JNA 装载写**基名**（`X11`/`Xtst`）就是对的：只装运行时的机器上只有 `libXtst.so.6`，
 * 没有 -dev 的 `.so` 链接；JNA 5.x 正常装载失败后会按 `libfoo.so.*` 做版本变体匹配。
 * **别**改成硬编 soname 或加 fallback 链 —— 那是多写的代码。
 */
private fun openX11(): X11 = try {
    val x11 = Native.load("X11", X11Api::class.java)
    val xtst = Native.load("Xtst", XtstApi::class.java)
    val dpy = x11.XOpenDisplay(null)
    if (dpy == null) {
        X11.Down("连不上 X 显示：本版不支持 Wayland 原生注入（X11/XWayland 下可用）")
    } else {
        X11.Ready(dpy, x11, xtst)
    }
} catch (t: Throwable) {
    X11.Down("系统缺 X11 注入库：${t.message?.take(80) ?: t.javaClass.simpleName}")
}

private val x11: X11 by lazy { openX11() }

/**
 * 键 → 本机布局下的物理键码（null = 解不出来：没库/没显示/布局里没有这个键）。
 *
 * **不注入**：测试经这一条在本机真跑 Xlib 的键名解析与布局映射（与 [injectX11] 的
 * 同一段代码，少掉的只有最后两条 XTest 调用），从而不必真往用户桌面打字就能钉住键表。
 */
internal fun x11KeycodeFor(key: VkKey): Int? {
    val s = x11 as? X11.Ready ?: return null
    val keysym = s.x11.XStringToKeysym(key.keysym)
    if (keysym == 0L) return null
    // XKeysymToKeycode 返回 unsigned char：JNA 映成 byte，先 & 0xFF 才是键码。
    val code = s.x11.XKeysymToKeycode(s.dpy, keysym).toInt() and 0xFF
    return if (code == 0) null else code
}

private fun injectX11(key: VkKey, down: Boolean): String? {
    (x11 as? X11.Down)?.let { return it.reason }
    val code = x11KeycodeFor(key) ?: return "当前键盘布局没有这个键（${key.label}）"
    val s = x11 as X11.Ready
    s.xtst.XTestFakeKeyEvent(s.dpy, code, if (down) 1 else 0, 0L)
    // Xlib 有发送缓冲：不 flush 就只是排在本进程里，用户那边一直没反应。
    s.x11.XFlush(s.dpy)
    return null
}

// ---------- Windows：SendInput ----------

/**
 * ⚠️ **本机无 Windows，这段只过编译，未在真机跑过**（别把编译通过说成能用）。
 *
 * 结构形态与 jna-platform 自己的用例同形：`INPUT.type = INPUT.INPUT_KEYBOARD`，联合体必须
 * `setType("ki")`（不选字段则默认写 `mi`，发出去的是鼠标事件），`wVk` = 虚拟键码，
 * `dwFlags & KEYEVENTF_KEYUP` 表示抬起。
 */
private fun injectWindows(key: VkKey, down: Boolean): String? = try {
    val input = WinUser.INPUT()
    // 常量在 INPUT 上（不是 WinUser 上）；nInputs/返回值都是 DWORD —— 都是 javap 核过的
    // 5.6.0 形态，不是凭印象写的。
    input.type = WinDef.DWORD(WinUser.INPUT.INPUT_KEYBOARD.toLong())
    input.input.setType("ki")
    input.input.ki.wVk = WinDef.WORD(key.vk.toLong())
    input.input.ki.wScan = WinDef.WORD(0L)
    input.input.ki.time = WinDef.DWORD(0L)
    input.input.ki.dwFlags =
        WinDef.DWORD(if (down) 0L else WinUser.KEYBDINPUT.KEYEVENTF_KEYUP.toLong())
    val n = User32.INSTANCE.SendInput(WinDef.DWORD(1L), arrayOf(input), input.size()).toInt()
    if (n == 1) null else "系统拒绝了按键注入（SendInput 返回 $n）"
} catch (t: Throwable) {
    "按键注入失败：${t.javaClass.simpleName}"
}
