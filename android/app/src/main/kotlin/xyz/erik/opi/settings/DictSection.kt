// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.settings

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Row
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.ListItem
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import xyz.erik.opi.engine.DictStore
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.IOException

/**
 * 设置页「词库」一节（2026-10-14，M3 的 2c **移动端半边**）。列表 = 引擎的 `dictList`
 * （系统库 + 导入库）；系统库只读（名字/条数，无开关无删除），导入库可开关可删除。
 *
 * 三件事各自钉在别处，这里只做编排：
 * · 启动装载与删除的「引擎 + 文件两个都做」在 [DictStore]（JVM 门禁 DictStoreTest）；
 * · 开关的**持久化**在 [ImePreferences.disabledDicts] —— 引擎里导入层是内存态，
 *   停用名单必须在每次装载之后重放，否则开关重启复活；
 * · 导入是**流拷**（[DictStore.importTsv]），整份读进内存的写法在本仓已被否决过一次。
 */
@Composable
internal fun DictSection(prefs: ImePreferences, toast: (String) -> Unit) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    // from(context) 无状态（目录 + JNI 四出口），每次组合建一个即可
    val store = remember { DictStore.from(context) }
    // 启动装载：目录里每个 *.tsv 进引擎，停用名单装载后重放。放本节而不是 Activity：
    // 失败提示就显示在下面，不必再往外传一层（跑在组合线程上，与既有各装载同一档）。
    var report by remember {
        mutableStateOf(store.loadAll(DictStore.parseDisabled(prefs.disabledDicts)))
    }
    var entries by remember { mutableStateOf(store.list()) }
    var confirmDelete by remember { mutableStateOf<String?>(null) }

    fun refresh() {
        entries = store.list()
    }

    ListItem(
        headlineContent = { Text("词库") },
        supportingContent = { Text("tsv 词库文件；系统库只读，导入的可开关、可删除") },
    )
    if (report.failed.isNotEmpty()) {
        ListItem(
            headlineContent = { Text("${report.failed.size} 个词库文件装载失败") },
            supportingContent = {
                Text("可能不是有效的 tsv：${report.failed.joinToString("、")}（重新导入同名文件可覆盖）")
            },
        )
    }
    for (e in entries) {
        HorizontalDivider()
        ListItem(
            headlineContent = { Text(if (e.system) "${e.name}（系统）" else e.name) },
            // 系统库里 trad/stroke 的 enabled = 「这次装载装上了没」；没装上显示 0 条
            // 是谎报（引擎的注释：不是「读不出来」，是真的没这层）
            supportingContent = { Text(if (e.system && !e.enabled) "未装载" else "${e.count} 条") },
            trailingContent = {
                if (!e.system) {
                    Row {
                        Switch(
                            checked = e.enabled,
                            onCheckedChange = { v ->
                                if (store.setEnabled(e.name, v)) {
                                    // 引擎认了才写名单：拒了（认不得的名字）不改持久状态
                                    prefs.disabledDicts =
                                        DictStore.withDisabled(prefs.disabledDicts, e.name, v)
                                    refresh()
                                } else {
                                    toast("改不了「${e.name}」：引擎拒绝")
                                }
                            },
                        )
                        TextButton(onClick = { confirmDelete = e.name }) { Text("删除") }
                    }
                }
            },
        )
    }
    HorizontalDivider()
    // 选择器：MIME 放宽成 octet-stream/text —— 云盘与 IM 转存的 .tsv 常被判成这两者，
    // 只收 text 会让文件在选择器里直接灰掉（那样就是静默的「干不了」）。
    val picker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        // uri == null 是用户按返回取消，不是失败
        if (uri != null) {
            scope.launch {
                // 拷贝与装载都进 io 线程：SAF 回调在主线程上，流拷一份大 tsv 就是 ANR 的配方
                val outcome = withContext(Dispatchers.IO) {
                    runCatching {
                        store.importTsv(displayNameOf(context, uri)) {
                            context.contentResolver.openInputStream(uri)
                                ?: throw IOException("打不开所选文件")
                        }
                    }
                }
                outcome.fold(
                    onSuccess = { r ->
                        when (r) {
                            is DictStore.ImportResult.Imported -> {
                                // 导入动作的语义 = 「我要用它」：清掉名单里的旧条目，
                                // 否则下次启动装载重放名单时，刚导入的词库又被停用
                                prefs.disabledDicts =
                                    DictStore.withDisabled(prefs.disabledDicts, r.name, enabled = true)
                                toast("已导入「${r.name}」${r.count} 条")
                            }

                            is DictStore.ImportResult.Rejected -> toast("导入失败：${r.reason}")
                        }
                        refresh()
                    },
                    // 死 .so（Error）也走这里：导入失败不该把设置页带走
                    onFailure = { toast("导入失败：${it.message ?: it}") },
                )
            }
        }
    }
    ListItem(
        headlineContent = { Text("导入词库文件") },
        supportingContent = { Text("选一个 tsv 拷进词库目录再装载（同名会覆盖）") },
        trailingContent = {
            TextButton(onClick = {
                picker.launch(arrayOf("text/plain", "application/octet-stream"))
            }) { Text("选择文件") }
        },
    )

    confirmDelete?.let { name ->
        AlertDialog(
            onDismissRequest = { confirmDelete = null },
            title = { Text("删除词库「$name」") },
            text = { Text("会从引擎和词库目录同时删除（不可恢复），确定吗？") },
            confirmButton = {
                TextButton(onClick = {
                    confirmDelete = null
                    store.remove(name)
                    // 名单里这一条顺手清掉（名单只对存在的词库有意义）
                    prefs.disabledDicts =
                        DictStore.withDisabled(prefs.disabledDicts, name, enabled = true)
                    refresh()
                    toast("已删除「$name」")
                }) { Text("删除") }
            },
            dismissButton = {
                TextButton(onClick = { confirmDelete = null }) { Text("取消") }
            },
        )
    }
}

/**
 * SAF Uri 的显示名（文件名）。`OpenableColumns.DISPLAY_NAME` 是**提供方答的**，
 * 少数云盘/IM 中转不答 ⇒ 回退 Uri 最后一段，再不行给「dict」（[DictStore.nameOf]
 * 拒绝空名，宁可给个能用的缺省）。
 */
private fun displayNameOf(context: Context, uri: Uri): String {
    context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
        ?.use { c ->
            if (c.moveToFirst()) {
                val i = c.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                if (i >= 0) c.getString(i)?.takeIf { it.isNotEmpty() }?.let { return it }
            }
        }
    return uri.lastPathSegment?.takeIf { it.isNotEmpty() } ?: "dict"
}
