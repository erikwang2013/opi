// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 五笔名字表/档位的**锁步门禁**（2026-10-14，M4 第二波）：Android 的
//! `WUBI_STYLE_NAMES` / `WUBI_MIXED_ENTRY_NAMES` / `WUBI_RETURN_NAMES` 与
//! `WUBI_CANDIDATE_LIMIT_DEFAULT` 必须与引擎 `crates/engine-core/src/engine_wubi.rs`
//! **同序同名同值** —— 顺序即 FFI 整数码（`wubiStyle` 等出口的入参口径），
//! 漂一头的表现是「选了五笔拼音、实际按纯五笔」，两端都不报错。
//!
//! 从源文件现读现比（不复述列表：复述的清单会跟着漂）。测试工作目录 = `android/app`
//! （同 FuzzyPairsLockstepTest 的口径）。

package xyz.erik.opi.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

class WubiNamesLockstepTest {

    private fun engineSource(): String {
        val p = File("../../crates/engine-core/src/engine_wubi.rs")
        assertTrue("引擎源不在了（搬家了？）：$p", p.exists())
        return p.readText()
    }

    private fun namesOf(enumName: String): List<String> =
        Regex("""$enumName::\w+ => "([^"]+)",""").findAll(engineSource())
            .map { it.groupValues[1] }.toList()

    @Test
    fun androidNamesMatchTheEngineOrder() {
        val style = namesOf("WubiStyle")
        val entry = namesOf("WubiMixedEntry")
        val ret = namesOf("WubiReturnStyle")
        assertTrue("三个名字表都该非空（引擎源格式变了？）", style.isNotEmpty())
        assertEquals("风格名字表必须与引擎同序同名（序 = FFI 整数码）", style, WUBI_STYLE_NAMES)
        assertEquals("混输进入方式名字表必须与引擎同序同名", entry, WUBI_MIXED_ENTRY_NAMES)
        assertEquals("回车名字表必须与引擎同序同名", ret, WUBI_RETURN_NAMES)
        assertEquals(0, wubiStyleIndexOf("纯五笔"))
        assertEquals(1, wubiStyleIndexOf("五笔拼音"))
        assertEquals("认不得 → 0（与引擎读侧哨兵同值）", 0, wubiReturnIndexOf("打错了"))
    }

    @Test
    fun candidateLimitFollowsTheEngineConstants() {
        val src = engineSource()
        val dflt = Regex("""WUBI_CANDIDATE_LIMIT_DEFAULT: usize = (\d+);""")
            .find(src)!!.groupValues[1].toInt()
        val max = Regex("""WUBI_CANDIDATE_LIMIT_MAX: usize = (\d+);""")
            .find(src)!!.groupValues[1].toInt()
        assertEquals("缺省档必须等于引擎常量", dflt, WUBI_CANDIDATE_LIMIT_DEFAULT)
        assertEquals("UI 档位上限 = 引擎上限", max, WUBI_CANDIDATE_LIMIT_CHOICES.last())
        assertTrue("缺省档必须在档位表里", WUBI_CANDIDATE_LIMIT_DEFAULT in WUBI_CANDIDATE_LIMIT_CHOICES)
    }

    @Test
    fun everyPlanHasARealTable() {
        // 摆了没有表的方案 = 「选了没候选」的假承诺（表随 APK 分发，见 EngineLoader 的
        // ASSET_NAME_WUBI86/98 —— 两份都要在 assets 里）。
        for (plan in WUBI_PLAN_NAMES) {
            val asset = File("src/main/assets/wubi$plan.opid")
            assertTrue("assets 里缺 wubi$plan.opid（EngineLoader 会装载它）：$asset", asset.exists())
            assertTrue("wubi$plan.opid 不该是空文件", asset.length() > 0)
        }
    }
}
