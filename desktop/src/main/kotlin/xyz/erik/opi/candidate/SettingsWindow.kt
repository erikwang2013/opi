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
//! 见 Settings.kt 的 notifyFcitxReload）；Windows 只落盘、下次激活生效。
//!
//! 本窗在**两个宿主**里都跑：`--settings` 模式（单独进程）与状态栏的「设置」按钮
//! （状态栏进程内开一个窗，见 StatusBar.kt）—— 所以别在这里假设自己是整个应用。
//!
//! 本窗还会开**符号页/表情页**（「符号与表情」页那两个按钮）：那是同一个 SymbolWindow
//! （见 SymbolsPage.kt），上屏走 desktopSymbolCommit —— **进程拓扑与状态栏那边一样**：
//! Linux 靠 opi-ctl，Windows 只能如实失败（管道在候选窗进程手里）。

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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
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
 * 纯函数：本平台该出哪些页。Windows 上没有皮肤（fcitx5 主题）也没有 `ShiftToggle`
 * 的读侧（见 OpSettings 头注）⇒ **按键与外观两页不出**（「宁可少一页，不做点不动的
 * 开关」——那条原则在本窗是写在「关于」页里的自述，别让这里漂）。
 */
internal fun settingsTabs(windows: Boolean): List<SettingsTab> =
    SettingsTab.entries.filter { !windows || (it != SettingsTab.KEYS && it != SettingsTab.APPEARANCE) }

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

    // 打开设置就意味着「用户在看/要调状态栏」：条该显示却没人拉起 ⇒ 直接拉一支
    // （2026-10-10 用户报障「设置里开了、实际没出现」的根因：装包后没重登录，
    // autostart 没跑；而设置窗原先**不拉起任何东西**）。只跑一次，失败进状态行。
    LaunchedEffect(Unit) {
        // 拉条进程 = fork/exec（还带一次锁文件探测）：挪到 IO，规矩与状态栏那边一致
        // —— 别在组合线程上碰进程与文件（2026-10-13 EDT 取证轮）。
        withContext(Dispatchers.IO) { ensureStatusBarRunning(s.statusbar) }?.let { status = it }
        // 内置词库首启种子（2026-10-16，康熙字典「全发」）：开窗一次把随包词库种进
        // 词库目录（标记门控；删过的不复活 —— 与 Android DictStore.seedBundled、fcitx5
        // opiSeedBundledDict 语义一字不差）。放**窗口启动**而不是词库页里：页面保持
        // 「只读目录、不写目录」的姿态（离屏探针直接渲页，页面里种会让它的空态不再空）。
        withContext(Dispatchers.IO) {
            seedBundledDict("kangxi", dictsDir()) {
                DictsResources::class.java.getResourceAsStream("/kangxi.tsv")
            }
        }
    }

    val scope = rememberCoroutineScope()
    // 落盘串行锁：从前拨一下 = EDT 同步写完；挪到 IO 后并发来自协程，锁保住「写在前、
    // 状态行在后」的旧次序（否则连拨两下可能后写先落，文件停在前一个值上）。
    val saveLock = remember { Mutex() }

    fun apply(next: OpSettings) {
        // 候选字号（2026-10-14）：**只在改这一档时**才写 classicui.conf —— 每次保存都写会
        // 在无关开关上把用户手改的 Font= 盖回（契约见 ClassicUiFont.kt）。外观页在 Windows
        // 上不出现 ⇒ 那条路永不触发（外加一道 isWindows 防线：免得手写的配置值把文件写到
        // Windows 的 ~/.config 去）。
        val fontChanged = !isWindows() && next.candidateFont != s.candidateFont
        s = next
        // ⚠️ saveSettings 里是文件写 + `notifyFcitxReload`（busctl 那条 `waitFor` **无超时**，
        // 2026-10-13 实抓 EDT 停在那里）—— 整个保存挪出组合线程；拨开关仍然即时可见（s
        // 已先写），状态行晚一步出现。措辞与状态栏的换肤提示共用一份（saveStatusMessage）。
        scope.launch {
            saveLock.withLock {
                val live = withContext(Dispatchers.IO) { saveSettings(next) }
                var msg = saveStatusMessage(live)
                if (fontChanged) {
                    // classicui 那条 = 文件写 + ReloadAddonConfig 子进程，同样挪到 IO
                    // （与状态栏那边的纪律一致；失败的措辞与 saveStatusMessage 同调）。
                    val uiOk = withContext(Dispatchers.IO) { applyClassicUiFont(next.candidateFont) }
                    msg += if (uiOk) {
                        "；候选字号已写入 classicui.conf 并重载"
                    } else {
                        "；候选字号已写入 classicui.conf，但没能通知 fcitx5 重载（它没在跑？重启输入法后生效）"
                    }
                }
                status = msg
            }
        }
    }

    Row(Modifier.fillMaxSize().background(Paper)) {
        NavColumn(tab) { tab = it }
        Column(Modifier.weight(1f).fillMaxHeight()) {
            Column(
                Modifier.weight(1f).fillMaxWidth()
                    .verticalScroll(rememberScrollState()).padding(horizontal = 22.dp, vertical = 6.dp),
            ) {
                when (tab) {
                    SettingsTab.COMMON -> CommonPage(s, ::apply)
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
                    SettingsTab.ABOUT -> AboutPage(status)
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
