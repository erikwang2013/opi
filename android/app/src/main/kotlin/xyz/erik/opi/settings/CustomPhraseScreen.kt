// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.settings

import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch
import xyz.erik.opi.engine.UserWordStore
import xyz.erik.opi.keyboard.OpiIcons
import java.io.IOException

/** 导入文本上限：短语表是手写的，1 MiB 远超正常规模；超限挡在读入之前（同用户词的有界读）。 */
private const val MAX_IMPORT_CHARS = 1024 * 1024

/** 编辑弹窗的目标：新增或改第 [PhraseDialog.Edit.index] 行。 */
private sealed interface PhraseDialog {
    data object Add : PhraseDialog
    data class Edit(val index: Int) : PhraseDialog
}

/**
 * 「自定义短语」设置页（2026-10-23）：启用开关 / 分页列表（每页 10 条）/
 * 增删改 / 导入导出（SAF，JSON + TSV 导出，JSON/TSV/CSV/等号/空白五种导入）。
 *
 * 数据流只有一条：**改之前先进宿主文件**（[CustomPhraseStore.save]），存盘成功才改
 * UI，再推引擎。存盘失败 = 什么都没变；引擎拒绝只提示、不回滚（宿主文件是唯一真源，
 * IME 下次启动会重推）。这样任何时刻屏幕显示的都等于磁盘上的那一份。
 *
 * @param english true = 英文套（2026-10-24）：独立文件、独立开关、只在英文键盘匹配；
 *   其余（分页/增删改/导入导出）与中文套共用这一份 UI 与 [CustomPhraseCodec]。
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CustomPhraseScreen(onBack: () -> Unit, english: Boolean = false) {
    val context = LocalContext.current
    val snackbar = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    val store = remember(english) {
        if (english) CustomPhraseStore.englishFrom(context) else CustomPhraseStore.from(context)
    }
    val initial = remember(store) { store.load() }
    var enabled by remember { mutableStateOf(initial.enabled) }
    var phrases by remember { mutableStateOf(initial.phrases) }
    var page by remember { mutableStateOf(0) }
    var dialog by remember { mutableStateOf<PhraseDialog?>(null) }
    var exportMenu by remember { mutableStateOf(false) }

    fun toast(msg: String) {
        scope.launch { snackbar.showSnackbar(msg) }
    }

    /** 一次变更的唯一落点：先存盘 → 再改 UI → 再推引擎。 */
    fun apply(nextEnabled: Boolean, next: List<CustomPhrase>, okMessage: String? = null) {
        val state = CustomPhraseStore.State(nextEnabled, next)
        val saveError = store.save(state)
        if (saveError != null) {
            toast("保存失败：$saveError")
            return
        }
        enabled = nextEnabled
        phrases = next
        page = CustomPhraseCodec.clampPage(page, next.size)
        val pushError = store.push(state)
        when {
            pushError != null -> toast("已保存，但引擎未接受：$pushError")
            okMessage != null -> toast(okMessage)
        }
    }

    fun export(uri: Uri, text: String) {
        try {
            val out = context.contentResolver.openOutputStream(uri) ?: throw IOException("打不开目标文件")
            out.bufferedWriter(Charsets.UTF_8).use { it.write(text) }
            toast("已导出")
        } catch (e: Throwable) {
            toast("导出失败：${e.message ?: e::class.simpleName}")
        }
    }

    // 两种导出格式各一个 CreateDocument：系统文件选择器按 MIME 给扩展名/默认应用。
    val exportJson = rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("application/json"),
    ) { uri ->
        if (uri != null) export(uri, CustomPhraseCodec.encodeCanonical(phrases))
    }
    val exportTsv = rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("text/tab-separated-values"),
    ) { uri ->
        if (uri != null) export(uri, CustomPhraseCodec.encodeTsv(phrases))
    }

    val importFile = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri == null) return@rememberLauncherForActivityResult // 用户取消不是失败
        val raw = try {
            val stream = context.contentResolver.openInputStream(uri)
                ?: throw IOException("打不开所选文件")
            stream.bufferedReader().use { r ->
                UserWordStore.readImportText(r, MAX_IMPORT_CHARS)
                    ?: throw IOException("文件超过 ${MAX_IMPORT_CHARS / 1024} KB")
            }
        } catch (e: Throwable) {
            toast("无法读取所选文件（${e.message ?: e::class.simpleName}）")
            return@rememberLauncherForActivityResult
        }
        when (val decoded = CustomPhraseCodec.decode(raw)) {
            is CustomPhraseDecode.Invalid -> toast("导入失败：${decoded.reason}")
            is CustomPhraseDecode.Ok -> {
                val merged = CustomPhraseCodec.merge(phrases, decoded.phrases)
                if (merged.phrases.size > CustomPhraseCodec.MAX_PHRASES) {
                    toast(
                        "导入失败：合并后达到上限（最多 ${CustomPhraseCodec.MAX_PHRASES} 条，" +
                            "当前 ${merged.phrases.size} 条）",
                    )
                    return@rememberLauncherForActivityResult
                }
                val state = CustomPhraseStore.State(enabled, merged.phrases)
                val saveError = store.save(state)
                if (saveError != null) {
                    // 整份拒收（不是部分写入）：解析已经全成功，只是没落盘成功。
                    toast("导入失败，未保存：$saveError")
                    return@rememberLauncherForActivityResult
                }
                phrases = merged.phrases
                page = 0
                val pushError = store.push(state)
                if (pushError != null) {
                    toast("已导入，但引擎未接受：$pushError")
                } else {
                    toast("导入完成：新增 ${merged.added} 条，更新 ${merged.updated} 条")
                }
            }
        }
    }

    Scaffold(modifier = Modifier.opiSettingsSceneBackground(), containerColor = Color.Transparent,
        topBar = {
            TopAppBar(
                title = { Text(if (english) "英文自定义短语" else "自定义短语") },
                colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent),
                navigationIcon = {
                    IconButton(onClick = onBack) { Icon(OpiIcons.ArrowBack, contentDescription = "返回") }
                },
            )
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { padding ->
        Box(
            modifier = Modifier.fillMaxSize().padding(padding),
            contentAlignment = Alignment.TopCenter,
        ) {
            Column(
                modifier = Modifier
                    .widthIn(max = 640.dp)
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState()),
            ) {
                ListItem(
                    headlineContent = { Text("启用自定义短语") },
                    supportingContent = {
                        Text(
                            if (english) {
                                "只在英文键盘匹配：输入完整字母后出现短语"
                            } else {
                                "输入完整字母后，候选栏出现对应短语"
                            },
                        )
                    },
                    trailingContent = {
                        Switch(
                            checked = enabled,
                            onCheckedChange = { v -> apply(v, phrases) },
                        )
                    },
                )
                HorizontalDivider()
                ListItem(
                    headlineContent = { Text("共 ${phrases.size} 条") },
                    supportingContent = { Text("每页 ${CustomPhraseCodec.PAGE_SIZE} 条") },
                    trailingContent = {
                        TextButton(onClick = { dialog = PhraseDialog.Add }) { Text("添加") }
                    },
                )

                if (phrases.isEmpty()) {
                    Text(
                        text = "还没有短语。点右上角「添加」，或从文件导入。",
                        modifier = Modifier.fillMaxWidth().padding(16.dp),
                        fontSize = 14.sp,
                    )
                } else {
                    val pageIndex = CustomPhraseCodec.clampPage(page, phrases.size)
                    val pageCount = CustomPhraseCodec.pageCount(phrases.size)
                    val visible = CustomPhraseCodec.pageSlice(phrases, pageIndex)
                    val start = pageIndex * CustomPhraseCodec.PAGE_SIZE
                    for ((offset, phrase) in visible.withIndex()) {
                        val row = start + offset
                        ListItem(
                            headlineContent = { Text(phrase.trigger) },
                            supportingContent = {
                                Text(phrase.text, maxLines = 2, overflow = TextOverflow.Ellipsis)
                            },
                            trailingContent = {
                                Row {
                                    TextButton(onClick = { dialog = PhraseDialog.Edit(row) }) {
                                        Text("编辑")
                                    }
                                    TextButton(onClick = {
                                        apply(
                                            enabled,
                                            phrases.filterIndexed { i, _ -> i != row },
                                            "已删除",
                                        )
                                    }) { Text("删除") }
                                }
                            },
                        )
                    }
                    Row(
                        modifier = Modifier.fillMaxWidth().padding(horizontal = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        TextButton(
                            onClick = { page = pageIndex - 1 },
                            enabled = pageIndex > 0,
                            modifier = Modifier.weight(1f),
                        ) { Text("‹ 上一页") }
                        Text(
                            text = "第 ${pageIndex + 1} / $pageCount 页",
                            modifier = Modifier.weight(1f),
                            textAlign = TextAlign.Center,
                            fontSize = 13.sp,
                        )
                        TextButton(
                            onClick = { page = pageIndex + 1 },
                            enabled = pageIndex < pageCount - 1,
                            modifier = Modifier.weight(1f),
                        ) { Text("下一页 ›") }
                    }
                }
                HorizontalDivider()
                ListItem(
                    headlineContent = { Text("导出短语") },
                    supportingContent = { Text("JSON（可再导入）/ TSV（字母⇥短语，便于表格软件打开）") },
                    trailingContent = {
                        TextButton(onClick = { exportMenu = true }) { Text("导出") }
                    },
                )
                HorizontalDivider()
                ListItem(
                    headlineContent = { Text("导入短语") },
                    supportingContent = {
                        Text("支持 JSON / TSV / CSV / 字母=短语 / 字母 空格 短语；重复字母以新导入为准")
                    },
                    trailingContent = {
                        TextButton(onClick = {
                            importFile.launch(
                                arrayOf(
                                    "application/json",
                                    "text/plain",
                                    "text/csv",
                                    "text/tab-separated-values",
                                    "application/octet-stream",
                                ),
                            )
                        }) { Text("导入") }
                    },
                )
            }
        }
    }

    if (exportMenu) {
        AlertDialog(
            onDismissRequest = { exportMenu = false },
            title = { Text("导出格式") },
            text = { Text("JSON 带版本号，可原样再导入；TSV 每行「字母⇥短语」，适合表格软件。") },
            confirmButton = {
                TextButton(onClick = {
                    exportMenu = false
                    exportJson.launch(if (english) "opi-custom-phrases-en.json" else "opi-custom-phrases.json")
                }) { Text("JSON") }
            },
            dismissButton = {
                TextButton(onClick = {
                    exportMenu = false
                    exportTsv.launch(if (english) "opi-custom-phrases-en.tsv" else "opi-custom-phrases.tsv")
                }) { Text("TSV") }
            },
        )
    }

    when (val d = dialog) {
        PhraseDialog.Add -> PhraseEditDialog(
            initial = null,
            existing = phrases,
            onDismiss = { dialog = null },
            onSave = { p ->
                dialog = null
                apply(enabled, phrases + p, "已添加")
            },
        )
        is PhraseDialog.Edit -> {
            val current = phrases.getOrNull(d.index)
            if (current != null) {
                PhraseEditDialog(
                    initial = current,
                    // 查重时排除被编辑的那一行自己（trigger 可以不改）
                    existing = phrases.filterIndexed { i, _ -> i != d.index },
                    onDismiss = { dialog = null },
                    onSave = { p ->
                        dialog = null
                        apply(enabled, phrases.toMutableList().also { it[d.index] = p }, "已保存")
                    },
                )
            }
        }
        null -> Unit
    }
}

/**
 * 新增/编辑弹窗。校验与导入共用 [CustomPhraseCodec.validTrigger]（同一份规则），
 * 另外挡住**与本表内其它行重复**的 trigger —— 引擎按字母匹配，重复行没有意义。
 */
@Composable
private fun PhraseEditDialog(
    initial: CustomPhrase?,
    existing: List<CustomPhrase>,
    onDismiss: () -> Unit,
    onSave: (CustomPhrase) -> Unit,
) {
    var trigger by remember(initial) { mutableStateOf(initial?.trigger ?: "") }
    var text by remember(initial) { mutableStateOf(initial?.text ?: "") }
    var error by remember(initial) { mutableStateOf<String?>(null) }

    fun submit() {
        val t = trigger.trim()
        val x = text.trim()
        val problem = when {
            t.isEmpty() -> "字母不能为空"
            !CustomPhraseCodec.validTrigger(t) -> "字母只能是英文字母（a–z）"
            t.length > CustomPhraseCodec.MAX_TRIGGER_LEN ->
                "字母最多 ${CustomPhraseCodec.MAX_TRIGGER_LEN} 个"
            x.isEmpty() -> "短语不能为空"
            existing.any { it.trigger == t.lowercase() } -> "字母「${t.lowercase()}」已被其它短语使用"
            initial == null && existing.size >= CustomPhraseCodec.MAX_PHRASES ->
                "达到上限（最多 ${CustomPhraseCodec.MAX_PHRASES} 条）"
            else -> null
        }
        error = problem
        if (problem == null) onSave(CustomPhrase(t.lowercase(), x))
    }

    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(if (initial == null) "添加短语" else "编辑短语") },
        text = {
            Column {
                OutlinedTextField(
                    value = trigger,
                    onValueChange = { trigger = it },
                    label = { Text("字母") },
                    singleLine = true,
                    supportingText = { Text("输入这些字母后出现短语（大小写不敏感）") },
                    modifier = Modifier.fillMaxWidth(),
                )
                OutlinedTextField(
                    value = text,
                    onValueChange = { text = it },
                    label = { Text("短语") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                if (error != null) {
                    Text(
                        text = error!!,
                        color = MaterialTheme.colorScheme.error,
                        fontSize = 12.sp,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }
            }
        },
        confirmButton = { TextButton(onClick = { submit() }) { Text("保存") } },
        dismissButton = { TextButton(onClick = onDismiss) { Text("取消") } },
    )
}
