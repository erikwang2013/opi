// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 状态栏按钮的**外部命令**部分：把点击变成一条命令，并把失败如实报回来。
//!
//! 契约（2026-10-10 冻结，Linux 侧由另一条轨实现，本侧**只调用**）：
//! **`opi-ctl <action>`**，action ∈ `symbols|trad|punct|fullwidth`；`opi-ctl` 在
//! `/usr/bin/opi-ctl`（PATH 上，按名调）。中英文切换走 `fcitx5-remote -t`（fcitx5
//! 自带工具，与 Shift 轻按同一条通路）。
//!
//! ⚠️ 这些动作的效果在**别处**（输入法状态），命令成败是本侧唯一看得见的东西 ——
//! 所以失败必须回到 UI 上（[runExternal] 的返回值就是给用户看的那句话）。

package xyz.erik.opi.candidate

import java.util.concurrent.TimeUnit

/**
 * 纯函数：按钮 → 要跑的命令；null = 本按钮不跑命令（另有动作：开窗/开关键盘/打开设置）。
 *
 * Windows 上没有 `opi-ctl` 也没有 fcitx5（那是 Linux 的输入法框架），所以调用方
 * 只在 Linux 上走这条路 —— 见 [statusButtonWired] 与 StatusBar.kt 的分派。
 */
fun statusActionCommand(b: StatusBarButton): List<String>? = when (b) {
    StatusBarButton.SYMBOLS -> listOf("opi-ctl", "symbols")
    StatusBarButton.TRAD -> listOf("opi-ctl", "trad")
    StatusBarButton.PUNCT -> listOf("opi-ctl", "punct")
    StatusBarButton.FULLWIDTH -> listOf("opi-ctl", "fullwidth")
    StatusBarButton.LANG -> listOf("fcitx5-remote", "-t")
    StatusBarButton.VK, StatusBarButton.LOGO, StatusBarButton.SETTINGS -> null
}

/**
 * 跑一条外部命令。返回 null = 退出码 0；非 null = **给用户看的**失败短语。
 *
 * 超时（默认 3s）不是防御性编程而是必须的：`opi-ctl` 要是卡在等输入法应答上，同步
 * 等它 = 点一下状态栏就把 UI 冻住。到点 `destroyForcibly`，按钮立刻恢复可用。
 *
 * 输出只在**失败**时读一行当补充说明（成功路径的效果在输入法那边，输出没意义）；
 * 子进程输出没人读会撑满管道缓冲而卡住它 —— 我们自己的小工具输出一行以内，真卡住
 * 还有超时兜底，故不为此开读线程。
 */
fun runExternal(cmd: List<String>, timeoutMs: Long = 3000): String? = try {
    val proc = ProcessBuilder(cmd).redirectErrorStream(true).start()
    if (!proc.waitFor(timeoutMs, TimeUnit.MILLISECONDS)) {
        proc.destroyForcibly()
        "「${cmd.joinToString(" ")}」超时（无响应 ${timeoutMs / 1000}s）"
    } else if (proc.exitValue() == 0) {
        null
    } else {
        val why = proc.inputStream.bufferedReader().readLine()?.trim()
        "「${cmd.joinToString(" ")}」退出码 ${proc.exitValue()}${if (why.isNullOrEmpty()) "" else "：$why"}"
    }
} catch (e: Exception) {
    // 命令不存在（没装/不在 PATH）也走这里 —— 报出命令名，用户才知道缺什么。
    "跑不了「${cmd.first()}」：${e.javaClass.simpleName}"
}
