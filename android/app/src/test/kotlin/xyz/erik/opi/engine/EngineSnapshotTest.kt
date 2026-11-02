// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/**
 * 状态快照（性能项 1+2）：refresh 优先一发 JNI 取全量；失败/旧 so 安全回退；
 * 读数只在中英条件需要时取；折叠一页、平铺 512。
 */
class EngineSnapshotTest {
    private fun cands(n: Int) = Array(n) { "c$it" }

    @Before fun allowAssociation() { AssociationGate.enabled = true }
    @After fun resetGate() { AssociationGate.enabled = false }

    @Test
    fun snapshotRefreshCarriesAllStateInOneCall() {
        val fake = FakeEngine().apply {
            snapshotEnabled = true
            buf = "wo"
            cands = arrayOf("我", "窝")
            displayBuf = "wo"
            pending = "我"
            learnerOn = false
            fuzzyOn = false
            jianpinOn = false
            fullwidthValue = true
        }
        val ctrl = EngineController(fake)

        assertEquals(1, fake.snapshotCalls) // init 只打这一发快照
        assertEquals("wo", ctrl.buffer)
        assertEquals(EngineMode.PINYIN, ctrl.mode)
        assertEquals(InputScheme.FULL, ctrl.inputScheme)
        assertFalse(ctrl.learnerEnabled)
        assertFalse(ctrl.fuzzyEnabled)
        assertFalse(ctrl.jianpinEnabled)
        assertTrue(ctrl.fullwidth)
        assertEquals(listOf("我", "窝"), ctrl.candidates)
        assertEquals(emptyList<String>(), ctrl.candidateReadings)
        assertEquals("wo", ctrl.reading)
        assertEquals("我", ctrl.pendingText)
        // 非读数方案：排名深度 = 回传窗口（普通栏一页），读数那次 JNI 不问。
        assertEquals(fake.pageSizeValue, fake.lastLimit)
        assertEquals(0, fake.candidateReadingsCalls)
    }

    @Test
    fun snapshotFailureFallsBackToLegacyReads() {
        val fake = FakeEngine().apply {
            snapshotThrows = UnsatisfiedLinkError("old so has no stateSnapshot")
            buf = "ab"
            cands = arrayOf("阿", "啊")
            schemeValue = InputScheme.NINE.value
            candReadings = arrayOf("a")
            displayBuf = "a"
            pending = "阿"
        }
        val ctrl = EngineController(fake)

        assertEquals("ab", ctrl.buffer)
        assertEquals(listOf("阿", "啊"), ctrl.candidates)
        assertEquals(listOf("a"), ctrl.candidateReadings)
        assertEquals("a", ctrl.reading)
        assertEquals("阿", ctrl.pendingText)
        ctrl.refresh()
        // 失败一次即永久回退：后续击键不再每次都抛 UnsatisfiedLinkError。
        assertEquals(1, fake.snapshotCalls)
    }

    @Test
    fun malformedSnapshotFallsBackWithoutCrash() {
        val fake = FakeEngine().apply {
            snapshotEnabled = true
            snapshotRaw = arrayOf("truncated") // 槽位不足 ⇒ 严格解析拒绝
            buf = "ni"
            cands = arrayOf("你")
            displayBuf = "ni"
        }
        val ctrl = EngineController(fake)

        assertEquals("ni", ctrl.buffer)
        assertEquals(listOf("你"), ctrl.candidates)
        assertEquals("ni", ctrl.reading)
        ctrl.refresh()
        assertEquals(1, fake.snapshotCalls)
    }

    @Test
    fun emptySnapshotBeforeEngineLoadRetriesAfterLoad() {
        // Service 构造先于 EngineLoader：这时真出口返回空数组（with_engine 未装载）。
        // 不能据此永久回退 —— 装载完成后的刷新必须能重新用上快照。
        val fake = FakeEngine().apply {
            snapshotEnabled = true
            snapshotRaw = emptyArray()
            buf = "ni"
            cands = arrayOf("你")
            displayBuf = "ni"
        }
        val ctrl = EngineController(fake)
        assertEquals(listOf("你"), ctrl.candidates) // 首刷回退逐出口

        fake.snapshotRaw = null
        fake.cands = arrayOf("你", "拟")
        ctrl.refresh()
        assertEquals(2, fake.snapshotCalls)
        assertEquals(listOf("你", "拟"), ctrl.candidates)
    }

    @Test
    fun readingsOnlyFetchedForChineseReadingsSchemes() {
        val fake = FakeEngine().apply {
            snapshotEnabled = true
            schemeValue = InputScheme.NINE.value
            cands = cands(100)
            candReadings = arrayOf("ni", "hao")
        }
        val ctrl = EngineController(fake)

        assertEquals(1, fake.candidateReadingsCalls)
        assertEquals(listOf("ni", "hao"), ctrl.candidateReadings)
        // 折叠栏只回一页；但 9 键读数要覆盖整份排名 ⇒ 内部排名仍是 512。
        assertEquals(fake.pageSizeValue, fake.snapshotLimit)
        assertEquals(fake.pageSizeValue, ctrl.candidates.size)
        assertEquals(FETCH_LIMIT_FULL, fake.lastLimit)

        fake.schemeValue = InputScheme.FULL.value
        ctrl.refresh()
        assertEquals(1, fake.candidateReadingsCalls) // 全键盘不再取读数
        assertEquals(fake.pageSizeValue, fake.lastLimit)
    }

    @Test
    fun snapshotSlotConstantsMatchRustLayout() {
        // 槽位是跨语言约定：Rust 改下标而 Kotlin 没跟，画面会静默错位。与
        // ReadingsSchemesLockstepTest 同口径，现读源文件比对（工作目录 = android/app）。
        val source = java.io.File("../../crates/opi-ffi/src/jni_snapshot.rs")
        assertTrue("快照出口源不在了（搬家了？）：$source", source.exists())
        val text = source.readText()
        val rust = Regex("""const SLOT_(\w+): usize = (\d+);""")
            .findAll(text)
            .associate { it.groupValues[1] to it.groupValues[2].toInt() }
        val kotlin = mapOf(
            "BUFFER" to SNAP_BUFFER,
            "BUFFER_CARRIED" to SNAP_BUFFER_CARRIED,
            "MODE" to SNAP_MODE,
            "SCHEME" to SNAP_SCHEME,
            "LEARNER" to SNAP_LEARNER,
            "FUZZY" to SNAP_FUZZY,
            "JIANPIN" to SNAP_JIANPIN,
            "FULLWIDTH" to SNAP_FULLWIDTH,
            "WUBI_HINT" to SNAP_WUBI_HINT,
            "DISPLAY" to SNAP_DISPLAY,
            "PENDING" to SNAP_PENDING,
            "CAND_COUNT" to SNAP_CAND_COUNT,
            "CAND_START" to SNAP_CAND_START,
        )
        assertEquals("SNAP_* 槽位必须与 jni_snapshot.rs 一致", kotlin, rust)
        val max = Regex("""const MAX_SNAPSHOT_CANDIDATES: usize = (\d+);""")
            .find(text)?.groupValues?.get(1)?.toInt()
        assertEquals("快照候选窗口上限必须与 Rust 一致", FETCH_LIMIT_FULL, max)
    }

    @Test
    fun tiledSnapshotFetchesFullWindowWhileFoldedStaysOnePage() {
        val fake = FakeEngine().apply {
            snapshotEnabled = true
            schemeValue = InputScheme.NINE.value
            cands = cands(600)
            candReadings = arrayOf("ni")
        }
        val ctrl = EngineController(fake)

        assertEquals(fake.pageSizeValue, ctrl.candidates.size)
        ctrl.setTiledFetch(true)
        assertEquals(FETCH_LIMIT_FULL, fake.snapshotLimit)
        assertEquals(FETCH_LIMIT_FULL, ctrl.candidates.size)
        ctrl.setTiledFetch(false)
        assertEquals(fake.pageSizeValue, fake.snapshotLimit)
        assertEquals(fake.pageSizeValue, ctrl.candidates.size)
    }
}
