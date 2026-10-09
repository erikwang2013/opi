// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 状态栏悬浮条的**单实例锁 + 按需拉起**。
//!
//! 缘起（2026-10-10 用户报障「设置里开了，实际没出现」）：装包后没重登录 ⇒ autostart
//! （只在登录时跑）没触发；而设置窗打开开关**不会拉起任何进程** —— 设置窗与状态栏是
//! 两个进程，原先它们之间没有拉起通道，用户唯一的路是「重登录」或手敲命令行。实测
//! 二进制本身没问题（真桌面起过、窗口置顶、位置正确），缺的就是这一环。
//!
//! 现在两头都有依据：状态栏进程启动时拿一把**OS 级独占锁**（文件锁，进程退出即释放）；
//! 设置窗在「条该显示」时直接拉起一支 `--statusbar` —— 重复的会自己在锁上安静退出。
//!
//! ⚠️ 探测（[statusBarRunning]）是「试拿一次锁」：**拿不到 = 有人在跑**。两个方向都
//! 想过：假「在跑」（文件系统不支持锁）⇒ 设置窗不拉起，用户还有 autostart 那条路；
//! 假「没跑」（探测与另一实例抢锁的毫秒级窗口）⇒ 拉起一支，锁上再分胜负，最坏多一次
//! JVM 启动。两个方向都不致命，所以不复用更花的机制。

package xyz.erik.opi.candidate

import java.io.RandomAccessFile
import java.nio.channels.FileLock
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths

/** 锁文件落点：运行时状态优先 XDG_RUNTIME_DIR，退回临时目录；按用户名区分多用户。 */
internal fun statusBarLockPath(): Path {
    val base = System.getenv("XDG_RUNTIME_DIR")?.takeIf { it.isNotEmpty() }
        ?: System.getProperty("java.io.tmpdir")
    val user = System.getProperty("user.name") ?: "unknown"
    return Paths.get(base, "opi-statusbar-$user.lock")
}

// 持锁引用：FileLock 本身不防 GC —— 若 RandomAccessFile 先被回收、fd 关闭，锁会**意外
// 释放**（然后第二条状态栏就能起来了）。挂在这两个字段上把它钉住，活到进程结束。
private var heldRaf: RandomAccessFile? = null
private var heldLock: FileLock? = null

/**
 * 状态栏进程启动时调一次。true = 本进程是唯一实例；false = 已有别的实例（调用方退出）。
 * 拿不到锁的任何异常（含同 JVM 重复拿的 OverlappingFileLockException）都算 false ——
 * 对「我是唯一实例吗」这个问题，拿不出独占证据就不是。
 */
internal fun acquireStatusBarLock(): Boolean {
    if (heldLock != null) return true
    val path = statusBarLockPath()
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
        // 连锁文件都建不出来（只读 FS 等）：**不因此拒绝启动** —— 条能跑比锁重要，
        // 代价只是回到「可能有重复实例」的旧形态。
        true
    }
}

/** 「已经有状态栏在跑吗」。设置窗拉起前问一次（探测法：试拿锁，拿得到=没人跑）。 */
internal fun statusBarRunning(): Boolean {
    if (heldLock != null) return true // 本进程就持有（设置窗开在状态栏进程里）
    val path = statusBarLockPath()
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
            true // 被别的实例持着 = 在跑
        } else {
            lock.release()
            raf.close()
            false
        }
    } catch (_: Exception) {
        // 探不出结论（锁文件都写不了）⇒ 按「在跑」不拉起：这种环境多半也拉不起
        // 第二支 JVM，失败面仍留着 autostart 那条路。
        true
    }
}

/** 拉起一支状态栏进程（分离式，不等待、不看输出）。null = 已发出；非 null = 失败原因。 */
internal fun spawnStatusBar(): String? {
    val exe = System.getProperty("jpackage.app-path")?.takeIf { it.isNotEmpty() }
        ?: ProcessHandle.current().info().command().orElse(null)
        ?: return "找不到自身可执行文件，拉不起状态栏"
    return try {
        ProcessBuilder(exe, "--statusbar")
            .redirectOutput(ProcessBuilder.Redirect.DISCARD)
            .redirectError(ProcessBuilder.Redirect.DISCARD)
            .start()
        null
    } catch (e: Exception) {
        "拉起状态栏失败：${e.message}"
    }
}

/** 设置窗用：条该显示（未隐藏）且没在跑 ⇒ 拉起。null = 不用管或已发出；非 null = 失败原因。 */
internal fun ensureStatusBarRunning(s: StatusBarSettings): String? {
    if (s.hidden) return null
    if (statusBarRunning()) return null
    return spawnStatusBar()
}
