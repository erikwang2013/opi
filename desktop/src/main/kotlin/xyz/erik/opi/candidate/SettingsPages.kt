// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 设置窗的**各页内容**（壳见 SettingsWindow.kt；2026-10-12 用户裁决「布局和功能参考
//! 搜狗输入法」——左分类导航 + 右内容的拆分之一）。
//!
//! 每页只列**引擎真的支持的开关**（本窗既有原则）：页里没有的东西不是「忘了」，
//! 是引擎还没有那个旋钮 —— 摆上来就是假承诺。「关于」页里那句自述必须跟着页集合走。
//!
//! 平台分叉：皮肤（fcitx5 主题）与 Shift 轻按（`ShiftToggle`）只有 Linux 读侧认，
//! Windows 上**整页不出现**（导航项由 SettingsWindow.kt 的 `settingsTabs` 过滤）。

package xyz.erik.opi.candidate

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

// ---------- 常用 ----------

@Composable
internal fun CommonPage(s: OpSettings, apply: (OpSettings) -> Unit) {
    SectionTitle("常规")
    SettingRow(
        "学习（记住选过的词）",
        "关掉后不再累计词频；已学的词仍在（清空要用各端自己的入口）",
        s.learn,
    ) { apply(s.copy(learn = it)) }
    SettingRow(
        "中文标点",
        "，。、 与成对引号的自动转换（与全角开关相互独立）",
        s.chinesePunct,
    ) { apply(s.copy(chinesePunct = it)) }
    SectionTitle("输入")
    // 模糊音**逐对多选**（2026-10-13 用户裁决「可多选类似 z=zh」）：逐组一开关；
    // 全不勾 = 全关（同时写下 Fuzzy=False 的兼容投影）。名字表见 FUZZY_PAIR_NAMES
    // （锁步门禁在 SettingsTest 钉住与引擎 fuzzy.rs 同序同名）。
    SettingRow(
        "模糊音（逐对）",
        "总开关：开 = 全勾、关 = 全不勾；下面是逐组微调。配置键名 FuzzyPairs",
        s.fuzzyPairSet().isNotEmpty(),
    ) { on ->
        apply(s.copy(fuzzyPairs = if (on) FUZZY_PAIR_NAMES.joinToString(",") else "", fuzzy = on))
    }
    val checked = s.fuzzyPairSet()
    for (name in FUZZY_PAIR_NAMES) {
        val (a, b) = name.split('/')
        SettingRow("　$a ↔ $b", "", name in checked) { apply(s.toggleFuzzyPair(name)) }
    }
    SettingRow(
        "简拼",
        "声母缩写：nh → 你好、zg → 中国",
        s.jianpin,
    ) { apply(s.copy(jianpin = it)) }
}

// ---------- 按键（只有 Linux 有真旋钮，见文件头）----------

@Composable
internal fun KeysPage(s: OpSettings, apply: (OpSettings) -> Unit) {
    SectionTitle("切换键")
    SettingRow(
        "Shift 轻按切换中英文",
        "组合键（Shift+字母）与长按不触发；配置键名 ShiftToggle",
        s.shiftToggle,
    ) { apply(s.copy(shiftToggle = it)) }
    SectionTitle("固定热键（暂不可改）")
    Spacer(Modifier.height(6.dp))
    for ((key, what) in FIXED_HOTKEYS) {
        Row(Modifier.fillMaxWidth().padding(vertical = 3.dp)) {
            Text(key, fontSize = 13.sp, color = Ink, modifier = Modifier.width(120.dp))
            Text(what, fontSize = 13.sp, color = Muted)
        }
    }
    Spacer(Modifier.height(8.dp))
    // 如实说：这些键位判在插件里（opi_mode.h / opi_fcitx5.cpp），本轮没做成可配置。
    Text(
        "热键判在输入法插件里，本窗只展示。要改键位目前得改代码（改完重新编译插件）。",
        fontSize = 12.sp, color = Muted,
    )
}

/** 固定热键表（真源 = `crates/fcitx5-opi/cpp/opi_mode.h` + opi_fcitx5.cpp 的常量点）。 */
private val FIXED_HOTKEYS = listOf(
    "Ctrl+'" to "英文 ⇄ 拼音",
    "Ctrl+\\" to "符号 ⇄ 拼音",
    "Ctrl+Shift+F" to "简体 ⇄ 繁体",
    "Shift+Space" to "全角 ⇄ 半角",
    "Ctrl+Shift+Z" to "符号大全（与编辑器「重做」冲突是已知项）",
)

// ---------- 外观（皮肤；仅 Linux）----------

@Composable
internal fun AppearancePage(s: OpSettings, apply: (OpSettings) -> Unit) {
    SectionTitle("皮肤")
    Spacer(Modifier.height(8.dp))
    SkinPicker(s.skin) { apply(s.copy(skin = it)) }
    Spacer(Modifier.height(7.dp))
    Text(
        "皮肤是 fcitx5 的全局主题（对所有输入法生效）；选一款即时换。" +
            "「不改动」= OPI 不介入主题。",
        fontSize = 12.sp, color = Muted,
    )
}

// ---------- 状态栏 ----------

@Composable
internal fun StatusBarPage(s: OpSettings, apply: (OpSettings) -> Unit, status: (String) -> Unit) {
    val scope = rememberCoroutineScope()
    SectionTitle("悬浮条")
    SettingRow(
        "显示状态栏悬浮条",
        "关掉整条隐藏（进程还在跑，拨回来就出现）；隐藏后就点不到条上的设置了，" +
            "要回来用应用菜单里的「opi输入法」再进本页",
        !s.statusbar.hidden,
    ) {
        val next = s.copy(statusbar = s.statusbar.copy(hidden = !it))
        apply(next)
        // 拨回「显示」时立刻拉起（拨到隐藏不用管 —— 隐藏的语义是「进程还在跑，只是不显示」）
        // ⚠️ 拉起 = fork/exec + 锁文件探测：挪到 IO（2026-10-13 EDT 取证轮），
        // 与设置窗首次拉起那条同规矩；开关本身已即时可见（apply 先写 s）。
        scope.launch {
            withContext(Dispatchers.IO) { ensureStatusBarRunning(next.statusbar) }?.let(status)
        }
    }
    SectionTitle("条上按钮（常驻）")
    // 只列有动作的按钮 —— 与条上同一份判定（statusButtonWired），两处各判一次必然漂移。
    for (b in StatusBarButton.entries.filter { statusButtonWired(it, isWindows()) }) {
        SettingRow(b.label, b.hint, s.statusbar.resident(b)) {
            apply(s.copy(statusbar = s.statusbar.with(b, it)))
        }
    }
    Text(
        "这些开关存在配置文件里（Linux 就是 fcitx5 的 addon 配置）。" +
            "⚠️ 用 fcitx5 自带配置工具改别的选项并保存时，这些键会不会被它清掉未经验证。",
        fontSize = 12.sp, color = Muted,
    )
}

// ---------- 符号与表情（2026-10-12：用户裁决「设置中增加符号、表情选择」）----------

@Composable
internal fun SymbolsPage(onOpenSymbols: () -> Unit, onOpenEmoji: () -> Unit) {
    // 打开的窗 = 状态栏「符号」按钮同一个 SymbolWindow（大页面、可翻页），只是入口在这里。
    // 上屏通路也是同一条（desktopSymbolCommit：Linux = opi-ctl insert）。
    SectionTitle("符号大全")
    ActionRow(
        "打开符号页",
        "22 类目、12×6 一页、左右滑动翻页；点一下就把符号插进当前输入框",
    ) { onOpenSymbols() }
    SectionTitle("表情")
    ActionRow(
        "打开表情页",
        "官方分组（笑脸/人物/动物/…/旗帜）；点分类条跳组，点表情上屏",
    ) { onOpenEmoji() }
    Spacer(Modifier.height(8.dp))
    Text(
        "上屏要求 fcitx5 里有焦点输入框（点一下要输入的地方再打开本页）；" +
            "没有焦点时页面会把失败原因显示出来。",
        fontSize = 12.sp, color = Muted,
    )
    Text(
        "表情表与引擎同一份（data/raw/emoji.tsv，构建期拷进程序）；符号表见 /usr/share/opi/。",
        fontSize = 12.sp, color = Muted,
    )
}

// ---------- 关于 ----------

@Composable
internal fun AboutPage(status: String) {
    Spacer(Modifier.height(6.dp))
    Text(status, fontSize = 12.sp, color = Muted)
    Spacer(Modifier.height(10.dp))
    // 一句话说明「为什么只有这几页」——搜狗那套有十几页，我们引擎现在只有这几个真旋钮；
    // 把引擎没有的开关摆上来就是假承诺（宁可少一页，不做点不动的开关）。
    Text(
        "本窗只列引擎真的支持的开关（常用 / 按键 / 外观 / 状态栏 / 符号与表情）。" +
            "搜索、云词库、按键自定义那些页要等引擎支持了再加。",
        fontSize = 12.sp, color = Muted,
    )
    Spacer(Modifier.height(10.dp))
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Text("MIT 许可 · 开源", fontSize = 12.sp, color = Muted)
        // 版权行（2026-10-09 用户裁决「页面增加版权」）。
        Text("© 2026 erik.xyz", fontSize = 12.sp, color = Muted)
    }
}

// ---------- 皮肤选择器（2026-10-09：可视卡，替掉纯文字清单）----------

/**
 * 一款皮肤的预览色（bg/fg/accent 三项）。
 * ⚠️ 十款的色值**不再各写一份**（2026-10-12）：统一取自 `SkinPalette.skinColors`
 * —— 那份的逐值对应由 SettingsTest 的 `skin palettes match the theme generator`
 * 现读现比（同一个「图与实」门禁，只是比对对象挪去了 SkinPalette.kt）。
 */
private data class SkinPreview(val bg: Color, val fg: Color, val accent: Color)

private fun previewOf(skin: SkinOption): SkinPreview = when (skin) {
    // 「不改动」不指定主题：预览画一浅一深的双色底（表意「跟随系统」）—— 这是预览
    // 专属的中性表示，**不是** SkinPalette.KeepColors（那个是状态栏的固定深色）。
    SkinOption.KEEP -> SkinPreview(Color(0xFFF2F2F2), Color(0xFF888888), Teal)
    else -> skinColors(skin).let { SkinPreview(it.bg, it.fg, it.accent) }
}

/**
 * 十张卡按 FlowRow 折行排开（内容宽 800-180-22×2 ≈ 570dp：一行 7×70 + 6×8 = 538 ≤ 570，
 * 十款固定两行）。窄了仍会折行（首版 76/10=420 > 416 时朱砂掉到第二行，截图复核时
 * 抓到过半宽问题本身）。
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun SkinPicker(selected: SkinOption, onSelect: (SkinOption) -> Unit) {
    FlowRow(
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        for (skin in SkinOption.entries) {
            SkinCard(skin, skin == selected) { onSelect(skin) }
        }
    }
}

/** 一张皮肤卡：候选面板样张（底色 + 样字 + 朱砂/青的强调条）+ 名字。选中 = 青边 + 青字。 */
@Composable
private fun SkinCard(skin: SkinOption, selected: Boolean, onClick: () -> Unit) {
    val pv = previewOf(skin)
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        modifier = Modifier.width(70.dp).clickable(onClick = onClick).padding(2.dp),
    ) {
        Box(
            modifier = Modifier.fillMaxWidth().height(40.dp)
                .background(pv.bg)
                .border(
                    width = if (selected) 2.dp else 1.dp,
                    color = if (selected) Teal else Hairline,
                ),
            contentAlignment = Alignment.Center,
        ) {
            if (skin == SkinOption.KEEP) {
                // 「跟随系统」：一浅一深的双色底（不写字，免得误读成某个具体主题）
                Row(Modifier.fillMaxSize()) {
                    Box(Modifier.weight(1f).fillMaxHeight().background(Color(0xFFF2F2F2)))
                    Box(Modifier.weight(1f).fillMaxHeight().background(Color(0xFF3A3A3A)))
                }
            } else {
                Text("你", fontSize = 17.sp, color = pv.fg)
            }
            // 强调条贴底：每款的 accent，命中面板的选中下划线
            Box(
                Modifier.align(Alignment.BottomCenter).fillMaxWidth(0.7f).height(3.dp)
                    .background(pv.accent),
            )
        }
        Spacer(Modifier.height(5.dp))
        Text(
            skin.value,
            fontSize = 12.sp,
            color = if (selected) Teal else Ink,
            fontWeight = if (selected) FontWeight.Bold else FontWeight.Normal,
        )
    }
}
