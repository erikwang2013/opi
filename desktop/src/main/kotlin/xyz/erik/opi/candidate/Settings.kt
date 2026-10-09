// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「属性设置」的配置模型与落盘。**一份 UI 两个平台**（`--settings`，见 Main.kt），
//! 落点按平台分流：
//!
//! · Linux：写的是 **fcitx5 的原生 addon 配置**
//!   （`$XDG_CONFIG_HOME/fcitx5/conf/opi_fcitx5.conf`；字段声明在
//!   `crates/fcitx5-opi/cpp/opi_config.h` 的 `OpiSettings`）。写完跑
//!   **`ReloadAddonConfig("opi_fcitx5")`**（busctl/dbus-send）即时生效：fcitx5 →
//!   addon 的 `reloadConfig()` → 引擎。⚠️ `fcitx5-remote -r` **不干这事**（实测，见
//!   [notifyFcitxReload] 的订正段）。
//! · Windows：`%LOCALAPPDATA%\opi\settings.ini`（与 `crates/tsf-opi/src/dict_path.rs`
//!   的候选目录同一处）。⚠️ **生效时机 = 输入法下一次激活**：TSF 侧没有在线刷新通道
//!   （upgrade path 写在 `crates/tsf-opi/src/settings.rs` 的模块头）。
//!
//! ⚠️ **键名三处逐字对应**（改一处 = 老配置文件全部失效）：
//!   本文件 ⇄ `crates/fcitx5-opi/cpp/opi_config.h`（Linux 读侧）⇄
//!   `crates/tsf-opi/src/settings.rs`（Windows 读侧）。
//!   ⚠️ `Skin` 的值是**中文字面值**（`SkinOption` / opi_config.h 的
//!   `FCITX_CONFIG_ENUM_NAME_WITH_I18N`）：两侧按字面匹配，写错字 = 静默回「不改动」。
//!   Windows 读侧没有这个键（未知键忽略）；设置窗在 Windows 上不显示皮肤一节。
//!
//! ⚠️ 布尔写 **`True`/`False`**（fcitx5 的 ini 就是这个拼法；读侧大小写不敏感，
//! 所以手改成 `true` 也能读，但**写回去统一成全大写首字母**，别两种混着出现）。
//!
//! ⚠️ `StatusBar*` 九个键（2026-10-10 加）**只有本窗读写**：两端 IME 读侧都不认识
//! 它们（未知键会被忽略，向前兼容）；但 Linux 侧它们仍写在同一份 fcitx5 addon 配置
//! 里 —— 用 fcitx5 配置工具保存时这些键**是否被保存方抹掉**本侧无法保证（上游行为，
//! 本机未取证）。若真发生，表现是「状态栏设置被重置」，不是启动失败。
//!
//! ⚠️ 保存走 [mergeIni]：**保留本文件不认识的行**（fcitx5 侧新增的选项如
//! `ShiftToggle`/`Skin`、手写注释、未知键）。2026-10-09 前是整文件重写 —— 那会让
//! 「设置窗保存一次 = 把 fcitx5 配置面里本窗不知道的选项全抹掉」。读侧一直忽略
//! 未知键（向前兼容），写侧从那天起同一条约定。

package xyz.erik.opi.candidate

import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths

/**
 * 「皮肤」的合法值。**字面值就是写进配置的字符串** —— 与 fcitx5 侧 `OpiSkin`
 * 的枚举字面值（`opi_config.h` 的 `FCITX_CONFIG_ENUM_NAME_WITH_I18N`）逐字相同：
 * Rust/C++ 读侧按字面匹配（`unmarshallOption`），写错一个字节 = 静默回默认值。
 * 五款的顺序与 `opi_skin.h` 的 `opiSkinThemeName` 一一对应。
 */
enum class SkinOption(val value: String) {
    KEEP("不改动"),
    LIGHT("浅色"),
    DARK("深色"),
    PAPER("宣纸"),
    CINNABAR("朱砂"),
    ;

    companion object {
        /** 未知值（手改坏/别的版本写的）回默认 —— 与 bool 的坏值处理同一档。 */
        fun fromValue(v: String?): SkinOption =
            entries.firstOrNull { it.value == v } ?: KEEP
    }
}

/**
 * 状态栏（悬浮条）设置：master 隐藏 + 8 个按钮的常驻开关（未常驻 = 不显示）。
 * 默认 = 2026-10-10 用户裁决「8 按钮全上」：master 不隐藏、八个按钮全部常驻。
 */
data class StatusBarSettings(
    val hidden: Boolean = false,
    val vk: Boolean = true,
    val symbols: Boolean = true,
    val trad: Boolean = true,
    val punct: Boolean = true,
    val lang: Boolean = true,
    val logo: Boolean = true,
    val fullwidth: Boolean = true,
    val settings: Boolean = true,
)

/**
 * 状态栏按钮。**枚举顺序 = 条上从左到右的展示顺序**（用户 2026-10-10 深夜**最终
 * 冻结**：logo / 中英 / 。，符号 / 简繁 / 月相 / 键盘 / Ω / 设置 —— 那个「。，」
 * 是用户字面序，别按中文标点惯例改成「，。」）。
 * `label` 兼作设置窗的开关名与状态栏的可访问性描述；条上的**图标**另有一套
 * （见 StatusBar.kt 的 iconText / MoonIcon / SettingsIcon）。
 */
enum class StatusBarButton(val label: String, val hint: String) {
    LOGO("logo", "品牌标识兼拖动把手；单击打开设置"),
    LANG("中英文切换", "与 Shift 轻按同一条通路（fcitx5-remote -t）"),
    SYMBOLS("符号大全", "符号页入口"),
    TRAD("简繁转换", "简 ⇄ 繁（调 opi-ctl）"),
    FULLWIDTH("全半角", "全角 ⇄ 半角（调 opi-ctl）"),
    VK("软键盘", "虚拟键盘窗：按键注入系统（会照常流经输入法）"),
    PUNCT("中英标点", "中英文标点切换（调 opi-ctl）"),
    SETTINGS("设置", "打开本设置窗"),
}

/** 该按钮是否被设为常驻（用户开关）。 */
fun StatusBarSettings.resident(b: StatusBarButton): Boolean = when (b) {
    StatusBarButton.VK -> vk
    StatusBarButton.SYMBOLS -> symbols
    StatusBarButton.TRAD -> trad
    StatusBarButton.PUNCT -> punct
    StatusBarButton.LANG -> lang
    StatusBarButton.LOGO -> logo
    StatusBarButton.FULLWIDTH -> fullwidth
    StatusBarButton.SETTINGS -> settings
}

/** 改一个按钮的常驻位（设置窗与单测共用的纯函数）。 */
fun StatusBarSettings.with(b: StatusBarButton, on: Boolean): StatusBarSettings = when (b) {
    StatusBarButton.VK -> copy(vk = on)
    StatusBarButton.SYMBOLS -> copy(symbols = on)
    StatusBarButton.TRAD -> copy(trad = on)
    StatusBarButton.PUNCT -> copy(punct = on)
    StatusBarButton.LANG -> copy(lang = on)
    StatusBarButton.LOGO -> copy(logo = on)
    StatusBarButton.FULLWIDTH -> copy(fullwidth = on)
    StatusBarButton.SETTINGS -> copy(settings = on)
}

/**
 * 该按钮在**本平台**有没有动作。Windows 上 `opi-ctl` / `fcitx5-remote` 都不存在
 * （那四个动作是 Linux 侧的）—— 没有动作的按钮不上条、也不在设置窗出开关
 * （「宁可少一页，不做点不动的开关」，见 SettingsWindow.kt 的既有原则）。
 */
internal fun statusButtonWired(b: StatusBarButton, windows: Boolean): Boolean = !windows ||
    b == StatusBarButton.VK || b == StatusBarButton.SYMBOLS ||
    b == StatusBarButton.LOGO || b == StatusBarButton.SETTINGS

/**
 * 纯函数：状态栏该显示哪些按钮（**master 隐藏优先**；未常驻不显示；Windows 过滤
 * 无动作项）。状态栏窗与设置窗共用这一份判定，两处各自算必然漂移。
 */
fun visibleStatusButtons(sb: StatusBarSettings, windows: Boolean): List<StatusBarButton> {
    if (sb.hidden) return emptyList()
    return StatusBarButton.entries.filter { sb.resident(it) && statusButtonWired(it, windows) }
}

/** 四个开关 + 皮肤 + 状态栏。默认值 = 引擎既有行为（开关全开、皮肤不改动），与两端的读侧一字不差。 */
data class OpSettings(
    val learn: Boolean = true,
    val chinesePunct: Boolean = true,
    val fuzzy: Boolean = true,
    val jianpin: Boolean = true,
    val skin: SkinOption = SkinOption.KEEP,
    val statusbar: StatusBarSettings = StatusBarSettings(),
) {
    /** 写成 ini。键名见文件头「三处逐字对应」。 */
    fun toIni(): String = buildString {
        appendLine("# OPI 属性设置 —— 由 `opi-candidates --settings` 写入，手工改也认。")
        // ⚠️ 别写「改完 fcitx5-remote -r 即时生效」—— 那条实测无效（见 notifyFcitxReload）。
        appendLine("# Linux：本文件就是 fcitx5 的 addon 配置；设置窗/配置工具保存即重载。")
        appendLine("# 布尔写 True/False；未知键会被两端的读侧忽略（向前兼容）。")
        appendLine("Learn=${boolText(learn)}")
        appendLine("ChinesePunct=${boolText(chinesePunct)}")
        appendLine("Fuzzy=${boolText(fuzzy)}")
        appendLine("Jianpin=${boolText(jianpin)}")
        // 皮肤只在 Linux 侧有意义（fcitx5 主题）；Windows 侧读侧忽略未知键，
        // 但设置窗在 Windows 上不显示这一节，写不写都到不了那条路 —— 写，
        // 是为了「同一份文件格式」不因平台分叉。
        appendLine("Skin=${skin.value}")
        // 状态栏九个键：只有本窗读写（见文件头 ⚠️）。同样不分平台地写全，
        // 「同一份文件格式」是这里的既有纪律。
        appendLine("StatusBarHidden=${boolText(statusbar.hidden)}")
        appendLine("StatusBarVk=${boolText(statusbar.vk)}")
        appendLine("StatusBarSymbols=${boolText(statusbar.symbols)}")
        appendLine("StatusBarTrad=${boolText(statusbar.trad)}")
        appendLine("StatusBarPunct=${boolText(statusbar.punct)}")
        appendLine("StatusBarLang=${boolText(statusbar.lang)}")
        appendLine("StatusBarLogo=${boolText(statusbar.logo)}")
        appendLine("StatusBarFullwidth=${boolText(statusbar.fullwidth)}")
        appendLine("StatusBarSettings=${boolText(statusbar.settings)}")
    }

    companion object {
        /** 解析 ini。缺键/坏值一律回默认 —— **不抛**：手改坏的配置不该让设置窗打不开。 */
        fun fromIni(text: String): OpSettings {
            val map = text.lineSequence()
                // 行内注释也认（fcitx5 的 ini 允许 `Key=True # 说明`）。
                .map { it.substringBefore('#').trim() }
                .filter { it.contains('=') }
                .associate {
                    val k = it.substringBefore('=').trim()
                    val v = it.substringAfter('=').trim()
                    k to v
                }
            fun bool(key: String, dflt: Boolean): Boolean = when (map[key]?.lowercase()) {
                "true", "1", "yes", "on" -> true
                "false", "0", "no", "off" -> false
                else -> dflt
            }
            return OpSettings(
                learn = bool("Learn", true),
                chinesePunct = bool("ChinesePunct", true),
                fuzzy = bool("Fuzzy", true),
                jianpin = bool("Jianpin", true),
                skin = SkinOption.fromValue(map["Skin"]),
                statusbar = StatusBarSettings(
                    hidden = bool("StatusBarHidden", false),
                    vk = bool("StatusBarVk", true),
                    symbols = bool("StatusBarSymbols", true),
                    trad = bool("StatusBarTrad", true),
                    punct = bool("StatusBarPunct", true),
                    lang = bool("StatusBarLang", true),
                    logo = bool("StatusBarLogo", true),
                    fullwidth = bool("StatusBarFullwidth", true),
                    settings = bool("StatusBarSettings", true),
                ),
            )
        }
    }
}

private fun boolText(v: Boolean) = if (v) "True" else "False"

/**
 * 把 [s] 管理的键（四开关 + Skin + 状态栏九个）合进 [existing]，**其它行原样保留**
 * （见文件头第 3 条 ⚠️）。
 *
 * 逐行处理：键在「本窗管理」集合里的行替换为新值，其余（注释/空行/未知键/别的
 * 选项）逐字保留；最后补齐没出现过的管理键。注释判定按 fcitx5 ini 的惯例 ——
 * 行内 `#` 之后是注释（`Fuzzy = false # 关模糊音` 也认）。
 */
fun mergeIni(existing: String, s: OpSettings): String {
    val managed = linkedMapOf(
        "Learn" to boolText(s.learn),
        "ChinesePunct" to boolText(s.chinesePunct),
        "Fuzzy" to boolText(s.fuzzy),
        "Jianpin" to boolText(s.jianpin),
        "Skin" to s.skin.value,
        "StatusBarHidden" to boolText(s.statusbar.hidden),
        "StatusBarVk" to boolText(s.statusbar.vk),
        "StatusBarSymbols" to boolText(s.statusbar.symbols),
        "StatusBarTrad" to boolText(s.statusbar.trad),
        "StatusBarPunct" to boolText(s.statusbar.punct),
        "StatusBarLang" to boolText(s.statusbar.lang),
        "StatusBarLogo" to boolText(s.statusbar.logo),
        "StatusBarFullwidth" to boolText(s.statusbar.fullwidth),
        "StatusBarSettings" to boolText(s.statusbar.settings),
    )
    val seen = mutableSetOf<String>()
    val out = StringBuilder()
    existing.lineSequence().forEach { line ->
        val key = line.substringBefore('#').substringBefore('=').trim()
        val isAssignment = '=' in line.substringBefore('#')
        if (isAssignment && key in managed) {
            out.appendLine("$key=${managed[key]}")
            seen += key
        } else {
            out.appendLine(line)
        }
    }
    for ((k, v) in managed) {
        if (k !in seen) {
            out.appendLine("$k=$v")
        }
    }
    return out.toString()
}

/** Windows 判定：fork 的 `os.name`（不引第三方库；本模块只需这一个判别）。 */
fun isWindows(): Boolean = System.getProperty("os.name").lowercase().contains("win")

/**
 * 配置文件落点（**纯函数**：环境量都当参数传，单测才能钉住这两条路径 ——
 * 它们是「本窗写哪个文件」与「IME 侧读哪个文件」的接缝，错了就是静默不生效）。
 *
 * · Linux = fcitx5 的 addon 配置：`$XDG_CONFIG_HOME/fcitx5/conf/opi_fcitx5.conf`
 *   （XDG_CONFIG_HOME 缺省 `$HOME/.config`；对侧读侧见 `opi_config.h`）。
 * · Windows = `%LOCALAPPDATA%\opi\settings.ini`（见 `crates/tsf-opi/src/settings.rs`，
 *   与 `dict_path.rs` 的候选目录同一处）。
 */
fun settingsPathFor(
    windows: Boolean,
    xdgConfigHome: String?,
    home: String?,
    localAppData: String?,
): Path {
    if (windows) {
        val base = localAppData?.takeIf { it.isNotEmpty() } ?: "."
        return Paths.get(base, "opi", "settings.ini")
    }
    val base = xdgConfigHome?.takeIf { it.isNotEmpty() }
        ?: ((home ?: "") + "/.config")
    return Paths.get(base, "fcitx5", "conf", "opi_fcitx5.conf")
}

/** 真实环境下的落点（`settingsPathFor` 的取环境量包装）。 */
fun settingsPath(): Path = settingsPathFor(
    windows = isWindows(),
    xdgConfigHome = System.getenv("XDG_CONFIG_HOME"),
    home = System.getenv("HOME"),
    localAppData = System.getenv("LOCALAPPDATA"),
)

/** 读当前配置（文件不存在/读不动 → 全默认，等价于「没配过」）。 */
fun loadSettings(): OpSettings = try {
    OpSettings.fromIni(Files.readString(settingsPath()))
} catch (_: Exception) {
    OpSettings()
}

/**
 * 保存 + 通知 IME 侧。
 *
 * Linux：调 fcitx5 的 **`ReloadAddonConfig("opi_fcitx5")`** 让本 addon 重读刚写的
 * 文件 —— 三级兜底 busctl（systemd 系，deepin 有）→ dbus-send（其余发行版）→
 * `fcitx5-remote -r`（旧路，聊胜于无）。
 *
 * ⚠️ **2026-10-09 实测订正**：此前这里只跑 `fcitx5-remote -r`（= D-Bus ReloadConfig），
 * 注释声称「会把本 addon 的 reloadConfig() 走一遍」—— **假的**。现场：文件里
 * `Skin=宣纸`、daemon 里还是 `Skin=深色`，`fcitx5-remote -r` 反复调 GetConfig 纹丝
 * 不动；换 `ReloadAddonConfig("opi_fcitx5")` 立刻回新值（本机 fcitx5 5.1.12 实测，
 * 皮肤也跟着换）。用户报障「桌面端设置不能直接生效」就是这条。
 *
 * ⚠️ 失败**不抛**、返回 false 并让 UI 如实显示：fcitx5 没在跑（或没装）时
 * 文件照样该写下去（下次启动生效），这不该是个错误弹窗。
 *
 * Windows：只落盘。返回 false（「本次没生效」），UI 据此提示「重启输入法后生效」。
 */
fun saveSettings(s: OpSettings): Boolean {
    val p = settingsPath()
    p.parent?.let { Files.createDirectories(it) }
    // 已有文件 → 合并写（保留未知键/注释）；文件不存在/为空 → 整份 toIni（含头部注释）。
    val existing = try {
        Files.readString(p)
    } catch (_: Exception) {
        ""
    }
    Files.writeString(p, if (existing.isBlank()) s.toIni() else mergeIni(existing, s))
    if (isWindows()) {
        return false
    }
    return notifyFcitxReload()
}

/**
 * 让 fcitx5 重载**本 addon** 的配置（= 重读刚写的文件）。
 *
 * 三级兜底，任一成功即 true：busctl → dbus-send → `fcitx5-remote -r`。
 * 前两条调的是 `ReloadAddonConfig("opi_fcitx5")`（实测有效）；最后一条是旧路
 * （实测**不会**让 addon 重读配置，只在 dbus 工具缺失时聊胜于无）。
 * ⚠️ addon 名硬编码没问题：这就是本程序的 addon（opi_fcitx5，见 addon conf 的 id）。
 */
internal fun notifyFcitxReload(): Boolean {
    val calls = listOf(
        listOf(
            "busctl", "--user", "call", "org.fcitx.Fcitx5", "/controller",
            "org.fcitx.Fcitx.Controller1", "ReloadAddonConfig", "s", "opi_fcitx5",
        ),
        listOf(
            "dbus-send", "--session", "--print-reply",
            "--dest=org.fcitx.Fcitx5", "/controller",
            "org.fcitx.Fcitx.Controller1.ReloadAddonConfig", "string:opi_fcitx5",
        ),
        listOf("fcitx5-remote", "-r"),
    )
    for (cmd in calls) {
        try {
            val proc = ProcessBuilder(cmd).redirectErrorStream(true).start()
            if (proc.waitFor() == 0) return true
        } catch (_: Exception) {
            // 这条工具没有/起不来 → 试下一条
        }
    }
    return false
}
