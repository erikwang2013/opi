// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「清空用户词」的纯函数部分：缓存文件路径推导与删除。
//! 传输（Windows 控制面 / Linux opi-ctl）分别在 ControlPlaneTest 与 CLI 测试里钉。

package xyz.erik.opi.candidate

import java.nio.file.Files
import java.nio.file.Paths
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Test

class UserWordsTest {
    @Test
    fun `cache file follows each platform's data dir`() {
        assertEquals(
            Paths.get("/x/opi/opi_user_words.json"),
            userWordsFile(false, "/x", "/home/u", null),
        )
        // XDG_DATA_HOME / LOCALAPPDATA 缺省与空串都按未设置处理（与 Dicts.kt 同口径）。
        assertEquals(
            Paths.get("/home/u/.local/share/opi/opi_user_words.json"),
            userWordsFile(false, null, "/home/u", null),
        )
        assertEquals(
            Paths.get("/home/u/.local/share/opi/opi_user_words.json"),
            userWordsFile(false, "", "/home/u", "C:/L"),
        )
        assertEquals(
            Paths.get("C:/L/opi/opi_user_words.json"),
            userWordsFile(true, null, null, "C:/L"),
        )
        assertEquals(Paths.get("./opi/opi_user_words.json"), userWordsFile(true, null, null, null))
    }

    @Test
    fun `cache delete removes the file and tolerates absence`() {
        val dir = Files.createTempDirectory("opi-user-words")
        val f = dir.resolve("opi_user_words.json")
        Files.writeString(f, """{"version":1,"words":[]}""")
        assertNull(deleteUserWordsCache(f))
        assertFalse("缓存文件还在", Files.exists(f))
        assertNull("本来就不存在不算失败", deleteUserWordsCache(f))
        Files.deleteIfExists(dir)
    }
}
