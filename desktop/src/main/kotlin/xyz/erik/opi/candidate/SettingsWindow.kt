// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「属性设置」窗口（Compose 自绘；配色沿用项目色 —— 宣纸白底、青、朱砂）。
//!
//! **布局 = 左分类导航 + 右内容 + 底部状态行**（2026-10-12 用户裁决「设置页面布局和
//! 功能参考主流输入法」；此前是单列长滚动）。页集合见 [SettingsTab]：每页的内容在
//! SettingsPages.kt，**只列引擎真的支持的开关**。
//!
//! 交互模型不变：**改一下就落盘 + 通知 IME**（不像桌面软件那样要再点「确定」）——
//! 参照对象正是主流输入法那类输入的属性设置：开关拨完即生效，底部只如实显示结果。
//! Linux 走 `ReloadAddonConfig`（即那条实测有效的路，`fcitx5-remote -r` **不是**，
//! 见 Settings.kt 的 notifyFcitxReload）；Windows 走控制面 `reload` 让 TSF 立即重读，
//! 候选进程没起时退回下次激活。
//!
//! 本窗在**两个宿主**里都跑：`--settings` 模式（单独进程）与状态栏的「设置」按钮
//! （状态栏进程内开一个窗，见 StatusBar.kt）—— 所以别在这里假设自己是整个应用。
//!
//! 本窗还会开**符号页/表情页**（「符号与表情」页那两个按钮）：那是同一个 SymbolWindow
//! （见 SymbolsPage.kt），上屏走 desktopSymbolCommit —— 与状态栏那边同一条：
//! Linux 靠 opi-ctl，Windows 走控制面 → 候选窗进程 broker → TSF（ControlPlane.kt）。

package xyz.erik.opi.candidate

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.Divider
import androidx.compose.material.Switch
import androidx.compose.material.SwitchDefaults
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import xyz.erik.opi.pet.OpiPet
import xyz.erik.opi.pet.PetMood

// ---------- 配色（与 docs/opi-pet.svg 同一套） ----------
// internal：SettingsPages.kt 共用（一个窗的两半）。
internal val Paper = Color(0xFFFBFAF6) // 宣纸白
internal val Ink = Color(0xFF22272E) // 正文
internal val Muted = Color(0xFF6B7671) // 次级文字
internal val Teal = Color(0xFF2E8B80) // 青（开关选中）
internal val Hairline = Color(0xFFE3E8E6) // 分隔线

// 导航列底色/选中态：比宣纸白略深一档，选中 = 白底 + 青色左条（主流输入法那类设置页的做法）。
private val NavBg = Color(0xFFF1F3F1)
private val NavSelBg = Color(0xFFFBFAF6)

/** 一页：导航项。**顺序 = 展示序**（与主流输入法一致的排法：常用在前、关于在最后）。 */
internal enum class SettingsTab(val label: String) {
    COMMON("常用"),
    KEYS("按键"),
    APPEARANCE("外观"),
    STATUSBAR("状态栏"),
    SYMBOLS("符号与表情"),
    // 词库页（2026-10-14）：两平台都出（目录推导分平台，页本身同构，见 Dicts.kt）。
    DICTS("词库"),
    // 五笔页（2026-10-14，M4 第二波）：两平台都出 —— 六键**两端读侧都认**
    // （fcitx5 的 opi_config.h / TSF 的 settings.rs），没有「一半平台够不着」的键。
    WUBI("五笔"),
    ABOUT("关于"),
}

/**
 * 纯函数：本平台该出哪些页。**2026-10-22 起两平台同集**：Windows 的 `ShiftToggle`
 * 读侧（`crates/tsf-opi/src/settings.rs`）已接通、皮肤也由候选窗/状态栏跟随，按键与
 * 外观两页不再是「点不动的开关」。`windows` 参数保留只为调用点形状稳定。
 */
internal fun settingsTabs(@Suppress("UNUSED_PARAMETER") windows: Boolean): List<SettingsTab> =
    SettingsTab.entries

// ---------- 小部件 ----------

/** 一行开关：左说明、右 Switch。`onChange` 由调用方负责落盘 + 提示。 */
@Composable
internal fun SettingRow(
    title: String,
    hint: String,
    checked: Boolean,
    onChange: (Boolean) -> Unit,
) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, fontSize = 15.sp, color = Ink)
            Text(hint, fontSize = 12.sp, color = Muted)
        }
        Switch(
            checked = checked,
            onCheckedChange = onChange,
            colors = SwitchDefaults.colors(checkedThumbColor = Color.White, checkedTrackColor = Teal),
        )
    }
}

/** 一行动作：左说明、右按钮（开窗这类不是开关的动作）。 */
@Composable
internal fun ActionRow(title: String, hint: String, onClick: () -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, fontSize = 15.sp, color = Ink)
            Text(hint, fontSize = 12.sp, color = Muted)
        }
        Box(
            modifier = Modifier
                .border(1.dp, Teal)
                .clickable(onClick = onClick)
                .padding(horizontal = 14.dp, vertical = 6.dp),
        ) {
            Text(title.removePrefix("打开"), fontSize = 13.sp, color = Teal)
        }
    }
}

@Composable
internal fun SectionTitle(text: String) {
    Spacer(Modifier.height(14.dp))
    Text(text, fontSize = 13.sp, fontWeight = FontWeight.Bold, color = Teal)
    Divider(color = Hairline)
}

// ---------- 主体 ----------

/** 设置窗主体。状态在本地；每次改动立刻 `saveSettings` 并把结果写进底部状态行。 */
@Composable
fun SettingsWindow() {
    var s by remember { mutableStateOf(loadSettings()) }
    var status by remember { mutableStateOf("配置文件：${settingsPath()}") }
    var tab by remember { mutableStateOf(SettingsTab.COMMON) }
    // 符号页/表情页：null = 关；false = 符号；true = 表情（initialEmoji）。
    var symbolWin by remember { mutableStateOf<Boolean?>(null) }
    // 更新检测（2026-10-22 用户裁决）：打开设置自动检测一次；「关于」页可手动再测；
    // 发现新版本在内容区顶部出一条可关闭的横幅。网络失败静默（不打扰输入）。
    var update by remember { mutableStateOf<UpdateCheck?>(null) }
    var updateChecking by remember { mutableStateOf(false) }
    // 按**版本号**记「已关闭」：关掉 1.21.1 的横幅后，同一会话里手动检测出 1.22.0
    // 仍应再弹一次（旧布尔会永久压住后续版本的横幅，review 2026-10-22）。
    var dismissedUpdate by remember { mutableStateOf<String?>(null) }

    // 打开设置就意味着「用户在看/要调状态栏」：条该显示却没人拉起 ⇒ 直接拉一支
    // （2026-10-10 用户报障「设置里开了、实际没出现」的根因：装包后没重登录，
    // autostart 没跑；而设置窗原先**不拉起任何东西**）。只跑一次，失败进状态行。
    LaunchedEffect(Unit) {
        // 拉条进程 = fork/exec（还带一次锁文件探测）：挪到 IO，规矩与状态栏那边一致
        // —— 别在组合线程上碰进程与文件（2026-10-13 EDT 取证轮）。
        statusBarGuard("设置窗拉起状态栏") {
            withContext(Dispatchers.IO) { ensureStatusBarRunning(s.statusbar) }?.let { status = it }
        }
        // 内置词库首启种子（2026-10-16，康熙字典「全发」）：开窗一次把随包词库种进
        // 词库目录（标记门控；删过的不复活 —— 与 Android DictStore.seedBundled、fcitx5
        // opiSeedBundledDict 语义一字不差）。放**窗口启动**而不是词库页里：页面保持
        // 「只读目录、不写目录」的姿态（离屏探针直接渲页，页面里种会让它的空态不再空）。
        statusBarGuard("设置窗种子词库") {
            withContext(Dispatchers.IO) {
                seedBundledDict("kangxi", dictsDir()) {
                    DictsResources::class.java.getResourceAsStream("/kangxi.tsv")
                }
            }
        }
    }

    val scope = rememberCoroutineScope()
    // 落盘串行锁：从前拨一下 = EDT 同步写完；挪到 IO 后并发来自协程，锁保住「写在前、
    // 状态行在后」的旧次序（否则连拨两下可能后写先落，文件停在前一个值上）。
    val saveLock = remember { Mutex() }

    /** 检测更新（自动/手动同一入口）。网络在 IO 上跑，绝不阻塞设置窗。 */
    fun triggerUpdateCheck(manual: Boolean = false) {
        if (updateChecking) return
        updateChecking = true
        scope.launch {
            try {
                val result = withContext(Dispatchers.IO) { checkForUpdate() }
                // 自动检测失败不打扰：保留上一次成功结论；只有手点「检测更新」才显示原因。
                update = if (!manual && result is UpdateCheck.Failed) update else result
            } catch (e: CancellationException) {
                throw e
            } catch (t: Throwable) {
                logStatusBarCrash("更新检测", t)
                if (manual) update = UpdateCheck.Failed("内部错误：${t.javaClass.simpleName}")
            } finally {
                updateChecking = false
            }
        }
    }
    // 用户裁决（2026-10-22）：打开设置时自动检测一次；发现新版本走顶部横幅。
    LaunchedEffect(Unit) { triggerUpdateCheck() }

    fun apply(next: OpSettings) {
        // 候选字号（2026-10-14）：**只在改这一档时**才写 classicui.conf —— 每次保存都写会
        // 在无关开关上把用户手改的 Font= 盖回（契约见 ClassicUiFont.kt）。Windows 没有
        // fcitx5：字号只写 settings.ini，由候选窗 1s 轮询跟随（CandidateWindow.kt）。
        val fontChanged = next.candidateFont != s.candidateFont
        val skinChanged = next.skin != s.skin
        s = next
        // ⚠️ saveSettings 里是文件写 + `notifyFcitxReload`（busctl 那条 `waitFor` **无超时**，
        // 2026-10-13 实抓 EDT 停在那里）—— 整个保存挪出组合线程；拨开关仍然即时可见（s
        // 已先写），状态行晚一步出现。措辞与状态栏的换肤提示共用一份（saveStatusMessage）。
        scope.launch {
            saveLock.withLock {
                try {
                    val live = withContext(Dispatchers.IO) { saveSettings(next) }
                    var msg = saveStatusMessage(live)
                    if (fontChanged && !isWindows()) {
                        // classicui 那条 = 文件写 + ReloadAddonConfig 子进程，同样挪到 IO
                        // （与状态栏那边的纪律一致；失败的措辞与 saveStatusMessage 同调）。
                        val uiOk = withContext(Dispatchers.IO) { applyClassicUiFont(next.candidateFont) }
                        msg += if (uiOk) {
                            "；候选字号已写入 classicui.conf 并重载"
                        } else {
                            "；候选字号已写入 classicui.conf，但没能通知 fcitx5 重载（它没在跑？重启输入法后生效）"
                        }
                    } else if (fontChanged) {
                        // Windows：候选窗与状态栏都 1s 轮询 settings.ini（同一套同步机制）。
                        msg += "；候选字号已保存，候选窗将在 1 秒内跟随"
                    }
                    if (skinChanged && isWindows()) {
                        msg += "；候选窗与状态栏将在 1 秒内跟随新皮肤"
                    }
                    status = msg
                } catch (e: CancellationException) {
                    throw e
                } catch (t: Throwable) {
                    // 磁盘满/权限/子进程异常：设置窗继续活着，状态行说人话，栈进崩溃日志。
                    logStatusBarCrash("设置保存", t)
                    status = "保存失败：${t.javaClass.simpleName}（详见状态栏崩溃日志）"
                }
            }
        }
    }

    Row(Modifier.fillMaxSize().background(Paper)) {
        NavColumn(tab) { tab = it }
        Column(Modifier.weight(1f).fillMaxHeight()) {
            // 新版本横幅（2026-10-22 用户裁决）：只在检测到更新时出现，可关闭；
            // 不挡任何页 —— 它在这条右栏的顶部，页面在它下面滚动。
            (update as? UpdateCheck.Available)?.let { available ->
                if (dismissedUpdate != available.latest) {
                    UpdateBanner(available.latest, available.download) {
                        dismissedUpdate = available.latest
                    }
                }
            }
            Column(
                Modifier.weight(1f).fillMaxWidth()
                    .verticalScroll(rememberScrollState()).padding(horizontal = 22.dp, vertical = 6.dp),
            ) {
                when (tab) {
                    SettingsTab.COMMON -> CommonPage(s, ::apply) { status = it }
                    SettingsTab.KEYS -> KeysPage(s, ::apply)
                    SettingsTab.APPEARANCE -> AppearancePage(s, ::apply)
                    SettingsTab.STATUSBAR -> StatusBarPage(s, ::apply) { status = it }
                    SettingsTab.SYMBOLS -> SymbolsPage(
                        onOpenSymbols = { symbolWin = false },
                        onOpenEmoji = { symbolWin = true },
                    )
                    // 尾 lambda 写进括号里：DictsPage 的最后一个参数是 dir（带缺省），
                    // 尾 lambda 只绑「最后一个参数是函数类型」的位置。
                    SettingsTab.DICTS -> DictsPage(s, ::apply, { status = it })
                    SettingsTab.WUBI -> WubiPage(s, ::apply)
                    SettingsTab.ABOUT -> AboutPage(
                        status = status,
                        update = update,
                        checking = updateChecking,
                        onCheckUpdate = { triggerUpdateCheck(manual = true) },
                    )
                }
            }
            // 底部状态行：拨完任何开关都能立刻看到结果（此前它在「关于」页里，换个页就看不见了）。
            Row(
                modifier = Modifier.fillMaxWidth().height(28.dp).background(NavBg)
                    .padding(horizontal = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(status, fontSize = 11.sp, color = Muted, maxLines = 1)
            }
        }
    }
    symbolWin?.let { emoji ->
        SymbolWindow(
            onCommit = ::desktopSymbolCommit,
            onClose = { symbolWin = null },
            initialEmoji = emoji,
        )
    }
}

/**
 * 新版本横幅：一句话 + 「前往下载」+ 关闭。出现在右栏顶部（所有页都看得见）。
 * 链接走系统默认浏览器（与「关于」页的外链同一条 `LocalUriHandler`）。
 */
@Composable
private fun UpdateBanner(latest: String, download: String, onDismiss: () -> Unit) {
    val uriHandler = LocalUriHandler.current
    Row(
        modifier = Modifier.fillMaxWidth()
            .background(Color(0xFFEAF4F2))
            .border(1.dp, Teal.copy(alpha = 0.35f))
            .padding(horizontal = 14.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text("发现新版本 v$latest（当前 v$OPI_VERSION）", fontSize = 12.sp, color = Ink)
        Spacer(Modifier.weight(1f))
        Text(
            "前往下载",
            fontSize = 12.sp,
            fontWeight = FontWeight.Bold,
            color = Teal,
            modifier = Modifier.clickable {
                runCatching { uriHandler.openUri(download) }
            },
        )
        Spacer(Modifier.width(14.dp))
        Text(
            "×",
            fontSize = 15.sp,
            color = Muted,
            modifier = Modifier.clickable(onClick = onDismiss),
        )
    }
}

/** 左导航列：顶部小欧 + 标题，中间分类，底部版本与版权。 */
@Composable
private fun NavColumn(selected: SettingsTab, onSelect: (SettingsTab) -> Unit) {
    Column(Modifier.width(180.dp).fillMaxHeight().background(NavBg)) {
        Row(
            modifier = Modifier.fillMaxWidth().padding(start = 14.dp, top = 14.dp, end = 10.dp, bottom = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            OpiPet(mood = PetMood.IDLE, size = 44.dp, animate = true) // 起侧壁刻 OPI；设置页常驻动效
            Spacer(Modifier.width(8.dp))
            Column {
                Text("小欧", fontSize = 18.sp, fontWeight = FontWeight.Bold, color = Ink)
                Text("属性设置", fontSize = 11.sp, color = Muted)
            }
        }
        for (t in settingsTabs(isWindows())) {
            val on = t == selected
            Row(
                modifier = Modifier.fillMaxWidth()
                    .background(if (on) NavSelBg else Color.Transparent)
                    .clickable { onSelect(t) }
                    .padding(start = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                // 青色左条：选中标记（比整行反色轻，与主流输入法那类设置页一致）
                Box(
                    Modifier.width(3.dp).height(20.dp)
                        .background(if (on) Teal else Color.Transparent),
                )
                Text(
                    t.label,
                    modifier = Modifier.padding(start = 11.dp, top = 9.dp, bottom = 9.dp),
                    fontSize = 14.sp,
                    fontWeight = if (on) FontWeight.Bold else FontWeight.Normal,
                    color = if (on) Teal else Ink,
                )
            }
        }
        Spacer(Modifier.weight(1f))
        Column(Modifier.fillMaxWidth().padding(start = 17.dp, bottom = 12.dp)) {
            // OPI_VERSION 是构建时生成的（build.gradle.kts 的 opiAppVersion）——
            // 此前这里写死过 "1.4.0"，到 1.6.0 就成了假话，单一来源才不会再漂。
            Text("回归本质、人人可用的多端输入法", fontSize = 10.sp, color = Muted)
            Text("v$OPI_VERSION · © 2026 erik.xyz", fontSize = 10.sp, color = Muted)
        }
    }
}
