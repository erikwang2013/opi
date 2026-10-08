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
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
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

/** 版本号与候选窗同源（`build.gradle.kts` 的 packageVersion）；此处只做展示。 */
private const val VERSION = "1.4.0"

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
                Text("回归本质、人人可用的多端输入法 · v$VERSION", fontSize = 12.sp, color = Muted)
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

        SectionTitle("关于")
        Spacer(Modifier.height(6.dp))
        Text(status, fontSize = 12.sp, color = Muted)
        Spacer(Modifier.height(4.dp))
        // 一句话说明「为什么没有别的页」——搜狗那套有十几页，我们引擎现在只有这几个
        // 真旋钮；把引擎没有的开关摆上来就是假承诺（宁可少一页，不做点不动的开关）。
        Text(
            "本页只列引擎真的支持的开关；外观 / 按键 / 词库那些页要等引擎先支持。",
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
        }
    }
}
