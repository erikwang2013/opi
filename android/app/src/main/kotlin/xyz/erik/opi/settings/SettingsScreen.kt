// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.settings

import android.app.Activity
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
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
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import xyz.erik.opi.engine.UserWordStore
import xyz.erik.opi.engine.clearUserWords
import xyz.erik.opi.ime.HandlerDebouncer
import xyz.erik.opi.engine.AssociationGate
import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.jni.OpiEngine
import xyz.erik.opi.keyboard.OpiIcons
import xyz.erik.opi.pet.OpiPet
import xyz.erik.opi.pet.petMood
import kotlinx.coroutines.launch
import java.io.File
import java.io.IOException

/**
 * 设置页（对齐 flutter settings_page.dart）：学习开关 / 清除用户词库（确认对话框）/
 * 导出词库 JSON 到剪贴板 / 从剪贴板或文件导入词库 JSON。JNI 直接调 Rust 静态单例——
 * 设置页与 IME 共享引擎与 Learner（spec §5），开关即时生效。
 *
 * @param dictionaryDegraded 词库装载是否失败（`EngineLoader.load` 返回 false）。
 *   **没有默认值是有意的**：默认 false 会让将来的调用方静默退回「永远显示一切正常」，
 *   那正是这个参数要修的毛病。
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(dictionaryDegraded: Boolean) {
    // 自定义短语子页（2026-10-23；2026-10-24 增英文套）：0=关闭 1=中文 2=英文。
    // 单 Activity 单 Compose 树，没有导航库 —— 用一个可保存的开关换根内容；
    // 返回箭头把它拨回来（旋转后仍停在子页）。
    var phrasePage by rememberSaveable { mutableStateOf(0) }
    if (phrasePage != 0) {
        // 系统返回键回设置页，而不是退出整个设置 Activity。
        BackHandler { phrasePage = 0 }
        CustomPhraseScreen(onBack = { phrasePage = 0 }, english = phrasePage == 2)
        return
    }
    val context = LocalContext.current
    val snackbar = remember { SnackbarHostState() }
    val scope = rememberCoroutineScope()
    // 开关落盘（2026-10-09，关掉 android.md §4.6 记的既有缺口）。顺序有讲究：**先把偏好推给引擎
    // 再读** —— 设置页可能先于 IME 启动，此刻引擎单例刚被本页 load 成默认值，
    // 不推一遍，下面三个 remember 读到的是默认而不是用户存过的。
    val prefs = remember { ImePreferences.from(context) }
    remember { EngineController().also { prefs.applyEngineSwitches(it) } }
    var learner by remember { mutableStateOf(prefs.learn) }
    var fuzzy by remember { mutableStateOf(prefs.fuzzy) }
    var fuzzyPairs by remember { mutableStateOf(prefs.fuzzyPairs) }
    var correction by remember { mutableStateOf(prefs.correction) }
    var jianpin by remember { mutableStateOf(prefs.jianpin) }
    var association by remember { mutableStateOf(prefs.association) }
    var showOwn by remember { mutableStateOf(prefs.showOwn) }
    var confirmClear by remember { mutableStateOf(false) }
    // 双拼布局（2026-10-13）：存**名字**（[SHUANGPIN_LAYOUTS_ANDROID] 之一）。
    var shuangpinLayout by remember { mutableStateOf(prefs.shuangpinLayout) }
    var showLayoutDialog by remember { mutableStateOf(false) }

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

    Scaffold(modifier = Modifier.opiSettingsSceneBackground(), containerColor = Color.Transparent,
        topBar = {
            TopAppBar(
                title = { Text("小欧输入法 设置") }, colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent),
                navigationIcon = {
                    // 从 IME 工具条进来时任务栈里没有上一页，返回箭头统一 finish（回桌面/输入处）
                    IconButton(onClick = { (context as? Activity)?.finish() }) {
                        Icon(OpiIcons.ArrowBack, contentDescription = "返回")
                    }
                },
            )
        },
        snackbarHost = { SnackbarHost(snackbar) },
    ) { padding ->
        // ⚠️ 2026-10-15（v1.9.2 装机核验发现，当时已发布）：这一页**从来不可滚动** ——
        // 建页起就没有 `verticalScroll`，内容比视口长时下半截被裁且滑不动（1080×2400@420dpi
        // 实测：页尾 简拼/拼音方式/词库/五笔/关于 全看不到；两种手势都不动，语义树无节点）。
        // 外面是 `Scaffold` content 槽，加这一行即可；门禁 `SettingsScrollableTest` 钉住它。
        // 平板/横屏内容列限宽 640dp 居中（整页撑满时标题与开关隔大半屏要横跳）；手机不变。
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
                // 小欧报到：表情跟着「学习」开关走 —— 一眼看出引擎在不在记事，
                // 比读一行说明文字快。
                //
                // 优先级：**降级 > 睡着**（petMood 里 dictionaryDegraded 在链首，是有意的）。
                // 所以词库没装上时，即便用户关了学习也不会显示睡着的表情 —— 断天线说的是
                // 「引擎坏了」，比「我不记事」紧急，也不该让用户以为自己关过什么开关。
                // 下面的文字必须跟着一起走：小欧是装饰性 mascot，语义由这行文本承担
                //（见 OpiPet 头注），表情断天线而文字说「醒着」就是自相矛盾。
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
                                prefs.learn = v
                            },
                        )
                    },
                )
                HorizontalDivider()
                // 模糊音**逐对多选**（2026-10-13 用户裁决「可多选类似 z=zh」）：总开关
                // （开=全勾/关=全不勾）+ 逐组微调；首次触碰即把旧布尔态具体化成列表
                // （合同：空列表 = 未设置 ⇒ 回落布尔；全不勾写空列表 + fuzzy=false）。
                // 名字表见 FUZZY_PAIRS_ANDROID（锁步门禁 FuzzyPairsLockstepTest）。
                val pairSet = if (fuzzyPairs.isNotEmpty()) {
                    fuzzyPairs.split(',').map { it.trim() }.filter { it in FUZZY_PAIRS_ANDROID }.toSet()
                } else if (fuzzy) {
                    FUZZY_PAIRS_ANDROID.toSet()
                } else {
                    emptySet()
                }
                fun writePairSet(next: Set<String>) {
                    val list = FUZZY_PAIRS_ANDROID.filter { it in next }.joinToString(",")
                    val any = next.isNotEmpty()
                    fuzzyPairs = list
                    fuzzy = any
                    OpiEngine.setFuzzyPairs(fuzzyMaskOf(list))
                    prefs.fuzzyPairs = list
                    prefs.fuzzy = any
                }
                ListItem(
                    headlineContent = { Text("模糊音（逐对）") },
                    supportingContent = { Text("总开关：开 = 全勾、关 = 全不勾；下面是逐组微调") },
                    trailingContent = {
                        Switch(
                            checked = pairSet.isNotEmpty(),
                            onCheckedChange = { v ->
                                writePairSet(if (v) FUZZY_PAIRS_ANDROID.toSet() else emptySet())
                            },
                        )
                    },
                )
                for (name in FUZZY_PAIRS_ANDROID) {
                    val (a, b) = name.split('/')
                    ListItem(
                        headlineContent = { Text("　$a ↔ $b") },
                        trailingContent = {
                            Switch(
                                checked = name in pairSet,
                                onCheckedChange = { v ->
                                    writePairSet(pairSet.toMutableSet().apply { if (v) add(name) else remove(name) })
                                },
                            )
                        },
                    )
                }
                HorizontalDivider()
                ListItem(
                    headlineContent = { Text("拼音纠错") },
                    supportingContent = { Text("相邻键错按 / 多打 / 少打一个字母也能出候选") },
                    trailingContent = {
                        Switch(
                            checked = correction,
                            onCheckedChange = { v ->
                                correction = v
                                OpiEngine.setCorrection(v)
                                prefs.correction = v
                            },
                        )
                    },
                )
                HorizontalDivider()
                ListItem(
                    headlineContent = { Text("简拼") },
                    supportingContent = { Text("声母缩写：nh → 你好、zg → 中国") },
                    trailingContent = {
                        Switch(
                            checked = jianpin,
                            onCheckedChange = { v ->
                                jianpin = v
                                OpiEngine.setJianpin(v)
                                prefs.jianpin = v
                            },
                        )
                    },
                )
                HorizontalDivider()
                // 智能联想（2026-10-28 用户裁决「默认关闭」）：无引擎出口，宿主侧 gate。
                ListItem(
                    headlineContent = { Text("智能联想") },
                    supportingContent = { Text("提交词语后联想常用续接词句（默认关闭）") },
                    trailingContent = {
                        Switch(
                            checked = association,
                            onCheckedChange = { v ->
                                association = v; AssociationGate.enabled = v; prefs.association = v
                            },
                        )
                    },
                )
                HorizontalDivider()
                // 自有表情（2026-10-20 用户裁决：保留小欧原图贴纸，缺省隐藏）。
                // 打开后表情页才出现「自有」分类；插入仍是 GIF/PNG → 文字标签。
                ListItem(
                    headlineContent = { Text("表情中显示自有") },
                    supportingContent = { Text("在表情页显示小欧贴纸分类（默认隐藏）") },
                    trailingContent = {
                        Switch(
                            checked = showOwn,
                            onCheckedChange = { v ->
                                showOwn = v
                                prefs.showOwn = v
                            },
                        )
                    },
                )
                HorizontalDivider()
                // 双拼布局（2026-10-13）：只在拼音方式选「拼音双拼」（输入模式面板）时参与
                // 解码 —— 其余方案下写它无害（引擎只在那一个方案里读）。名字表 =
                // SHUANGPIN_LAYOUTS_ANDROID（锁步门禁 ShuangpinLayoutsLockstepTest）。
                ListItem(
                    headlineContent = { Text("双拼布局") },
                    supportingContent = { Text("拼音方式选「拼音双拼」时生效（主流同款六套）") },
                    trailingContent = {
                        TextButton(onClick = { showLayoutDialog = true }) { Text(shuangpinLayout) }
                    },
                )
                HorizontalDivider()
                // 自定义短语（2026-10-23）：输入字母直接上屏一段常用文本。
                // 落盘 = filesDir/opi_custom_phrases.json（见 CustomPhraseStore）。
                ListItem(
                    headlineContent = { Text("自定义短语") },
                    supportingContent = { Text("输入字母，快速上屏常用短语") },
                    trailingContent = {
                        TextButton(onClick = { phrasePage = 1 }) { Text("管理") }
                    },
                )
                HorizontalDivider()
                // 英文自定义短语（2026-10-24）：独立文件与独立开关，只在英文键盘匹配。
                ListItem(
                    headlineContent = { Text("英文自定义短语") },
                    supportingContent = { Text("输入字母，在英文键盘上屏常用短语") },
                    trailingContent = {
                        TextButton(onClick = { phrasePage = 2 }) { Text("管理") }
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
                AsrModelSection(prefs)
                ClipSection(prefs, ::toast)
                HorizontalDivider()
                // 词库一节（2026-10-14，M3 的 2c 移动端半边）：系统库只读 + 导入库开关/删除 + tsv 导入。
                DictSection(prefs, ::toast)
                // 五笔（2026-10-14，M4 第二波）：六键（方案/风格/混输进入方式/个数/回车/自动上屏）。
                WubiSection(prefs, ::toast)

                // 关于（2026-10-15，用户裁决「移动端：增加关于按钮」）：与词库/五笔两节同一套路
                // —— 单独文件 + 这一行调用（见 AboutSection.kt）。
                AboutSection(::toast)
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
    }

    if (confirmClear) {
        AlertDialog(
            onDismissRequest = { confirmClear = false },
            title = { Text("清除用户词库") },
            text = { Text("将删除所有学习到的用户词，确定吗？") },
            confirmButton = {
                TextButton(onClick = {
                    confirmClear = false
                    // 与键盘内设置面板共用同一处两半清理（顺序与成因见 clearUserWords）。
                    clearUserWords(store)
                    toast("已清除用户词库")
                }) { Text("清除") }
            },
            dismissButton = {
                TextButton(onClick = { confirmClear = false }) { Text("取消") }
            },
        )
    }

    // 双拼布局选择（六选一）。用 TextButton 列而不是 RadioButton：这里没有 androidx
    // material 的 RadioButton 依赖（本页从没用过它），为六行选项引一个新组件不值当；
    // 选中态用「✓ 」前缀表达，读屏走文本本身。
    if (showLayoutDialog) {
        AlertDialog(
            onDismissRequest = { showLayoutDialog = false },
            title = { Text("双拼布局") },
            text = {
                // 六行选项在横屏/大字号对话框里可能超过可用高度：可滚动，否则最后
                // 两个布局永远点不到（对话框本身不会替你滚）。
                Column(modifier = Modifier.verticalScroll(rememberScrollState())) {
                    for (name in SHUANGPIN_LAYOUTS_ANDROID) {
                        TextButton(onClick = {
                            shuangpinLayout = name
                            prefs.shuangpinLayout = name
                            // 即时生效（引擎的布局只改「下次解码怎么切」，不清缓冲）
                            OpiEngine.setShuangpinLayout(shuangpinLayoutIndexOf(name))
                            showLayoutDialog = false
                        }) { Text(if (name == shuangpinLayout) "✓ $name" else name) }
                    }
                }
            },
            confirmButton = {
                TextButton(onClick = { showLayoutDialog = false }) { Text("关闭") }
            },
        )
    }
}
