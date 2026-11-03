// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 设置页顶部的小欧报到（2026-11-03 从 SettingsScreen.kt 拆出守 500 行）。

package xyz.erik.opi.settings

import androidx.compose.material3.ListItem
import androidx.compose.runtime.Composable
import androidx.compose.ui.unit.dp
import xyz.erik.opi.i18n.Text
import xyz.erik.opi.pet.OpiPet
import xyz.erik.opi.pet.petMood

@Composable
internal fun SettingsMascot(dictionaryDegraded: Boolean, learner: Boolean) {
    // 优先级：**降级 > 睡着**（petMood 里 dictionaryDegraded 在链首，是有意的）。
    // 断天线说的是「引擎坏了」，比「我不记事」紧急，也不该让用户以为自己关过什么开关。
    ListItem(
        leadingContent = {
            OpiPet(
                mood = petMood(
                    buffer = "",
                    candidateCount = 0,
                    learnerEnabled = learner,
                    dictionaryDegraded = dictionaryDegraded,
                ),
                size = 72.dp,
            )
        },
        headlineContent = { Text("Open People Input") },
        supportingContent = {
            Text(
                when {
                    dictionaryDegraded -> "小欧的词典没装上 —— 现在只认得几十个常用词"
                    learner -> "小欧醒着 —— 你选的词它都记着"
                    else -> "小欧睡着了 —— 不记词，也不联网"
                },
            )
        },
    )
}
