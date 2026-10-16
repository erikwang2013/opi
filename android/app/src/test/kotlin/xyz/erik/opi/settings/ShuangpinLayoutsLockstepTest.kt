// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 双拼方案名字表的**锁步门禁**（2026-10-14，主流式设置面 M2）：
//! Android 的 `SHUANGPIN_LAYOUTS_ANDROID`（设置页下拉与名字→下标换算的名字源）必须与
//! 引擎 `crates/engine-core/src/shuangpin_tables.rs`（**生成物**）的 `SHUANGPIN_TABLES`
//! 表序**同序同名** —— 顺序即 FFI 下标（`setShuangpinLayout(i)` 的口径），漂一头的表现是
//! 「选了微软、实际按自然码解码」，且两端都不会报错。
//!
//! 从源文件现读现比（不复述列表：复述的清单会跟着漂）。测试工作目录 = `android/app`
//! （同 FuzzyPairsLockstepTest 的口径）。

package xyz.erik.opi.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

class ShuangpinLayoutsLockstepTest {

    /** `shuangpin_tables.rs` 里 `name: "…"` 的出现序 = 表序 = FFI 下标。 */
    private fun engineNames(): List<String> {
        val p = File("../../crates/engine-core/src/shuangpin_tables.rs")
        assertTrue("生成表不在了（搬家了？）：$p", p.exists())
        return Regex("name: \"([^\"]+)\"")
            .findAll(p.readText())
            .map { it.groupValues[1] }
            .toList()
    }

    @Test
    fun androidLayoutNamesMatchTheEngineOrder() {
        val names = engineNames()
        assertEquals("六套生成表该都在（主流同款全套）", 6, names.size)
        assertEquals("Android 名字表必须与引擎同序同名（序 = FFI 下标）", names, SHUANGPIN_LAYOUTS_ANDROID)
    }

    @Test
    fun indexMappingFollowsTheTableOrder() {
        // 下标就是表序：0=自然码、5=拼音加加（与 jni_scheme.rs 的注释同值）。
        assertEquals(0, shuangpinLayoutIndexOf("自然码"))
        assertEquals(5, shuangpinLayoutIndexOf("拼音加加"))
        // 认不得 → 0（与引擎/FFI 的钳制口径一致：坏值钳到确定值）。
        assertEquals(0, shuangpinLayoutIndexOf("打错了"))
    }
}
