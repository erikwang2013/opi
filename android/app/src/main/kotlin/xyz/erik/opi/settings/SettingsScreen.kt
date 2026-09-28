// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.settings

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
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
import xyz.erik.opi.engine.UserWordStore
import xyz.erik.opi.ime.HandlerDebouncer
import xyz.erik.opi.jni.OpiEngine
import xyz.erik.opi.pet.OpiPet
import xyz.erik.opi.pet.petMood
import kotlinx.coroutines.launch
import java.io.File
import java.io.IOException

/**
 * 设置页（对齐 flutter settings_page.dart）：学习开关 / 清除用户词库（确认对话框）/
 * 导出词库 JSON 到剪贴板 / 从剪贴板或文件导入词库 JSON。JNI 直接调 Rust 静态单例——
 * 设置页与 IME 共享引擎与 Learner（spec §5），开关即时生效。
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

    /**
     * 设置页侧的落盘实例（第二个 UserWordStore，与 IME 那个同文件同进程 —— 世代号
     * [UserWordStore.invalidate] 就是为这种双实例场景准备的）。
     *
     * 导入必须自己落盘：引擎内存里的词只有 IME 下次选词时才会被写进文件，用户导入完
     * 不敲字就重启，IME 启动时又拿**旧文件**盖回去 —— 这不是「导入没生效」而是
     * 「导入生效过又消失」，比直接失败更难查。
     */
    val store = remember {
        UserWordStore(
            file = File(context.filesDir, UserWordStore.FILE_NAME),
            importJson = { OpiEngine.importUserWords(it) },
            exportJson = { OpiEngine.exportUserWords() },
            debouncer = HandlerDebouncer(),
            // 写盘跑在 io 线程：这里经 scope.launch 回到主线程再弹（Compose 状态只能在主线程改）
            onWriteFailure = { toast("导入已生效，但保存失败（重启后会丢）：$it") },
        )
    }

    /** 导入结果 → 提示。两条导入路径共用这一份，别让文案各写各的漂移。 */
    fun reportImport(result: UserWordStore.ImportResult) {
        toast(
            when (result) {
                is UserWordStore.ImportResult.Imported ->
                    if (result.count == 0) "词表里没有词条" else "已导入 ${result.count} 条"
                is UserWordStore.ImportResult.Rejected -> "导入失败：${result.reason}"
            }
        )
    }

    val picker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        // uri == null 是用户按返回取消，不是失败，别弹「导入失败」吓人
        if (uri != null) {
            // MIME 放宽到 octet-stream/text：云盘与 IM 转存的 .json 常被判成这两者，
            // 只收 application/json 会让文件在选择器里直接灰掉（那样就是静默的「干不了」）
            reportImport(
                store.importFrom {
                    // 有界读：回调在主线程上，`readText()` 会把整份文件变成 String 之后
                    // 引擎的条数上限才生效 —— 巨大文件就是主线程 OOM/ANR（见 readImportText）
                    val stream = context.contentResolver.openInputStream(uri)
                        ?: throw IOException("打不开所选文件")
                    stream.bufferedReader().use { r ->
                        UserWordStore.readImportText(r)
                            ?: throw IOException("文件过大，超过导入上限")
                    }
                }
            )
        }
    }

    /** 从剪贴板导入：与「导出到剪贴板」闭环（复制→粘回来），不必经文件管理器中转。 */
    fun importFromClipboard() {
        // 读剪贴板放在 readText 里（而不是先读好再传值）：读失败要变成那条可见的 Rejected，
        // 而不是在组合里抛出去崩设置页。API 29+ 只有前台应用能读剪贴板 —— 设置页在前台。
        val cm = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        reportImport(
            store.importFrom("剪贴板") {
                val text = cm.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)
                    ?.coerceToText(context)?.toString()
                    ?: throw IOException("剪贴板里没有文本")
                // 与文件路径同一个上限（见 readImportText）：超限要给可见的 Rejected，
                // 而不是主线程上漫长的解析 —— 剪贴板是别的应用能写的内容。
                UserWordStore.readImportText(text) ?: throw IOException("剪贴板内容过大，超过导入上限")
            }
        )
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
            // 小欧报到：表情跟着「学习」开关走 —— 一眼看出引擎在不在记事，
            // 比读一行说明文字快。
            ListItem(
                leadingContent = {
                    OpiPet(
                        mood = petMood(buffer = "", candidateCount = 0, learnerEnabled = learner),
                        size = 72.dp,
                    )
                },
                headlineContent = { Text("Open People Input") },
                supportingContent = {
                    Text(
                        if (learner) "小欧醒着 —— 你选的词它都记着"
                        else "小欧睡着了 —— 不记词，也不联网",
                    )
                },
            )
            HorizontalDivider()
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
            ListItem(
                headlineContent = { Text("从剪贴板导入") },
                supportingContent = { Text("粘贴导出的 JSON（与上面的复制闭环，不经文件）") },
                trailingContent = {
                    TextButton(onClick = { importFromClipboard() }) { Text("粘贴") }
                },
            )
            HorizontalDivider()
            ListItem(
                headlineContent = { Text("导入词库 JSON 文件") },
                supportingContent = { Text("选择设备上的用户词表文件") },
                trailingContent = {
                    TextButton(onClick = {
                        // 多类型一起给：见 picker 里的 MIME 注释
                        picker.launch(
                            arrayOf("application/json", "text/plain", "application/octet-stream"),
                        )
                    }) { Text("选择文件") }
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
                    UserWordStore.invalidate()
                    OpiEngine.clearUserWords()
                    // 落盘文件同步删掉：IME 下次启动会 import 它，不删就等于「清除」在
                    // 重启后被撤销（设置页与输入法同进程，共享同一引擎与同一 filesDir）
                    File(context.filesDir, UserWordStore.FILE_NAME).delete()
                    toast("已清除用户词库")
                }) { Text("清除") }
            },
            dismissButton = {
                TextButton(onClick = { confirmClear = false }) { Text("取消") }
            },
        )
    }
}
