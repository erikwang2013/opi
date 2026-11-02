// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

//! 全角映射（2026-11-02 自 EngineController.kt 机械搬出：状态快照改动后那个文件
//! 近 500 行门禁）。同包扩展函数，调用点一字未改。

/**
 * 全角化待上屏文本（**宽档**：ASCII 可打印平移 +0xFEE0；空格与非 ASCII 恒等）。
 * 开关关 ⇒ 原样返回。
 *
 * 为什么要在宿主侧再写一遍映射：**英文/数字模式下的字母数字各端都是直传**，
 * 不进引擎（[xyz.erik.opi.ime.KeyRouter] 的英文直传与数字面板直提），引擎收不到
 * 那些字符 —— 规则搬不过来，只能镜像；语义源 = engine-core 的
 * `punctuation::ascii_fullwidth_wide` 与 `Engine::fullwidthize`（那边的头注写着
 * 「宿主直传点的唯一入口」）。⚠️ 改一边必须改另一边：`FullwidthTextTest` 现读
 * `punctuation.rs` 的常数与范围，两边的用例集相同。
 */
fun EngineController.fullwidthize(text: String): String =
    if (!fullwidth) text else text.map(::widenAscii).joinToString("")

/**
 * 全角宽档的单字符映射（**镜像** engine-core 的 `punctuation::ascii_fullwidth_wide`）：
 * ASCII 可打印（0x21..0x7E：字母/数字/标点，**除空格**）平移 +0xFEE0；其余原样。
 * ⚠️ 空格：全角空格是 U+3000，不在那条连续段上（0x20+0xFEE0 = U+FF00，未分配），
 * 且引擎侧**有意不宽化空格**（见 `Engine::fullwidthize` 头注）—— 这里保持一致。
 * 锁同步：`FullwidthTextTest` 现读 `punctuation.rs` 的常数与范围。
 */
internal fun widenAscii(ch: Char): Char =
    if (ch.code in 0x21..0x7E) (ch.code + 0xFEE0).toChar() else ch
