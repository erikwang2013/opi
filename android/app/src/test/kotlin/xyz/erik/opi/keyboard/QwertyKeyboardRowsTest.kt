// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import org.junit.Assert.assertEquals
import org.junit.Test

/** 英文 26 键顶部数字行（2026-10-19）：可选、在字母行**上方**、默认关闭。 */
class QwertyKeyboardRowsTest {

    @Test
    fun numberRowIsOffByDefault() {
        val rows = qwertyRows(showNumberRow = false)
        assertEquals("默认仍是三行字母", 3, rows.size)
        assertEquals("q", rows[0].first())
    }

    @Test
    fun englishLayoutPutsBackspaceOnTheShiftRowAtTheFarRight() {
        // 2026-10-23 用户裁决：⌫ 与 ↵ 互换 —— 退格回字母行最右，回车去底行。
        val rows = qwertyRows(showNumberRow = true, englishLayout = true)
        assertEquals(4, rows.size)
        assertEquals("⇧", rows[3].first())
        assertEquals("⌫", rows[3].last())
        assertEquals("z", rows[3][1])
        assertEquals("m", rows[3][rows[3].size - 2])
    }

    @Test
    fun numberRowSitsAboveTheLetterRows() {
        val withRow = qwertyRows(showNumberRow = true)
        assertEquals(4, withRow.size)
        assertEquals(
            listOf("1", "2", "3", "4", "5", "6", "7", "8", "9", "0"),
            withRow[0],
        )
        assertEquals("数字行之外仍是原字母三行", qwertyRows(false), withRow.drop(1))
    }
}
