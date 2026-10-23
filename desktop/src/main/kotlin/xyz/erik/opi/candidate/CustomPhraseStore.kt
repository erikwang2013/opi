// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 自定义短语的**宿主真源**（2026-10-24）：启用开关 + 整张短语表分别落在
//! `custom_phrases.json`（中文套）与 `custom_phrases_en.json`（英文套，2026-10-24）。
//! 两端 IME 各自在启动/配置重载时读**同一个文件**：
//!
//! · Linux = `$XDG_DATA_HOME/opi/`（缺省 `~/.local/share/opi/`；读侧
//!   `fcitx5-opi/src/custom_phrases.rs`）
//! · Windows = `%LOCALAPPDATA%\opi\`
//!   （读侧 `tsf-opi/src/custom_phrases.rs`，由 `dict_path::dicts_dir` 的父目录推导）
//!
//! 路径推导是**纯函数**（环境量当参数传），单测钉住两条路径 —— 写错 = 设置窗看着
//! 保存成功、输入法永远读不到，静默失效。
//!
//! 失败语义：读 = 缺失/坏文件 → 启用 + 空表（不崩、不删文件，下次保存原子覆盖）；
//! 写 = 唯一临时名 + fsync + rename（[writeTextAtomically]），失败返回面向用户的原因，
//! 此时旧文件原样保留、调用方不该改屏幕。
//!
//! ⚠️ 与 Android 的宿主文件（`filesDir/opi_custom_phrases.json`）**同 schema、不同
//! 落点**：`{"version":1,"enabled":true,"phrases":[...]}`。编解码见
//! [CustomPhraseCodec]（与 Android 那份逐字同源）。

package xyz.erik.opi.candidate

import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths

/** 文件名（与 Rust 两轨、Android 宿主逐字一致）。 */
const val CUSTOM_PHRASES_FILE_NAME = "custom_phrases.json"

/** 英文套文件名：与中文套同 schema、同目录，**独立文件**。 */
const val CUSTOM_PHRASES_EN_FILE_NAME = "custom_phrases_en.json"

/**
 * 宿主文件读取上限：正常最大表 ≈ 2000 条 × 每条 ≤ 2000 字 ≈ 十几 MB；超过这个数
 * 只可能是坏文件/别的文件，按「非法 ⇒ 默认值」处理 —— 不把它整个读进内存。
 */
private const val MAX_HOST_FILE_BYTES = 64L * 1024 * 1024

/** 宿主文件内容：启用态 + 规范化短语表。缺省启用 = 与引擎/Android 同一姿态。 */
data class CustomPhraseState(
    val enabled: Boolean = true,
    val phrases: List<CustomPhrase> = emptyList(),
)

/**
 * 宿主文件路径（**纯函数**：环境量都当参数传）。空串环境量等同未设置 —— 与
 * `dictsDirFor` / `settingsPathFor` / Rust `xdg_data_dir` 同口径。
 */
private fun phrasesPathFor(
    fileName: String,
    windows: Boolean,
    xdgDataHome: String?,
    home: String?,
    localAppData: String?,
): Path = if (windows) {
    val base = localAppData?.takeIf { it.isNotEmpty() } ?: "."
    Paths.get(base, "opi", fileName)
} else {
    val base = xdgDataHome?.takeIf { it.isNotEmpty() } ?: ((home ?: "") + "/.local/share")
    Paths.get(base, "opi", fileName)
}

fun customPhrasesPathFor(
    windows: Boolean,
    xdgDataHome: String?,
    home: String?,
    localAppData: String?,
): Path = phrasesPathFor(CUSTOM_PHRASES_FILE_NAME, windows, xdgDataHome, home, localAppData)

/** 英文套路径（纯函数）：与中文套同一目录、不同文件。 */
fun englishCustomPhrasesPathFor(
    windows: Boolean,
    xdgDataHome: String?,
    home: String?,
    localAppData: String?,
): Path = phrasesPathFor(CUSTOM_PHRASES_EN_FILE_NAME, windows, xdgDataHome, home, localAppData)

private fun hostPath(fileName: String): Path = phrasesPathFor(
    fileName,
    windows = isWindows(),
    xdgDataHome = System.getenv("XDG_DATA_HOME"),
    home = System.getenv("HOME"),
    localAppData = System.getenv("LOCALAPPDATA"),
)

/** 真实环境下的宿主文件（[customPhrasesPathFor] 的取环境量包装）。 */
fun customPhrasesPath(): Path = hostPath(CUSTOM_PHRASES_FILE_NAME)

/** 真实环境下的英文套宿主文件。 */
fun englishCustomPhrasesPath(): Path = hostPath(CUSTOM_PHRASES_EN_FILE_NAME)

/**
 * 读宿主文件。缺失/读不出/超限/内容非法 → **默认值（启用、空表）**，不删文件：
 * 坏文件留着可人工排查，下一次保存会原子覆盖它（与 Android `CustomPhraseStore.load`
 * 同一姿态）。
 */
fun loadCustomPhrases(path: Path = customPhrasesPath()): CustomPhraseState {
    val text = try {
        if (Files.size(path) > MAX_HOST_FILE_BYTES) return CustomPhraseState()
        Files.newBufferedReader(path).use { it.readText() }
    } catch (_: Exception) {
        return CustomPhraseState()
    }
    return when (val d = CustomPhraseCodec.decode(text)) {
        is CustomPhraseDecode.Ok -> CustomPhraseState(d.enabled ?: true, d.phrases)
        is CustomPhraseDecode.Invalid -> CustomPhraseState()
    }
}

/**
 * 原子写宿主文件（唯一临时名 + fsync + rename，见 [writeTextAtomically]）。
 * 返回 null = 成功；否则是**面向用户**的原因。写失败时旧文件原样保留。
 */
fun saveCustomPhrases(state: CustomPhraseState, path: Path = customPhrasesPath()): String? = try {
    path.parent?.let { Files.createDirectories(it) }
    writeTextAtomically(path, CustomPhraseCodec.encodeState(state.enabled, state.phrases))
    null
} catch (e: Throwable) {
    "写盘失败：${e.message ?: e::class.simpleName}"
}

/** 英文套读取：失败语义与 [loadCustomPhrases] 完全相同（缺失/坏文件 = 启用 + 空表）。 */
fun loadEnglishCustomPhrases(path: Path = englishCustomPhrasesPath()): CustomPhraseState =
    loadCustomPhrases(path)

/** 英文套原子写：与 [saveCustomPhrases] 同一份实现，只是默认路径不同。 */
fun saveEnglishCustomPhrases(
    state: CustomPhraseState,
    path: Path = englishCustomPhrasesPath(),
): String? = saveCustomPhrases(state, path)
