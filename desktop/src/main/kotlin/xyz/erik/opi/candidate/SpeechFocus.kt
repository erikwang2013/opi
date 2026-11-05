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

    /**
     * 上屏前调：返回 true = 当前前台不是我们自己的窗口，SendInput 会落到输入框。
     * 前台被浮层/状态栏抢走时尝试恢复点话筒前的目标窗；恢复失败返回 false，
     * 调用方必须改走 TSF —— 否则 SendInput 会“成功”打进我们自己的窗口。
     * 用户主动切到别的应用时，把那个应用记为新的目标（不抢回旧窗）。
     */
    fun prepare(): Boolean {
        if (!isWindows()) return true
        val current = User32.INSTANCE.GetForegroundWindow() ?: return true
        val pid = IntByReference()
        User32.INSTANCE.GetWindowThreadProcessId(current, pid)
        if (pid.value != myPid) {
            target = current
            return true
        }
        val wanted = target ?: return false
        if (wanted == current) return false
        return User32.INSTANCE.SetForegroundWindow(wanted)
    }
}
