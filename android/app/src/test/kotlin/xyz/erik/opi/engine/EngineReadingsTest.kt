// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test

/**
 * 候选读数（9 键左列）的**取数时机**与刷新口径 —— 2026-10-28 从 `EngineControllerTest`
 * 拆出（那边撞 500 行门禁）：本类只问「什么时候问引擎、拿回什么」。
 */
class EngineReadingsTest {

    // 假引擎允许「空缓冲 + 候选」（真引擎里这种形态只属于联想态）。本类用空缓冲触发
    // 刷新，开着门禁让候选原样通过；门禁关闭时要清空的行为由 EngineControllerAssociationTest 钉。
    @Before fun allowAssociation() { AssociationGate.enabled = true }
    @After fun resetGate() { AssociationGate.enabled = false }

    @Test
    fun candidateReadingsComeThroughRefresh() {
        // 读数（9 键左列的数据源，2026-10-09 用户裁决）：与 candidates 同一次刷新
        // 读出（**去重保序在引擎出口**做，本类只透传）；JNI 返回 null 时退空列表
        // （不崩、不当成一条空读数）。
        val fake = FakeEngine().apply {
            cands = arrayOf("你", "你好")
            candReadings = arrayOf("ni", "ni'hao")
            schemeValue = InputScheme.NINE.value // 读数只在 9/14/双拼 + 中文模式下问引擎
        }
        val ctrl = EngineController(fake)
        assertEquals(listOf("你", "你好"), ctrl.candidates)
        assertEquals(listOf("ni", "ni'hao"), ctrl.candidateReadings)
        fake.candReadings = null
        ctrl.refresh()
        assertEquals(emptyList<String>(), ctrl.candidateReadings)
    }

    @Test
    fun candidateReadingsAreNotAskedOutsideReadingsSchemes() {
        // 引擎只在中文 + 9/14/双拼下给读数（engine_readings.rs 开头两道守卫）—— 其余
        // 方案直接跳过那次 JNI（每次击键一次），不拿空数组当结果。
        val fake = FakeEngine().apply {
            cands = arrayOf("你")
            candReadings = arrayOf("ni")
        }
        val ctrl = EngineController(fake) // 缺省 FULL
        assertEquals(emptyList<String>(), ctrl.candidateReadings)
        assertEquals(0, fake.candidateReadingsCalls)

        fake.schemeValue = InputScheme.NINE.value
        ctrl.refresh()
        assertEquals(listOf("ni"), ctrl.candidateReadings)
        assertEquals(1, fake.candidateReadingsCalls)

        fake.schemeValue = InputScheme.STROKE.value
        ctrl.refresh()
        assertEquals(emptyList<String>(), ctrl.candidateReadings)
        assertEquals(1, fake.candidateReadingsCalls) // 没再多问引擎

        fake.schemeValue = InputScheme.NINE.value
        fake.mode = EngineMode.ENGLISH.value
        ctrl.refresh()
        assertEquals(emptyList<String>(), ctrl.candidateReadings)
        assertEquals(1, fake.candidateReadingsCalls)
    }
}
