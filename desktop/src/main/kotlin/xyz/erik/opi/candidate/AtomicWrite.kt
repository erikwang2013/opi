// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 配置文件的原子替换（2026-10-22 review）。
//!
//! 候选窗/状态栏每秒读 `settings.ini`。原先 `Files.writeString` 是 truncate+write，
//! 某一拍可能读到空/半截内容，`loadSettings` 回默认值 → 皮肤/字号闪一下。这里先写
//! 同目录临时文件，再 move 覆盖：读侧要么看到旧全文、要么看到新全文。
//! 原子 move 不被支持/被占用时退回普通 REPLACE_EXISTING（与旧写入同一条失败面）。

package xyz.erik.opi.candidate

import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.StandardCopyOption

internal fun writeTextAtomically(p: Path, text: String) {
    val tmp = p.resolveSibling(p.fileName.toString() + ".tmp")
    Files.writeString(tmp, text)
    try {
        try {
            Files.move(tmp, p, StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE)
        } catch (_: Exception) {
            Files.move(tmp, p, StandardCopyOption.REPLACE_EXISTING)
        }
    } finally {
        runCatching { Files.deleteIfExists(tmp) }
    }
}
