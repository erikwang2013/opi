// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 语音上屏的目标窗口。点话筒也会把状态栏激活成前台（用户看到的「点一下话筒」），
//! 那一刻的 `GetForegroundWindow` 往往已经是状态栏自己 —— 拿它当目标只能退回 TSF，
//! 连续分段第二次起就没有可落的输入框。这里在后台采样「最近一个别的进程的前台窗」，
//! 点话筒时优先用它；录音浮层/状态栏抢走前台时再把焦点还回去。
//! 只在“前台仍是我们自己的进程”时恢复 —— 用户主动切到别的应用就不抢。

package xyz.erik.opi.candidate

import com.sun.jna.platform.win32.Kernel32
import com.sun.jna.platform.win32.User32
import com.sun.jna.platform.win32.WinDef
import com.sun.jna.ptr.IntByReference
import java.util.concurrent.atomic.AtomicReference

internal object SpeechFocus {
    @Volatile
    private var target: WinDef.HWND? = null

    /** 最近一个不属于本进程的前台窗口（点话筒前用户真正在打字的那个）。 */
    private val recentOther = AtomicReference<WinDef.HWND?>(null)

    private val myPid: Int by lazy { Kernel32.INSTANCE.GetCurrentProcessId() }

    @Volatile
    private var samplerStarted = false

    private fun pidOf(hwnd: WinDef.HWND): Int {
        val pid = IntByReference()
        User32.INSTANCE.GetWindowThreadProcessId(hwnd, pid)
        return pid.value
    }

    /** 后台采样前台窗口：状态栏被点中前，输入框通常已经在这里记下了。 */
    private fun ensureSampler() {
        if (samplerStarted) return
        synchronized(this) {
            if (samplerStarted) return
            samplerStarted = true
            Thread({
                while (true) {
                    runCatching {
                        val h = User32.INSTANCE.GetForegroundWindow()
                        if (h != null && pidOf(h) != myPid) recentOther.set(h)
                    }
                    try {
                        Thread.sleep(120L)
                    } catch (_: InterruptedException) {
                        return@Thread
                    }
                }
            }, "opi-speech-focus").apply {
                isDaemon = true
                start()
            }
        }
    }

    /** 点击话筒时调（状态栏 UI 线程）：优先记最近的非本进程前台窗。 */
    fun capture() {
        if (!isWindows()) return
        ensureSampler()
        val current = User32.INSTANCE.GetForegroundWindow()
        target = if (current != null && pidOf(current) != myPid) {
            current
        } else {
            recentOther.get() ?: current
        }
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
        if (pidOf(current) != myPid) {
            target = current
            return true
        }
        val wanted = target ?: return false
        if (wanted == current) return false
        // 目标窗口可能已经关闭：pid 取不到就别 SetForegroundWindow 一个死句柄。
        if (pidOf(wanted) == 0) return false
        return User32.INSTANCE.SetForegroundWindow(wanted)
    }
}
