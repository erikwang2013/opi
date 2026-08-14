package io.opi.input.candidate

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.KeyboardArrowLeft
import androidx.compose.material.icons.filled.KeyboardArrowRight
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.opi.input.engine.EngineController
import io.opi.input.engine.EngineMode

/**
 * 候选栏：拼音缓冲 + 每屏 8 候选，点击选择；页数>1 时显示 ‹ n/m › 翻页。
 * 无状态：数据与翻页状态均在 EngineController（单一状态源）。
 */
@Composable
fun CandidateBar(controller: EngineController, onTap: (Int) -> Unit) {
    // english 模式：候选栏退化为模式条，切换中/英有明确区域反馈
    if (controller.mode == EngineMode.ENGLISH) {
        Row(
            modifier = Modifier.fillMaxWidth().height(44.dp).background(Color(0xFFEEEEEE)),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                "EN",
                modifier = Modifier.padding(horizontal = 12.dp),
                fontSize = 16.sp,
                fontWeight = FontWeight.Bold,
                color = Color(0xFF455A64),
            )
            Text("字母直接上屏", fontSize = 13.sp, color = Color(0xFF757575))
        }
        return
    }
    val pageCount = controller.candidatePageCount
    val candidates = controller.pageCandidates
    Row(
        modifier = Modifier.fillMaxWidth().height(44.dp).background(Color(0xFFEEEEEE)),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            controller.buffer,
            modifier = Modifier.padding(horizontal = 12.dp),
            fontSize = 18.sp,
            color = Color(0x8A000000),
        )
        Row(
            modifier = Modifier.weight(1f).horizontalScroll(rememberScrollState()),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            for ((i, c) in candidates.withIndex()) {
                Text(
                    c,
                    modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp).clickable { onTap(i) },
                    fontSize = 20.sp,
                )
            }
            // 拼音无候选：给出可见反馈，避免"打字无反应"错觉
            if (candidates.isEmpty() && controller.buffer.isNotEmpty()) {
                Text(
                    "无匹配",
                    modifier = Modifier.padding(horizontal = 12.dp),
                    fontSize = 13.sp,
                    color = Color(0xFF9E9E9E),
                )
            }
        }
        if (pageCount > 1) {
            IconButton(onClick = controller::prevPage) {
                Icon(Icons.Filled.KeyboardArrowLeft, contentDescription = "上一页")
            }
            Text("${controller.candidatePage + 1}/$pageCount", fontSize = 13.sp)
            IconButton(onClick = controller::nextPage) {
                Icon(Icons.Filled.KeyboardArrowRight, contentDescription = "下一页")
            }
        }
    }
}
