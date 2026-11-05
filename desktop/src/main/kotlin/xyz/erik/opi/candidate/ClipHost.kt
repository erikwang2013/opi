// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 状态栏常驻进程里的剪贴板宿主入口（从 ClipService.kt 拆出守 500 行门禁）。

package xyz.erik.opi.candidate

/** 入口：状态栏组合时调用（幂等）。 */
internal object ClipHost {
    private var service: ClipService? = null

    @Synchronized
    fun ensureStarted(): ClipService {
        service?.let { return it }
        val s = ClipService()
        service = s
        s.start()
        return s
    }

    fun setEnabled(on: Boolean) {
        ensureStarted().setEnabled(on)
    }

    fun setDeviceName(name: String) {
        ensureStarted().setDeviceName(name)
    }
}
