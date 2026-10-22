// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 设置窗「清空用户词」（2026-10-23）：引擎学习记忆 + 本地缓存文件，两处一起清。
//!
//! 引擎在**输入法进程**里（Linux fcitx5 插件 / Windows TSF DLL），设置窗碰不到它 ——
//! 走与状态栏动作同一条控制面：Windows `\\.\pipe\opi-control` → TSF；Linux
//! `opi-ctl clearuserwords` → D-Bus Trigger。两端的服务端动作同名，见
//! `crates/tsf-opi/src/ctl.rs` 与 `crates/fcitx5-opi/cpp/opi_engine.h`。
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

/**
 * 清空用户词：先让输入法进程清引擎学习记忆，成功后再删本地缓存。
 * null = 成功；非 null = 给用户看的原因（引擎失败时**不**删缓存 —— 别让两处状态分家）。
 */
fun clearUserWords(timeoutMs: Long = 3000): String? {
    val why = if (isWindows()) {
        controlAction("clearuserwords", timeoutMs)
    } else {
        runExternal(listOf("opi-ctl", "clearuserwords"), timeoutMs)
    }
    if (why != null) return why
    return deleteUserWordsCache(userWordsPath())
}

/** 设置页里的按钮行 + 确认框。 */
@Composable
internal fun ClearUserWordsRow(status: (String) -> Unit) {
    var confirm by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    ActionRow(
        "清空用户词",
        "删除学习到的用户词与本地缓存（不可恢复）；不影响已导入的词库",
        onClick = { confirm = true },
    )
    if (confirm) {
        AlertDialog(
            onDismissRequest = { confirm = false },
            title = { Text("清空用户词") },
            text = { Text("将清空输入法学习到的用户词和本地缓存，且不可恢复。确定吗？") },
            buttons = {
                Row(
                    Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 8.dp),
                    horizontalArrangement = Arrangement.End,
                ) {
                    TextButton(onClick = { confirm = false }) { Text("取消") }
                    TextButton(onClick = {
                        confirm = false
                        scope.launch {
                            val why = withContext(Dispatchers.IO) { clearUserWords() }
                            status(why ?: "已清空用户词")
                        }
                    }) { Text("清空") }
                }
            },
        )
    }
}
