// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 文本/剪贴板路由的 FFI 薄封装（从 ClipService.kt 拆出守 500 行门禁）。
//! 返回 null = 已设置；非 null = 内核错误文本。

package xyz.erik.opi.candidate

import com.sun.jna.Pointer

/** 设置窗进程内的路由选择；关掉再打开设置时仍能保持，而不是悄悄回到广播。 */
internal object ClipRouteUi {
    var target = ""
    var onlySelected = false
}

internal fun setClipTextRoutes(
    f: OpiClipFfi,
    handle: Pointer,
    routes: Pair<String, String>,
): String? {
    val (send, receive) = routes
    val a = Utf16Arg(send)
    val b = Utf16Arg(receive)
    return if (f.opi_clip_set_text_routes(handle, a.memory, a.len, b.memory, b.len) == 0.toByte()) {
        readLastError(f)
    } else {
        null
    }
}
