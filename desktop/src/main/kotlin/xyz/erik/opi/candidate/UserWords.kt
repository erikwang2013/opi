// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 设置窗「清空用户词」（2026-10-23）：引擎学习记忆 + 本地缓存文件，两处一起清。
//!
//! 引擎在**输入法进程**里（Linux fcitx5 插件 / Windows TSF DLL），设置窗碰不到它 ——
//! 走与状态栏动作同一条控制面：Windows `\\.\pipe\opi-control-v2` → TSF；Linux
//! `opi-ctl clearuserwords` → D-Bus Trigger。两端的服务端动作同名，见
//! `crates/tsf-opi/src/ctl.rs` 与 `crates/fcitx5-opi/cpp/opi_engine.h`。
//!
//! 两半**不绑定成败**：本地缓存由本进程直删（控制通道失败也删，那是本进程能独立
//! 完成的一半）；引擎内存按控制面结果如实回报。Windows 的引擎内存按宿主进程各一份：
//! 本进程先写「清词世代」标记，各宿主下次激活/聚焦时自清（见
//! `crates/tsf-opi/src/clear_generation.rs`）；控制面补发只负责当前连上的那个。
//! 更新前已打开的宿主加载的是旧 DLL，需关闭重开（见 [clearUserWords] 与 ControlPlane.kt 头）。
//!
//! 本地缓存文件与 Rust 侧 `engine-data::user_words` 同名同目录推导（Linux
//! `$XDG_DATA_HOME/opi`、Windows `%LOCALAPPDATA%\opi`）。当前两端引擎都不落盘，
//! 删它是为旧版本/将来落盘留下的残骸 —— 不存在不算失败。
//!
//! 拆分自 SettingsPages.kt（那边贴着 500 行门禁）。

package xyz.erik.opi.candidate

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material.AlertDialog
import androidx.compose.material.Text
import androidx.compose.material.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.util.UUID
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** 用户词缓存文件的推导（与 `Dicts.kt::dictsDirFor` 同一套环境量口径）。 */
internal fun userWordsFile(
    windows: Boolean,
    xdgDataHome: String?,
    home: String?,
    localAppData: String?,
): Path = if (windows) {
    val base = localAppData?.takeIf { it.isNotEmpty() } ?: "."
    Paths.get(base, "opi", "opi_user_words.json")
} else {
    val base = xdgDataHome?.takeIf { it.isNotEmpty() } ?: ((home ?: "") + "/.local/share")
    Paths.get(base, "opi", "opi_user_words.json")
}

/** 真实环境下的缓存文件路径。 */
internal fun userWordsPath(): Path = userWordsFile(
    windows = isWindows(),
    xdgDataHome = System.getenv("XDG_DATA_HOME"),
    home = System.getenv("HOME"),
    localAppData = System.getenv("LOCALAPPDATA"),
)

/** 删缓存文件。null = 成功（含本来就不存在）；非 null = 给用户看的原因。 */
internal fun deleteUserWordsCache(path: Path): String? = try {
    Files.deleteIfExists(path)
    null
} catch (e: Exception) {
    "用户词缓存删不掉（${e.javaClass.simpleName}）：$path"
}

/** Windows 的「清词世代」文件：与 Rust `clear_generation.rs` 同名同目录，必须一致。 */
internal fun clearGenerationFile(localAppData: String?): Path {
    val base = localAppData?.takeIf { it.isNotEmpty() } ?: "."
    return Paths.get(base, "opi", "clear_generation")
}

/**
 * 写一个新的世代 token（**不是计数器**：并发清词时读-改-写会互相盖回，
 * token 每次落盘都不同）。null = 成功；非 null = 给用户看的原因。
 * 引擎侧读到与自己上次见过不同的 token 才清 Learner（见 Rust 同名模块）——
 * 写失败时退回「其它应用关闭重开」。
 */
internal fun writeClearGeneration(path: Path): String? = try {
    path.parent?.let { Files.createDirectories(it) }
    Files.writeString(path, UUID.randomUUID().toString())
    null
} catch (e: Exception) {
    "清除标记写不进（${e.javaClass.simpleName}）：$path"
}

/**
 * 清空用户词：**本地缓存总是删**，引擎内存尽力清，返回给用户看的一句话
 * （部分成功也要说清边界 —— 不再用 null 表示成功）。
 *
 * Windows 的引擎在 TSF 宿主进程里（每个已打开的应用各一份）：
 * · **先写世代标记**：每个宿主下次激活/聚焦时读到新 token 就清自己的 Learner
 *   （`crates/tsf-opi/src/clear_generation.rs`）—— 这是「所有已打开应用」那条路；
 * · 再走控制面清当前连上的宿主（有连接当场清；断线则 broker 记待发、重连补发）；
 * · 标记写不进去才退回旧边界：其它已打开的应用需关闭重开；
 * · 更新前就已打开的宿主仍加载旧 DLL（不认识标记），需关闭重开一次。
 * Linux 只有一个 fcitx5 引擎实例，没有这些边界。
 */
fun clearUserWords(timeoutMs: Long = 3000): String {
    // 缓存先删：它是本进程能独立完成的那一半，不能因控制通道失败而留下。
    val cacheWhy = deleteUserWordsCache(userWordsPath())
    val windows = isWindows()
    val genWhy = if (windows) {
        writeClearGeneration(clearGenerationFile(System.getenv("LOCALAPPDATA")))
    } else {
        null
    }
    val engine = if (windows) {
        controlClearUserWords(timeoutMs)
    } else {
        val why = runExternal(listOf("opi-ctl", "clearuserwords"), timeoutMs)
        ClearEngineResult(ok = why == null, queued = false, why = why)
    }
    if (!windows) {
        return when {
            engine.ok && cacheWhy == null -> "已清空用户词：本地缓存与学习记忆"
            engine.ok -> "已清空学习记忆；$cacheWhy"
            cacheWhy == null -> "已清本地缓存；学习记忆未清除（${engine.why ?: "原因未知"}）"
            else -> "用户词未清除：$cacheWhy；${engine.why ?: "原因未知"}"
        }
    }
    if (genWhy == null) {
        // 各宿主下次激活/聚焦时自清，不依赖控制面这一格成没成。
        val cache = cacheWhy ?: "已清本地缓存"
        return "$cache；已打开的应用下次切回时清除内存（更新前已打开的应用需重开）"
    }
    // 标记写不进去：退回旧边界 —— 控制面只能清当前连上的宿主，其余需关闭重开。
    val fallback = "已打开的应用需关闭重开（清除标记写入失败：$genWhy）"
    return when {
        engine.ok && cacheWhy == null -> "已清空用户词：本地缓存与当前应用的学习记忆；$fallback"
        engine.ok -> "已清空当前应用的学习记忆；$cacheWhy；$fallback"
        engine.queued && cacheWhy == null -> "已清本地缓存；当前应用下次连接时清除；$fallback"
        engine.queued -> "已记录清除请求；$cacheWhy；$fallback"
        cacheWhy == null -> "已清本地缓存；学习记忆未清除（${engine.why ?: "原因未知"}）；$fallback"
        else -> "用户词未清除：$cacheWhy；${engine.why ?: "原因未知"}；$fallback"
    }
}

/** 设置页里的按钮行 + 确认框。 */
@Composable
internal fun ClearUserWordsRow(status: (String) -> Unit) {
    var confirm by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    ActionRow(
        "清空用户词",
        "删除学习到的用户词与本地缓存（不可恢复）；不影响已导入的词库。" +
            "Windows 上已打开的应用会在下次切回时清除内存；更新前已打开的应用需重开",
        onClick = { confirm = true },
    )
    if (confirm) {
        AlertDialog(
            onDismissRequest = { confirm = false },
            title = { Text("清空用户词") },
            text = {
                Text(
                    "将清空输入法学习到的用户词和本地缓存，且不可恢复。" +
                        "Windows 上已打开的应用会在下次切回时清除内存中的学习记录；" +
                        "更新前已打开的应用需重开后才会清除。确定吗？",
                )
            },
            buttons = {
                Row(
                    Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 8.dp),
                    horizontalArrangement = Arrangement.End,
                ) {
                    TextButton(onClick = { confirm = false }) { Text("取消") }
                    TextButton(onClick = {
                        confirm = false
                        scope.launch {
                            status(withContext(Dispatchers.IO) { clearUserWords() })
                        }
                    }) { Text("清空") }
                }
            },
        )
    }
}
