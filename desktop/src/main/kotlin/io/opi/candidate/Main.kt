// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! C3：候选窗（Compose Desktop / JVM）—— OPI 拼音输入法 Windows 桌面候选窗。
//!
//! 【线协议：NDJSON over named pipe】（与 crates/tsf-opi/src/candidate_io.rs
//! 头注释互为镜像，改协议须同步两处）
//!
//! ```text
//! 管道名：\\.\pipe\opi-candidates（单实例，字节流模式，'\n' 分帧，UTF-8）
//! 角色：本窗口为 SERVER 监听管道；TSF 插件进程为 CLIENT（窗口启动后连接）。
//!
//! TSF(CLIENT) → 本窗(SERVER)：
//!   show      {"type":"show","buffer":"ni","candidates":["你","尼",...],
//!              "page":1,"page_count":3,"mode":"pinyin"}
//!             （page/page_count 为 1 起；candidates 为当前页文本）
//!   hide      {"type":"hide"}
//!   position  {"type":"position","x":120,"y":340}  // caret 提示（骨架降级固定位置）
//!
//! 本窗(SERVER) → TSF(CLIENT)：
//!   select    {"type":"select","index":0}   // 用户点击第 index（页内 0 起）候选
//!   next_page {"type":"next_page"} / {"type":"prev_page"}
//! ```
//!
//! 传输实现：JNA（net.java.dev.jna 5.6.0，本机缓存）直调 kernel32 ——
//! CreateNamedPipeW/ConnectNamedPipe 为 raw Function（jna-platform 的 Kernel32
//! 接口不含二者），ReadFile/WriteFile/DisconnectNamedPipe/CloseHandle 用
//! Kernel32.INSTANCE。jna-platform 是跨平台 jar，Linux 上可编译（kernel32
//! 仅 Windows 存在 —— 验收阶段在 Windows 上运行）。
//!
//! 窗口行为：无边框 + 置顶 + 不抢焦点（输入法候选窗语义）；初始隐藏，
//! show 消息到达才显示；position 消息 → 跟随 caret（缺省固定默认位置）。
//! 翻页：本地即时翻转（响应性）+ 同时发消息给 TSF（权威页码以 TSF 回发的
//! show 为准）。
//!
//! 【拆分】协议全貌在此；NDJSON 行解析见 Protocol.kt，named pipe 服务器见
//! PipeServer.kt，Compose 自绘 UI 见 CandidateWindow.kt。

package io.opi.candidate

import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.WindowPosition
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState

// ---------- 窗口状态（snapshot state，服务器线程跨线程写入） ----------

private const val FIXED_X = 200
private const val FIXED_Y = 320

/** 候选窗渲染状态：由管道服务器线程更新，Compose 层读取。 */
class CandidateModel {
    var visible by mutableStateOf(false)
    var buffer by mutableStateOf("")
    var candidates by mutableStateOf(emptyList<String>())
    var page by mutableIntStateOf(1)
    var pageCount by mutableIntStateOf(1)
    var mode by mutableStateOf("pinyin")
    var x by mutableIntStateOf(FIXED_X)
    var y by mutableIntStateOf(FIXED_Y)
}

// ---------- 入口 ----------

fun main() = application {
    val model = remember { CandidateModel() }
    val server = remember { PipeServer(model).also { it.start() } }
    val windowState = rememberWindowState(width = 320.dp, height = 168.dp)

    Window(
        onCloseRequest = ::exitApplication,
        state = windowState,
        // 可见性跟随模型（初始隐藏，show 消息到达才显示）。
        visible = model.visible,
        undecorated = true,
        alwaysOnTop = true,
        resizable = false,
        focusable = false,
        title = "OPI 候选窗",
    ) {
        // 位置跟随（position 消息；缺省固定默认位置）。
        LaunchedEffect(model.x, model.y, model.visible) {
            if (model.visible) {
                windowState.position = WindowPosition(model.x.dp, model.y.dp)
            }
        }
        CandidatePanel(
            model = model,
            onSelect = server::sendSelect,
            onNext = server::sendNextPage,
            onPrev = server::sendPrevPage,
        )
    }
}
