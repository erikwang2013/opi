// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 状态栏悬浮条（2026-10-10 用户裁决①：自绘悬浮条，8 个按钮全上；软键盘是否
//! 默认上条见 [StatusBarSettings] —— 2026-10-10 中午起默认关）。
//!
//! 进程形态：`opi --statusbar`（Linux 随包装进 `/etc/xdg/autostart/` 随会话
//! 自启；见 build.gradle.kts 的 injectSettingsDesktopEntry）。**独立进程**，与候选窗
//! 进程无关 —— 这一条决定了两件事：
//!  1. 设置靠**轮询配置文件**同步；输入法状态另有事件唤醒（见 [StatusBarApp]）；
//!  2. 符号页的上屏**按平台两条路**（见 [StatusBarApp] 的 `symbolCommit` 参数与
//!     SymbolsPage.kt 头注释）：Linux = `opi-ctl insert`（D-Bus → addon 提交，与进程
//!     拓扑无关）；Windows = 控制面 `\\.\pipe\opi-control-v2` → 候选窗进程 broker →
//!     TSF（ControlPlane.kt）。状态读与按钮动作同一条控制面。
//!
//! Windows：EXE 安装器/登录 Run 会拉起 `--statusbar`（scripts/windows/opi.iss），
//! 但**只在 TSF 激活时显示**：控制面读不到状态（im 为空）就隐藏，切换回 opi 后
//! 事件立刻让它出现，1s 轮询兜底（2026-10-21 用户裁决「切换到这个输入法才显示」；
//! 2026-10-25 用户要求切走/切回都要快）。Linux 仍
//! 保留「未知 ⇒ 维持可见」的旧语义，避免 fcitx5 旧 glue 缺 im= 时闪条。
//!
//! 条上按钮（从左到右 = [StatusBarButton] 的枚举序）：小欧 / 中英 / 。，标点(双态) /
//! 简繁 / **英文专业** / **模式** / 月相全半角 / 软键盘 / Ω符号大全 / 设置。前八枚
//! 2026-10-10 **最终冻结**；**模式**是 2026-10-14 用户裁决加的（原话「在简繁按钮后
//! 增加模式切换按钮」）；**英文专业**2026-10-19 用户裁决移到简繁之后（旧位置在标点
//! 之前，已废止）——枚举序即显示序，见 SkinAndStatus.kt 的 [StatusBarButton] 头注释。
//! logo = 品牌标识兼拖动把手（第 1 位），**单击换下一款皮肤**（2026-10-13 用户裁决
//! 「点击 logo 的颜色变化」；取代早先「单击打开设置」③ —— 设置仍走右侧「设置」钮）。
//!
//! 图标口径：**没有图片资产、没有新依赖**。自绘（键盘/月相/四小方块/小欧）或单字与
//! 符号（Ω 简/繁 中/英，标点两态）。**符号大全 = 静态 Ω；中英标点 = 双态**
//! （punct on 「。，」/ off 「.,」）—— 这是 2026-10-10 **中午**按用户对按钮行为的描述
//! 订正的结果（深夜 B 版曾把 。， 放符号大全、Ω 静态放标点，已废止）；逐项与时间线
//! 见 iconText 的注释，别按任一半截「改回去」。
//! **面上的字/图标一律「按按钮取最大且不溢出」**（2026-10-14 用户裁决，原话与实现
//! 见 BarGlyphs.kt 文件头）；**2026-10-15 用户再裁决「显示太大，缩小40％」** ⇒ 面内容
//! 整体 × `FACE_SCALE`（0.6，落点唯一：StatusBarButtonFace 里那个 `face` 盒；本文件
//! 传的仍是按钮盒 —— 100% = 30dp，其余按 `StatusBarScale` 等比，见 [BarMetrics]）—— 面
//! （字形排版、三个图标、每按钮的槽位表）整体在 BarGlyphs.kt，本文件只管条、按钮框、动作与轮询。
//!
//! **Windows 更小、且是胶囊**（2026-10-24 用户报障「win端的状态栏整体大了，边角也不是
//! 弧形」）：绘制尺寸再乘 [WINDOWS_BAR_FACTOR]（0.85，配置里的 scale 不动），窗底置透明
//! 并把根盒裁成圆角（[barShape]）—— Linux 不乘、保持直角，行为不变。
//!
//! **状态实时可见**（2026-10-10 深夜裁决）：简/繁、中/英、月相随输入法真实状态翻转。
//! 状态 = `opi-ctl state`（契约见 Actions.kt），1s 轮询 + Windows 客户端接入/断开
//! 事件 + **每次按钮动作返回后立刻补读一次**（「即时生效」的落点，见
//! [actionWithStateRefresh]）。读不到/坏串/缺键
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
//! ⇒ 整条隐藏；切回 opi 由事件/1s 轮询让它自己出现；缺键/空串 = 未知 ⇒ 维持现状（不闪）。
//! 判定全在 [barVisible]（纯函数，四态有单测钉住）。

package xyz.erik.opi.candidate

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
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
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.boundsInWindow
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.WindowPosition
import androidx.compose.ui.window.WindowState
import androidx.compose.ui.window.rememberWindowState
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import xyz.erik.opi.pet.OpiPetWindowIcon

// ---------- 配色与尺寸 ----------

// 配色在 SkinPalette.kt（值的唯一源 = scripts/fcitx5_skins.py）：条随 `Skin=` 走
// —— 2026-10-12 用户报障「设置皮肤后状态栏皮肤没同步变化」的修复（此前五个颜色
// 全是写死的深色常量，条从没读过皮肤）。同步靠本文件既有的 1s 配置轮询，自动。
// 「不改动」= SkinPalette.KeepColors（= 上述那五个常量原值，行为一字不变）。
// 尺寸表（BTN/GAP/PAD/BAR_H、BarMetrics、barMetrics、statusBarWidthDp）在 SkinAndStatus.kt：
// 2026-10-21 为守住 500 行门禁从这里拆出，同包 internal 引用不变。

// barVisible 也移到了 SkinAndStatus.kt（2026-10-21 再次撞 500 行）；它是纯函数，
// 与本文件的状态栏窗口无耦合。

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
 * Linux = `opi-ctl insert`（失败短语来自 runExternal，逐字有用）；Windows = 控制面
 * `commit` → 候选窗进程 broker → TSF（ControlPlane.kt）。
 */
@Composable
fun StatusBarApp(symbolCommit: (String) -> String?) {
    var s by remember { mutableStateOf(loadSettings()) }
    // 起始 = 中性缺省，循环第一拍立即补上真实状态 —— 不在组合期做进程调用。
    var imeState by remember { mutableStateOf(ImeState()) }
    // 连续「状态读不到」的拍数：短暂抖动保留上一条状态（不闪），连续失败才回到中性
    // （Windows 上 im 空 = 隐藏）。没有这个上限，宿主进程退出后条会停在旧状态不收。
    var stateFails by remember { mutableIntStateOf(0) }
    // 模式档位（模式按钮面用）。2026-10-14 起 `opi-ctl state` 带 `mode=` 位（modeplumb 落地），
    // 所以真值来自轮询、不是本进程的记忆：**点击 = 乐观更新，轮询 = 权威**（下面每拍
    // `mode = barMode(st)`）。两者不会打架：`opi-ctl mode` 只在引擎切完之后才返回新 token，
    // 所以下一拍 state 必然与它一致。初值 = 拼音（用户裁决的默认档；首拍立即补齐）。
    var mode by remember { mutableStateOf(MODE_DEFAULT) }
    var settingsOpen by remember { mutableStateOf(false) }
    var vkOpen by remember { mutableStateOf(false) }
    var symbolsOpen by remember { mutableStateOf(false) }
    var notice by remember { mutableStateOf<String?>(null) }

    // 1s 轮询两件事，并进这一个循环：设置文件（设置窗可能在**另一个进程**里，进程间
    // 没有通知通道）+ 输入法状态（简/繁、中/英、月相要跟着真实状态走，用户裁决
    // 2026-10-10 深夜）。一拍 = 一次小文件读 + 一个短命进程，代价可忽略。
    //
    // 2026-10-25：Windows 上另有一条**事件**通道（StatusEventWatcher）—— TSF 客户端
    // 接入/断开候选管道时候选窗进程 SetEvent，本循环立刻醒来补读，不再等 1s；无客户端
    // 时控制面直接回 `im=` 空状态，状态栏立即隐藏（用户要求切走别的输入法要快）。
    val stateEvents = remember { Channel<Unit>(Channel.CONFLATED) }
    LaunchedEffect(Unit) {
        StatusEventWatcher { stateEvents.trySend(Unit) }.start()
        while (true) {
            // ⚠️ 一拍 = 一次小文件读 + 一个短命进程（`opi-ctl state`，超时 1.5s）：必须挪出
            // 组合线程（= AWT EDT）。fcitx5 的 D-Bus 一忙，`opi-ctl` 答不上来的那 1.5s 里
            // EDT 就停摆 —— 用户报的「切换输入法状态栏卡死」正是这条（2026-10-13 jstack
            // 实抓 EDT 停在 ProcessImpl.waitFor ← Actions.runCapture ← readImeState）。
            // 循环仍是单条协程顺序执行：不可重入、次序（设置→状态）与超时值都不变。
            val read = statusBarGuard("状态栏轮询") {
                withContext(Dispatchers.IO) { loadSettings() to readImeStateOrNull() }
            }
            if (read == null) {
                // 守卫层自身失败（不是一次状态读失败）：退避，别忙转。
                withTimeoutOrNull(200) { stateEvents.receive() }
                continue
            }
            val (next, st) = read
            if (next != s) s = next // 变了才写状态：否则每秒一次无谓重组
            // 读失败（null）：先保留上一次状态（一次管道抖动不把用户正要点的按钮变没），
            // 但连续 3 拍（≈3s）都读不到就回到中性 —— 否则宿主退出后条会永远停在旧状态。
            // 计数/清空判定是纯函数（nextStatePoll，单测钉住第 3 拍即清）。Windows 的
            // 「无客户端」不是读失败：控制面回 im= 空状态，readOk=true，立即隐藏。
            val poll = nextStatePoll(stateFails, readOk = st != null, imPresent = imeState.im.isNotEmpty())
            stateFails = poll.fails
            if (poll.clear) imeState = ImeState()
            if (st != null) {
                if (st != imeState) imeState = st
                mode = barMode(st) // 档位跟着 state 走（缺 mode 位的老插件 ⇒ barMode 回落默认档）
            }
            // 事件来了立即重读；没有事件就 1s 后再来一轮（设置文件也要同步）。
            withTimeoutOrNull(1000) { stateEvents.receive() }
        }
    }
    LaunchedEffect(notice) {
        if (notice != null) {
            delay(4000)
            notice = null
        }
    }

    // Windows 整体缩一档 + 胶囊圆角（2026-10-24 用户报障「win端的状态栏整体大了，
    // 边角也不是弧形」）；Linux 不乘、保持直角 —— 见 barMetrics/barShape。
    val windows = isWindows()
    val buttons = visibleStatusButtons(s.statusbar, windows)
    val metrics = barMetrics(s.statusbar.scale, windows)
    val width = statusBarWidthDp(buttons.size, s.statusbar.scale, windows)
    val shape = barShape(windows)
    val windowState = rememberWindowState(
        width = width.dp,
        height = metrics.height,
        position = remember { barPosition(width.toInt()) },
    )
    // 按钮增删（设置窗里拨的开关）与统一缩放（StatusBarScale）→ 条跟着变。位置**不动**：
    // 拖过就保住用户放的地方。设置窗可能是另一进程 ⇒ 这里等 1s 轮询把新值送来。
    LaunchedEffect(buttons.size, s.statusbar.scale) {
        statusBarGuard("状态栏尺寸") {
            windowState.size = DpSize(statusBarWidthDp(buttons.size, s.statusbar.scale, windows).dp, metrics.height)
        }
    }
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

    /**
     * LOGO 单击 = 换下一款皮肤（2026-10-13 用户裁决，见文件头）。生效走**既有那条通道**
     * （写 `Skin=` + 通知 fcitx5 重载 = [saveSettings]），条自己**立刻**换装 —— `s` 就是
     * 上面配色的来源，改它即重画，不必等 1s 轮询。成功**不吭声**：换色本身就是反馈，
     * 而提示期间按钮列会被替掉（见下），连点换肤时反而挡路；没生效才占一行。
     */
    fun cycleSkin() {
        val next = s.copy(skin = nextSkin(s.skin))
        s = next
        report(if (saveSettings(next)) null else saveStatusMessage(live = false))
    }

    fun dispatch(b: StatusBarButton) {
        when (b) {
            StatusBarButton.VK -> vkOpen = !vkOpen
            StatusBarButton.LOGO -> cycleSkin()
            StatusBarButton.SETTINGS -> settingsOpen = true
            StatusBarButton.SYMBOLS -> report(openSymbols())
            // 模式：循环一次（拼 → 五 → en26 → 拼，引擎侧与 token 派生见
            // Actions.cycleMode）。返回**新档位 token**，当场换字（不等 1s 轮询；
            // 轮询随后会由 state 的 mode= 位确认，见文件顶部 mode 变量）。
            // **空串 = 没切成**（通道不通/超时/应答异形）⇒ 保留原档 + 报一句 ——
            // Actions.cycleMode 的契约就是「token 或空串」，认不得的应答不许当档位
            // 画到条上（把空串写进 mode 的话面会退成默认「拼」，等于谎报切成了拼音）。
            StatusBarButton.MODE -> {
                val next = cycleMode()
                if (next.isEmpty()) report("模式切换失败：输入法无应答或应答不认得") else mode = next
            }
            // 英文专业键（2026-10-19 用户裁决）：只控制英文是否查单词库，且只在英文
            // 输入法下有效。现场重读 state（不拿 1s 轮询的旧值）—— 刚切中/英就点它
            // 也不会误判；老插件的 state 没有 enpro= 位，会得到「重启 fcitx5」的提示，
            // 而不是把 err:5「未知动作」原样弹给用户。
            StatusBarButton.ENPRO -> {
                val st = readImeStateOrNull()
                if (st == null) report("输入法无应答，请稍后重试")
                else {
                    val why = enproBlockReason(st)
                    if (why != null) report(why) else report(runStatusAction(b))
                }
            }
            // Windows 走控制面（runStatusAction 内的平台分派），Linux 仍是 opi-ctl。
            else -> report(runStatusAction(b))
        }
    }

    val scope = rememberCoroutineScope()
    // 动作串行锁：从前「点一下 = EDT 冻到动作跑完」，后续点击排在 AWT 事件队列里按序执行；
    // 挪到 IO 后并发来自协程，这把锁把旧次序（一次一个、动作在前、补读在后）原样保住。
    val actionLock = remember { Mutex() }

    fun onButton(b: StatusBarButton) {
        // 本进程内的窗口开关：直接在 UI 线程执行，不排 IO 队列。2026-10-21 用户报
        // 「打开设置好久才会有」：这三个动作不做任何进程/管道调用，却曾和 `opi-ctl`
        // 一起排进串行 IO 锁，前面还可能堵着一次最长 1.5s 的状态补读。
        when (b) {
            StatusBarButton.SETTINGS -> { settingsOpen = true; return }
            StatusBarButton.VK -> { vkOpen = !vkOpen; return }
            StatusBarButton.SYMBOLS -> { symbolsOpen = !symbolsOpen; return }
            else -> {}
        }
        // 动作一返回就补读状态（点完马上看到翻转，不等下一秒的轮询）。补读与轮询并存：
        // fcitx5 应用切换是异步的，个别时候补读会早于状态落定 —— 1s 轮询是兜底。
        //
        // ⚠️ 整段（动作 + 补读）都在 IO 线程上：动作分支里是子进程 —— `opi-ctl`（≤3s）与
        // 换肤那条的 `notifyFcitxReload`（三条 D-Bus 工具，`waitFor` **无超时**），补读又是
        // 一次 1.5s；留在 EDT 上就是「点一下冻 5 秒」（2026-10-13 实抓：EDT 停在
        // notifyFcitxReload 的 waitFor ← saveSettings ← cycleSkin ← dispatch ← onButton）。
        // dispatch 里那几个赋值（s/settingsOpen/notice）从 IO 线程写 Snapshot 状态是安全的：
        // Compose 的 Snapshot 状态跨线程可写，重组合照旧排到 UI 线程。
        scope.launch {
            actionLock.withLock {
                statusBarGuard("状态栏动作 ${b.name}") {
                    withContext(Dispatchers.IO) {
                        // 补读失败（null）**不覆盖**已有状态：动作刚返回时 broker 可能还忙/超时，
                        // 把 im 清空会让 Windows 下整条立即隐藏（用户报「点按钮条就没了」）。
                        actionWithStateRefresh({ dispatch(b) }) {
                            readImeStateOrNull()?.let { imeState = it }
                        }
                    }
                }
            }
        }
    }

    // 条隐藏时**连内容一起退出组合**：提示窗是独立 Window，只藏条窗不会带走它
    // （悬停状态也可能收不到 Exit）⇒ 必须把按钮/提示从组合里摘掉。
    // 本条自己打开的窗口（设置/符号/软键盘）开着时**强制显示**：这些窗口会拿走焦点，
    // TSF Deactivate 后 im 读不到；若此时隐藏，用户看到的就是「点一下条就没了」。
    val shown = barVisible(buttons, imeState.im, requireActive = windows) ||
        settingsOpen || vkOpen || symbolsOpen
    Window(
        // 无边框窗没有关闭入口；「隐藏」走设置里的 master 开关（进程留着，见 visible 一行）。
        onCloseRequest = {},
        state = windowState,
        // master 隐藏 / 按钮全关 → 整条隐藏（进程继续轮询 ⇒ 设置里拨回来它就自己出现）；
        // 切到别的输入法（im 非空且 ≠ opi）也隐藏，切回来 1s 轮询让它自己出现 —— 判定
        // 全在 [barVisible]，四态单测钉住。
        visible = shown,
        undecorated = true,
        // Windows：窗底色透明 —— 圆角外的四角才真的透出桌面（形状见根 Box 的 clip）。
        // Linux 保持不透明（未报障，行为不变）。
        transparent = windows,
        alwaysOnTop = true,
        resizable = false,
        // 不抢焦点：点状态栏不该把用户正在打字的窗口的焦点夺走（与候选窗同一条理由）。
        focusable = false,
        title = "小欧输入法 状态栏",
        // 任务栏/窗口列表图标 = 小欧（与设置页那只同一套形状，见 PetIcon.kt）。
        icon = OpiPetWindowIcon,
    ) {
        // 不进任务栏/菜单栏（用户裁决 2026-10-10 下午）：X11 属性直写，机制与取证状态
        // 见 SkipTaskbar.kt（关键实现：新窗创建时注册一次，之后它自己跟随每次显示重贴）。
        LaunchedEffect(window) { statusBarGuard("SkipTaskbar") { SkipTaskbar.apply(window) } }
        // 条随 `Skin=` 换装（≤1s，见文件头的同步说明）：根部 provide 一次，子
        // composable 读 LocalBarPalette —— 与 Android 侧 LocalKeyboardPalette 同套做法。
        val palette = skinColors(s.skin)
        if (shown) CompositionLocalProvider(LocalBarPalette provides palette) {
            // Windows = 胶囊：整条裁成圆角（Linux 的 shape 是 RectangleShape，像素不变）。
            Box(Modifier.fillMaxSize().clip(shape).background(palette.bg).border(1.dp, palette.line, shape)) {
                if (notice == null) {
                    Row(
                        modifier = Modifier.fillMaxSize().padding(horizontal = metrics.pad),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(metrics.gap),
                    ) {
                        for (b in buttons) BarButton(b, imeState, mode, metrics, windowState, window, ::onButton)
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
            // 与 `--settings` 模式同一个尺寸（800×560，2026-10-12 左导航布局起）
            state = rememberWindowState(width = 800.dp, height = 560.dp),
            title = "小欧输入法 设置",
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

/**
 * 一枚按钮：[BarMetrics.button] 见方的框 + 悬停底 + 点击（LOGO 另挂拖动）。**面**
 * （图标/字形）在 [StatusBarButtonFace]（BarGlyphs.kt）—— 分开是为了让门禁能离屏直接渲那一支。
 *
 * 传进去的就是**这块按钮本身**（100% = 30dp，其余尺寸按 `StatusBarScale` 等比）：
 * 用户 2026-10-14 的「根据按钮显示最大」按的是按钮这块盒，字号由 BarGlyphs 按墨迹取满
 * （见其文件头）。别在这里再垫留白 —— 垫掉多少、字就小多少（曾经的 26dp「面盒」就是
 * 这么把 logo 从 30 缩回 26 的）。**缩到 60% 那条 2026-10-15 的缩放不在这里**：它是面
 * 内部的事（`FACE_SCALE`，BarGlyphs.kt），在这边再乘一次就是乘两遍。
 *
 * enpro/en26 与普通键**始终收同一个 [BarMetrics.button]**：键帽只是在这块盒里按
 * 26/30 画（KeycapFace.kt），没有自己的尺寸来源。
 */
@Composable
private fun BarButton(
    b: StatusBarButton,
    st: ImeState,
    mode: String,
    metrics: BarMetrics,
    windowState: WindowState,
    barWindow: java.awt.Window,
    onClick: (StatusBarButton) -> Unit,
) {
    val interaction = remember { MutableInteractionSource() }
    val hovered by interaction.collectIsHoveredAsState()
    // 按下反馈（2026-10-21 用户裁决「按钮要可操作」）：动作走控制面时可能等上几十~几百 ms，
    // 没有按下态的话用户会以为没点上而连点。按下 = accent 半透明，悬停 = hover 色。
    val pressed by interaction.collectIsPressedAsState()
    val palette = LocalBarPalette.current
    // 英文专业键常亮（2026-10-19）：否则切了 pro 看不出任何变化。用既有 hover 色，
    // 不新造颜色/角色（见 SkinPalette.kt 的角色映射）。键帽本身是固定白底（设计图），
    // 这一圈 hover 只露在键帽外缘 —— 激活态仍可见，且不染色键帽。
    val active = b == StatusBarButton.ENPRO && st.englishPro
    var tip by remember { mutableStateOf(false) }
    // 按钮在条窗内的像素矩形 —— 提示窗要靠它换算屏幕坐标。
    var anchor by remember { mutableStateOf(Rect.Zero) }
    // 悬停 400ms 出提示。提示必须是独立 Window：Compose 的 Popup 会被夹进 42dp 高的条窗
    // （clippingEnabled=false 也救不了，xwininfo 实测无独立弹层），见 BarTooltip.kt。
    LaunchedEffect(hovered) { tip = false; if (hovered) { delay(400); tip = true } }
    Box(
        modifier = Modifier
            .size(metrics.button)
            .background(
                when {
                    pressed -> palette.accent.copy(alpha = 0.30f)
                    hovered || active -> palette.hover
                    else -> Color.Transparent
                },
            )
            .clickable(interactionSource = interaction, indication = null) { onClick(b) }
            .then(if (b == StatusBarButton.LOGO) dragToMove(windowState) else Modifier)
            .onGloballyPositioned { anchor = it.boundsInWindow() }
            .semantics { contentDescription = b.label },
        contentAlignment = Alignment.Center,
    ) {
        StatusBarButtonFace(b, st, mode, metrics.button)
        if (tip) BarTooltip(b, anchor, windowState.position, palette, barWindow)
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
