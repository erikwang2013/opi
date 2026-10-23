// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 设置窗「高级 → 自定义短语 / 英文自定义短语」页（2026-10-24）：启用开关 /
//! 分页列表（每页 10 条）/ 增删改 / 导入导出。
//!
//! `english = false` 读写 `custom_phrases.json`（中文键盘）；`english = true` 读写
//! `custom_phrases_en.json`（只在英文键盘匹配）。两套文件、开关、列表完全独立，
//! 页面逻辑同一份（[CustomPhraseCodec] 与 [CustomPhraseState] 共用）。
//!
//! 数据流只有一条：**先写宿主文件，成功后才改屏幕，再请输入法重读**
//! （[reloadImeSettings]：Linux = ReloadAddonConfig → Rust 重读两个文件；
//! Windows = 控制面 reload → TSF 下次激活兜底）。写失败 = 屏幕保持磁盘上的那一份；
//! 这样任何时刻页面显示的都等于磁盘上的内容（与 Android 设置页同一数据流）。
//!
//! 导入语义（与 Android/引擎同一份）：JSON / TSV / CSV / `字母=短语` / `字母 空格 短语`；
//! 后导入的同字母覆盖已有；任何一条非法整份拒收；合并后超过 2000 条整份拒收并明说。
//! 导出 JSON = 引擎合同形状（可再导入），TSV = 每行「字母⇥短语」。

package xyz.erik.opi.candidate

import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.AlertDialog
import androidx.compose.material.Text
import androidx.compose.material.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

/** 编辑弹窗的目标：新增或改第 [PhraseDialog.Edit.index] 行。 */
private sealed interface PhraseDialog {
    data object Add : PhraseDialog
    data class Edit(val index: Int) : PhraseDialog
}

/** 「高级」页的一节（开关 + 列表 + 导入导出）；[english] 选择英文套宿主文件。 */
@Composable
internal fun CustomPhrasesPage(english: Boolean = false, status: (String) -> Unit) {
    val scope = rememberCoroutineScope()
    var state by remember { mutableStateOf<CustomPhraseState?>(null) }
    var page by remember { mutableStateOf(0) }
    var dialog by remember { mutableStateOf<PhraseDialog?>(null) }
    var confirmDelete by remember { mutableStateOf<Int?>(null) }
    val hostPath = if (english) englishCustomPhrasesPath() else customPhrasesPath()
    val title = if (english) "英文自定义短语" else "自定义短语"
    val hint = if (english) {
        "只在英文键盘匹配：输入完整字母后，候选栏出现对应短语"
    } else {
        "输入完整字母后，候选栏出现对应短语"
    }
    // 写串行锁：两次快速保存（连点开关）落到同一文件时，后写不能插到前写中间；
    // 且变更在锁内基于**最新** state 计算，连点不会互相覆盖。
    val saveLock = remember(english) { Mutex() }

    LaunchedEffect(english) {
        state = null
        page = 0
        state = withContext(Dispatchers.IO) {
            if (english) loadEnglishCustomPhrases(hostPath) else loadCustomPhrases(hostPath)
        }
    }

    /** 一次变更的唯一落点：改最新状态 → 原子落盘 → 改屏幕 → 通知 IME。 */
    fun mutate(done: String? = null, change: (CustomPhraseState) -> CustomPhraseState) {
        scope.launch {
            saveLock.withLock {
                val cur = state ?: return@withLock
                val next = change(cur)
                val error = withContext(Dispatchers.IO) {
                    if (english) {
                        saveEnglishCustomPhrases(next, hostPath)
                    } else {
                        saveCustomPhrases(next, hostPath)
                    }
                }
                if (error != null) {
                    status("保存失败：$error")
                    return@withLock
                }
                state = next
                page = CustomPhraseCodec.clampPage(page, next.phrases.size)
                val live = withContext(Dispatchers.IO) { reloadImeSettings() }
                status(listOfNotNull(done, saveStatusMessage(live)).joinToString("；"))
            }
        }
    }

    fun export(json: Boolean) {
        val cur = state ?: return
        val text = if (json) {
            CustomPhraseCodec.encodeCanonical(cur.phrases)
        } else {
            CustomPhraseCodec.encodeTsv(cur.phrases)
        }
        scope.launch {
            val ext = if (json) "json" else "tsv"
            val base = if (english) "opi-custom-phrases-en" else "opi-custom-phrases"
            val target = withContext(Dispatchers.IO) {
                pickSaveFile("导出短语", "$base.$ext", ext)
            } ?: return@launch
            val error = withContext(Dispatchers.IO) {
                runCatching { writeTextAtomically(target, text) }.exceptionOrNull()
            }
            status(
                if (error == null) {
                    "已导出：$target"
                } else {
                    "导出失败：${error.message ?: error.javaClass.simpleName}"
                },
            )
        }
    }

    fun importFile() {
        scope.launch {
            val picked = withContext(Dispatchers.IO) { pickImportFile() } ?: return@launch
            val raw = try {
                withContext(Dispatchers.IO) { readImportText(picked) }
            } catch (e: Exception) {
                status("无法读取所选文件（${e.message ?: e.javaClass.simpleName}）")
                return@launch
            }
            when (val decoded = CustomPhraseCodec.decode(raw)) {
                is CustomPhraseDecode.Invalid -> status("导入失败：${decoded.reason}")
                is CustomPhraseDecode.Ok -> saveLock.withLock {
                    val cur = state ?: return@withLock
                    val merged = CustomPhraseCodec.merge(cur.phrases, decoded.phrases)
                    if (merged.phrases.size > CustomPhraseCodec.MAX_PHRASES) {
                        status(
                            "导入失败：达到上限（最多 ${CustomPhraseCodec.MAX_PHRASES} 条）；" +
                                "合并后 ${merged.phrases.size} 条，未导入",
                        )
                        return@withLock
                    }
                    val next = cur.copy(phrases = merged.phrases)
                    val error = withContext(Dispatchers.IO) {
                        if (english) {
                            saveEnglishCustomPhrases(next, hostPath)
                        } else {
                            saveCustomPhrases(next, hostPath)
                        }
                    }
                    if (error != null) {
                        // 整份拒收（不是部分写入）：解析全成功，只是没落盘成功。
                        status("导入失败，未保存：$error")
                        return@withLock
                    }
                    state = next
                    page = 0
                    val live = withContext(Dispatchers.IO) { reloadImeSettings() }
                    status(
                        "导入完成：新增 ${merged.added} 条，更新 ${merged.updated} 条；" +
                            saveStatusMessage(live),
                    )
                }
            }
        }
    }

    SectionTitle(title)
    val cur = state
    if (cur == null) {
        Text("正在读取…", fontSize = 12.sp, color = Muted)
        return
    }

    SettingRow(
        "启用$title",
        hint,
        cur.enabled,
    ) { on -> mutate { it.copy(enabled = on) } }

    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text("共 ${cur.phrases.size} 条", fontSize = 15.sp, color = Ink)
            Text("每页 ${CustomPhraseCodec.PAGE_SIZE} 条", fontSize = 12.sp, color = Muted)
        }
        SmallButton("添加") { dialog = PhraseDialog.Add }
    }

    if (cur.phrases.isEmpty()) {
        Text(
            "还没有短语。点「添加」，或从文件导入。",
            fontSize = 12.sp,
            color = Muted,
            modifier = Modifier.padding(vertical = 6.dp),
        )
    } else {
        val pageIndex = CustomPhraseCodec.clampPage(page, cur.phrases.size)
        val pageCount = CustomPhraseCodec.pageCount(cur.phrases.size)
        val visible = CustomPhraseCodec.pageSlice(cur.phrases, pageIndex)
        val start = pageIndex * CustomPhraseCodec.PAGE_SIZE
        for ((offset, phrase) in visible.withIndex()) {
            val row = start + offset
            PhraseRow(
                phrase = phrase,
                onEdit = { dialog = PhraseDialog.Edit(row) },
                onDelete = { confirmDelete = row },
            )
        }
        Row(
            modifier = Modifier.fillMaxWidth().padding(vertical = 2.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            TextButton(
                onClick = { page = pageIndex - 1 },
                enabled = pageIndex > 0,
                modifier = Modifier.weight(1f),
            ) { Text("‹ 上一页") }
            Text(
                "第 ${pageIndex + 1} / $pageCount 页",
                modifier = Modifier.weight(1f),
                textAlign = TextAlign.Center,
                fontSize = 12.sp,
                color = Muted,
            )
            TextButton(
                onClick = { page = pageIndex + 1 },
                enabled = pageIndex < pageCount - 1,
                modifier = Modifier.weight(1f),
            ) { Text("下一页 ›") }
        }
    }

    SectionTitle("导入 / 导出")
    Column(Modifier.fillMaxWidth().padding(vertical = 6.dp)) {
        Text("短语文件", fontSize = 15.sp, color = Ink)
        Text(
            "导入支持 JSON / TSV / CSV / 字母=短语 / 字母 空格 短语；" +
                "重复字母以新导入为准。JSON 导出可直接再导入。",
            fontSize = 12.sp,
            color = Muted,
        )
        Row(Modifier.padding(top = 6.dp), verticalAlignment = Alignment.CenterVertically) {
            SmallButton("导入") { importFile() }
            Spacer(Modifier.width(8.dp))
            SmallButton("导出 JSON") { export(json = true) }
            Spacer(Modifier.width(8.dp))
            SmallButton("导出 TSV") { export(json = false) }
        }
    }
    Text(
        "保存后输入法立即重读（Windows = 控制面 reload；Linux = 重载 addon）；" +
            "没在跑时下次启动/激活生效。文件：$hostPath",
        fontSize = 12.sp,
        color = Muted,
    )

    when (val d = dialog) {
        PhraseDialog.Add -> PhraseEditDialog(
            initial = null,
            existing = cur.phrases,
            onDismiss = { dialog = null },
            onSave = { p ->
                dialog = null
                mutate("已添加") { it.copy(phrases = it.phrases + p) }
            },
        )
        is PhraseDialog.Edit -> {
            val current = cur.phrases.getOrNull(d.index)
            if (current != null) {
                PhraseEditDialog(
                    initial = current,
                    // 查重时排除被编辑的那一行自己（字母可以不改）
                    existing = cur.phrases.filterIndexed { i, _ -> i != d.index },
                    onDismiss = { dialog = null },
                    onSave = { p ->
                        dialog = null
                        mutate("已保存") { st ->
                            st.copy(
                                phrases = st.phrases.mapIndexed { i, x ->
                                    if (i == d.index) p else x
                                },
                            )
                        }
                    },
                )
            }
        }
        null -> Unit
    }

    val deleting = confirmDelete
    if (deleting != null) {
        val phrase = cur.phrases.getOrNull(deleting)
        if (phrase != null) {
            AlertDialog(
                onDismissRequest = { confirmDelete = null },
                title = { Text("删除短语「${phrase.trigger}」") },
                text = { Text("将删除这条短语（不可恢复），确定吗？") },
                confirmButton = {
                    TextButton(onClick = {
                        confirmDelete = null
                        mutate("已删除") { st ->
                            st.copy(phrases = st.phrases.filterIndexed { i, _ -> i != deleting })
                        }
                    }) { Text("删除") }
                },
                dismissButton = {
                    TextButton(onClick = { confirmDelete = null }) { Text("取消") }
                },
            )
        }
    }
}

/** 新增/编辑弹窗。校验与导入共用 [CustomPhraseCodec.validTrigger]（同一份规则）。 */
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
            existing.any { it.trigger == t.lowercase() } ->
                "字母「${t.lowercase()}」已被其它短语使用"
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
                PhraseField(
                    label = "字母",
                    hint = "如 dz（大小写不敏感，最多 32 个）",
                    value = trigger,
                    onValueChange = { trigger = it },
                )
                PhraseField(
                    label = "短语",
                    hint = "输入字母后上屏的内容",
                    value = text,
                    onValueChange = { text = it },
                )
                error?.let {
                    Text(it, fontSize = 12.sp, color = Color(0xFFB3261E))
                }
            }
        },
        confirmButton = { TextButton(onClick = { submit() }) { Text("保存") } },
        dismissButton = { TextButton(onClick = onDismiss) { Text("取消") } },
    )
}

/** 一行输入框（无边框 Material 依赖，样式与设置页同一套色；占位符只在空时画）。 */
@Composable
private fun PhraseField(
    label: String,
    hint: String,
    value: String,
    onValueChange: (String) -> Unit,
) {
    Column(Modifier.fillMaxWidth().padding(bottom = 10.dp)) {
        Text(label, fontSize = 12.sp, color = Muted)
        Box(
            Modifier.fillMaxWidth().border(1.dp, Hairline)
                .padding(horizontal = 8.dp, vertical = 6.dp),
        ) {
            if (value.isEmpty()) Text(hint, fontSize = 12.sp, color = Muted)
            BasicTextField(
                value = value,
                onValueChange = onValueChange,
                singleLine = true,
                textStyle = TextStyle(color = Ink, fontSize = 14.sp),
                cursorBrush = SolidColor(Teal),
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}

/** 列表一行：字母 / 短语 / 编辑 / 删除。 */
@Composable
private fun PhraseRow(phrase: CustomPhrase, onEdit: () -> Unit, onDelete: () -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth().padding(vertical = 5.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(phrase.trigger, fontSize = 14.sp, color = Ink)
            Text(
                phrase.text,
                fontSize = 12.sp,
                color = Muted,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
        SmallButton("编辑", onEdit)
        Spacer(Modifier.width(8.dp))
        SmallButton("删除", onDelete)
    }
}

/** 小按钮：与词库页的「删除」按钮同款（青边青字）。 */
@Composable
private fun SmallButton(text: String, onClick: () -> Unit) {
    Box(
        Modifier.border(1.dp, Teal).clickable(onClick = onClick)
            .padding(horizontal = 10.dp, vertical = 4.dp),
    ) {
        Text(text, fontSize = 12.sp, color = Teal)
    }
}

