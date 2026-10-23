// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.keyboard.LocalKeyboardPalette
import xyz.erik.opi.settings.FUZZY_PAIRS_ANDROID
import xyz.erik.opi.settings.ImePreferences
import xyz.erik.opi.settings.SHUANGPIN_LAYOUTS_ANDROID
import xyz.erik.opi.settings.fuzzyMaskOf
import xyz.erik.opi.settings.shuangpinLayoutIndexOf

/**
 * **设置面板**（2026-10-22 用户裁决：工具条齿轮不再启动 app 设置页，改成像「关于」那样
 * 的键盘内面板）。
 *
 * 只收打字时顺手会调的六项：学习 / 模糊音总开关 / 拼音纠错 / 简拼 / 表情中显示自有 /
 * 双拼布局，外加「清除用户词库」（2026-10-23 用户裁决）。逐对模糊音、词库导入导出与
 * 词库/五笔仍留在 app 设置页 —— 那些要么不是逐次调的东西，要么必须离开输入目标去操作。
 * 清除入口与设置页共用 [xyz.erik.opi.engine.clearUserWords]（引擎内存 + 落盘两半）。
 *
 * 落盘与生效和设置页逐条同路：偏好走 [ImePreferences]，引擎写侧走服务里那个
 * [EngineController]（写引擎 + 重读，见其 `setXxx` 注释的单一写入口约定）。模糊音只有
 * 总开关，但必须写**逐对掩码**：列表非空时引擎以列表为准，只写布尔关不掉。
 */
@Composable
fun SettingsPanel(
    controller: EngineController,
    onClose: () -> Unit,
    onClearUserWords: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val p = LocalKeyboardPalette.current
    val context = LocalContext.current
    val prefs = remember { ImePreferences.from(context) }
    var learner by remember { mutableStateOf(prefs.learn) }
    var fuzzy by remember { mutableStateOf(prefs.fuzzy) }
    var fuzzyPairs by remember { mutableStateOf(prefs.fuzzyPairs) }
    var correction by remember { mutableStateOf(prefs.correction) }
    var jianpin by remember { mutableStateOf(prefs.jianpin) }
    var showOwn by remember { mutableStateOf(prefs.showOwn) }
    var layout by remember { mutableStateOf(prefs.shuangpinLayout) }
    var layoutOpen by remember { mutableStateOf(false) }
    var clearOpen by remember { mutableStateOf(false) }
    // 成功/失败反馈：清除会碰 JNI，失败不能静默（否则用户以为已清）。
    var clearFailed by remember { mutableStateOf<Boolean?>(null) }
    // 总开关的勾选态与设置页同一条判据：逐对列表非空以列表为准，空则回落旧布尔。
    val fuzzyOn = if (fuzzyPairs.isEmpty()) {
        fuzzy
    } else {
        fuzzyPairs.split(',').any { it.trim() in FUZZY_PAIRS_ANDROID }
    }

    Column(modifier = modifier.fillMaxSize().background(p.panelBg)) {
        PanelHeader("设置", "", onClose)
        Column(
            modifier = Modifier.fillMaxWidth().weight(1f)
                .verticalScroll(rememberScrollState())
                .padding(start = 14.dp, end = 14.dp, bottom = 10.dp),
        ) {
            ToggleRow("学习", learner) { v ->
                learner = v
                controller.setLearner(v)
                prefs.learn = v
            }
            ToggleRow("模糊音", fuzzyOn) { v ->
                // 开 = 全勾、关 = 全不勾（与设置页总开关同一语义；逐对微调在 app 设置页）。
                val list = if (v) FUZZY_PAIRS_ANDROID.joinToString(",") else ""
                fuzzy = v
                fuzzyPairs = list
                controller.setFuzzyPairs(fuzzyMaskOf(list))
                prefs.fuzzyPairs = list
                prefs.fuzzy = v
            }
            ToggleRow("拼音纠错", correction) { v ->
                correction = v
                controller.setCorrection(v)
                prefs.correction = v
            }
            ToggleRow("简拼", jianpin) { v ->
                jianpin = v
                controller.setJianpin(v)
                prefs.jianpin = v
            }
            ToggleRow("表情中显示自有", showOwn) { v ->
                showOwn = v
                prefs.showOwn = v
            }
            // 双拼布局：内联展开，不用 Dialog —— IME 宿主是 Service，弹窗没有 Activity token。
            Row(
                modifier = Modifier.fillMaxWidth()
                    .clip(RoundedCornerShape(6.dp))
                    .clickable(role = Role.Button) { layoutOpen = !layoutOpen }
                    .padding(vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text("双拼布局", fontSize = 12.sp, color = p.panelFg, modifier = Modifier.weight(1f))
                Text("$layout ${if (layoutOpen) "▴" else "▾"}", fontSize = 12.sp, color = p.accent)
            }
            if (layoutOpen) {
                for (name in SHUANGPIN_LAYOUTS_ANDROID) {
                    val selected = name == layout
                    Text(
                        (if (selected) "✓ " else "　") + name,
                        fontSize = 12.sp,
                        color = if (selected) p.accent else p.panelFg,
                        modifier = Modifier.fillMaxWidth()
                            .clip(RoundedCornerShape(6.dp))
                            .clickable(role = Role.Button) {
                                layout = name
                                prefs.shuangpinLayout = name
                                // 即时生效：引擎只改「下次解码怎么切」，不清缓冲。
                                controller.setShuangpinLayout(shuangpinLayoutIndexOf(name))
                                layoutOpen = false
                            }
                            .padding(vertical = 6.dp),
                    )
                }
            }
            // 清除用户词库（2026-10-23 用户裁决）：内联二次确认，不用 Dialog —— 同上，
            // IME 是 Service，没有 Activity token。
            if (!clearOpen) {
                Text(
                    "清除用户词库",
                    fontSize = 12.sp,
                    color = p.panelFg,
                    modifier = Modifier.fillMaxWidth()
                        .clip(RoundedCornerShape(6.dp))
                        .clickable(role = Role.Button) {
                            clearOpen = true
                            clearFailed = null
                        }
                        .padding(vertical = 8.dp),
                )
            } else {
                Text(
                    "将删除所有学习到的用户词，确定吗？",
                    fontSize = 12.sp,
                    color = p.panelFg,
                    modifier = Modifier.fillMaxWidth().padding(top = 8.dp),
                )
                Row(verticalAlignment = Alignment.CenterVertically) {
                    TextButton(onClick = { clearOpen = false }) { Text("取消") }
                    TextButton(onClick = {
                        clearOpen = false
                        clearFailed = try {
                            onClearUserWords()
                            false
                        } catch (e: Throwable) {
                            // 落盘那半已先清；引擎失败（如死 .so）必须让用户看见。
                            true
                        }
                    }) { Text("清除") }
                }
            }
            if (clearFailed != null) {
                Text(
                    if (clearFailed == true) "清除失败（引擎不可用）" else "已清除用户词库",
                    fontSize = 12.sp,
                    color = if (clearFailed == true) p.accent else p.barDimFg,
                    modifier = Modifier.fillMaxWidth().padding(bottom = 6.dp),
                )
            }
            // 更新检测（2026-10-23）：齿轮面板与关于面板共用同一节，进面板即静默检测。
            UpdateSection()
        }
    }
}

/** 一行开关：标题 12sp（与关于面板同档），右侧 Material Switch。 */
@Composable
private fun ToggleRow(label: String, checked: Boolean, onChange: (Boolean) -> Unit) {
    val p = LocalKeyboardPalette.current
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 2.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(label, fontSize = 12.sp, color = p.panelFg, modifier = Modifier.weight(1f))
        Switch(
            checked = checked,
            onCheckedChange = onChange,
            // 跟随皮肤色板：缺省 Material 紫与各套皮肤冲突（开关是面板里唯一的系统控件）。
            colors = SwitchDefaults.colors(
                checkedThumbColor = p.panelBg,
                checkedTrackColor = p.accent,
                uncheckedThumbColor = p.barDimFg,
                uncheckedTrackColor = p.keyBg,
                uncheckedBorderColor = p.barDimFg,
            ),
            modifier = Modifier.semantics { contentDescription = label },
        )
    }
}
