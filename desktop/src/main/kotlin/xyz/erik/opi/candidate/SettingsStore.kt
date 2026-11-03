// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 配置落点与读写（2026-11-03 从 Settings.kt 拆出：桌面独有键多了一份落点后
//! 那边 561 行撞 500 行门禁；同包 internal 引用不变）。
//!
//! Linux 是**两份文件**：fcitx5 addon 认识的键写 `opi_fcitx5.conf`，桌面独有键写
//! `$XDG_CONFIG_HOME/opi/settings.ini`。原因：fcitx5 的 autosave 只序列化 schema
//! 认识的键，未知键整段丢弃（2026-11-03 本机实测）—— 同文件时状态栏/剪贴板/语音
//! 设置每 30 分钟被静默复位。Windows 没有 fcitx5，仍是单文件。
//!
//! 保存：合并写（保留注释/未知键）→ Linux 再 `ReloadAddonConfig("opi_fcitx5")`。

package xyz.erik.opi.candidate

import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.nio.file.attribute.BasicFileAttributes
import java.nio.file.attribute.FileTime

/**
 * 配置文件落点（**纯函数**：环境量都当参数传，单测才能钉住这两条路径 ——
 * 它们是「本窗写哪个文件」与「IME 侧读哪个文件」的接缝，错了就是静默不生效）。
 *
 * · Linux = fcitx5 的 addon 配置：`$XDG_CONFIG_HOME/fcitx5/conf/opi_fcitx5.conf`
 *   （XDG_CONFIG_HOME 缺省 `$HOME/.config`；对侧读侧见 `opi_config.h`）。
 *   桌面独有键另有第二份，见 [desktopSettingsPathFor]。
 * · Windows = `%LOCALAPPDATA%\opi\settings.ini`（见 `crates/tsf-opi/src/settings.rs`，
 *   与 `dict_path.rs` 的候选目录同一处；单文件，桌面键也在里面）。
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
    return fcitxConfDir(xdgConfigHome, home).resolve("opi_fcitx5.conf")
}

/**
 * fcitx5 的配置目录（`$XDG_CONFIG_HOME/fcitx5/conf`，缺省 `$HOME/.config/…`；空串等同
 * 未设置）。**单独抽出来**是因为本窗要写两个文件：本 addon 配置与 classicui.conf
 * （2026-10-14 的候选字号，见 ClassicUiFont.kt）—— 推导只有这一处，别各写一份。
 */
internal fun fcitxConfDir(xdgConfigHome: String?, home: String?): Path {
    val base = xdgConfigHome?.takeIf { it.isNotEmpty() }
        ?: ((home ?: "") + "/.config")
    return Paths.get(base, "fcitx5", "conf")
}

/** 真实环境下的落点（`settingsPathFor` 的取环境量包装）。 */
fun settingsPath(): Path = settingsPathFor(
    windows = isWindows(),
    xdgConfigHome = System.getenv("XDG_CONFIG_HOME"),
    home = System.getenv("HOME"),
    localAppData = System.getenv("LOCALAPPDATA"),
)

/**
 * 桌面独有键的落点：Linux = `$XDG_CONFIG_HOME/opi/settings.ini`；Windows 与
 * [settingsPathFor] 同一个文件（TSF 读侧只认那份）。
 *
 * 与 fcitx5 的 addon 配置分开存的原因见文件头 ⚠️：fcitx5 autosave 会丢弃未知键。
 */
fun desktopSettingsPathFor(
    windows: Boolean,
    xdgConfigHome: String?,
    home: String?,
    localAppData: String?,
): Path {
    if (windows) return settingsPathFor(windows, xdgConfigHome, home, localAppData)
    val base = xdgConfigHome?.takeIf { it.isNotEmpty() }
        ?: ((home ?: "") + "/.config")
    return Paths.get(base, "opi", "settings.ini")
}

/** 真机环境下的桌面独有键落点（[desktopSettingsPathFor] 的取环境量包装）。 */
fun desktopSettingsPath(): Path = desktopSettingsPathFor(
    windows = isWindows(),
    xdgConfigHome = System.getenv("XDG_CONFIG_HOME"),
    home = System.getenv("HOME"),
    localAppData = System.getenv("LOCALAPPDATA"),
)

private fun readSettingsText(p: Path): String = try {
    Files.readString(p)
} catch (_: Exception) {
    ""
}

/**
 * 读当前配置（文件不存在/读不动 → 全默认，等价于「没配过」）。
 *
 * Linux 读两份并让桌面文件覆盖 addon 文件：迁移期旧 fcitx5 配置里可能还留着这些键
 * （fcitx5 下一次 autosave 会清掉），读到也能用；下一次保存就落到新文件。
 */
fun loadSettings(): OpSettings {
    val addon = readSettingsText(settingsPath())
    val desktopPath = desktopSettingsPath()
    val desktop = if (desktopPath == settingsPath()) "" else readSettingsText(desktopPath)
    return OpSettings.fromIni(if (desktop.isEmpty()) addon else "$addon\n$desktop")
}

private data class SettingsFileStamp(val mtime: FileTime, val size: Long)

private fun settingsFileStamp(p: Path): SettingsFileStamp? = try {
    val attrs = Files.readAttributes(p, BasicFileAttributes::class.java)
    SettingsFileStamp(attrs.lastModifiedTime(), attrs.size())
} catch (_: Exception) {
    null // 文件不存在/读不动：与 loadSettings 的全默认同一语义
}

// 1s 轮询缓存：两个文件的 (mtime, size)。状态栏与候选窗每秒各读一次配置，绝大多数拍
// 文件根本没动 —— 命中缓存就省掉一次读 + 解析。**跨进程**写入（设置窗是另一个进程）
// 也会体现在 mtime/size 上，所以仍能在一个轮询周期内看到新值。
private var cachedSettings: OpSettings? = null
private var cachedSettingsStamp: Pair<SettingsFileStamp?, SettingsFileStamp?>? = null

/**
 * [loadSettings] 的轮询版：两个文件的 (mtime, size) 都没变就复用上次解析结果。
 * 只给每秒轮询用；需要「刚写完必读新值」的一次性读仍用 [loadSettings]。
 */
@Synchronized
internal fun loadSettingsCached(): OpSettings {
    val addonPath = settingsPath()
    val desktopPath = desktopSettingsPath()
    val stamp = settingsFileStamp(addonPath) to
        if (desktopPath == addonPath) null else settingsFileStamp(desktopPath)
    val cached = cachedSettings
    if (cached != null && stamp == cachedSettingsStamp) return cached
    return loadSettings().also {
        cachedSettings = it
        cachedSettingsStamp = stamp
    }
}

/**
 * 保存 + 通知 IME 侧。
 *
 * Linux 写两份：[settingsPath]（fcitx5 认识的键）+ [desktopSettingsPath]（桌面独有键，
 * fcitx5 autosave 不会碰）。然后调 fcitx5 的 **`ReloadAddonConfig("opi_fcitx5")`**
 * 让本 addon 重读第一份 —— 三级兜底 busctl（systemd 系，deepin 有）→ dbus-send
 * （其余发行版）→ `fcitx5-remote -r`（旧路，聊胜于无）。
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
 * Windows：落盘后经控制面 `reload` 让 TSF 重读；没连上返回 false，UI 提示下次激活生效。
 */
fun saveSettings(s: OpSettings): Boolean {
    val addonPath = settingsPath()
    val desktopPath = desktopSettingsPath()
    if (desktopPath == addonPath) {
        // Windows：单文件（TSF 读侧只认这份，也没有 fcitx5 抢着写）。
        saveOne(addonPath, s, only = null, header = null)
    } else {
        // Linux：fcitx5 只拿它认识的键；桌面独有键另落一份，autosave 碰不到。
        saveOne(addonPath, s, only = ADDON_CONFIG_KEYS, header = ADDON_CONF_HEADER)
        saveOne(desktopPath, s, only = DESKTOP_CONFIG_KEYS, header = DESKTOP_CONF_HEADER)
    }
    return reloadImeSettings()
}

private const val ADDON_CONF_HEADER =
    "# OPI 设置（fcitx5 addon 选项）—— 由 `opi --settings` 写入，手工改也认。\n" +
        "# 桌面端自绘 UI 的键单独存放（fcitx5 的 autosave 会丢弃不认识的键）。\n"
private const val DESKTOP_CONF_HEADER =
    "# OPI 桌面端设置 —— 由 `opi --settings` 写入，手工改也认。\n" +
        "# 只含状态栏/剪贴板/语音/界面等桌面自绘 UI 的键；fcitx5 的 addon 选项在\n" +
        "# \$XDG_CONFIG_HOME/fcitx5/conf/opi_fcitx5.conf（分开是为了不被 autosave 抹掉）。\n"

/** 写一份落点：已有文件 → 合并（保留注释/未知键）；不存在/为空 → 生成。 */
private fun saveOne(p: Path, s: OpSettings, only: Set<String>?, header: String?) {
    p.parent?.let { Files.createDirectories(it) }
    val existing = readSettingsText(p)
    val text = when {
        existing.isNotBlank() -> mergeIni(existing, s, only)
        header != null -> header + mergeIni("", s, only)
        else -> s.toIni() // Windows 新文件：整份模板（含头部注释）
    }
    writeTextAtomically(p, text) // 原子替换，见 AtomicWrite.kt
}
