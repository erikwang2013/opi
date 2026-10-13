// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「候选字号」（2026-10-14 用户裁决）。用户原话：「候选词有点小了至少12px」——
//! Linux 上用户看到的候选面板是 **fcitx5 的 classicui 画的**（不是本仓自绘的
//! CandidateWindow），所以这一档改的是 **classicui.conf 的 `Font=` 键**。
//!
//! 三档 = pango 描述（`Font=` 是 pango 串；**裸数字是 pt**，10pt ≈ 13.3px @96DPI）：
//! 小 = `Sans 10`（fcitx5 框架默认）/ 中 = `Sans 12` / 大 = `Sans 14`。
//! 选择本身存在我们自己的 ini 键 `CandidateFont=小|中|大`（名字，不存 pt —— 与
//! Skin/拼音方式同一条惯例，见 Settings.kt）。
//!
//! ⚠️ **只管理 `Font` 这一行**：classicui.conf 是 fcitx5 的共享配置文件（Theme/
//! MenuFont/… 与用户手写注释都在里面），所以复用 [mergeClassicUiFont]（只有 Font 的
//! 小 merge）。**别**拿 Settings.kt 的 `mergeIni` 顶替 —— 那个管着我们一长串键，拿来
//! 写 classicui 会把整套 opi 设置倒进 fcitx5 的主题文件。
//!
//! ⚠️ 重载：**`ReloadAddonConfig("classicui")` 才有效**。2026-10-14 真机实测（本机
//! fcitx5 在跑，用户配置已还原）：写文件不重载 ⇒ GetConfig 仍 `Sans 10`；
//! `fcitx5-remote -r`（ReloadConfig）**退出码 0、GetConfig 纹丝不动**（假绿）；
//! 换 `ReloadAddonConfig("classicui")` 立刻回新值（busctl 与 dbus-send 两条都验过）。
//! 原因朴素：classicui 是个 **addon**，不是核心配置。⇒ 这条链**不挂**
//! `fcitx5-remote -r` 兜底：它在这是「成功编码成 0 的无声无效」，比没有兜底更坏。

package xyz.erik.opi.candidate

import java.nio.file.Files
import java.nio.file.Path

/**
 * 三档：档位名（存 ini 的值）↔ pango 描述（写 classicui.conf 的值）。
 * 顺序 = 设置面展示序（小→中→大）。**这是唯一的映射处**，设置页与写入都读它。
 */
internal val CLASSIC_UI_FONT_CHOICES = listOf(
    "小" to "Sans 10",
    "中" to "Sans 12",
    "大" to "Sans 14",
)

/** 档位名表（解析 ini 用；顺序同上）。 */
internal val CLASSIC_UI_FONT_NAMES: List<String> = CLASSIC_UI_FONT_CHOICES.map { it.first }

/** 缺键/认不得 → 中（2026-10-14 裁决：默认中档）。 */
internal const val CLASSIC_UI_FONT_DEFAULT = "中"

/** 档位名 → pango 描述；认不得的名字按中档（与 fromIni 的回落同一口径，不抛）。 */
internal fun classicUiFontDesc(name: String): String =
    (CLASSIC_UI_FONT_CHOICES.firstOrNull { it.first == name }
        ?: CLASSIC_UI_FONT_CHOICES.first { it.first == CLASSIC_UI_FONT_DEFAULT }).second

/** classicui.conf 的纯函数落点（基目录推导与 [settingsPathFor] 共用 [fcitxConfDir]）。 */
fun classicUiPathFor(xdgConfigHome: String?, home: String?): Path =
    fcitxConfDir(xdgConfigHome, home).resolve("classicui.conf")

/** 真实环境下的落点。⚠️ 只有 Linux 用得上（外观页在 Windows 上不出现）。 */
fun classicUiPath(): Path = classicUiPathFor(
    xdgConfigHome = System.getenv("XDG_CONFIG_HOME"),
    home = System.getenv("HOME"),
)

/**
 * 把 `Font=` 合进 [existing]（classicui.conf 的现存内容）：**只动 Font 行**，其余
 * （Theme/MenuFont/注释/空行/未知键）逐字保留；没有 Font 行就追加一行。行内 `#` 之后
 * 算注释（与 [mergeIni] 同一判定：`Font = Sans 10 # 手调` 这种也认）。
 *
 * 文件里**重复的 Font 行**：首行换成新值，其余整行丢弃 —— 两条同键在 fcitx5 里读哪条
 * 是实现细节，留着是给我们自己埋的坑；这是本函数唯一会「删」的东西。
 */
fun mergeClassicUiFont(existing: String, fontDesc: String): String {
    var seen = false
    val body = rewriteIniLines(existing) { line ->
        val isAssignment = '=' in line.substringBefore('#')
        val key = line.substringBefore('#').substringBefore('=').trim()
        if (isAssignment && key == "Font") {
            if (seen) null else { seen = true; "Font=$fontDesc" }
        } else {
            line
        }
    }
    return if (seen) body else body + "Font=$fontDesc\n"
}

/**
 * 写 classicui.conf（不存在则连同目录创建）。返回写后的全文（门禁直接比对它）。
 * 空文件（0 字节/全空白）当「没有」处理 —— 与 saveSettings 的 existing.isBlank() 同口径。
 */
internal fun writeClassicUiFont(p: Path, name: String): String {
    val desc = classicUiFontDesc(name)
    val existing = try {
        Files.readString(p)
    } catch (_: Exception) {
        ""
    }
    val text = if (existing.isBlank()) "Font=$desc\n" else mergeClassicUiFont(existing, desc)
    p.parent?.let { Files.createDirectories(it) }
    Files.writeString(p, text)
    return text
}

/**
 * 用户选了 [name]：落盘 + 通知 fcitx5 重载 classicui。返回 true = 重载请求送达
 * （fcitx5 没在跑/没有 dbus 工具 ⇒ false，UI 如实显示「写下去了但没生效」）。
 * ⚠️ 失败**不抛**：fcitx5 没跑时文件照样该写（下次启动生效），与 [saveSettings] 同一条。
 */
fun applyClassicUiFont(name: String): Boolean {
    writeClassicUiFont(classicUiPath(), name)
    // legacyRemote = false：`fcitx5-remote -r` 对 classicui 是假绿（见文件头实测）。
    return reloadAddonConfig("classicui", legacyRemote = false)
}
