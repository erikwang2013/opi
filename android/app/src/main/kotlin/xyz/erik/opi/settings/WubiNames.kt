// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.settings

//! 五笔的名字表与「名字 → FFI 整数码」换算（2026-10-14，M4 第二波）。
//!
//! **顺序 = 引擎枚举声明序 = FFI/JNI 的 Int 值**（`engine_wubi.rs` 的
//! `WubiStyle/WubiMixedEntry/WubiReturnStyle`）；名字与档位表**不许在本文件之外再写
//! 一份** —— 锁步门禁 `WubiNamesLockstepTest` 现读 `crates/engine-core/src/engine_wubi.rs`
//! 逐项比对（漂开的表现：设置页存「五笔拼音」而引擎只认「纯五笔」⇒ 改了没反应）。
//!
//! 方案代号表另加一条门禁：**每份代号都要有真码表**（`data/raw/wubi{plan}.tsv`，
//! 摆了没有表的方案就是「选了没候选」的假承诺）。

/** 输入方案代号（值 = 码表文件名里那段：`wubi86.opid` / `wubi98.opid`）。 */
internal val WUBI_PLAN_NAMES = listOf("86", "98")

/** 输入风格（顺序 = `WubiStyle` 枚举序：0=纯五笔、1=五笔拼音混输）。 */
internal val WUBI_STYLE_NAMES = listOf("纯五笔", "五笔拼音")

/** 混输进入方式（顺序 = `WubiMixedEntry` 枚举序：0=直接字母、1=z键）。 */
internal val WUBI_MIXED_ENTRY_NAMES = listOf("直接字母", "z键")

/** 回车行为（顺序 = `WubiReturnStyle` 枚举序：0=清空、1=上屏原文）。 */
internal val WUBI_RETURN_NAMES = listOf("清空", "上屏原文")

/** 候选个数档位（3..9：下限 3 是产品档位，上限 = 引擎 `WUBI_CANDIDATE_LIMIT_MAX`）。 */
internal val WUBI_CANDIDATE_LIMIT_CHOICES = (3..9).toList()

/** 候选个数缺省（= 引擎 `WUBI_CANDIDATE_LIMIT_DEFAULT`，门禁现读比对）。 */
internal const val WUBI_CANDIDATE_LIMIT_DEFAULT = 5

/** 名字 → 整数码（认不得 → 0，与引擎「认不得就不动」的读侧哨兵同值）。 */
internal fun wubiStyleIndexOf(name: String): Int =
    WUBI_STYLE_NAMES.indexOf(name).coerceAtLeast(0)

internal fun wubiMixedEntryIndexOf(name: String): Int =
    WUBI_MIXED_ENTRY_NAMES.indexOf(name).coerceAtLeast(0)

internal fun wubiReturnIndexOf(name: String): Int =
    WUBI_RETURN_NAMES.indexOf(name).coerceAtLeast(0)
