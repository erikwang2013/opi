// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! ini 合并写（2026-10-31 从 Settings.kt 拆出：加 Background= 后那边 511 行撞
//! 500 行门禁；同包 internal 引用不变）。

package xyz.erik.opi.candidate

internal fun boolText(v: Boolean) = if (v) "True" else "False"

/**
 * fcitx5 addon schema 认识的键（唯一出处 = `crates/fcitx5-opi/cpp/opi_config.h` 的
 * FCITX_CONFIGURATION 块）。它们必须留在 `opi_fcitx5.conf`：保存后靠
 * `ReloadAddonConfig` 让 fcitx5 重读，fcitx5 的 autosave 也只写回这些键。
 */
internal val ADDON_CONFIG_KEYS: Set<String> = linkedSetOf(
    "Learn", "ChinesePunct", "Fuzzy", "FuzzyPairs", "Jianpin", "Association",
    "Correction", "ShuangpinLayout", "PinyinMethod", "ShiftToggle", "Skin",
    "DisabledDicts",
    // 五笔六键（Wubi.kt 的 managed()；opi_config.h 同名）。
    "WubiPlan", "WubiStyle", "WubiMixedEntry", "WubiCandidateLimit", "WubiReturn",
    "WubiAutoCommit",
)

/**
 * 只有桌面端自绘 UI 读的键。**不能写进 fcitx5 的 addon 配置**：它的 autosave 只
 * 序列化 schema 认识的键，未知键整段丢失（2026-11-03 本机实测：写探针键 →
 * ReloadAddonConfig → Save → 探针消失）。Linux 上它们落在
 * `$XDG_CONFIG_HOME/opi/settings.ini`；Windows 仍是单文件（见 settingsPathFor）。
 */
internal val DESKTOP_CONFIG_KEYS: Set<String> = linkedSetOf(
    "ShowOwn", "Background", "CandidateFont",
    "StatusBarHidden", "StatusBarVk", "StatusBarSymbols", "StatusBarMic",
    "StatusBarTrad", "StatusBarPunct", "StatusBarLang", "StatusBarEnPro",
    "StatusBarLogo", "StatusBarFullwidth", "StatusBarSettings", "StatusBarScale",
    "ClipEnabled", "ClipDeviceName", "SpeechEnabled", "SpeechLanguage", "UiLanguage",
)

/**
 * 本窗管理的「键 → 新值」全表。两个落点只是它的子集（[ADDON_CONFIG_KEYS] /
 * [DESKTOP_CONFIG_KEYS]），值口径只在这里拼一份。
 */
internal fun managedValues(s: OpSettings): LinkedHashMap<String, String> = linkedMapOf(
    "Learn" to boolText(s.learn),
    "ChinesePunct" to boolText(s.chinesePunct),
    "Fuzzy" to boolText(s.fuzzy),
    "FuzzyPairs" to s.fuzzyPairs,
    "Jianpin" to boolText(s.jianpin),
    "Association" to boolText(s.association),
    "Correction" to boolText(s.correction),
    "ShowOwn" to boolText(s.showOwn),
    "ShuangpinLayout" to s.shuangpinLayout,
    "PinyinMethod" to s.pinyinMethod,
    "ShiftToggle" to boolText(s.shiftToggle),
    "Skin" to s.skin.value,
    "Background" to (s.background?.id ?: ""),
    // 候选字号只在本窗读写（真落点是 classicui.conf）；进 managed 同 StatusBar*。
    "CandidateFont" to s.candidateFont,
    // 词库停用名单（2026-10-14）：与 StatusBar*/CandidateFont 同一条。
    "DisabledDicts" to s.disabledDicts,
    "StatusBarHidden" to boolText(s.statusbar.hidden),
    "StatusBarVk" to boolText(s.statusbar.vk),
    "StatusBarSymbols" to boolText(s.statusbar.symbols),
    "StatusBarMic" to boolText(s.statusbar.mic),
    "StatusBarTrad" to boolText(s.statusbar.trad),
    "StatusBarPunct" to boolText(s.statusbar.punct),
    "StatusBarLang" to boolText(s.statusbar.lang),
    "StatusBarEnPro" to boolText(s.statusbar.enpro),
    "StatusBarLogo" to boolText(s.statusbar.logo),
    "StatusBarFullwidth" to boolText(s.statusbar.fullwidth),
    "StatusBarSettings" to boolText(s.statusbar.settings),
    "StatusBarScale" to s.statusbar.scale.toString(),
    // 跨设备剪贴板（2026-11-01）：默认关。
    "ClipEnabled" to boolText(s.clipEnabled),
    "ClipDeviceName" to s.clipDeviceName,
    "SpeechEnabled" to boolText(s.speechEnabled),
    "SpeechLanguage" to s.speechLanguage,
    // 界面语言：曾经只在 toIni() 里写、不在 managed 表里 ⇒ 已有文件改不了它（补齐）。
    "UiLanguage" to s.uiLanguage,
).apply {
    // 五笔六键（2026-10-14）：整族并进管理键表 —— `putAll` 而不是逐键列一遍，
    // 防两处各写一份（值口径与 toIniLines 同一份，见 WubiSettings.managed()）。
    putAll(s.wubi.managed())
}

/**
 * 把 [s] 管理的键合进 [existing]，**其它行原样保留**（见文件头第 3 条 ⚠️）。
 * [only] 非 null 时只处理该集合里的键：Linux 的两个落点各写自己那一半，
 * 不在集合里的行（包括另一半的键）逐字保留。
 *
 * 逐行处理：键在「本窗管理」集合里的行替换为新值，其余（注释/空行/未知键/别的
 * 选项）逐字保留；最后补齐没出现过的管理键。注释判定按 fcitx5 ini 的惯例 ——
 * 行内 `#` 之后是注释（`Fuzzy = false # 关模糊音` 也认）。
 */
fun mergeIni(existing: String, s: OpSettings, only: Set<String>? = null): String {
    val all = managedValues(s)
    val managed: Map<String, String> = if (only == null) all else all.filterKeys { it in only }
    val seen = mutableSetOf<String>()
    val body = rewriteIniLines(existing) { line ->
        val key = line.substringBefore('#').substringBefore('=').trim()
        val isAssignment = '=' in line.substringBefore('#')
        if (isAssignment && key in managed) {
            seen += key
            "$key=${managed[key]}"
        } else {
            line
        }
    }
    val out = StringBuilder(body)
    for ((k, v) in managed) {
        if (k !in seen) {
            out.appendLine("$k=$v")
        }
    }
    return out.toString()
}

/**
 * 逐行改写 ini 文本，[rewrite] 返回 null = 丢弃该行。**末尾的「空行」不当行处理**：
 * 文件以 `\n` 结尾时按行切会多出一个空段，把它当独立行再拼回去 = 每保存一次文件末尾
 * 多一行空行（2026-10-14 写 ClassicUiFont 的 merge 时被门禁抓到；`mergeIni` 同形，
 * 一起改 —— 真机 `~/.config/fcitx5/conf/opi_fcitx5.conf` 末尾就躺着一行多出来的空行）。
 * 空行**在中间**照旧逐字保留（只有「表示结尾」的那一个空段被吸收）。
 */
internal fun rewriteIniLines(existing: String, rewrite: (String) -> String?): String {
    if (existing.isEmpty()) return ""
    val body = existing.split('\n').let { if (existing.endsWith("\n")) it.dropLast(1) else it }
    val out = ArrayList<String>(body.size + 4)
    for (line in body) {
        rewrite(line)?.let { out += it }
    }
    return out.joinToString("\n", postfix = if (out.isEmpty()) "" else "\n")
}
