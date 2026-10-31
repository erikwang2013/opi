// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.clip

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** 回环抑制：远端写入后的剪贴板回调不能再发布回去（适配器的纯逻辑部分）。 */
class ClipEchoGuardTest {

    @Test
    fun remoteWriteIsSuppressedIncludingDuplicateCallbacks() {
        val guard = ClipEchoGuard()
        guard.rememberRemote("远端文本")
        assertTrue(guard.shouldSuppress("远端文本"))
        // 部分 ROM 对一次 setPrimaryClip 连发多次回调：每一次都必须抑制。
        assertTrue(guard.shouldSuppress("远端文本"))
    }

    @Test
    fun localTextIsNotSuppressedAndLiftsTheGuard() {
        val guard = ClipEchoGuard()
        guard.rememberRemote("远端文本")
        assertFalse(guard.shouldSuppress("用户自己复制的"))
        // 已经换成别的内容：之后再复制回同一段文本是真实的新事件，不该再被压制。
        assertFalse(guard.shouldSuppress("远端文本"))
    }

    @Test
    fun nothingIsSuppressedBeforeAnyRemoteWrite() {
        val guard = ClipEchoGuard()
        assertFalse(guard.shouldSuppress("随便什么"))
    }

    @Test
    fun resetDropsPendingSuppression() {
        val guard = ClipEchoGuard()
        guard.rememberRemote("x")
        guard.reset()
        assertFalse(guard.shouldSuppress("x"))
    }
}
