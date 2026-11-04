// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 桌面传输的目标设备行：选中态用整行底色 + 加粗边框，避免只靠按钮禁用态看不出。

package xyz.erik.opi.candidate

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

@Composable
internal fun TransferTargetRow(
    name: String,
    selected: Boolean,
    onClick: () -> Unit,
) {
    val shape = RoundedCornerShape(9.dp)
    Row(
        Modifier.fillMaxWidth()
            .padding(vertical = 2.dp)
            .clip(shape)
            .background(if (selected) Teal.copy(alpha = 0.12f) else Color.Transparent)
            .border(if (selected) 2.dp else 1.dp, if (selected) Teal else Hairline, shape)
            .clickable(enabled = !selected, onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 9.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(
                name,
                fontSize = 13.sp,
                color = if (selected) Teal else Ink,
                fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal,
            )
            if (selected) {
                Text("当前发送目标", fontSize = 10.sp, color = Teal)
            }
        }
        Spacer(Modifier.width(8.dp))
        Text(
            if (selected) "已选 ✓" else "选择",
            fontSize = 12.sp,
            color = if (selected) Teal else Muted,
            fontWeight = if (selected) FontWeight.Bold else FontWeight.Normal,
        )
    }
}
