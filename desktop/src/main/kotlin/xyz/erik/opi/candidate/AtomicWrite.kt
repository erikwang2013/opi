// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 配置文件的原子替换（2026-10-22 review；2026-10-24 加唯一临时名 + fsync）。
//!
//! 候选窗/状态栏每秒读 `settings.ini`。原先 `Files.writeString` 是 truncate+write，
//! 某一拍可能读到空/半截内容，`loadSettings` 回默认值 → 皮肤/字号闪一下。这里先写
//! 同目录临时文件，再 move 覆盖：读侧要么看到旧全文、要么看到新全文。
//!
//! ⚠️ 临时名**必须唯一**（`createTempFile`）：固定 `.tmp` 在两个并发写方（设置窗与
//! 状态栏、或两次快速保存）之间会互相截断/把对方的文件 rename 走，轻则丢一次保存，
//! 重则目标文件变成半截（本仓 Android 侧同款事故的修法）。`force(true)` = 内容先落盘
//! 再改名：掉电后不会留下「新名字 + 空内容」。
//! 原子 move 不被支持/被占用时退回普通 REPLACE_EXISTING（与旧写入同一条失败面）。

package xyz.erik.opi.candidate

import java.nio.ByteBuffer
import java.nio.channels.FileChannel
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.StandardCopyOption
import java.nio.file.StandardOpenOption

internal fun writeTextAtomically(p: Path, text: String) {
    val dir = p.toAbsolutePath().parent ?: p.toAbsolutePath()
    val tmp = Files.createTempFile(dir, p.fileName.toString() + ".", ".tmp")
    try {
        FileChannel.open(
            tmp,
            StandardOpenOption.WRITE,
            StandardOpenOption.TRUNCATE_EXISTING,
        ).use { ch ->
            val buf = ByteBuffer.wrap(text.toByteArray(Charsets.UTF_8))
            while (buf.hasRemaining()) ch.write(buf)
            ch.force(true)
        }
        try {
            Files.move(tmp, p, StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE)
        } catch (_: Exception) {
            Files.move(tmp, p, StandardCopyOption.REPLACE_EXISTING)
        }
    } finally {
        runCatching { Files.deleteIfExists(tmp) }
    }
}
