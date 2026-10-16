// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 五笔的**设置模型与设置页**（2026-10-14，M4 第二波；lead 冻结的五个配置键）。
//! 独立成文件同 Dicts.kt：Settings.kt 与 SettingsPages.kt 都贴着 500 行门禁，
//! 而五笔这一族（键名表 + ini 读写 + 一页 UI）是自足的一份。
//!
//! ⚠️ **值一律存「名字」不存实现值**（与 Skin/拼音方式/双拼布局同一条惯例）：
//! `WubiStyle=纯五笔`、`WubiReturn=清空` 存进配置的就是中文名；数字只有两处 ——
//! 候选个数（`WubiCandidateLimit`，3..9 的档位）与方案（`WubiPlan` 的 86/98 是
//! **码表代号**，也是字面值）。名字表与引擎枚举**同序**（序 = FFI/JNI 的 Int 值），
//! 锁步门禁在 `SettingsLockstepTest` 现读 `engine_wubi.rs` 逐项比对。
//!
//! ⚠️ 键名是**四端逐字对应**的契约（桌面设置窗 ⇄ fcitx5 `opi_config.h` ⇄ TSF
//! `settings.rs` ⇄ Android 偏好键）：见 `WubiSettings` 里每个键的注释与
//! `Settings.kt` 文件头的「三处逐字对应」段（那段说的是整个文件，五笔键同规矩）。
//! fcitx5/TSF 的读侧**已接**（2026-10-14 同一波的第二块：六键都进 conf/ini 并在
//! 配置生效时推给引擎）；**未接的只有两项**，页上如实写着：回车行为的**消费**
//! （设置位已存，四端路由未接线）与「编码提示」的宿主 UI（读侧出口已备）。
//!
//! ⚠️ **本页不摆「编码提示」**：它的读侧是引擎 `wubi_next_keys`（JNI `wubiNextKeys`），
//! 消费点是「键盘上方提示条」那条 UI，四端都还没接线 —— 摆一个拨不动的开关就是
//! 假承诺（本窗的既有原则，见 SettingsPages.kt 文件头）。接线后**加一行读侧展示**，
//! 不是加开关（编码提示没有开关，只有出口）。

package xyz.erik.opi.candidate

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.ui.Modifier
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.material.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * 五笔设置五键（lead 2026-10-14 冻结，四端同一套）：
 *
 * | 键 | 值域 | 缺省 |
 * |---|---|---|
 * | `WubiPlan` | [WUBI_PLAN_NAMES]（86/98） | 86 |
 * | `WubiStyle` | [WUBI_STYLE_NAMES]（纯五笔/五笔拼音） | 纯五笔 |
 * | `WubiMixedEntry` | [WUBI_MIXED_ENTRY_NAMES]（直接字母/z键） | 直接字母 |
 * | `WubiCandidateLimit` | [WUBI_CANDIDATE_LIMIT_CHOICES]（3..9） | 5 |
 * | `WubiReturn` | [WUBI_RETURN_NAMES]（清空/原文） | 清空 |
 * | `WubiAutoCommit` | True/False | True |
 *
 * 缺省值 = **引擎既有行为**（`WubiStyle::Pure` / `WubiReturnStyle::Clear` /
 * `WUBI_CANDIDATE_LIMIT_DEFAULT=5` / `wubi_auto_commit: true`，逐一由
 * `SettingsLockstepTest` 从 `engine_wubi.rs` / `engine.rs` 现读现比）。
 * 认不得的值**一律回缺省**（与 [OpSettings.fromIni] 的其它名字键同一条容错，
 * 不抛）：注意这与引擎的**钳制**语义不完全等价 —— 手写 `WubiCandidateLimit=1`
 * 引擎会收下（钳域 [1, 9]）、本窗读回来是缺省 5（本窗只认 UI 摆得出的档位，
 * 与 Skin/拼音方式「认不得就回缺省」同一档；写回去即覆盖，不会两边各算各的）。
 */
data class WubiSettings(
    /** 方案：`86` / `98`（两套码表，文件名 `wubi86.opid` / `wubi98.opid`）。 */
    val plan: String = WUBI_PLAN_NAMES.first(),
    /** 输入风格：纯五笔（只出码表候选）/ 五笔拼音（混输，码表 ∪ 拼音主库）。 */
    val style: String = WUBI_STYLE_NAMES.first(),
    /** 混输时拼音的进入方式（直接在字母串里混 / z 开头那段按拼音）—— 风格=纯五笔时不参与。 */
    val mixedEntry: String = WUBI_MIXED_ENTRY_NAMES.first(),
    /** 候选个数档位（3..9，数字选词键 1–9 的物理上限那一档，见引擎常量）。 */
    val candidateLimit: Int = WUBI_CANDIDATE_LIMIT_DEFAULT,
    /** 回车键（缓冲非空时）：清空 / 原文。⚠️ 本版**只存设置**，四端路由未接线。 */
    val returnStyle: String = WUBI_RETURN_NAMES.first(),
    /** 四码唯一自动上屏（默认开）。 */
    val autoCommit: Boolean = true,
) {
    /**
     * ini 行（键名逐字对应契约，见文件头）。顺序 = 表格序（`Settings.kt` 的 toIni
     * 把本段整体 append 在 Skin/候选字号之后）。
     */
    fun toIniLines(): String = buildString {
        appendLine("WubiPlan=$plan")
        appendLine("WubiStyle=$style")
        appendLine("WubiMixedEntry=$mixedEntry")
        appendLine("WubiCandidateLimit=$candidateLimit")
        appendLine("WubiReturn=$returnStyle")
        appendLine("WubiAutoCommit=${if (autoCommit) "True" else "False"}")
    }

    /**
     * 本族五个键的「管理键 → 新值」表（`mergeIni` 与 `toIniLines` 同一份值口径）。
     * 抽出来是防两处各拼一遍漂开（`Settings.mergeIni` 直接 putAll 进它的 managed 表）。
     */
    fun managed(): Map<String, String> = linkedMapOf(
        "WubiPlan" to plan,
        "WubiStyle" to style,
        "WubiMixedEntry" to mixedEntry,
        "WubiCandidateLimit" to candidateLimit.toString(),
        "WubiReturn" to returnStyle,
        "WubiAutoCommit" to if (autoCommit) "True" else "False",
    )

    companion object {
        /**
         * 从 `OpSettings.fromIni` 已解析的键值表读五笔五键；缺键/坏值一律回缺省
         * （不抛 —— 手改坏的配置不该让设置窗打不开，与其余名字键同一条）。
         */
        fun fromIni(map: Map<String, String>): WubiSettings = WubiSettings(
            plan = map["WubiPlan"]?.takeIf { it in WUBI_PLAN_NAMES } ?: WUBI_PLAN_NAMES.first(),
            style = map["WubiStyle"]?.takeIf { it in WUBI_STYLE_NAMES } ?: WUBI_STYLE_NAMES.first(),
            mixedEntry = map["WubiMixedEntry"]?.takeIf { it in WUBI_MIXED_ENTRY_NAMES }
                ?: WUBI_MIXED_ENTRY_NAMES.first(),
            candidateLimit = map["WubiCandidateLimit"]?.toIntOrNull()
                ?.takeIf { it in WUBI_CANDIDATE_LIMIT_CHOICES }
                ?: WUBI_CANDIDATE_LIMIT_DEFAULT,
            returnStyle = map["WubiReturn"]?.takeIf { it in WUBI_RETURN_NAMES }
                ?: WUBI_RETURN_NAMES.first(),
            autoCommit = map["WubiAutoCommit"]?.lowercase()?.let {
                when (it) {
                    "true", "1", "yes", "on" -> true
                    "false", "0", "no", "off" -> false
                    else -> null
                }
            } ?: true,
        )
    }
}

/**
 * 方案代号表（顺序 = 展示序；值是**文件名里那段**：`wubi86.opid` / `wubi98.opid`）。
 * 真源是 `data/generated/` 的两张码表（生成器 `scripts/gen_wubi_dict.py`）——
 * `SettingsLockstepTest` 现读 `data/raw/wubi{plan}.tsv` 存在性把「表里没这份码表」
 * 钉红（摆了 06 却没有 06.opid 就是「选了没候选」的假承诺）。
 */
internal val WUBI_PLAN_NAMES = listOf("86", "98")

/**
 * 输入风格名表（顺序 = 引擎 `WubiStyle` 的枚举声明序 = FFI/JNI 的 Int：
 * 0=纯五笔、1=五笔拼音）。锁步门禁见 `SettingsLockstepTest`。
 */
internal val WUBI_STYLE_NAMES = listOf("纯五笔", "五笔拼音")

/**
 * 混输进入方式名表（顺序 = 引擎 `WubiMixedEntry` 枚举序：0=直接字母、1=z键）。
 * 只在风格=五笔拼音时有语义（引擎侧文档：z 段去掉前导 z 查拼音、非 z 段查码；
 * 代价是混输 + z 键档下 z 码打不出来 —— 页上如实写）。
 */
internal val WUBI_MIXED_ENTRY_NAMES = listOf("直接字母", "z键")

/**
 * 回车行为名表（顺序 = 引擎 `WubiReturnStyle` 枚举序：0=清空、1=原文）。
 * ⚠️ 本版引擎只存这个设置位（消费在四端路由，未接线）—— 页上如实写。
 */
internal val WUBI_RETURN_NAMES = listOf("清空", "上屏原文")

/** 候选个数档位（3..9：下限 3 是产品档位 —— 1–2 个候选不成档，与主流输入法同量级）。 */
internal val WUBI_CANDIDATE_LIMIT_CHOICES = (3..9).toList()

/**
 * 候选个数缺省档（= 引擎 `WUBI_CANDIDATE_LIMIT_DEFAULT`，锁步门禁现读比对）。
 * 别在这里顺手改成 `WUBI_CANDIDATE_LIMIT_CHOICES.first()`：那会与引擎的 5 漂开
 * （首个档位 3 ≠ 引擎缺省 5），而「缺省选中哪个 chip」正是易漂的一处。
 */
internal val WUBI_CANDIDATE_LIMIT_DEFAULT = 5

/**
 * 设置窗「五笔」页。平台分叉见 `settingsTabs`（读侧只认一半的键要在那边过滤，
 * 原则同 ShiftToggle/皮肤：「宁可少一页，不做点不动的开关」）。
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun WubiPage(s: OpSettings, apply: (OpSettings) -> Unit) {
    val w = s.wubi
    SectionTitle("输入方案")
    Text(
        "五笔码表随安装包分发，两套并存：86（老版本）与 98（新版本）。" +
            "同一串键在两边对应的字不同，选哪套用哪套。配置键名 WubiPlan",
        fontSize = 12.sp, color = Muted,
    )
    Spacer(Modifier.height(8.dp))
    // 名字表 = WUBI_PLAN_NAMES（锁步门禁现读 data/raw 的码表存在性）。
    FlowRow(
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        for (name in WUBI_PLAN_NAMES) {
            LayoutChip(name, name == w.plan) { apply(s.copy(wubi = w.copy(plan = name))) }
        }
    }

    SectionTitle("输入风格")
    Text(
        "纯五笔：候选只来自五笔码表。五笔拼音（混输）：码表候选与拼音候选并在一栏，" +
            "两边都像的键串按同一套分数排。配置键名 WubiStyle",
        fontSize = 12.sp, color = Muted,
    )
    Spacer(Modifier.height(8.dp))
    // 名字表 = WUBI_STYLE_NAMES（顺序 = 引擎 WubiStyle 枚举序，锁步门禁在 SettingsLockstepTest）。
    FlowRow(
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        for (name in WUBI_STYLE_NAMES) {
            LayoutChip(name, name == w.style) { apply(s.copy(wubi = w.copy(style = name))) }
        }
    }

    SectionTitle("混输进入方式")
    Text(
        "只在输入风格 =「五笔拼音」时管用：直接字母 = 字母串两边都查；" +
            "z 键 = z 开头那段按拼音解（代价：混输 + z 键档下 z 码打不出来）。" +
            "配置键名 WubiMixedEntry",
        fontSize = 12.sp, color = Muted,
    )
    Spacer(Modifier.height(8.dp))
    FlowRow(
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        for (name in WUBI_MIXED_ENTRY_NAMES) {
            LayoutChip(name, name == w.mixedEntry) {
                apply(s.copy(wubi = w.copy(mixedEntry = name)))
            }
        }
    }

    SectionTitle("特殊习惯")
    SettingRow(
        "四码唯一自动上屏",
        "打满四码且候选只有一个时自动上屏（同码多词不自动，留给你选）；配置键名 WubiAutoCommit",
        w.autoCommit,
    ) { apply(s.copy(wubi = w.copy(autoCommit = it))) }
    Spacer(Modifier.height(6.dp))
    Text("回车键（缓冲非空时）；配置键名 WubiReturn", fontSize = 12.sp, color = Muted)
    Spacer(Modifier.height(6.dp))
    FlowRow(
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        for (name in WUBI_RETURN_NAMES) {
            LayoutChip(name, name == w.returnStyle) {
                apply(s.copy(wubi = w.copy(returnStyle = name)))
            }
        }
    }
    // ⚠️ 如实写（engine_wubi.rs 的 WubiReturnStyle 头注：本波只落设置位，消费点在
    // 四端路由，未接线）—— 别把「已存设置」写成「已生效」。
    Text(
        "本版只存这个设置：回车行为要等各端路由接上它才生效（接线前回车仍是各端既有行为）。",
        fontSize = 12.sp, color = Muted,
    )
    Spacer(Modifier.height(6.dp))
    Text(
        "候选个数（五笔候选一栏最多显示几个）；配置键名 WubiCandidateLimit",
        fontSize = 12.sp, color = Muted,
    )
    Spacer(Modifier.height(6.dp))
    // 档位表 = WUBI_CANDIDATE_LIMIT_CHOICES（上限与引擎 WUBI_CANDIDATE_LIMIT_MAX 锁步）。
    FlowRow(
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        for (n in WUBI_CANDIDATE_LIMIT_CHOICES) {
            LayoutChip("$n", n == w.candidateLimit) {
                apply(s.copy(wubi = w.copy(candidateLimit = n)))
            }
        }
    }
    Spacer(Modifier.height(10.dp))
    // 「编码提示」不摆：读侧出口（wubiNextKeys）四端都还没接线（见文件头）。
    Text(
        "设置写入配置文件，输入法在下次启动（Windows = 下次激活）时读；" +
            "Linux 轨（fcitx5）改完即时生效。码表随安装包分发（data/generated/ 的两张）。",
        fontSize = 12.sp, color = Muted,
    )
}
