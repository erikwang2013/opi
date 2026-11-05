// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 设备名状态（从 ClipService 拆出守 500 行门禁）：自定义名/主机名清洗只算一次入口。

package xyz.erik.opi.candidate

internal class ClipDeviceNameState(private val hostname: String?) {
    @Volatile
    var current: String = clipDeviceName(hostname)
        private set

    /** 返回是否真的变了（变了才需要重建内核）。 */
    fun update(custom: String): Boolean {
        val next = clipDeviceName(hostname, custom)
        if (next == current) return false
        current = next
        return true
    }
}
