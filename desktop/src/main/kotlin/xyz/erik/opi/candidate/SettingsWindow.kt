// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「属性设置」窗口（Compose 自绘；配色沿用项目色 —— 宣纸白底、青、朱砂）。
//!
//! 交互模型：**改一下就落盘 + 通知 IME**（不像桌面软件那样要再点「确定」）——
//! 参照对象是搜狗那类输入的属性设置：开关拨完即生效，底部只如实显示结果。
//! Linux 走 `fcitx5-remote -r`（即时）；Windows 只落盘、下次激活生效（见 Settings.kt）。

package xyz.erik.opi.candidate

import androidx.compose.foundation.Canvas
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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.Divider
import androidx.compose.material.Switch
import androidx.compose.material.SwitchDefaults
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import xyz.erik.opi.pet.OpiPet
import xyz.erik.opi.pet.PetMood

// ---------- 配色（与 docs/opi-pet.svg 同一套） ----------
private val Paper = Color(0xFFFBFAF6) // 宣纸白
private val Ink = Color(0xFF22272E) // 正文
private val Muted = Color(0xFF6B7671) // 次级文字
private val Teal = Color(0xFF2E8B80) // 青（开关选中）
private val Hairline = Color(0xFFE3E8E6) // 分隔线

/** 一行开关：左说明、右 Switch。`onChange` 由调用方负责落盘 + 提示。 */
@Composable
private fun SettingRow(
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

@Composable
private fun SectionTitle(text: String) {
    Spacer(Modifier.height(14.dp))
    Text(text, fontSize = 13.sp, fontWeight = FontWeight.Bold, color = Teal)
    Divider(color = Hairline)
}

// ---------- 皮肤选择器（2026-10-09：外层皮肤设置做成可视卡，替掉纯文字清单）----------

/**
 * 一款皮肤的预览色（bg/fg/accent 三项）。
 * ⚠️ 与 `scripts/gen_fcitx5_themes.py` 的 PALETTES 逐值对应 —— 改了主题生成器
 * 要同步这里，否则「图与实不符」，而这种错不报任何错（本仓的老毛病）。
 */
private data class SkinPreview(val bg: Color, val fg: Color, val accent: Color)

private fun previewOf(skin: SkinOption): SkinPreview = when (skin) {
    // 「不改动」不指定主题：预览画一浅一深的双色底（表意「跟随系统」）。
    SkinOption.KEEP -> SkinPreview(Color(0xFFF2F2F2), Color(0xFF888888), Teal)
    SkinOption.LIGHT -> SkinPreview(Color(0xFFFFFFFF), Color(0xFF1F1F1F), Color(0xFFC0392B))
    SkinOption.DARK -> SkinPreview(Color(0xFF1F1F1F), Color(0xFFE6E6E6), Color(0xFFFF8A80))
    SkinOption.PAPER -> SkinPreview(Color(0xFFF5EFE1), Color(0xFF3D3222), Color(0xFF8A5A2B))
    SkinOption.CINNABAR -> SkinPreview(Color(0xFFB3272D), Color(0xFFFFF6EC), Color(0xFFFFD9A0))
}

/**
 * 五张卡一行排开（窗口内容宽 460-22×2=416dp；5×70 + 4×8 = 382 ≤ 416 —— 首版
 * 76/10 是 420 > 416，朱砂会掉到第二行，截图复核时抓到的）。窄了仍会折行。
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

/** 设置窗主体。状态在本地；每次改动立刻 `saveSettings` 并把结果写进状态行。 */
@Composable
fun SettingsWindow() {
    var s by remember { mutableStateOf(loadSettings()) }
    var status by remember { mutableStateOf("配置文件：${settingsPath()}") }

    fun apply(next: OpSettings) {
        s = next
        val live = saveSettings(next)
        status = if (live) {
            "已保存并通知 fcitx5 重载（立即生效）"
        } else if (isWindows()) {
            "已保存；Windows 侧在**输入法下一次激活**时生效"
        } else {
            "已保存到文件；但没能通知 fcitx5 重载（它没在跑？重启输入法后生效）"
        }
    }

    Column(
        modifier = Modifier.fillMaxSize().background(Paper)
            .verticalScroll(rememberScrollState()).padding(22.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            OpiPet(mood = PetMood.IDLE, size = 64.dp) // 64dp 起侧壁刻 OPI
            Spacer(Modifier.width(14.dp))
            Column {
                Text("OPI 属性设置", fontSize = 21.sp, fontWeight = FontWeight.Bold, color = Ink)
                // OPI_VERSION 是构建时生成的（build.gradle.kts 的 opiAppVersion）——
                // 此前这里写死过 "1.4.0"，到 1.6.0 就成了假话，单一来源才不会再漂。
                Text("回归本质、人人可用的多端输入法 · v$OPI_VERSION", fontSize = 12.sp, color = Muted)
            }
        }

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
        SettingRow(
            "模糊音",
            "z↔zh、c↔ch、s↔sh、n↔l、f↔h 等近音互查",
            s.fuzzy,
        ) { apply(s.copy(fuzzy = it)) }
        SettingRow(
            "简拼",
            "声母缩写：nh → 你好、zg → 中国",
            s.jianpin,
        ) { apply(s.copy(jianpin = it)) }

        // 皮肤只在 Linux 侧有意义（fcitx5 主题）；Windows 上不显示 ——「宁可少一页，
        // 不做点不动的开关」是这页的既有原则（见下面「关于」一节）。
        if (!isWindows()) {
            SectionTitle("外观")
            Spacer(Modifier.height(8.dp))
            SkinPicker(s.skin) { apply(s.copy(skin = it)) }
            Spacer(Modifier.height(7.dp))
            Text(
                "皮肤是 fcitx5 的全局主题（对所有输入法生效）；选一款即时换。" +
                    "「不改动」= OPI 不介入主题。",
                fontSize = 12.sp,
                color = Muted,
            )
        }

        SectionTitle("关于")
        Spacer(Modifier.height(6.dp))
        Text(status, fontSize = 12.sp, color = Muted)
        Spacer(Modifier.height(4.dp))
        // 一句话说明「为什么没有别的页」——搜狗那套有十几页，我们引擎现在只有这几个
        // 真旋钮；把引擎没有的开关摆上来就是假承诺（宁可少一页，不做点不动的开关）。
        Text(
            "本页只列引擎真的支持的开关；按键 / 词库那些页要等引擎支持了再加。",
            fontSize = 12.sp,
            color = Muted,
        )
        Spacer(Modifier.height(10.dp))
        Canvas(Modifier.fillMaxWidth().height(9.dp)) {
            // 朱砂短线：与小欧头顶的声调天线同一笔（本窗唯一的暖色）。
            drawLine(
                color = Color(0xFFD9553F),
                start = Offset(0f, size.height / 2),
                end = Offset(104.dp.toPx(), size.height / 2),
                strokeWidth = size.height,
            )
        }
        Spacer(Modifier.height(4.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Text("MIT 许可 · 开源", fontSize = 12.sp, color = Muted)
            // 版权行（2026-10-09 用户裁决「页面增加版权」）。
            Text("© 2026 erik.xyz", fontSize = 12.sp, color = Muted)
        }
    }
}
