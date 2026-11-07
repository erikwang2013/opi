// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 符号窗的键盘交还（Linux/X11）：点搜索框时记住当时获得焦点的 X 窗口，点符号
//! 交还键盘时把 X 焦点放回去。
//!
//! 为什么需要：符号窗平时 `focusable=false`，点符号不会抢焦点；但搜索框要打字，
//! 点它会把窗变可聚焦并拿走焦点。交还时只把窗设回不可聚焦，本机 WM 会把焦点丢给
//! root（实测 `XGetInputFocus` 返回 1），目标程序再也收不到字。这里用独立的 Xlib
//! 连接（与 KeyInject 分连接，避免和 XTest 注入互相影响），只记一个窗口 id。
//!
//! Windows 下整个对象是 no-op；Windows 的符号窗交还走 AWT 的窗口焦点（见 SymbolsPage）。

package xyz.erik.opi.candidate

import com.sun.jna.Library
import com.sun.jna.Native
import com.sun.jna.Pointer
import com.sun.jna.ptr.IntByReference
import com.sun.jna.ptr.LongByReference

private interface XFocusApi : Library {
    fun XOpenDisplay(name: String?): Pointer?
    fun XGetInputFocus(display: Pointer, focus: LongByReference, revertTo: IntByReference): Int
    fun XSetInputFocus(display: Pointer, focus: Long, revertTo: Int, time: Long): Int
    fun XFlush(display: Pointer)
}

internal object XFocus {
    private val api: XFocusApi? by lazy {
        if (isWindows()) null else runCatching { Native.load("X11", XFocusApi::class.java) }.getOrNull()
    }
    private val display: Pointer? by lazy { api?.XOpenDisplay(null) }
    private var saved: Long = 0

    /** 搜索框按下时调：记住目标程序当前的 X 焦点窗口（只记，不改焦点）。 */
    fun remember() {
        val a = api ?: return
        val d = display ?: return
        val focus = LongByReference()
        val revert = IntByReference()
        if (a.XGetInputFocus(d, focus, revert) == 0) return
        val id = focus.value
        // 0 = None、1 = PointerRoot：都不是可恢复的目标窗。
        saved = if (id > 1) id else 0
    }

    /** 点符号交还键盘时调：把焦点放回 [remember] 记下的窗口。返回是否发出恢复。 */
    fun restore(): Boolean {
        val a = api ?: return false
        val d = display ?: return false
        val id = saved
        saved = 0
        if (id <= 1) return false
        // revert_to = RevertToParent(2)；CurrentTime(0) 让服务器立刻处理。
        a.XSetInputFocus(d, id, 2, 0)
        a.XFlush(d)
        return true
    }
}
