// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.clip

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

/**
 * 文本/剪贴板的设备路由（2026-11-04 用户要求：多设备时可指定收发对象）。
 *
 * 发送目标与接收过滤共用同一台“选中设备”；[onlySelected] 关闭时保持原来的
 * 广播/全收行为（空串 = 内核不过滤）。状态放在这里而不是单个面板里，是因为
 * 自动剪贴板同步在面板关闭后仍要按同一路由工作。
 */
internal object ClipRouting {
    var selectedPeer by mutableStateOf("")
        private set
    var onlySelected by mutableStateOf(false)
        private set

    private var apply: ((String, String) -> Unit)? = null

    /** 内核启动/停止时挂上或摘掉 setter；挂上立刻推一次当前路由。 */
    fun bind(setter: ((String, String) -> Unit)?) {
        apply = setter
        push()
    }

    fun select(peerId: String) {
        if (selectedPeer == peerId) return
        selectedPeer = peerId
        push()
    }

    fun setOnlySelectedMode(on: Boolean) {
        if (onlySelected == on) return
        onlySelected = on
        push()
    }

    private fun push() {
        val peer = if (onlySelected) selectedPeer else ""
        apply?.invoke(peer, peer)
    }
}
