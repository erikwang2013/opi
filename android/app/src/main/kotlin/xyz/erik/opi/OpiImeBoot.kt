// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi

import android.util.Log
import xyz.erik.opi.engine.DictStore
import xyz.erik.opi.jni.EngineLoader
import xyz.erik.opi.settings.CustomPhraseStore
import java.io.IOException

/**
 * 引擎/词库启动的**慢半边**（2026-10-28 从 `onCreateInputView` 拆出，那边撞 500 行门禁，
 * 且这段要整体搬去 [EngineLoader.io]）：只碰文件与 `OpiEngine` 直接出口。
 *
 * 偏好恢复（`applyEngineSwitches` / `imeState.restore`）**不在这里**：它们走
 * `EngineController` → `refresh()` → `pushComposingText()`，会摸输入法连接和窗口，
 * 只能留在主线程的 ready 回调里。
 *
 * 返回值 = 词库装载结果（false = 回退内置 35 词库），喂给宠物画降级态。
 */
internal fun OpiImeService.bootEngine(): Boolean {
    // luna 词库编排（幂等：size 校验重拷；失败回退内置词库；与设置页共享 Rust 单例）
    // 返回值是词库装载结果（false = 回退到内置 35 词库），接住它喂给宠物：降级时
    // 候选栏待命位的小欧折断天线（DEGRADED），而不是照旧笑嘻嘻。
    val dictionaryOk = EngineLoader.load(this)
    // 用户词导入必须在词库装载之后、建视图之前：文件缺失/损坏静默降级（不崩 IME），
    // 且要赶在第一次刷新候选之前生效
    userWords.load()
    // 自定义短语（2026-10-23；2026-10-24 英文套）：宿主文件（中文/英文各一个）是唯一
    // 真源，引擎侧是内存态 —— 每次建视图时两套各自整表重推。设置页改动会即时推，
    // 但进程被杀后只有这里装得回去。失败只留日志，不能因此崩输入法。
    CustomPhraseStore.from(this).loadAndPush { Log.w(OpiImeService.TAG, "custom phrases: $it") }
    CustomPhraseStore.englishFrom(this).loadAndPush { Log.w(OpiImeService.TAG, "english custom phrases: $it") }
    // 词库目录装载（2026-10-14，M3）：目录里每个 *.tsv 进引擎，**停用名单在装载后重放**
    // —— 引擎里导入层是内存态，同名重导入会置回启用（`api/dict.rs` 裁决 2 把重放
    // 划给宿主）。单个失败不挡启动，只记一条日志（设置页「词库」节会把失败的显示出来）。
    // 内置词库首启种子（2026-10-16，康熙字典「全发」）：首启把 assets/kangxi.tsv 种进
    // 词库目录**一次**（标记门控；用户删过的不复活 —— 见 DictStore.seedBundled）。
    val dictStore = DictStore.from(this)
    val seeded = dictStore.seedBundled("kangxi") {
        try {
            assets.open("kangxi.tsv")
        } catch (e: IOException) {
            Log.w(OpiImeService.TAG, "内置词库资产缺失：$e")
            null
        }
    }
    if (seeded) Log.i(OpiImeService.TAG, "已种下内置词库 kangxi.tsv")
    val dictReport = dictStore.loadAll(DictStore.parseDisabled(prefs.disabledDicts))
    if (dictReport.failed.isNotEmpty()) {
        Log.w(OpiImeService.TAG, "词库装载失败 ${dictReport.failed.size} 个：${dictReport.failed.joinToString("、")}")
    }
    return dictionaryOk
}
