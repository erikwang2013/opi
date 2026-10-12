// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 状态栏悬浮条（2026-10-10 用户裁决①：自绘悬浮条，8 个按钮全上；软键盘是否
//! 默认上条见 [StatusBarSettings] —— 2026-10-10 中午起默认关）。
//!
//! 进程形态：`opi-candidates --statusbar`（Linux 随包装进 `/etc/xdg/autostart/` 随会话
//! 自启；见 build.gradle.kts 的 injectSettingsDesktopEntry）。**独立进程**，与候选窗
//! 进程无关 —— 这一条决定了两件事：
//!  1. 设置、可见性靠**轮询配置文件**同步（见 [StatusBarApp] 的 1s 轮询）；
//!  2. 符号页的上屏**按平台两条路**（见 [StatusBarApp] 的 `symbolCommit` 参数与
//!     SymbolsPage.kt 头注释）：Linux = `opi-ctl insert`（D-Bus → addon 提交，与进程
//!     拓扑无关，2026-10-12 落地）；Windows = 命名管道的 `commit_text`，而管道被
//!     **候选窗进程**持有（单实例）⇒ 本条进程里的符号页**只能如实失败**。lead 裁决
//!     （2026-10-10）：不开第二条 UI→TSF 通路 —— 现状如实保留，TSF 接线落地时再定
//!     UI 进程拓扑（候选窗/状态栏/符号页同进程是首选）。
//!
//! ⚠️ Windows：同构尽力，但**本轮没有拉起触发器**（lead 裁决「代码就绪、无拉起触发
//! 器」）——`--statusbar` 在 Windows 上能跑，只是没有东西会去起它。写到这儿而不是
//! 埋在代码里。
//!
//! 条上按钮（2026-10-10 **最终冻结**，从左到右 = [StatusBarButton] 的枚举序）：
//! 小欧 / 中英 / 。，标点(双态) / 简繁 / 月相全半角 / 软键盘 / Ω符号大全 / 设置。
//! logo = 品牌标识兼拖动把手（第 1 位），单击打开设置（用户裁决③）。
//!
//! 图标口径：**没有图片资产、没有新依赖**。自绘（键盘/月相/四小方块/小欧）或单字与
//! 符号（Ω 简/繁 中/英，标点两态）。**符号大全 = 静态 Ω；中英标点 = 双态**
//! （punct on 「。，」/ off 「.,」）—— 这是 2026-10-10 **中午**按用户对按钮行为的描述
//! 订正的结果（深夜 B 版曾把 。， 放符号大全、Ω 静态放标点，已废止）；逐项与时间线
//! 见 iconText 的注释，别按任一半截「改回去」。
//!
//! **状态实时可见**（2026-10-10 深夜裁决）：简/繁、中/英、月相随输入法真实状态翻转。
//! 状态 = `opi-ctl state`（契约见 Actions.kt），1s 轮询 + **每次按钮动作返回后立刻
//! 补读一次**（「即时生效」的落点，见 [actionWithStateRefresh]）。读不到/坏串/缺键
//! ⇒ 中性缺省（中/简/半月）—— 旧原则「读不到状态就不画成当前状态」被本裁决取代，
//! 缺省语义定案见 Actions.kt 的 [ImeState]。
//!
//! **条窗不进任务栏/菜单栏**（用户裁决 2026-10-10 下午：「桌面端状态栏是不在菜单栏
//! 显示图标的」）：机制 = X11 属性直写（`_NET_WM_WINDOW_TYPE_UTILITY` +
//! `_NET_WM_STATE_SKIP_TASKBAR/SKIP_PAGER`），全在 [SkipTaskbar]，**已真机 xprop 验过**。
//! ⚠️ 别改回 `java.awt.Window.setType(UTILITY)`：JDK 的 XAWT 里那条路是死的（构造器
//! 字段初始化把 preInit 抄的值覆成 NORMAL，实证见 SkipTaskbar.kt 头注释），试过了。
//! 窗口图标（小欧）那条裁决不受影响 —— 说的是「任务栏条目」，不是窗口图标。
//!
//! **切到别的输入法就藏**（用户裁决 2026-10-10 下午）：`state` 的 `im=` 位非空且 ≠ "opi"
//! ⇒ 整条隐藏；切回 opi 由 1s 轮询让它自己出现；缺键/空串 = 未知 ⇒ 维持现状（不闪）。
//! 判定全在 [barVisible]（纯函数，四态有单测钉住）。

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
import androidx.compose.runtime.CompositionLocalProvider
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
import xyz.erik.opi.pet.OpiPetWindowIcon
import xyz.erik.opi.pet.PetMood
import xyz.erik.opi.pet.PetPalette

// ---------- 配色与尺寸 ----------

// 配色在 SkinPalette.kt（值的唯一源 = scripts/fcitx5_skins.py）：条随 `Skin=` 走
// —— 2026-10-12 用户报障「设置皮肤后状态栏皮肤没同步变化」的修复（此前五个颜色
// 全是写死的深色常量，条从没读过皮肤）。同步靠本文件既有的 1s 配置轮询，自动。
// 「不改动」= SkinPalette.KeepColors（= 上述那五个常量原值，行为一字不变）。

private const val BTN = 30 // 按钮边长
private const val GAP = 4
private const val PAD = 6
private const val BAR_H = 42

/** 条宽随按钮数变（隐藏的按钮不占位）。纯函数，单测钉住。 */
internal fun statusBarWidthDp(buttonCount: Int): Int =
    PAD * 2 + BTN * buttonCount + GAP * (buttonCount - 1).coerceAtLeast(0)

/**
 * 纯函数：条该不该显示（[StatusBarApp] 的 visible 用它）。
 *
 * 规则（2026-10-10 下午用户裁决「切到其他输入法时状态栏消失」）：
 * `enabled && (im 未知 || im == "opi")` ——
 * · [buttons] 空 = master 隐藏 / 按钮全关 ⇒ 恒隐藏（老规则，优先级最高，与 [im] 无关）；
 * · [im] 缺键/空串 = **未知** ⇒ 维持现状（可见），不闪 —— glue 的 `im=` 还没合时旧输出
 *   正好落这里，轮询读失败（中性缺省）也落这里；
 * · [im] 非空且 ≠ "opi"（当前是别的输入法）⇒ 隐藏；切回 opi 由 1s 轮询让它自己出现。
 *
 * ⚠️ `"opi"` 是**跨轨字面**（glue 侧追加的 `im=` 值 = 本 addon 的 uniqueName）。对不上的
 * 表现是「opi 激活时条一直藏着」——响的错，不是静默错；但改它必须两侧一起改。
 */
internal fun barVisible(buttons: List<StatusBarButton>, im: String): Boolean =
    buttons.isNotEmpty() && (im.isEmpty() || im == "opi")

/** 初始位置：屏幕底部居中、离下沿一截（别贴边，也别压任务栏）。 */
private fun barPosition(widthDp: Int): WindowPosition {
    val screen = java.awt.Toolkit.getDefaultToolkit().screenSize
    return WindowPosition(((screen.width - widthDp) / 2).dp, (screen.height - 128).dp)
}

// ---------- 应用 ----------

/**
 * 状态栏应用（含它开的几个窗：设置/虚拟键盘/符号页）。
 *
 * [symbolCommit] 由入口注入（形状与 [SymbolWindow] 的 `onCommit` 同款：null = 送上去了，
 * 非 null = **给用户看的**失败原因，原样透传）。两条通路见 Main.kt 的 symbolCommit：
 * Linux = `opi-ctl insert`（失败短语来自 runExternal，逐字有用）；Windows = 如实失败
 * —— **只有候选窗进程能上屏**（管道单实例，见文件头第 2 条）。
 */
@Composable
fun StatusBarApp(symbolCommit: (String) -> String?) {
    var s by remember { mutableStateOf(loadSettings()) }
    // 起始 = 中性缺省，首拍轮询（≤1s）补上真实状态 —— 不在组合期做进程调用。
    var imeState by remember { mutableStateOf(ImeState()) }
    var settingsOpen by remember { mutableStateOf(false) }
    var vkOpen by remember { mutableStateOf(false) }
    var symbolsOpen by remember { mutableStateOf(false) }
    var notice by remember { mutableStateOf<String?>(null) }

    // 1s 轮询两件事，并进这一个循环：设置文件（设置窗可能在**另一个进程**里，进程间
    // 没有通知通道）+ 输入法状态（简/繁、中/英、月相要跟着真实状态走，用户裁决
    // 2026-10-10 深夜）。一拍 = 一次小文件读 + 一个短命进程，代价可忽略。
    LaunchedEffect(Unit) {
        while (true) {
            delay(1000)
            val next = loadSettings()
            if (next != s) s = next // 变了才写状态：否则每秒一次无谓重组
            val st = readImeState()
            if (st != imeState) imeState = st
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
    fun openSymbols(): String? {
        // 2026-10-12 用户裁决「点击后要是个**大的**页面，可以翻页选择」：**两个平台都开
        // 大页面**（此前 Linux 走 `opi-ctl symbols` = fcitx5 的小候选条）。上屏通道：
        // Windows = 候选窗进程的命名管道；Linux = `opi-ctl insert`（插件提交进当前输入框，
        // 见 Main.kt 注入的那个 symbolCommit）。引擎的候选条浏览仍可由 opi-ctl/热键唤起，
        // 只是不再挂在这个按钮上。
        symbolsOpen = !symbolsOpen
        return null
    }

    fun dispatch(b: StatusBarButton) {
        when (b) {
            StatusBarButton.VK -> vkOpen = !vkOpen
            StatusBarButton.LOGO, StatusBarButton.SETTINGS -> settingsOpen = true
            StatusBarButton.SYMBOLS -> report(openSymbols())
            else -> report(statusActionCommand(b)?.let { runExternal(it) })
        }
    }

    fun onButton(b: StatusBarButton) {
        // 动作一返回就补读状态（点完马上看到翻转，不等下一秒的轮询）。补读与轮询并存：
        // fcitx5 应用切换是异步的，个别时候补读会早于状态落定 —— 1s 轮询是兜底。
        actionWithStateRefresh({ dispatch(b) }) { imeState = readImeState() }
    }

    Window(
        // 无边框窗没有关闭入口；「隐藏」走设置里的 master 开关（进程留着，见 visible 一行）。
        onCloseRequest = {},
        state = windowState,
        // master 隐藏 / 按钮全关 → 整条隐藏（进程继续轮询 ⇒ 设置里拨回来它就自己出现）；
        // 切到别的输入法（im 非空且 ≠ opi）也隐藏，切回来 1s 轮询让它自己出现 —— 判定
        // 全在 [barVisible]，四态单测钉住。
        visible = barVisible(buttons, imeState.im),
        undecorated = true,
        alwaysOnTop = true,
        resizable = false,
        // 不抢焦点：点状态栏不该把用户正在打字的窗口的焦点夺走（与候选窗同一条理由）。
        focusable = false,
        title = "opi输入法 状态栏",
        // 任务栏/窗口列表图标 = 小欧（与设置页那只同一套形状，见 PetIcon.kt）。
        icon = OpiPetWindowIcon,
    ) {
        // 不进任务栏/菜单栏（用户裁决 2026-10-10 下午）：X11 属性直写，机制与取证状态
        // 见 SkipTaskbar.kt（关键实现：新窗创建时注册一次，之后它自己跟随每次显示重贴）。
        LaunchedEffect(window) { SkipTaskbar.apply(window) }
        // 条随 `Skin=` 换装（≤1s，见文件头的同步说明）：根部 provide 一次，子
        // composable 读 LocalBarPalette —— 与 Android 侧 LocalKeyboardPalette 同套做法。
        val palette = skinColors(s.skin)
        CompositionLocalProvider(LocalBarPalette provides palette) {
            Box(Modifier.fillMaxSize().background(palette.bg).border(1.dp, palette.line)) {
                if (notice == null) {
                    Row(
                        modifier = Modifier.fillMaxSize().padding(horizontal = PAD.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(GAP.dp),
                    ) {
                        for (b in buttons) BarButton(b, imeState, windowState, ::onButton)
                    }
                } else {
                    // 提示期间**替掉**按钮列（而不是盖在上面）：否则按钮还在下面接着点击，
                    // 用户会在「看不见的按钮」上点出错愕的结果。4s 后自动让开。
                    Box(Modifier.fillMaxSize().padding(horizontal = 8.dp), contentAlignment = Alignment.Center) {
                        Text(notice!!, fontSize = 11.sp, color = palette.accent, maxLines = 2)
                    }
                }
            }
        }
    }

    if (settingsOpen) {
        Window(
            onCloseRequest = { settingsOpen = false },
            state = rememberWindowState(width = 460.dp, height = 600.dp),
            title = "opi输入法 设置",
            icon = OpiPetWindowIcon,
        ) {
            SettingsWindow() // 与 `--settings` 模式同一个主体（那份自带落盘 + 通知）
        }
    }
    if (vkOpen) VirtualKeyboardWindow(onClose = { vkOpen = false }, onSymbols = ::openSymbols)
    if (symbolsOpen) {
        // 失败原因**原样透传**（2026-10-12）：此前这里把 false 统一翻成一句写死的
        // Windows 解释 —— Linux 上「没有焦点输入框」这类真实原因会被顶掉。
        SymbolWindow(onCommit = symbolCommit, onClose = { symbolsOpen = false })
    }
}

// ---------- 一个按钮 ----------

@Composable
private fun BarButton(
    b: StatusBarButton,
    st: ImeState,
    windowState: WindowState,
    onClick: (StatusBarButton) -> Unit,
) {
    val interaction = remember { MutableInteractionSource() }
    val hovered by interaction.collectIsHoveredAsState()
    val palette = LocalBarPalette.current
    Box(
        modifier = Modifier
            .size(BTN.dp)
            .background(if (hovered) palette.hover else Color.Transparent)
            .clickable(interactionSource = interaction, indication = null) { onClick(b) }
            .then(if (b == StatusBarButton.LOGO) dragToMove(windowState) else Modifier)
            .semantics { contentDescription = b.label },
        contentAlignment = Alignment.Center,
    ) {
        when (b) {
            // 品牌标识兼拖动把手：就是设置页里那只小欧，缩小到按钮里。
            StatusBarButton.LOGO -> OpiPet(mood = PetMood.IDLE, palette = PetPalette.Dark, size = 26.dp)
            StatusBarButton.VK -> VkIcon()
            StatusBarButton.FULLWIDTH -> MoonIcon(st.fullwidth)
            StatusBarButton.SETTINGS -> SettingsIcon()
            else -> Text(iconText(b, st), fontSize = 12.sp, color = palette.fg, maxLines = 1)
        }
    }
}

/**
 * 按钮面上的字。两轮裁决的**时间线**（防止按任一半截「改回去」）：
 * · 2026-10-10 深夜 B 版：SYMBOLS = 「。，」静态、PUNCT = 「Ω」静态；
 * · 同日**中午订正**（按用户对按钮行为的描述）：两枚**互换**且 PUNCT 变**双态** ——
 *   SYMBOLS（符号大全）= 静态「Ω」；PUNCT（中英标点）= punct=on「。，」/ off「.,」。
 * 字面都是用户写的：「。，」是中文侧（**先句号**，勿按惯例改成「，。」）；「.,」是
 * 英文侧（先点号）—— 都不是笔误。
 * · TRAD / LANG = 单字，且是**当前状态**（简/繁、中/英），随输入法状态翻转 ——
 *   用户原话「文字标签全图标化」：多字标签（简繁/中英）全去。
 * 全半角、设置、键盘、小欧不出字：画月相/四小方块/键盘/小欧。
 */
internal fun iconText(b: StatusBarButton, st: ImeState): String = when (b) {
    StatusBarButton.SYMBOLS -> "Ω"
    StatusBarButton.TRAD -> tradIcon(st)
    StatusBarButton.PUNCT -> punctIcon(st)
    StatusBarButton.LANG -> langIcon(st)
    StatusBarButton.FULLWIDTH, StatusBarButton.SETTINGS,
    StatusBarButton.VK, StatusBarButton.LOGO -> "" // 这四个画图标，不出字
}

/** 简繁按钮面的字 = **当前输出**（转繁开着写「繁」）；用户裁决②：随真实状态翻转。 */
internal fun tradIcon(st: ImeState): String = if (st.trad) "繁" else "简"

/** 中英按钮面的字 = 当前模式（zh/en）。 */
internal fun langIcon(st: ImeState): String = if (st.zh) "中" else "英"

/**
 * 标点按钮面的字 = **当前标点态**（2026-10-10 中午订正：双态）。
 * on = 中文标点「。，」（用户字面序，先句号）；off = 英文「.,」（用户字面序，先点号）。
 */
internal fun punctIcon(st: ImeState): String = if (st.punct) "。，" else ".,"

/**
 * 全半角按钮 = 月相（用户裁决②：满月 = 全角、半月 = 半角）。
 * 满月 = 实心圆；半月 = 描边圆 + 右半实心（缺的半边露的是条自己的底 —— 窗不透明，
 * 不会透出背后的窗口内容）。
 */
@Composable
private fun MoonIcon(fullwidth: Boolean) {
    val fg = LocalBarPalette.current.fg
    Canvas(Modifier.size(17.dp)) {
        val r = size.minDimension / 2f - 1.dp.toPx()
        val center = Offset(size.width / 2f, size.height / 2f)
        if (fullwidth) {
            drawCircle(color = fg, radius = r)
        } else {
            drawCircle(color = fg, radius = r, style = Stroke(1.5.dp.toPx()))
            drawArc(
                color = fg,
                startAngle = -90f,
                sweepAngle = 180f, // 右半
                useCenter = true,
                topLeft = Offset(center.x - r, center.y - r),
                size = Size(r * 2f, r * 2f),
            )
        }
    }
}

/** 设置按钮 = 四小方块（用户裁决②：多字的「设」去掉，画图标）。 */
@Composable
private fun SettingsIcon() {
    val fg = LocalBarPalette.current.fg
    Canvas(Modifier.size(13.dp)) {
        val s = 5.dp.toPx()
        val gap = 3.dp.toPx()
        for (row in 0 until 2) {
            for (col in 0 until 2) {
                drawRoundRect(
                    color = fg,
                    topLeft = Offset(col * (s + gap), row * (s + gap)),
                    size = Size(s, s),
                    cornerRadius = CornerRadius(1.dp.toPx()),
                )
            }
        }
    }
}

/** 小键盘图标：一个键帽外框 + 六个键点。 */
@Composable
private fun VkIcon() {
    val fg = LocalBarPalette.current.fg
    Canvas(Modifier.size(17.dp)) {
        val stroke = 1.5.dp.toPx()
        val h = size.height * 0.72f
        val top = (size.height - h) / 2f
        drawRoundRect(
            color = fg,
            topLeft = Offset(0f, top),
            size = Size(size.width, h),
            cornerRadius = CornerRadius(2.5.dp.toPx()),
            style = Stroke(stroke),
        )
        for (r in 0 until 2) {
            for (c in 0 until 3) {
                drawCircle(
                    color = fg,
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
