// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 模糊音名字表的**锁步门禁**（2026-10-13，主流式设置面 M1a）：
//! Android 的 `FUZZY_PAIRS_ANDROID`（设置页多选与掩码解析的名字源）必须与引擎
//! `crates/engine-core/src/fuzzy.rs` 的 `FUZZY_PAIRS` **同序同名** —— 顺序即掩码位序
//! （`fuzzyMaskOf` 的判据），漂一头的表现是「勾了 A 实际开了 B」，且两端都不会报错。
//!
//! 从**源文件现读现比**（不复述列表：复述的清单会跟着漂）。测试工作目录 = `android/app`
//! （同 SymbolCategoriesTest 的口径）。

package xyz.erik.opi.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

class FuzzyPairsLockstepTest {

    @Test
    fun androidPairNamesMatchTheEngineOrder() {
        val p = File("../../crates/engine-core/src/fuzzy.rs")
        assertTrue("引擎源不在了（搬家了？）：$p", p.exists())
        val src = p.readText()
        val block = src.substringAfter("pub const FUZZY_PAIRS").substringBefore("];")
        val a = Regex("\"([a-z]+)\", \"[a-z]+\"").findAll(block).map { it.groupValues[1] }.toList()
        val b = Regex("\"[a-z]+\", \"([a-z]+)\"").findAll(block).map { it.groupValues[1] }.toList()
        val pairs = a.zip(b).map { (x, y) -> "$x/$y" }
        assertEquals("Android 名字表必须与引擎同序同名（位序 = 掩码位序）", pairs, FUZZY_PAIRS_ANDROID)
    }

    @Test
    fun maskParsingFollowsTheTableOrder() {
        // 位序就是表序：第 0 位 = zh/z、第 6 位 = n/l。
        assertEquals(1L, fuzzyMaskOf("zh/z").toLong())
        assertEquals(1L shl 6, fuzzyMaskOf("n/l").toLong())
        assertEquals((1L shl 0) or (1L shl 6), fuzzyMaskOf("zh/z, n/l").toLong())
        assertEquals("认不得的整条忽略", 0L, fuzzyMaskOf("xx/yy").toLong())
        assertEquals("全表 = 全开", 0xFFL, fuzzyMaskOf(FUZZY_PAIRS_ANDROID.joinToString(",")).toLong())
    }
}
