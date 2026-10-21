// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 状态栏崩溃取证（2026-10-22 用户报障「使用一段时间后语言栏间歇性崩溃」）。
//!
//! 目的只有一个：**下次真的崩时留下证据**，而不是靠「重启 fcitx5」把现场抹掉。
//! 默认路径 = `~/.local/state/opi/statusbar-crash.log`（遵循 `XDG_STATE_HOME`），
//! 同时原样打一份到 stderr（自启动进程的 stderr 在 journal 里）。
//!
//! 两类记录：
//!  1. 进程级未捕获异常（AWT EDT、Compose 协程、后台线程）—— [installStatusBarCrashLogger]；
//!  2. 已知会抛异常的边界（轮询、按钮动作、符号页过滤）—— [statusBarGuard]，**吞掉之前
//!     先记日志**，让 1s 轮询/后续点击继续活着，而不是整进程退出。
//!
//! 日志本身**绝不再抛**：写文件失败只剩 stderr 一份；不能为了取证把进程再弄崩一次。

package xyz.erik.opi.candidate

import kotlinx.coroutines.CancellationException
import java.io.File
import java.io.PrintWriter
import java.io.StringWriter
import java.time.Instant

private val crashLogLock = Any()

/** 日志落点：`$XDG_STATE_HOME/opi/statusbar-crash.log`，缺省 `~/.local/state/...`。 */
internal fun statusBarCrashLogPath(): File {
    val stateHome = System.getenv("XDG_STATE_HOME")?.takeIf { it.isNotBlank() }
        ?: ((System.getProperty("user.home") ?: ".") + "/.local/state")
    return File(stateHome, "opi/statusbar-crash.log")
}

private fun currentPid(): Long = try {
    ProcessHandle.current().pid()
} catch (_: Throwable) {
    -1L
}

/** 一行事件（启动/退出/重复实例）——也给 journal 留时间线。 */
internal fun logStatusBarEvent(message: String) {
    appendStatusBarLog("${Instant.now()} pid=${currentPid()} $message")
}

/** 异常：类型 + 消息 + 完整栈 + 发生线程；写 stderr 与文件。 */
internal fun logStatusBarCrash(label: String, t: Throwable,
                               threadName: String = Thread.currentThread().name) {
    val stack = StringWriter().also { t.printStackTrace(PrintWriter(it)) }.toString()
    appendStatusBarLog(
        "${Instant.now()} pid=${currentPid()} thread=$threadName " +
            "label=$label error=${t.javaClass.name}: ${t.message}\n$stack",
    )
}

private fun appendStatusBarLog(text: String) {
    // stderr 先写：即使状态目录不可写，journal 里也有证据。
    try {
        System.err.print(text.trimEnd() + "\n")
    } catch (_: Throwable) {
        // stderr 都写不了（已关闭/重定向坏）—— 只能继续尝试文件。
    }
    synchronized(crashLogLock) {
        try {
            val file = statusBarCrashLogPath()
            file.parentFile?.mkdirs()
            file.appendText(text.trimEnd() + "\n")
        } catch (_: Throwable) {
            // 取证失败不能再抛，否则又制造一次崩溃。
        }
    }
}

/**
 * 安装进程级取证：启动/退出时间线 + 未捕获异常处理器。
 * 只应在 `--statusbar` 分支调用一次（[Main]）。
 */
internal fun installStatusBarCrashLogger() {
    logStatusBarEvent("statusbar start version=$OPI_VERSION log=${statusBarCrashLogPath()}")
    Runtime.getRuntime().addShutdownHook(Thread {
        logStatusBarEvent("statusbar exit")
    })
    val previous = Thread.getDefaultUncaughtExceptionHandler()
    Thread.setDefaultUncaughtExceptionHandler { thread, t ->
        if (t !is CancellationException) {
            logStatusBarCrash("uncaught", t, thread.name)
        }
        // 交回原处理器（通常是 JVM 默认打印）；日志里已经有一份完整栈。
        previous?.uncaughtException(thread, t)
    }
}

/**
 * 已知边界的异常护栏：记日志后返回 null，让调用方继续活着；协程取消与 VM 错误照旧抛出。
 *
 * `inline` 是必需的 —— 调用点会把 `withContext`/挂起函数直接写在 [block] 里。
 */
internal inline fun <T> statusBarGuard(label: String, block: () -> T): T? =
    try {
        block()
    } catch (e: CancellationException) {
        throw e
    } catch (e: VirtualMachineError) {
        throw e
    } catch (t: Throwable) {
        logStatusBarCrash(label, t)
        null
    }
