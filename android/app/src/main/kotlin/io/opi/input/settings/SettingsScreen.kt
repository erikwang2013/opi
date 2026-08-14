package io.opi.input.settings

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.ListItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.opi.input.jni.OpiEngine
import kotlinx.coroutines.launch

/**
 * 设置页（对齐 flutter settings_page.dart）：学习开关 / 清除用户词库（确认对话框）/
 * 导出词库 JSON 到剪贴板。JNI 直接调 Rust 静态单例——设置页与 IME 共享引擎与
 * Learner（spec §5），开关即时生效。
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen() {
    val context = LocalContext.current
    val snackbar = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    var learner by remember { mutableStateOf(OpiEngine.learnerEnabled()) }
    var confirmClear by remember { mutableStateOf(false) }

    fun toast(msg: String) {
        scope.launch { snackbar.showSnackbar(msg) }
    }

    Scaffold(
        topBar = { TopAppBar(title = { Text("OPI 设置") }) },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding),
        ) {
            ListItem(
                headlineContent = { Text("学习") },
                supportingContent = { Text("根据选词学习用户词频") },
                trailingContent = {
                    Switch(
                        checked = learner,
                        onCheckedChange = { v ->
                            learner = v
                            OpiEngine.setLearner(v)
                        },
                    )
                },
            )
            HorizontalDivider()
            ListItem(
                headlineContent = { Text("清除用户词库") },
                supportingContent = { Text("删除所有学习到的用户词") },
                trailingContent = {
                    TextButton(onClick = { confirmClear = true }) { Text("清除") }
                },
            )
            HorizontalDivider()
            ListItem(
                headlineContent = { Text("导出词库 JSON") },
                supportingContent = { Text("复制到剪贴板（为云同步预留格式）") },
                trailingContent = {
                    TextButton(onClick = {
                        val json = OpiEngine.exportUserWords()
                        val cm = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                        cm.setPrimaryClip(ClipData.newPlainText("opi-words", json))
                        toast("已复制到剪贴板")
                    }) { Text("复制") }
                },
            )
            HorizontalDivider()
            Text(
                text = "注：学习/词库作用于本应用内嵌引擎实例；设置页与输入法共享同一引擎（Rust 静态单例）。",
                fontSize = 12.sp,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(16.dp),
                textAlign = TextAlign.Start,
            )
        }
    }

    if (confirmClear) {
        AlertDialog(
            onDismissRequest = { confirmClear = false },
            title = { Text("清除用户词库") },
            text = { Text("将删除所有学习到的用户词，确定吗？") },
            confirmButton = {
                TextButton(onClick = {
                    confirmClear = false
                    OpiEngine.clearUserWords()
                    toast("已清除用户词库")
                }) { Text("清除") }
            },
            dismissButton = {
                TextButton(onClick = { confirmClear = false }) { Text("取消") }
            },
        )
    }
}
