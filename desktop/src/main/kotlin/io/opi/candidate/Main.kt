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
//!   position  {"type":"position","x":120,"y":340}
//!             // caret 提示（骨架降级固定位置）。x/y 单位 = **AWT 逻辑像素**
//!             // （= Compose 的 .dp 数值）：收端直接 WindowPosition(x.dp, y.dp)，
//!             // 而 CMP 1.11.1 的窗口定位不乘 density（反编译 setPositionImpl
//!             // 确认），故发 200 就落在离屏幕左缘 200 逻辑像素处。发端不得
//!             // 预乘 density/缩放，收端不得按设备像素解读。
//!             // 两侧协议头现已同步（crates/tsf-opi/src/candidate_io.rs 的
//!             // position 条目有同样一段）；改动单位语义时**必须两处同改**。
//!
//! 本窗(SERVER) → TSF(CLIENT)：
//!   select    {"type":"select","index":0}   // 用户点击第 index（页内 0 起）候选
//!   next_page {"type":"next_page"} / {"type":"prev_page"}
//! ```
//!
//! 传输实现：JNA（net.java.dev.jna 5.6.0，本机缓存）直调 kernel32，全部经
//! `Kernel32.INSTANCE`：CreateNamedPipe/ConnectNamedPipe/ReadFile/WriteFile/
//! DisconnectNamedPipe/CloseHandle。**不用 raw `Function.getFunction`** ——
//! Kernel32 接口两者都声明（javap 核对 5.6.0），且 `Kernel32.INSTANCE` 以
//! W32APIOptions 装载：函数名 Unicode 消歧义与 `Native.getLastError()`
//! （ConnectNamedPipe 判 ERROR_PIPE_CONNECTED 必需）都只有这条路径成立。
//! jna-platform 是跨平台 jar，Linux 上可编译（kernel32 仅 Windows 存在 ——
//! 验收阶段在 Windows 上运行）。
//!
//! 窗口行为：无边框 + 置顶 + 不抢焦点（输入法候选窗语义）；初始隐藏，
//! show 消息到达才显示；position 消息 → 跟随 caret（缺省固定默认位置）。
//!
//! 翻页：**本窗只发消息，不改本地页码** —— 页码的唯一真源是 TSF 回发的 show。
//! TSF 侧已接线：`candidate_io.rs` 的 `SharedAction::on_next_page`/`on_prev_page`
//! → `TsfSharedState::next_page`/`prev_page` → `TsfLogic::next_page`/`prev_page`，
//! 随后 `SharedAction::refresh` 回发一条带新页码的 show，故点了箭头窗口会跟着翻。
//! 本窗仍**不得**自行改页码：页数上界与「页内索引 → 全局下标」的换算只在引擎侧
//! （`TsfLogic::select_from` 的 `global = page * PAGE_SIZE + index` 只此一份），
//! 前端各存一份必然漂移。
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
