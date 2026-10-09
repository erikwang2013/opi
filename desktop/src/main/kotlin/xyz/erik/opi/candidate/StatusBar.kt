// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 状态栏悬浮条（2026-10-10 用户裁决①：自绘悬浮条，8 个按钮全上）。
//!
//! 进程形态：`opi-candidates --statusbar`（Linux 随包装进 `/etc/xdg/autostart/` 随会话
//! 自启；见 build.gradle.kts 的 injectSettingsDesktopEntry）。**独立进程**，与候选窗
//! 进程无关 —— 这一条决定了两件事：
//!  1. 设置、可见性靠**轮询配置文件**同步（见 [StatusBarApp] 的 1s 轮询）；
//!  2. 符号页的上屏（`commit_text`）走命名管道，而管道被**候选窗进程**持有
//!     （单实例）⇒ 本条进程里的符号页**上屏会失败**并如实报出来（见 [StatusBarApp]
//!     的 `symbolCommit` 参数与 SymbolsPage.kt 头注释）。lead 裁决（2026-10-10）：
//!     本轮不开第二条 UI→TSF 通路 —— 现状如实保留，TSF 接线落地时再定 UI 进程拓扑
//!     （候选窗/状态栏/符号页同进程是首选）。
//!
//! ⚠️ Windows：同构尽力，但**本轮没有拉起触发器**（lead 裁决「代码就绪、无拉起触发
//! 器」）——`--statusbar` 在 Windows 上能跑，只是没有东西会去起它。写到这儿而不是
//! 埋在代码里。
//!
//! 条上按钮：软键盘 / 符号大全 / 简繁转换 / 中英文标点 / 中英文切换 / logo /
//! 全半角 / 设置（顺序 = [StatusBarButton] 的枚举序）。logo = 品牌标识兼拖动把手，
//! 单击打开设置（用户裁决③）。
//!
//! 图标口径：**没有图片资产、没有新依赖**。形状能一眼认出来的（键盘、小欧）用
//! DrawScope 自绘；其余用一两个字 —— 这些开关的真实状态在**输入法里**，本进程读不到
//! （没有任何查询通道），所以画的是「它切换的那对状态」（简繁/中英/，./全半），
//! 不假扮当前状态。

package xyz.erik.opi.candidate

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.WindowPosition
import androidx.compose.ui.window.WindowState
import androidx.compose.ui.window.rememberWindowState
import kotlinx.coroutines.delay
import xyz.erik.opi.pet.OpiPet
import xyz.erik.opi.pet.PetMood
import xyz.erik.opi.pet.PetPalette

// ---------- 配色与尺寸（沿用候选窗那套深色） ----------

private val BarBg = Color(0xFF2D2D2D)
private val BarFg = Color(0xFFE8E8E8)
private val BarHover = Color(0xFF3F3F3F)
private val BarLine = Color(0xFF4A4A4A)
private val BarDim = Color(0xFFFF8A80)

private const val BTN = 30 // 按钮边长
private const val GAP = 4
private const val PAD = 6
private const val BAR_H = 42

/** 条宽随按钮数变（隐藏的按钮不占位）。纯函数，单测钉住。 */
internal fun statusBarWidthDp(buttonCount: Int): Int =
    PAD * 2 + BTN * buttonCount + GAP * (buttonCount - 1).coerceAtLeast(0)

/** 初始位置：屏幕底部居中、离下沿一截（别贴边，也别压任务栏）。 */
private fun barPosition(widthDp: Int): WindowPosition {
    val screen = java.awt.Toolkit.getDefaultToolkit().screenSize
    return WindowPosition(((screen.width - widthDp) / 2).dp, (screen.height - 128).dp)
}

// ---------- 应用 ----------

/**
 * 状态栏应用（含它开的几个窗：设置/虚拟键盘/符号页）。
 *
 * [symbolCommit] 由入口注入：**Windows 上只有候选窗进程能上屏**（管道单实例），
 * 其它进程只能如实失败 —— 见文件头第 2 条。返回 false = 没送出去。
 */
@Composable
fun StatusBarApp(symbolCommit: (String) -> Boolean) {
    var s by remember { mutableStateOf(loadSettings()) }
    var settingsOpen by remember { mutableStateOf(false) }
    var vkOpen by remember { mutableStateOf(false) }
    var symbolsOpen by remember { mutableStateOf(false) }
    var notice by remember { mutableStateOf<String?>(null) }

    // 1s 轮询配置文件：设置窗可能在**另一个进程**里（Linux 无参数模式就是设置窗），
    // 进程间没有通知通道 —— 轮询是这里唯一可靠的一手信号。文件小、每秒一次，代价可忽略。
    LaunchedEffect(Unit) {
        while (true) {
            delay(1000)
            val next = loadSettings()
            if (next != s) s = next // 变了才写状态：否则每秒一次无谓重组
        }
    }
    LaunchedEffect(notice) {
        if (notice != null) {
            delay(4000)
            notice = null
        }
    }

    val buttons = visibleStatusButtons(s.statusbar, isWindows())
    val width = statusBarWidthDp(buttons.size)
    val windowState = rememberWindowState(
        width = width.dp,
        height = BAR_H.dp,
        position = remember { barPosition(width) },
    )
    // 按钮增删（设置窗里拨的开关）→ 条跟着变宽。位置**不动**：拖过就保住用户放的地方。
    LaunchedEffect(buttons.size) { windowState.size = DpSize(statusBarWidthDp(buttons.size).dp, BAR_H.dp) }

    /** 失败必须看得见：写进条上的提示，也打一行到 stdout（自启动进程的日志在 journal 里）。 */
    fun report(msg: String?) {
        if (msg != null) {
            notice = msg
            println("[opi-statusbar] $msg")
        }
    }

    /**
     * 「符号」动作的**单点**（状态栏按钮与虚拟键盘「符」键共用，2026-10-10 用户裁决
     * 「虚拟键盘要加符键」——两条入口必须是同一条动作，别再各写一份平台分支）。
     * 返回 null = 成功；非 null = 给用户看的失败原因。
     *
     * 符号页两端不是同一个东西：Linux 是 fcitx5 的候选面板（由 opi-ctl 拉起），
     * Windows 才是本进程里这个 Compose 页。
     */
    fun openSymbols(): String? =
        if (isWindows()) {
            symbolsOpen = !symbolsOpen
            null
        } else {
            statusActionCommand(StatusBarButton.SYMBOLS)?.let { runExternal(it) }
        }

    fun onButton(b: StatusBarButton) {
        when (b) {
            StatusBarButton.VK -> vkOpen = !vkOpen
            StatusBarButton.LOGO, StatusBarButton.SETTINGS -> settingsOpen = true
            StatusBarButton.SYMBOLS -> report(openSymbols())
            else -> report(statusActionCommand(b)?.let { runExternal(it) })
        }
    }

    Window(
        // 无边框窗没有关闭入口；「隐藏」走设置里的 master 开关（进程留着，见 visible 一行）。
        onCloseRequest = {},
        state = windowState,
        // master 隐藏 / 按钮全关 → 整条隐藏。进程继续轮询 ⇒ 设置里拨回来它就自己出现。
        visible = buttons.isNotEmpty(),
        undecorated = true,
        alwaysOnTop = true,
        resizable = false,
        // 不抢焦点：点状态栏不该把用户正在打字的窗口的焦点夺走（与候选窗同一条理由）。
        focusable = false,
        title = "opi输入法 状态栏",
    ) {
        Box(Modifier.fillMaxSize().background(BarBg).border(1.dp, BarLine)) {
            if (notice == null) {
                Row(
                    modifier = Modifier.fillMaxSize().padding(horizontal = PAD.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(GAP.dp),
                ) {
                    for (b in buttons) BarButton(b, windowState, ::onButton)
                }
            } else {
                // 提示期间**替掉**按钮列（而不是盖在上面）：否则按钮还在下面接着点击，
                // 用户会在「看不见的按钮」上点出错愕的结果。4s 后自动让开。
                Box(Modifier.fillMaxSize().padding(horizontal = 8.dp), contentAlignment = Alignment.Center) {
                    Text(notice!!, fontSize = 11.sp, color = BarDim, maxLines = 2)
                }
            }
        }
    }

    if (settingsOpen) {
        Window(
            onCloseRequest = { settingsOpen = false },
            state = rememberWindowState(width = 460.dp, height = 600.dp),
            title = "opi输入法 设置",
        ) {
            SettingsWindow() // 与 `--settings` 模式同一个主体（那份自带落盘 + 通知）
        }
    }
    if (vkOpen) VirtualKeyboardWindow(onClose = { vkOpen = false }, onSymbols = ::openSymbols)
    if (symbolsOpen) {
        SymbolWindow(
            onCommit = { text ->
                if (symbolCommit(text)) null else "上屏失败：没连上输入法（符号页要由候选窗进程上屏）"
            },
            onClose = { symbolsOpen = false },
        )
    }
}

// ---------- 一个按钮 ----------

@Composable
private fun BarButton(b: StatusBarButton, windowState: WindowState, onClick: (StatusBarButton) -> Unit) {
    val interaction = remember { MutableInteractionSource() }
    val hovered by interaction.collectIsHoveredAsState()
    Box(
        modifier = Modifier
            .size(BTN.dp)
            .background(if (hovered) BarHover else Color.Transparent)
            .clickable(interactionSource = interaction, indication = null) { onClick(b) }
            .then(if (b == StatusBarButton.LOGO) dragToMove(windowState) else Modifier)
            .semantics { contentDescription = b.label },
        contentAlignment = Alignment.Center,
    ) {
        when {
            b == StatusBarButton.LOGO ->
                // 品牌标识兼拖动把手：就是设置页里那只小欧，缩小到按钮里。
                OpiPet(mood = PetMood.IDLE, palette = PetPalette.Dark, size = 26.dp)
            b == StatusBarButton.VK -> VkIcon()
            else -> Text(iconText(b), fontSize = 12.sp, color = BarFg, maxLines = 1)
        }
    }
}

/**
 * 按钮面上的字。动作 = 单字（符/设）；开关 = 它切换的**那对状态**（简繁/中英/，./全半）
 * —— 见文件头「图标口径」：真实状态在本进程里读不到，就不画成当前状态。
 */
private fun iconText(b: StatusBarButton): String = when (b) {
    StatusBarButton.SYMBOLS -> "符"
    StatusBarButton.TRAD -> "简繁"
    StatusBarButton.PUNCT -> "，."
    StatusBarButton.LANG -> "中英"
    StatusBarButton.FULLWIDTH -> "全半"
    StatusBarButton.SETTINGS -> "设"
    StatusBarButton.VK, StatusBarButton.LOGO -> "" // 这两个画图标，不出字
}

/** 小键盘图标：一个键帽外框 + 六个键点。 */
@Composable
private fun VkIcon() {
    Canvas(Modifier.size(17.dp)) {
        val stroke = 1.5.dp.toPx()
        val h = size.height * 0.72f
        val top = (size.height - h) / 2f
        drawRoundRect(
            color = BarFg,
            topLeft = Offset(0f, top),
            size = Size(size.width, h),
            cornerRadius = CornerRadius(2.5.dp.toPx()),
            style = Stroke(stroke),
        )
        for (r in 0 until 2) {
            for (c in 0 until 3) {
                drawCircle(
                    color = BarFg,
                    radius = 1.1.dp.toPx(),
                    center = Offset(size.width * (0.28f + c * 0.22f), top + h * (0.35f + r * 0.3f)),
                )
            }
        }
    }
}

/**
 * 把窗口拖动交给这个修饰符（只有 logo 挂它）。
 *
 * 指针增量单位是**像素**，`WindowPosition` 要 dp ⇒ 经 LocalDensity 换算；不换算的话
 * 高 DPI 下拖起来会「跑」（指针走 1dp，窗口走 2dp）。key 用 density：换 DPI 后重建，
 * 免得一直用旧的换算比。
 */
@Composable
private fun dragToMove(windowState: WindowState): Modifier {
    val density = LocalDensity.current
    return Modifier.pointerInput(density) {
        detectDragGestures { change, drag ->
            change.consume()
            val cur = windowState.position
            windowState.position = WindowPosition(
                cur.x + with(density) { drag.x.toDp() },
                cur.y + with(density) { drag.y.toDp() },
            )
        }
    }
}
