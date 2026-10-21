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
//!             // caret 位置（2026-10-22 起真发：TSF 每次 composition 变化取
//!             // caret，窗口贴着输入处；发端见 crates/tsf-opi/src/caret.rs）。
//!             // x/y 单位 = **AWT 逻辑像素**（= Compose 的 .dp 数值）：收端直接
//!             // WindowPosition(x.dp, y.dp)，而 CMP 1.11.1 的窗口定位不乘 density
//!             // （反编译 setPositionImpl 确认），故发 200 就落在离屏幕左缘 200
//!             // 逻辑像素处。发端不得预乘 density/缩放，收端不得按设备像素解读
//!             // （TSF 的物理像素由发端按 DPI 折算）。
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
//! 窗口行为（2026-10-21 用户裁决「长条的，和搜狗输入法一样」）：无边框 + 置顶 +
//! 不抢焦点（输入法候选窗语义）；初始隐藏，show 消息到达才显示。**两行横条**
//! （2026-10-22 用户裁决「拼音要在候选词上方」）：第一行拼音 + 模式、第二行候选 +
//! 翻页控件，高度固定 76dp；宽度按两行里更宽的一行实测（`rememberCandidateBarSize`）
//! 在屏幕内伸缩，候选放不下时该区域横向滚动、翻页控件钉在右端。position 消息 →
//! 窗口贴着 caret（屏幕边缘处收回屏内）；没收到过 caret 位置时水平居中，宽度变化时
//! 自动重算。**皮肤与候选字号**跟设置窗/状态栏同一份 `settings.ini`（本进程 1s
//! 轮询，与状态栏的同步机制一致）：皮肤 = `Skin=`，字号 = `CandidateFont=小|中|大`
//! → `candidateFontScale` 缩放整张字号表。
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

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalFontFamilyResolver
import androidx.compose.ui.text.font.createFontFamilyResolver
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.WindowPosition
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
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
    /**
     * 是否收到过真实 caret 位置（`position` 消息）。否 = 用横条自己的默认落点
     * （[candidateBarPosition]：按宽度居中，避免长条超出屏幕）；是 = 跟 caret 走。
     */
    var positioned by mutableStateOf(false)
}

// ---------- 入口 ----------

fun main(args: Array<String>) = application {
    // 三个模式共用一个可执行文件（jpackage 只有一份 mainClass / 一个安装包 / 一套图标）：
    //   --settings        = 「属性设置」窗（两个平台都有）
    //   --statusbar       = 状态栏悬浮条（Linux 随会话自启，见 build.gradle.kts 的
    //     /etc/xdg/autostart；Windows 由安装器/登录 Run 拉起，但仅在 TSF 激活时显示）
    //   无参数（Windows）  = 候选窗（TSF 经命名管道驱动；见本文件头部的线协议）
    //   无参数（Linux）    = 也进「属性设置」—— **这不是偷懒，是语义**：候选窗是
    //     TSF 的东西（命名管道两端都在 Windows），Linux 上这个应用**只有设置一个用途**，
    //     所以「无参数的默认」在那边就该是设置窗（否则从启动器点开是一片空白）。
    // `args` 与操作系统在进程生命周期里恒定 ⇒ 这个分支**不会来回切**（Compose 的早返回安全）。
    if ("--statusbar" in args) {
        // 崩溃取证（2026-10-22）：先装处理器，连「重复实例/启动失败」也留时间线。
        installStatusBarCrashLogger()
        // 单实例锁（2026-10-10）：设置窗会按需拉起本进程（见 StatusBarLock.kt），
        // 重复的实例在这里安静退出 —— 否则会在桌面上叠出第二条、第三条。
        if (!acquireStatusBarLock()) {
            logStatusBarEvent("duplicate statusbar instance, exiting")
            println("[opi-statusbar] 已有一个状态栏在跑，本实例退出")
            // 硬退出，**不能**只 return@application：Compose/AWT 的事件线程会把 JVM
            // 吊住（实测：只 return 时进程活着、没窗口，pgrep 里多一具）
            exitProcess(0)
        }
        // 符号页/表情页的上屏：共用 desktopSymbolCommit（Actions.kt 的单点：Linux =
        // `opi-ctl insert`、Windows = 控制面 `commit`）。设置窗开的那两个窗走同一条。
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

    // 候选窗分支：application 根组合默认**没有** LocalFontFamilyResolver，而横条宽度要在
    // 这一层用 TextMeasurer 算（必须赶在 Window 创建前 —— Compose 1.11 实测：首次组合里
    // 再改 WindowState.size 不会作用于首个 show，要等下一次重组合；初始 state 直接给正确
    // 尺寸才没有这一帧的错位）。补一个平台 resolver，只影响本分支。
    CompositionLocalProvider(
        LocalFontFamilyResolver provides remember { createFontFamilyResolver() },
    ) {
        val model = remember { CandidateModel() }
        val server = remember { PipeServer(model).also { it.start() } }
        // 状态栏/设置进程的控制面 broker（ControlPlane.kt）：本进程是唯一连着 TSF 的一端。
        remember { ControlServer(server).also { it.start() } }
        // 外观：状态栏/设置窗只会写 settings.ini（**另一个进程**），本进程与状态栏
        // 用同一条既有机制跟上 —— 1s 读一次小文件；首拍立即读，不等 1s。
        // · 皮肤（2026-10-22 用户报障「点击状态栏logo切换背景，候选词框的背景没变化」）
        // · 候选字号（设置窗「外观 → 候选字号」，三档缩放见 candidateFontScale）
        var skin by remember { mutableStateOf(SkinOption.KEEP) }
        var fontName by remember { mutableStateOf(CLASSIC_UI_FONT_DEFAULT) }
        LaunchedEffect(Unit) {
            while (true) {
                val st = withContext(Dispatchers.IO) { loadSettings() }
                if (st.skin != skin) skin = st.skin
                if (st.candidateFont != fontName) fontName = st.candidateFont
                delay(1000)
            }
        }
        val fontScale = candidateFontScale(fontName)
        // 2026-10-21 用户裁决：Windows 候选窗 = **搜狗式横条**；2026-10-22 起为
        // **两行**（拼音行 + 候选行，按内容定宽、高 76dp）。
        val barSize = rememberCandidateBarSize(model, fontScale)
        val windowState = rememberWindowState(
            width = barSize.width,
            height = barSize.height,
            position = candidateBarPosition(barSize),
        )
        // 翻页/换页 → 宽度跟着内容走（首帧以后的变化都是重组合后的 ComponentUpdater 应用）。
        LaunchedEffect(barSize) { windowState.size = barSize }
        // 位置：收到过 caret 位置就跟它走；否则用横条默认落点（按宽度居中，长条不出屏）。
        // caret 贴近屏幕右/下边缘时要收回屏内 —— 发端只知道 caret 坐标，不知道窗宽高。
        LaunchedEffect(model.x, model.y, model.visible, model.positioned, barSize) {
            if (model.visible) {
                windowState.position = if (model.positioned) {
                    val screen = virtualScreenBounds()
                    val maxX = (screen.x + screen.width - barSize.width.value.toInt())
                        .coerceAtLeast(screen.x)
                    val maxY = (screen.y + screen.height - barSize.height.value.toInt())
                        .coerceAtLeast(screen.y)
                    WindowPosition(
                        model.x.coerceIn(screen.x, maxX).dp,
                        model.y.coerceIn(screen.y, maxY).dp,
                    )
                } else {
                    candidateBarPosition(barSize)
                }
            }
        }

        // 登录自启后，进程可能整场都没有候选要显示。曾尝试「首次 show 之前不建窗」
        // 以省下 Skiko/D3D 的开销，但 2026-10-21 真机验证发现：Compose 1.11 的
        // `application` 在**没有任何 Window** 时会在 `Recomposer.close/join` 后直接返回
        // （反编译 Application_desktopKt$awaitApplication$2$1$2 确认），于是候选进程
        // 启动即退出 —— `\\.\pipe\opi` / `\\.\pipe\opi-control` 都不存在，候选窗与
        // 状态栏都拿不到 TSF 状态。**必须保留一个 Window 让 application 活着**；
        // 不可见时不绘制（见 Skiko Direct3DRedrawer.frameDispatcher 的 isShowing 分支）。
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
            // 皮肤在 Window 内容里 provide 一次：候选面板与状态栏读同一份角色表
            // （SkinPalette.kt），换肤只改这一个来源。`skin` 由上面的 1s 轮询更新。
            CompositionLocalProvider(LocalBarPalette provides skinColors(skin)) {
                CandidatePanel(
                    model = model,
                    onSelect = server::sendSelect,
                    onNext = server::sendNextPage,
                    onPrev = server::sendPrevPage,
                    windowSize = barSize,
                    fontScale = fontScale,
                )
            }
        }
    }
}
