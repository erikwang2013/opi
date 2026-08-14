package io.opi.input.keyboard

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** 通用键：圆角灰底，支持长按与高亮（⇧ 激活态用）。 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun KeyButton(
    label: String,
    modifier: Modifier = Modifier,
    onTap: (() -> Unit)? = null,
    onLongPress: (() -> Unit)? = null,
    highlighted: Boolean = false,
) {
    val bg = if (highlighted) Color(0xFF546E7A) else Color(0xFFE0E0E0) // blueGrey600 / grey300
    Box(
        modifier = modifier
            .padding(1.dp)
            .clip(RoundedCornerShape(6.dp))
            .background(bg)
            .combinedClickable(onClick = { onTap?.invoke() }, onLongClick = onLongPress)
            .fillMaxSize(),
        contentAlignment = Alignment.Center,
    ) {
        Text(label, fontSize = 18.sp, maxLines = 1)
    }
}
