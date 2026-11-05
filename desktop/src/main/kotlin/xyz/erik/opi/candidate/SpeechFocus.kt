// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 语音上屏的目标窗口。点话筒时记下前台 HWND；若录音浮层/状态栏把自己变成了
//! 前台（某些 Windows 窗口管理器不认 focusable=false），上屏前把焦点还给输入框。
//! 只在“前台仍是我们自己的进程”时恢复 —— 用户主动切到别的应用就不抢。

package xyz.erik.opi.candidate

import com.sun.jna.platform.win32.Kernel32
import com.sun.jna.platform.win32.User32
import com.sun.jna.platform.win32.WinDef
import com.sun.jna.ptr.IntByReference

internal object SpeechFocus {
    @Volatile
    private var target: WinDef.HWND? = null

    private val myPid: Int by lazy { Kernel32.INSTANCE.GetCurrentProcessId() }

    /** 点击话筒时调（状态栏 UI 线程）：记下当时的输入框窗口。 */
    fun capture() {
        if (!isWindows()) return
        target = User32.INSTANCE.GetForegroundWindow()
    }

    /** 上屏前调：只有前台被我们自己的窗口抢走时才恢复，用户切走的窗口不动。 */
    fun restore() {
        if (!isWindows()) return
        val wanted = target ?: return
        val current = User32.INSTANCE.GetForegroundWindow() ?: return
        val pid = IntByReference()
        User32.INSTANCE.GetWindowThreadProcessId(current, pid)
        if (pid.value == myPid) User32.INSTANCE.SetForegroundWindow(wanted)
    }
}
