// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 传输的宿主侧执行器：只在剪贴板 owner 线程上调用，句柄不跨线程。
//!
//! 状态归约是纯逻辑（TransferProtocol.kt），这里只做三件事：调用 `opi_clip_*transfer*`
//! FFI、把结果写进 [TransferUiState]、把失败原因交给宿主显示。目录展开与上限的最终
//! 裁决仍在内核（`transfer::validate_send`），宿主侧的预检只是提前拦。

package xyz.erik.opi.candidate

import com.sun.jna.Pointer

/** 内核 last_error；空串时给一条通用原因。 */
internal fun readLastError(f: OpiClipFfi): String {
    val message = runCatching { f.opi_clip_last_error().takeText(f) }.getOrNull().orEmpty()
    return message.takeIf { it.isNotBlank() } ?: "剪贴板内核调用失败"
}

/** 异常文本可能带路径/端口，但不会带剪贴板内容；只留类型 + 短消息。 */
internal fun transportError(t: Throwable): String =
    "剪贴板内核异常：${t.javaClass.simpleName}" +
        (t.message?.take(120)?.let { "：$it" } ?: "")

internal class TransferService(
    private val ffi: () -> OpiClipFfi?,
    private val handle: () -> Pointer?,
    private val enabled: () -> Boolean,
    private val reportError: (String?) -> Unit,
) {
    var state: TransferUiState = TransferUiState()
        private set

    /** 关闭传输时清空：既不显示旧卡片，也不保留任何待处理 ID。 */
    fun reset() {
        state = TransferUiState()
    }

    fun apply(event: TransferEvent) {
        state = state.apply(event)
    }

    fun send(peer: String, filesJson: String) {
        val f = ffi()
        val h = handle()
        if (!enabled() || f == null || h == null) {
            reportError("传输内核未启动，未发送")
            return
        }
        val peerArg = Utf16Arg(peer)
        val filesArg = Utf16Arg(filesJson)
        val id = try {
            f.opi_clip_send_files(
                h, peerArg.memory, peerArg.len, filesArg.memory, filesArg.len,
            ).takeText(f)
        } catch (t: Throwable) {
            reportError(transportError(t))
            return
        }
        // 空串 = 失败（未配对/未连接/超限）；详情在内核 last_error。
        reportError(if (id.isEmpty()) readLastError(f) else null)
    }

    fun respond(id: String, accept: Boolean, saveDir: String?) {
        val f = ffi()
        val h = handle()
        if (!enabled() || f == null || h == null) {
            fail(id, "传输内核未启动")
            return
        }
        if (accept && saveDir.isNullOrEmpty()) {
            fail(id, "接受传输需要保存目录")
            return
        }
        val idArg = Utf16Arg(id)
        val dir = saveDir?.takeIf { it.isNotEmpty() }?.let(::Utf16Arg)
        val ok = try {
            f.opi_clip_respond_transfer(
                h, idArg.memory, idArg.len, if (accept) 1.toByte() else 0.toByte(),
                dir?.memory, dir?.len ?: 0L,
            ) != 0.toByte()
        } catch (t: Throwable) {
            fail(id, transportError(t))
            return
        }
        if (!ok) {
            fail(id, readLastError(f))
        } else {
            state = state.mark(id, if (accept) TransferState.ACTIVE else TransferState.CANCELED)
            reportError(null)
        }
    }

    fun cancel(id: String) {
        val f = ffi()
        val h = handle()
        if (!enabled() || f == null || h == null) {
            reportError("传输内核未启动")
            return
        }
        val idArg = Utf16Arg(id)
        val ok = try {
            f.opi_clip_cancel_transfer(h, idArg.memory, idArg.len) != 0.toByte()
        } catch (t: Throwable) {
            reportError(transportError(t))
            return
        }
        reportError(if (ok) null else readLastError(f))
    }

    private fun fail(id: String, message: String) {
        state = state.mark(id, TransferState.FAILED, message)
        reportError(message)
    }
}
