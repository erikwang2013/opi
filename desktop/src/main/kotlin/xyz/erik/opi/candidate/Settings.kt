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
//!   的候选目录同一处）。写完经控制面 `reload`（候选窗进程 broker）让 TSF 立即重读；
//!   候选进程没起时退回下一次激活（读侧语义见 `crates/tsf-opi/src/settings.rs`）。
//!
//! ⚠️ **键名三处逐字对应**（改一处 = 老配置文件全部失效）：
//!   本文件 ⇄ `crates/fcitx5-opi/cpp/opi_config.h`（Linux 读侧）⇄
//!   `crates/tsf-opi/src/settings.rs`（Windows 读侧）。
//!   ⚠️ `Skin` 的值是**中文字面值**（`SkinOption` / opi_config.h 的
//!   `FCITX_CONFIG_ENUM_NAME_WITH_I18N`）：两侧按字面匹配，写错字 = 静默回「不改动」。
//!   **2026-10-22 起 Windows 也用**：候选窗/状态栏两个桌面进程 1s 轮询本文件并跟随
//!   （`Main.kt`/`StatusBar.kt`），设置窗「外观」页两平台都出。
//!
//! ⚠️ 布尔写 **`True`/`False`**（fcitx5 的 ini 就是这个拼法；读侧大小写不敏感，
//! 所以手改成 `true` 也能读，但**写回去统一成全大写首字母**，别两种混着出现）。
//!
//! ⚠️ `StatusBar*` 十一个键（2026-10-10 加；2026-10-19 加 `StatusBarEnPro`/`StatusBarScale`）**只有本窗读写**：两端 IME 读侧都不认识
//! 它们（未知键会被忽略，向前兼容）；但 Linux 侧它们仍写在同一份 fcitx5 addon 配置
//! 里 —— 用 fcitx5 配置工具保存时这些键**是否被保存方抹掉**本侧无法保证（上游行为，
//! 本机未取证）。若真发生，表现是「状态栏设置被重置」，不是启动失败。
//! `CandidateFont`（2026-10-14 候选字号）同理 —— 且它的**真落点**根本不在本文件：
//! 见 ClassicUiFont.kt（fcitx5 classicui.conf 的 `Font=`）。
//!
//! ⚠️ 保存走 [mergeIni]：**保留本文件不认识的行**（fcitx5 侧新增的选项、手写注释、
//! 未知键）。2026-10-09 前是整文件重写 —— 那会让
//! 「设置窗保存一次 = 把 fcitx5 配置面里本窗不知道的选项全抹掉」。读侧一直忽略
//! 未知键（向前兼容），写侧从那天起同一条约定。

package xyz.erik.opi.candidate

import xyz.erik.opi.scene.SceneTheme
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths

/**
 * 五个开关 + 皮肤 + 状态栏。默认值 = 引擎既有行为（开关全开、皮肤不改动），与两端的
 * 读侧一字不差。
 *
 * ⚠️ [shiftToggle]：**2026-10-22 起两平台都认**。Linux 读侧 = `opi_config.h` 的
 * `ShiftToggle`（C++ glue 的 `OpiShiftToggle`）；Windows 读侧 = `crates/tsf-opi/src/
 * settings.rs` 同名键（胶水三拍在 `tsf_shift.rs` 查它）。设置窗两平台都列这一行。
 */
data class OpSettings(
    val learn: Boolean = true,
    val chinesePunct: Boolean = true,
    // 缺省 false = 关（2026-10-15 用户裁决「默认关闭模糊音」）。
    val fuzzy: Boolean = false,
    /**
     * 模糊音**逐对**列表（2026-10-13 用户裁决「可多选类似 z=zh」）：
     * 逗号分隔组名，名字表 = [FUZZY_PAIR_NAMES]（顺序即引擎位序，见那个常量）。
     *
     * **空串 = 未设置 ⇒ 各端回落 [fuzzy] 布尔**（老配置行为一字不变）；非空 ⇒ 以列表
     * 为准（[fuzzy] 此时只是「任一组开着」的兼容投影，由设置页在改动时同步写）。
     */
    val fuzzyPairs: String = "",
    val jianpin: Boolean = true,
    /** 智能联想（2026-10-28 用户裁决「默认关闭」；键 `Association`）：缺省 **false**，与 fcitx5/TSF 同步。 */
    val association: Boolean = false,
    /** 拼音纠错（2026-10-13 用户裁决「混合参与排序」；键 `Correction`）。 */
    val correction: Boolean = true,
    /**
     * 表情页显示「自有」分类（2026-10-20 用户裁决；键 `ShowOwn`）：false（缺省）=
     * 顶栏不出现自有子页；true = 出现小欧贴纸子页（点选仍走图片剪贴板，无字体字符）。
     */
    val showOwn: Boolean = false,
    /**
     * 双拼布局（2026-10-13；键 `ShuangpinLayout`）：**方案名**（[SHUANGPIN_LAYOUT_NAMES]
     * 之一，与 fcitx5 的枚举字面值、TSF/Android 的偏好同名同表）。只在引擎方案 =
     * 双拼时参与解码，其余方案下写它无害。缺键/认不得 → 表首（自然码）。
     */
    val shuangpinLayout: String = SHUANGPIN_LAYOUT_NAMES.first(),
    /**
     * 拼音方式（2026-10-14；键 `PinyinMethod`）：**方案名**（[SCHEME_NAMES] 之一 ——
     * 名字表 = 引擎 `engine_core::Scheme::name`）。只在桌面可达的是全拼/双拼
     * （9/14 键/笔画是移动端的方案；桌面的枚举只列这两项）。缺键/认不得 → 全拼。
     */
    val pinyinMethod: String = SCHEME_NAMES.first(),
    val shiftToggle: Boolean = true,
    /** 纯色皮肤（2026-10-31 默认改为清爽；`Skin=` 中文字面值）。 */
    val skin: SkinOption = SkinOption.FRESH,
    /**
     * 动态主题（2026-10-30；键 `Background=`，值 = [SceneTheme.id]）：状态栏条外
     * 角色与手机键盘插画共用一套。**只影响小欧自绘 UI，不推引擎**；缺键/空/未知 =
     * 纯色皮肤（配色见 [skin]）。手机端存自己的偏好（`kb_scene`），语义同集。
     */
    val background: SceneTheme? = null,
    /**
     * 候选字号（2026-10-14 用户裁决；键 `CandidateFont`）：**档位名**
     * （[CLASSIC_UI_FONT_NAMES] 之一 —— 小/中/大）。存的只是名字，真正的落点是 fcitx5
     * classicui 的 `Font=`（三档 ↔ pango 描述与写入见 ClassicUiFont.kt）——
     * 与 Skin/拼音方式同一条「存名字不存实现值」惯例。缺键/认不得 → 中。
     */
    val candidateFont: String = CLASSIC_UI_FONT_DEFAULT,
    /**
     * 导入词库的**停用名单**（2026-10-14 词库页；键 `DisabledDicts`）：逗号分隔的词库名
     * （= 词库目录里的 `.tsv` 文件名去后缀，见 Dicts.kt 的文件头契约）。**空 = 全部启用**
     * （缺省）。名字随用户导入的文件走，不是枚举 —— 认不得的名字只是没有对应文件，无害。
     */
    val disabledDicts: String = "",
    /**
     * 五笔六键（2026-10-14，M4 第二波）：方案 / 风格 / 混输进入方式 / 候选个数 /
     * 回车 / 自动上屏。自成一族（模型 + 页 + 名字表都在 `Wubi.kt`）。值一律存
     * **名字/档位**（与 Skin/拼音方式同惯例）；缺键/认不得回缺省
     * （见 [WubiSettings.fromIni]）。
     */
    val wubi: WubiSettings = WubiSettings(),
    val statusbar: StatusBarSettings = StatusBarSettings(),
) {
    /** 写成 ini。键名见文件头「三处逐字对应」。 */
    fun toIni(): String = buildString {
        appendLine("# OPI 属性设置 —— 由 `opi --settings` 写入，手工改也认。")
        // ⚠️ 别写「改完 fcitx5-remote -r 即时生效」—— 那条实测无效（见 notifyFcitxReload）。
        appendLine("# Linux：本文件就是 fcitx5 的 addon 配置；设置窗/配置工具保存即重载。")
        appendLine("# 布尔写 True/False；未知键会被两端的读侧忽略（向前兼容）。")
        appendLine("Learn=${boolText(learn)}")
        appendLine("ChinesePunct=${boolText(chinesePunct)}")
        appendLine("Fuzzy=${boolText(fuzzy)}")
        appendLine("FuzzyPairs=$fuzzyPairs")
        appendLine("Jianpin=${boolText(jianpin)}")
        appendLine("Association=${boolText(association)}")
        appendLine("Correction=${boolText(correction)}")
        // 表情页「自有」（2026-10-20）：表情窗打开时读；不分平台地写全（同一份文件格式）。
        appendLine("ShowOwn=${boolText(showOwn)}")
        appendLine("ShuangpinLayout=$shuangpinLayout")
        appendLine("PinyinMethod=$pinyinMethod")
        // Shift 轻按切换中英文（Linux 读侧 = opi_config.h 的 ShiftToggle；Windows 忽略此键）。
        appendLine("ShiftToggle=${boolText(shiftToggle)}")
        // 皮肤只在 Linux 侧有意义；Windows 读侧忽略未知键，写它只为同一份文件格式不分叉。
        appendLine("Skin=${skin.value}")
        // 动态主题（2026-10-30）：只给小欧自绘 UI 读，引擎不认；写 id（ASCII）而不是
        // 中文名。空值 = 纯色皮肤（见 [background]）。
        appendLine("Background=${background?.id ?: ""}")
        // 候选字号：存的只是档位名；真正的落点是 fcitx5 classicui.conf 的 Font=
        // （保存时按需写过去，见 ClassicUiFont.kt）。两侧 IME 读侧都不认这个键
        // （与 StatusBar* 同一条：未知键忽略），写全只为文件格式不因平台分叉。
        appendLine("CandidateFont=$candidateFont")
        // 词库页的停用名单（2026-10-14）：逗号分隔的词库名，空 = 全部启用。
        // 与其它键同一份文件；保存后由 IME 侧重读（Windows = 控制面 reload）。
        appendLine("DisabledDicts=$disabledDicts")
        // 五笔六键（2026-10-14）：整段 append（值口径与 mergeIni 的 managed() 同一份）。
        append(wubi.toIniLines())
        // 状态栏十一个键：只有本窗读写（见文件头 ⚠️）。同样不分平台地写全，
        // 「同一份文件格式」是这里的既有纪律。
        appendLine("StatusBarHidden=${boolText(statusbar.hidden)}")
        appendLine("StatusBarVk=${boolText(statusbar.vk)}")
        appendLine("StatusBarSymbols=${boolText(statusbar.symbols)}")
        appendLine("StatusBarTrad=${boolText(statusbar.trad)}")
        appendLine("StatusBarPunct=${boolText(statusbar.punct)}")
        appendLine("StatusBarLang=${boolText(statusbar.lang)}")
        appendLine("StatusBarEnPro=${boolText(statusbar.enpro)}")
        appendLine("StatusBarLogo=${boolText(statusbar.logo)}")
        appendLine("StatusBarFullwidth=${boolText(statusbar.fullwidth)}")
        appendLine("StatusBarSettings=${boolText(statusbar.settings)}")
        // 统一键大小（2026-10-19）：整数百分比，状态栏进程 1s 轮询后自行重排。
        appendLine("StatusBarScale=${statusbar.scale}")
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
                // 缺键回落 **false**（2026-10-15 用户裁决「默认关闭模糊音」）；
                // 与 [OpSettings.fuzzy] 的数据类缺省、引擎 `Engine` 初值（掩码 0）三处同步。
                fuzzy = bool("Fuzzy", false),
                fuzzyPairs = map["FuzzyPairs"] ?: "",
                jianpin = bool("Jianpin", true),
                association = bool("Association", false),
                correction = bool("Correction", true),
                showOwn = bool("ShowOwn", false),
                shuangpinLayout = map["ShuangpinLayout"]
                    ?.takeIf { it in SHUANGPIN_LAYOUT_NAMES }
                    ?: SHUANGPIN_LAYOUT_NAMES.first(),
                pinyinMethod = map["PinyinMethod"]
                    ?.takeIf { it in SCHEME_NAMES }
                    ?: SCHEME_NAMES.first(),
                shiftToggle = bool("ShiftToggle", true),
                skin = SkinOption.fromValue(map["Skin"]),
                background = map["Background"]?.takeIf { it.isNotEmpty() }
                    ?.let { id -> SceneTheme.entries.firstOrNull { it.id == id } },
                candidateFont = map["CandidateFont"]
                    ?.takeIf { it in CLASSIC_UI_FONT_NAMES }
                    ?: CLASSIC_UI_FONT_DEFAULT,
                // 缺键 = 空 = 全部启用（不是全停用！）—— 老配置升级后词库照常可用。
                disabledDicts = map["DisabledDicts"] ?: "",
                wubi = WubiSettings.fromIni(map),
                statusbar = StatusBarSettings(
                    hidden = bool("StatusBarHidden", false),
                    // ⚠️ 缺键回落 false（2026-10-10 中午裁决：软键盘默认关）。别照着
                    // 邻近行复制成 true —— SettingsTest 的「config without statusbar
                    // keys」用例比对的是整个 StatusBarSettings()，两处漂了就红在那。
                    vk = bool("StatusBarVk", false),
                    symbols = bool("StatusBarSymbols", true),
                    trad = bool("StatusBarTrad", true),
                    punct = bool("StatusBarPunct", true),
                    lang = bool("StatusBarLang", true),
                    enpro = bool("StatusBarEnPro", true),
                    logo = bool("StatusBarLogo", true),
                    fullwidth = bool("StatusBarFullwidth", true),
                    settings = bool("StatusBarSettings", true),
                    // 缺键/坏值回落 100%，越界值收紧（与数据类缺省同一常量）。
                    scale = map["StatusBarScale"]?.toIntOrNull()
                        ?.let(::statusBarScalePercent) ?: STATUS_BAR_SCALE_DEFAULT,
                ),
            )
        }
    }
}


/**
 * 模糊音组名表（顺序 = 引擎 `engine-core/src/fuzzy.rs` 的 `FUZZY_PAIRS` 数组序 =
 * 掩码位序）。**锁步门禁**：`SettingsTest` 文本读 `fuzzy.rs` 逐项比对 —— 那边加组
 * 这里不加会红。展示用规范名 `zh/z`（与引擎 `fuzzy::pair_names()` 同形）。
 */
internal val FUZZY_PAIR_NAMES = listOf(
    "zh/z", "ch/c", "sh/s", "an/ang", "en/eng", "in/ing", "n/l", "f/h",
)

/**
 * 双拼方案名表（顺序 = 引擎 `engine-core/src/shuangpin_tables.rs` 的
 * `SHUANGPIN_TABLES` 表序 = FFI 下标）。**锁步门禁**：`SettingsTest` 文本读那张生成表
 * 逐项比对 —— 那边加方案这里不加会红。配置文件里存的就是这些**名字**
 * （fcitx5 的枚举字面值同名）。
 */
internal val SHUANGPIN_LAYOUT_NAMES = listOf(
    "自然码", "微软", "小鹤", "智能ABC", "紫光", "拼音加加",
)

/**
 * 拼音方式名表（顺序 = 引擎 `Scheme` 的码序，但桌面只列**可达的两项**：全拼/双拼）。
 * 名字的真源是 `engine_core::scheme::Scheme::name`（Rust 侧 `from_name` 解析同一套
 * 字符串）；第三份镜像在 fcitx5 的 `OpiPinyinMethod` i18n 字面值（配置字面值 =
 * 这里的值）。**锁步门禁**：`SettingsTest` 现读 `scheme.rs` 与 `opi_config.h` 比对。
 */
internal val SCHEME_NAMES = listOf("全拼", "双拼")

/**
 * 当前**生效**的模糊音组集（设置页与门禁共用；口径 = 各端读侧那一套）：
 * `fuzzyPairs` 非空 ⇒ 解析它；空 ⇒ 回落 [OpSettings.fuzzy] 布尔（全开/全关）。
 * 解析容错与引擎的 `fuzzy::mask_from_names` 一致：逗号切、去空白、认不得的忽略。
 */
internal fun OpSettings.fuzzyPairSet(): Set<String> =
    if (fuzzyPairs.isNotEmpty()) {
        fuzzyPairs.split(',').map { it.trim() }.filter { it in FUZZY_PAIR_NAMES }.toSet()
    } else if (fuzzy) {
        FUZZY_PAIR_NAMES.toSet()
    } else {
        emptySet()
    }

/**
 * 勾/取消一组：**具体化**成 `fuzzyPairs` 列表（首次触碰即把旧布尔态落成显式列表，
 * 这是「空 = 未设置」合同下的正确单程票），并同步 [OpSettings.fuzzy] 投影
 * （任一组开着）。一个都不勾 ⇒ 列表为空串 + `fuzzy=false` —— 仍表达「全关」。
 */
internal fun OpSettings.toggleFuzzyPair(name: String): OpSettings {
    val next = fuzzyPairSet().toMutableSet().apply { if (!add(name)) remove(name) }
    return copy(
        fuzzyPairs = FUZZY_PAIR_NAMES.filter { it in next }.joinToString(","),
        fuzzy = next.isNotEmpty(),
    )
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
 * Windows：落盘后经控制面 `reload` 让 TSF 重读；没连上返回 false，UI 提示下次激活生效。
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
    writeTextAtomically(p, if (existing.isBlank()) s.toIni() else mergeIni(existing, s)) // 原子替换，见 AtomicWrite.kt
    return reloadImeSettings()
}

/**
 * 让正在运行的输入法重读配置与词库（设置保存 / 词库导入删除后调用）。
 * false = 没连上（候选进程没起；文件已写好，下次激活仍生效）。
 * Linux = ReloadAddonConfig；Windows = 控制面 reload（ControlPlane.kt）。
 */
internal fun reloadImeSettings(): Boolean =
    if (isWindows()) controlReload() else notifyFcitxReload()

/**
 * 保存结果的**用户可见**措辞 —— 与 [saveSettings] 的三个面一一对应。
 * 设置窗的状态行与状态栏的提示行**共用这一份**：两处各写一句必然会漂，而「没立即生效」
 * 正是最容易被写错的一句。星号（`**…**`）一概不加：两处都是纯 Text，会原样显示。
 */
internal fun saveStatusMessage(live: Boolean): String = when {
    live && isWindows() -> "已保存；已通知输入法重载（立即生效）"
    live -> "已保存并通知 fcitx5 重载（立即生效）"
    isWindows() -> "已保存；Windows 侧在输入法下一次激活时生效"
    else -> "已保存到文件；但没能通知 fcitx5 重载（它没在跑？重启输入法后生效）"
}

/**
 * 让 fcitx5 重载**本 addon** 的配置（= 重读刚写的文件）。
 * addon 名 `opi_fcitx5` 就是本程序的 addon（见 addon conf 的 id）；保留旧路兜底
 * （[reloadAddonConfig] 的 `legacyRemote`，语义见那个函数）。
 */
internal fun notifyFcitxReload(): Boolean = reloadAddonConfig("opi_fcitx5", legacyRemote = true)

/**
 * 让 fcitx5 重载**某个 addon** 的配置：两级，任一成功即 true ——
 * busctl（systemd 系，deepin 有）→ dbus-send（其余发行版），都调
 * `ReloadAddonConfig(name)`（2026-10-09 实测有效的那条）。
 *
 * [legacyRemote] = 是否再挂 `fcitx5-remote -r` 当最后一级。它**不会**让 addon 重读
 * 配置（实测），只在没有 dbus 工具时聊胜于无 ⇒ 本 addon 那条保留（旧路，别动）；
 * **classicui 不挂**：那边实测它是「退出码 0、无声无效」的假绿（见 ClassicUiFont.kt）。
 */
internal fun reloadAddonConfig(name: String, legacyRemote: Boolean): Boolean {
    val calls = listOfNotNull(
        listOf(
            "busctl", "--user", "call", "org.fcitx.Fcitx5", "/controller",
            "org.fcitx.Fcitx.Controller1", "ReloadAddonConfig", "s", name,
        ),
        listOf(
            "dbus-send", "--session", "--print-reply",
            "--dest=org.fcitx.Fcitx5", "/controller",
            "org.fcitx.Fcitx.Controller1.ReloadAddonConfig", "string:$name",
        ),
        if (legacyRemote) listOf("fcitx5-remote", "-r") else null,
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
