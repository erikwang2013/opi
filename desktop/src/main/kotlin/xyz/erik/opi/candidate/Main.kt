// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! C3：候选窗（Compose Desktop / JVM）—— OPI 拼音输入法 Windows 桌面候选窗。
//!
//! 【线协议：NDJSON over named pipe】（与 crates/tsf-opi/src/candidate_io.rs
//! 头注释互为镜像，改协议须同步两处）
//!
//! ```text
//! 管道名：\\.\pipe\opi（单实例，字节流模式，'\n' 分帧，UTF-8）
//! 角色：本窗口为 SERVER 监听管道；TSF 插件进程为 CLIENT（窗口启动后连接）。
//! 单实例的共享方式：宿主失活（焦点切走/切换应用）时客户端主动断开
//! （crates/tsf-opi/src/tsf.rs 的 Deactivate → TsfSink::disconnect），本侧
//! serveLoop 的 finally 隐藏窗口并重建实例，下一个应用按键时再惰性连上。
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
//!   commit_text {"text":"、","type":"commit_text"}   // 符号页点选 → 上屏
//!             // 键序 = serde_json 的字典序（text 在前）；text 经 Protocol.kt 的
//!             // jsonString 转义（符号里就有 `"` 和 `\`）。语义（engine-track 冻结）：
//!             // 在**文档插入点**插入，不动 composition/候选，也不隐藏候选窗；
//!             // 当时没有焦点文档则静默跳过。整行由 PipeServer.commitTextLine 生成。
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

package xyz.erik.opi.candidate

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
import kotlin.system.exitProcess
import xyz.erik.opi.pet.OpiPetWindowIcon

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

fun main(args: Array<String>) = application {
    // 三个模式共用一个可执行文件（jpackage 只有一份 mainClass / 一个安装包 / 一套图标）：
    //   --settings        = 「属性设置」窗（两个平台都有）
    //   --statusbar       = 状态栏悬浮条（Linux 随会话自启，见 build.gradle.kts 的
    //     /etc/xdg/autostart；Windows 侧**代码就绪，但本轮没有拉起触发器**）
    //   无参数（Windows）  = 候选窗（TSF 经命名管道驱动；见本文件头部的线协议）
    //   无参数（Linux）    = 也进「属性设置」—— **这不是偷懒，是语义**：候选窗是
    //     TSF 的东西（命名管道两端都在 Windows），Linux 上这个应用**只有设置一个用途**，
    //     所以「无参数的默认」在那边就该是设置窗（否则从启动器点开是一片空白）。
    // `args` 与操作系统在进程生命周期里恒定 ⇒ 这个分支**不会来回切**（Compose 的早返回安全）。
    if ("--statusbar" in args) {
        // 单实例锁（2026-10-10）：设置窗会按需拉起本进程（见 StatusBarLock.kt），
        // 重复的实例在这里安静退出 —— 否则会在桌面上叠出第二条、第三条。
        if (!acquireStatusBarLock()) {
            println("[opi-statusbar] 已有一个状态栏在跑，本实例退出")
            // 硬退出，**不能**只 return@application：Compose/AWT 的事件线程会把 JVM
            // 吊住（实测：只 return 时进程活着、没窗口，pgrep 里多一具）
            exitProcess(0)
        }
        // 符号页/表情页的上屏：共用 desktopSymbolCommit（Actions.kt 的单点：Linux =
        // `opi-ctl insert`、Windows = 如实失败）。设置窗开的那两个窗走同一条。
        StatusBarApp(symbolCommit = ::desktopSymbolCommit)
        return@application
    }

    if ("--settings" in args || !isWindows()) {
        Window(
            onCloseRequest = ::exitApplication,
            state = rememberWindowState(width = 800.dp, height = 560.dp),
            title = "小欧输入法 设置",
            icon = OpiPetWindowIcon, // 窗口/任务栏图标 = 小欧（PetIcon.kt）
        ) {
            SettingsWindow()
        }
        return@application
    }

    val model = remember { CandidateModel() }
    val server = remember { PipeServer(model).also { it.start() } }
    val windowState = rememberWindowState(width = 320.dp, height = 168.dp)

    // 登录自启后，进程可能整场都没有候选要显示。**首次 show 之前不建窗**：
    // Window 一组合就会创建 AWT 窗口并初始化 Skiko/D3D 表面，而这份 GPU 资源
    // 对「还没开始打字」的会话是纯成本。首次显示后窗口保留（隐藏时不绘制，
    // 见 Skiko 0.144.6 Direct3DRedrawer.frameDispatcher 的 isShowing 分支），
    // 后续 show 无重建延迟。
    // 零窗口不会结束 application：isOpen 只由 exitApplication() 置 false
    // （反编译 Application.desktop.kt 的 awaitApplication 确认）。
    var windowCreated by remember { mutableStateOf(false) }
    LaunchedEffect(model.visible) {
        if (model.visible) windowCreated = true
    }

    if (windowCreated) {
        Window(
            onCloseRequest = ::exitApplication,
            state = windowState,
            // 可见性跟随模型（初始隐藏，show 消息到达才显示）。
            visible = model.visible,
            undecorated = true,
            alwaysOnTop = true,
            resizable = false,
            focusable = false,
            title = "小欧输入法 候选窗",
            icon = OpiPetWindowIcon, // 窗口/任务栏图标 = 小欧（PetIcon.kt）
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
}
