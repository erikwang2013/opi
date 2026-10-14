// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

/**
 * 设置页「五笔」一节（2026-10-14，M4 第二波）。六键：方案 / 风格 / 混输进入方式 /
 * 候选个数 / 回车 / 自动上屏。
 *
 * 三件事各自钉在别处，这里只做编排（与 [DictSection] 同一分工）：
 * · 名字表与「名字 → FFI 整数码」换算在 `WubiNames.kt`（锁步门禁 `WubiNamesLockstepTest`
 *   现读 `engine_wubi.rs` 比对）；
 * · **方案**（86/98）决定装哪张码表 —— 落点是 `EngineLoader.load`（读偏好、按方案装
 *   一张），本节只存偏好 + 提示「重启生效」；
 * · 其余五键即时推引擎（[ImePreferences.applyWubiSettings]）。
 *
 * ⚠️ 两条如实写、不许写成「已生效」：
 * · **回车行为**：引擎侧判据已接线（`Engine::enter_action`，fcitx5 / Windows 已消费），
 *   **Android 的 JNI 出口尚未接** —— 文案如实写「等待接线」，不写「已生效」；
 * · **编码提示**：Android 候选栏已接（2026-10-14，缓冲右边的暗色小字）—— 本节不摆开关
 *   （摆一个「显示提示」的开关在设置页会让人以为默认是关的；它跟着五笔态自动出现）。
 */
@Composable
internal fun WubiSection(prefs: ImePreferences, toast: (String) -> Unit) {
    var plan by remember { mutableStateOf(prefs.wubiPlan) }
    var style by remember { mutableStateOf(prefs.wubiStyle) }
    var mixedEntry by remember { mutableStateOf(prefs.wubiMixedEntry) }
    var limit by remember { mutableStateOf(prefs.wubiCandidateLimit) }
    var returnStyle by remember { mutableStateOf(prefs.wubiReturn) }
    var autoCommit by remember { mutableStateOf(prefs.wubiAutoCommit) }
    // ⚠️ 必须是 remember 状态：普通局部变量每次重组都回到 null ⇒ dialog 永远不弹。
    var pending by remember { mutableStateOf<String?>(null) }

    fun push() {
        prefs.wubiStyle = style
        prefs.wubiMixedEntry = mixedEntry
        prefs.wubiCandidateLimit = limit
        prefs.wubiReturn = returnStyle
        prefs.wubiAutoCommit = autoCommit
        prefs.applyWubiSettings()
    }

    HorizontalDivider()
    ListItem(
        headlineContent = { Text("五笔") },
        supportingContent = { Text("输入方案 / 风格 / 特殊习惯；拼音方式选「五笔」时生效") },
        trailingContent = {
            TextButton(onClick = { pending = "plan" }) { Text("方案 $plan") }
        },
    )
    ListItem(
        headlineContent = { Text("　输入风格") },
        supportingContent = { Text("纯五笔只出码表候选；五笔拼音把码表与拼音候选并在一栏") },
        trailingContent = { TextButton(onClick = { pending = "style" }) { Text(style) } },
    )
    ListItem(
        headlineContent = { Text("　混输进入方式") },
        supportingContent = { Text("只在「五笔拼音」时管用：z 键档下 z 码打不出来（如实）") },
        trailingContent = { TextButton(onClick = { pending = "entry" }) { Text(mixedEntry) } },
    )
    ListItem(
        headlineContent = { Text("　候选个数") },
        supportingContent = { Text("五笔候选一栏最多显示几个") },
        trailingContent = { TextButton(onClick = { pending = "limit" }) { Text("$limit") } },
    )
    ListItem(
        headlineContent = { Text("　回车键（缓冲非空时）") },
        supportingContent = {
            Text("本版只存设置：桌面端（fcitx5 / Windows）已按它走；Android 的回车要等接线（在此之前回车仍是既有行为：选首候选）")
        },
        trailingContent = { TextButton(onClick = { pending = "return" }) { Text(returnStyle) } },
    )
    ListItem(
        headlineContent = { Text("　四码唯一自动上屏") },
        supportingContent = { Text("打满四码且候选只有一个时自动上屏（同码多词不自动）") },
        trailingContent = {
            Switch(
                checked = autoCommit,
                onCheckedChange = { v ->
                    autoCommit = v
                    push()
                    toast(if (v) "四码唯一自动上屏：开" else "四码唯一自动上屏：关")
                },
            )
        },
    )

    val options: List<String>? = when (pending) {
        "plan" -> WUBI_PLAN_NAMES
        "style" -> WUBI_STYLE_NAMES
        "entry" -> WUBI_MIXED_ENTRY_NAMES
        "limit" -> WUBI_CANDIDATE_LIMIT_CHOICES.map { it.toString() }
        "return" -> WUBI_RETURN_NAMES
        else -> null
    }
    if (options != null) {
        AlertDialog(
            onDismissRequest = { pending = null },
            title = { Text("五笔 · 选择") },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    for (opt in options) {
                        val current = when (pending) {
                            "plan" -> plan
                            "style" -> style
                            "entry" -> mixedEntry
                            "limit" -> limit.toString()
                            else -> returnStyle
                        }
                        TextButton(onClick = {
                            when (pending) {
                                "plan" -> {
                                    plan = opt
                                    prefs.wubiPlan = opt
                                    // 码表在启动时按方案装 —— 这里如实说清生效时机。
                                    toast("方案切到 $opt：码表在输入法下次启动时装载")
                                }
                                "style" -> {
                                    style = opt
                                    push()
                                }
                                "entry" -> {
                                    mixedEntry = opt
                                    push()
                                }
                                "limit" -> {
                                    limit = opt.toInt()
                                    push()
                                }
                                else -> {
                                    returnStyle = opt
                                    push()
                                }
                            }
                            pending = null
                        }) { Text(if (opt == current) "✓ $opt" else opt) }
                    }
                }
            },
            confirmButton = { TextButton(onClick = { pending = null }) { Text("关闭") } },
        )
    }

    // 五笔的「词库」入口复用词库一节（五笔码表随包分发、属于系统库那一档；
    // 用户导入 tsv 走上面的「词库」节）—— 这里只留一句指引，不再摆第二个列表。
    ListItem(
        headlineContent = { Text("　五笔词库") },
        supportingContent = {
            Text(
                "五笔码表（86/98）随安装包分发，属系统库；导入自定义词库用上面的「词库」一节" +
                    "（导入层在五笔下同样参与候选）",
            )
        },
    )
    // 编码提示（2026-10-14 接线）：Android 候选栏已把它画在缓冲右边（暗色小字），
    // 无需开关 —— 这里只留一句「在哪里看」的指引。
    Text(
        "编码提示（还能接哪些键）显示在候选栏缓冲右边（五笔态、打码时出现）。",
        style = MaterialTheme.typography.bodySmall,
        modifier = Modifier.padding(horizontal = 16.dp),
    )
}
