// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 状态变化事件（2026-10-25）：候选窗进程 → 状态栏进程的**即时**唤醒。
//!
//! 状态栏原先是 1s 轮询 + 连续 3 拍读失败才隐藏；用户报「切换其他输入法时状态栏
//! 隐藏太慢」（2026-10-25）。TSF 客户端连上/断开候选管道时，候选窗进程立刻
//! `SetEvent`；状态栏线程 `WaitForSingleObject` 醒来，马上补读一次状态 ——
//! 不再等 1s 轮询，也不再走 3 拍宽限（无客户端时控制面返回 `im=` 空状态，
//! 见 ControlPlane 的 `controlNoClientState`）。
//!
//! 用命名事件而不是又一条管道：只传「有变化」这一位，状态仍由既有控制面读取。
//! `Local\` 前缀 = 同一登录会话可见，不跨会话。两个进程谁先启动都行：
//! `CreateEvent` 对已存在的同名对象返回它的句柄。

package xyz.erik.opi.candidate

import com.sun.jna.Library
import com.sun.jna.Native
import com.sun.jna.platform.win32.Kernel32
import com.sun.jna.platform.win32.WinBase
import com.sun.jna.platform.win32.WinNT
import kotlin.concurrent.thread

/** 会话内唯一的事件名；与状态栏/候选窗两侧共用。 */
internal const val STATUS_EVENT_NAME = "Local\\opi-state-changed-v2"

private const val WAIT_OBJECT_0 = 0
private const val WAIT_TIMEOUT = 258

/** 创建或打开事件（自动复位、初始无信号）。失败返回 null（调用方退化为轮询）。 */
internal fun createStatusEvent(): WinNT.HANDLE? = try {
    val h = Kernel32.INSTANCE.CreateEvent(null, false, false, STATUS_EVENT_NAME)
    if (h == null || h == WinBase.INVALID_HANDLE_VALUE) null else h
} catch (_: Throwable) {
    null
}

/**
 * 候选窗进程用：单例句柄 + signal()。首次 signal 时创建；失败后每次重试，
 * 事件对象被另一个进程持有时也能打开。
 */
internal object StatusEvent {
    private var handle: WinNT.HANDLE? = null

    @Synchronized
    fun signal() {
        if (handle == null) handle = createStatusEvent()
        val h = handle ?: return // 事件暂不可用：下一次接入/断开再试
        try {
            if (!Kernel32.INSTANCE.SetEvent(h)) {
                Kernel32.INSTANCE.CloseHandle(h)
                handle = null
            }
        } catch (_: Throwable) {
            handle = null
        }
    }
}

/**
 * 状态栏进程用：后台线程等事件，每次信号调 [onChange]（在事件线程上执行，
 * 调用方只该做 `trySend`/置状态，别做 IO）。超时 1s 醒来一次，句柄失效时重建。
 */
internal class StatusEventWatcher(private val onChange: () -> Unit) {
    fun start() {
        if (!isWindows()) return
        thread(isDaemon = true, name = "opi-status-events") { loop() }
    }

    private fun loop() {
        var handle: WinNT.HANDLE? = createStatusEvent()
        while (true) {
            val h = handle
            if (h == null) {
                Thread.sleep(500)
                handle = createStatusEvent()
                continue
            }
            try {
                when (Kernel32.INSTANCE.WaitForSingleObject(h, 1000)) {
                    WAIT_OBJECT_0 -> onChange()
                    WAIT_TIMEOUT -> {} // 只是一次心跳，状态仍由 1s 轮询兜底
                    else -> {
                        Kernel32.INSTANCE.CloseHandle(h)
                        handle = null
                    }
                }
            } catch (_: Throwable) {
                handle = null
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 高精度定时器：候选管道读循环靠 Thread.sleep(2) 轮询，Windows 默认计时器粒度
// 会把 2ms 合并成 ~15.6ms（见 PipeServer.kt 的 READ_POLL_MS 注释）。IME 的
// 按键→候选窗路径要压到 10ms 内，必须把粒度降到 1ms。只影响本进程的等待。
// ---------------------------------------------------------------------------

interface Winmm : Library {
    fun timeBeginPeriod(period: Int): Int
    fun timeEndPeriod(period: Int): Int
}

private val winmm: Winmm? by lazy {
    if (!isWindows()) null
    else try {
        Native.load("winmm", Winmm::class.java)
    } catch (_: Throwable) {
        null
    }
}

/** 启动时调用一次；进程退出时 OS 回收，不依赖 timeEndPeriod 配对。 */
internal fun beginHighResolutionTimers() {
    try {
        winmm?.timeBeginPeriod(1)
    } catch (_: Throwable) {
        // 拿不到高精度也不致命：只是候选窗可能慢一个计时器粒度。
    }
}
