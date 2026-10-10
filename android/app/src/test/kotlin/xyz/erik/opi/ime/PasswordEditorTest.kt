// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi

import android.text.InputType
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * 密码类编辑器判定（2026-10-10 用户报障「弹框里没法输入」的第二来源：
 * 密码/英文字段被当拼音吃 → 自动临时英文）。纯函数，边界逐条钉。
 */
class PasswordEditorTest {

    private fun text(vararg flags: Int): Int =
        InputType.TYPE_CLASS_TEXT or flags.fold(0) { acc, f -> acc or f }

    @Test
    fun `text password variants are password editors`() {
        assertEquals(true, isPasswordEditor(text(InputType.TYPE_TEXT_VARIATION_PASSWORD)))
        assertEquals(true, isPasswordEditor(text(InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD)))
        assertEquals(true, isPasswordEditor(text(InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD)))
    }

    @Test
    fun `ordinary text editors are not password editors`() {
        assertEquals(false, isPasswordEditor(text(InputType.TYPE_TEXT_VARIATION_NORMAL)))
        assertEquals(false, isPasswordEditor(text(InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS)))
        assertEquals(false, isPasswordEditor(text(InputType.TYPE_TEXT_VARIATION_URI)))
        assertEquals(false, isPasswordEditor(InputType.TYPE_NULL))
    }

    @Test
    fun `number password stays out on purpose`() {
        // 数字类密码不在此列：数字面板默认直出数字，没有「被拼音吃掉」的问题。
        // 这条是**有意**的边界，防止将来有人「顺手」把数字密码也切英文键盘。
        val numberPassword =
            InputType.TYPE_CLASS_NUMBER or InputType.TYPE_NUMBER_VARIATION_PASSWORD
        assertEquals(false, isPasswordEditor(numberPassword))
    }
}
