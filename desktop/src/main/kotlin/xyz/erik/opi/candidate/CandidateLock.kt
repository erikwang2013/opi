// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 候选窗进程的单实例锁（2026-10-25）。
//!
//! 背景：候选窗只有无参数分支，Run 键、安装完成页和开始菜单都会拉起它；重复实例
//! 抢不到 `\\.\pipe\opi-v2` 时只是每 500ms 重试，进程本身不退出 —— 现场会看到
//! 多个 `opi.exe` 常驻（用户报「任务管理器里有 4 个 opi IME 进程」）。它们不占
//! 管道却一直活着，升级后还可能是旧版本。
//!
//! 与状态栏的锁同形（[StatusBarLock]）：OS 文件锁，进程退出即释放；拿不到就安静
//! 退出。**旧版本没有这把锁**，所以新版仍能在旧候选窗进程旁边启动（`opi-v2` 与
//! 旧版不冲突），安装器会负责结束旧进程。

package xyz.erik.opi.candidate

import java.io.RandomAccessFile
import java.nio.channels.FileLock
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths

/** 与状态栏锁同目录（XDG_RUNTIME_DIR 优先，退回临时目录），按用户名区分。 */
internal fun candidateLockPath(): Path {
    val base = System.getenv("XDG_RUNTIME_DIR")?.takeIf { it.isNotEmpty() }
        ?: System.getProperty("java.io.tmpdir")
    val user = System.getProperty("user.name") ?: "unknown"
    return Paths.get(base, "opi-candidate-$user.lock")
}

// 与 StatusBarLock 相同：锁对象必须被强引用钉住，否则 fd 被回收会意外解锁。
private var heldRaf: RandomAccessFile? = null
private var heldLock: FileLock? = null

/**
 * true = 本进程是唯一候选窗实例；false = 已有实例在跑（调用方 exitProcess(0)）。
 * 任何拿不到独占证据的异常都返回 false —— 对「我是唯一实例吗」这个问题，拿不出
 * 证据就不是。锁文件建不出来（只读 FS 等）时放行，宁可多一个候选窗也不要起不来。
 */
internal fun acquireCandidateLock(): Boolean {
    if (heldLock != null) return true
    val path = candidateLockPath()
    return try {
        Files.createDirectories(path.parent)
        val raf = RandomAccessFile(path.toFile(), "rw")
        val lock = try {
            raf.channel.tryLock()
        } catch (_: Exception) {
            null
        }
        if (lock == null) {
            raf.close()
            false
        } else {
            heldRaf = raf
            heldLock = lock
            true
        }
    } catch (_: Exception) {
        true
    }
}
