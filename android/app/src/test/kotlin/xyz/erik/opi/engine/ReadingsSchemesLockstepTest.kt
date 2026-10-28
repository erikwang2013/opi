// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 候选读数的方案/模式守卫**锁步门禁**（2026-10-28）：Kotlin 侧 `needsCandidateReadings`
//! 决定要不要问引擎（省一次 JNI），判据必须与 `crates/engine-core/src/engine_readings.rs`
//! 的 `candidate_readings()` 开头两道守卫一致 —— 漂一头的画面是「某方案左列一直是空的」，
//! 两端都不报错。从源文件现读现比（不复述清单），测试工作目录 = `android/app`
//! （同 WubiNamesLockstepTest 的口径）。

package xyz.erik.opi.engine

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

class ReadingsSchemesLockstepTest {

    private fun engineSource(): String {
        val p = File("../../crates/engine-core/src/engine_readings.rs")
        assertTrue("引擎源不在了（搬家了？）：$p", p.exists())
        return p.readText()
    }

    /** 只取 `candidate_readings()` 的函数体（display_buffer 也有同形守卫，别取错那条）。 */
    private fun readingsBody(): String =
        engineSource().substringAfter("pub fn candidate_readings").substringBefore("\n    pub fn ")

    /** 取 `if !matches!(self.<field>(), Xxx::A | Xxx::B) {` 里的变体名（小写）。 */
    private fun guardValues(field: String): Set<String> =
        Regex("""\w+::(\w+)""")
            .findAll(readingsBody().substringAfter("self.$field(),").substringBefore(") {"))
            .map { it.groupValues[1].lowercase() }.toSet()

    @Test
    fun schemeGuardMatchesKotlin() {
        val rust = guardValues("scheme")
        assertTrue("引擎守卫没解析出来（源格式变了？）", rust.isNotEmpty())
        val kotlin = InputScheme.entries
            .filter { needsCandidateReadings(EngineMode.PINYIN, it) }
            .map { it.name.lowercase() }.toSet()
        assertEquals("读数方案集合必须与 engine_readings.rs 一致", rust, kotlin)
    }

    @Test
    fun modeGuardMatchesKotlin() {
        val rust = guardValues("mode")
        assertTrue("引擎守卫没解析出来（源格式变了？）", rust.isNotEmpty())
        val kotlin = EngineMode.entries
            .filter { needsCandidateReadings(it, InputScheme.NINE) }
            .map { it.name.lowercase() }.toSet()
        assertEquals("读数模式集合必须与 engine_readings.rs 一致", rust, kotlin)
    }
}
