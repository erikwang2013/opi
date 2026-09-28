// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

import xyz.erik.opi.pet.PetMood
import xyz.erik.opi.pet.petMood
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** 假引擎：模拟 Rust 引擎状态（buffer/candidates/mode/shift），JVM 测试无需 JNI。 */
class FakeEngine : OpiEngineApi {
    var buf = ""
    var mode = EngineMode.PINYIN.value
    var cands: Array<String>? = null
    var lastLimit = -1
    val shiftCalls = mutableListOf<Boolean>()
    val inputCalls = mutableListOf<String>()
    var spaceResult = ""
    val selectResults = mutableMapOf<Int, String>()
    var backspaceCalls = 0
    var clearCalls = 0
    val switchCalls = mutableListOf<Int>()
    val removed = mutableListOf<String>()
    var throwOnRemove: Throwable? = null

    /** 学习开关（真引擎：learnerEnabled()/setLearner()）。 */
    var learner = true

    /** 键 → 引擎要上屏的文本（真引擎：标点键有文本、字母/数字键没有）。 */
    var inputResults = mutableMapOf<String, String>()

    override fun inputKey(ch: String): String {
        inputCalls += ch
        val out = inputResults[ch] ?: ""
        if (out.isEmpty()) buf += ch else buf = ""
        return out
    }

    override fun backspace() {
        backspaceCalls++
        if (buf.isNotEmpty()) buf = buf.dropLast(1)
    }

    override fun clear() {
        clearCalls++
        buf = ""
    }

    override fun select(index: Int): String {
        val r = selectResults[index] ?: ""
        if (r.isNotEmpty()) buf = ""
        return r
    }

    override fun switchMode(mode: Int) {
        switchCalls += mode
        this.mode = mode
    }

    override fun setShift(on: Boolean) {
        shiftCalls += on
    }

    override fun inputSpace(): String {
        if (spaceResult.isNotEmpty()) buf = ""
        return spaceResult
    }

    override fun candidates(limit: Int): Array<String>? {
        lastLimit = limit
        return cands
    }

    override fun buffer(): String = buf

    override fun mode(): Int = mode

    override fun learnerEnabled(): Boolean = learner

    /** 删用户词：记录文本；真引擎删完候选表里就没有它了（这里同样摘掉）。 */
    override fun removeUserWord(text: String) {
        throwOnRemove?.let { throw it }
        removed += text
        cands = cands?.filter { it != text }?.toTypedArray()
    }
}

class EngineControllerTest {
    private fun cands(n: Int) = Array(n) { "c$it" }

    @Test
    fun fetchLimitIs64() {
        val fake = FakeEngine()
        EngineController(fake)
        assertEquals(64, fake.lastLimit)
    }

    @Test
    fun pageResetsOnBufferChange() {
        val fake = FakeEngine().apply { cands = cands(20) }
        val ctrl = EngineController(fake)
        ctrl.nextPage()
        ctrl.nextPage()
        assertEquals(2, ctrl.candidatePage)
        fake.buf = "ab"
        ctrl.refresh()
        assertEquals(0, ctrl.candidatePage)
    }

    @Test
    fun inputResetsPageOnBufferChange() {
        val fake = FakeEngine().apply { cands = cands(20) }
        val ctrl = EngineController(fake)
        ctrl.nextPage()
        ctrl.input("w")
        assertEquals(0, ctrl.candidatePage)
    }

    @Test
    fun inputReturnsEngineText() {
        // 引擎的返回值是标点上屏文本，此前被丢弃；调用方（KeyRouter）据此提交
        val fake = FakeEngine().apply { inputResults[","] = "，" }
        val ctrl = EngineController(fake)
        assertEquals("，", ctrl.input(","))
        // 字母键无提交文本（JUnit4 的消息参数在最前，别写成 Kotlin 版顺序）
        assertEquals("", ctrl.input("a"))
    }

    @Test
    fun pageClampsWhenCandidatesShrink() {
        val fake = FakeEngine().apply { cands = cands(20) }
        val ctrl = EngineController(fake)
        ctrl.nextPage()
        ctrl.nextPage()
        fake.cands = cands(8)
        ctrl.refresh()
        assertEquals(0, ctrl.candidatePage)
        fake.cands = null
        ctrl.refresh()
        assertEquals(0, ctrl.candidatePage)
        assertTrue(ctrl.pageCandidates.isEmpty())
    }

    @Test
    fun pageCandidatesSlicesPerPage() {
        val fake = FakeEngine().apply { cands = cands(20) }
        val ctrl = EngineController(fake)
        assertEquals(3, ctrl.candidatePageCount)
        assertEquals((0 until 8).map { "c$it" }, ctrl.pageCandidates)
        ctrl.nextPage()
        assertEquals((8 until 16).map { "c$it" }, ctrl.pageCandidates)
        ctrl.nextPage()
        assertEquals((16 until 20).map { "c$it" }, ctrl.pageCandidates)
        ctrl.nextPage() // 越界不动
        assertEquals(2, ctrl.candidatePage)
        ctrl.prevPage()
        assertEquals(1, ctrl.candidatePage)
        ctrl.prevPage()
        ctrl.prevPage()
        assertEquals(0, ctrl.candidatePage)
    }

    @Test
    fun inputRoutesToEngineAndUpdatesBuffer() {
        val fake = FakeEngine()
        val ctrl = EngineController(fake)
        ctrl.input("w")
        assertEquals(listOf("w"), fake.inputCalls)
        assertEquals("w", ctrl.buffer)
    }

    @Test
    fun shiftTapCyclesOffAndSingle() {
        val fake = FakeEngine()
        val ctrl = EngineController(fake)
        assertEquals(ShiftState.OFF, ctrl.shiftState)
        ctrl.shiftTap()
        assertEquals(ShiftState.SINGLE, ctrl.shiftState)
        assertEquals(true, fake.shiftCalls.last())
        ctrl.shiftTap()
        assertEquals(ShiftState.OFF, ctrl.shiftState)
        assertEquals(false, fake.shiftCalls.last())
    }

    @Test
    fun shiftLongPressLocksAndTapTurnsOff() {
        val fake = FakeEngine()
        val ctrl = EngineController(fake)
        ctrl.shiftLongPress()
        assertEquals(ShiftState.LOCK, ctrl.shiftState)
        assertEquals(true, fake.shiftCalls.last())
        ctrl.shiftTap()
        assertEquals(ShiftState.OFF, ctrl.shiftState)
        assertEquals(false, fake.shiftCalls.last())
    }

    @Test
    fun consumeSingleShiftResetsOnlySingle() {
        val fake = FakeEngine()
        val ctrl = EngineController(fake)
        ctrl.shiftTap()
        ctrl.consumeSingleShift()
        assertEquals(ShiftState.OFF, ctrl.shiftState)
        assertEquals(false, fake.shiftCalls.last())
        // lock 不受 consume 影响
        ctrl.shiftLongPress()
        val n = fake.shiftCalls.size
        ctrl.consumeSingleShift()
        assertEquals(ShiftState.LOCK, ctrl.shiftState)
        assertEquals(n, fake.shiftCalls.size)
    }

    @Test
    fun switchModeClearsLockedShiftSoItCannotLeakAcrossModes() {
        // 中→繁→英 后长按 ⇧ 锁定，点「中」/「繁」离开：状态必须复位，否则再切回 English
        // 时 ⇧ 仍高亮锁定、字母全大写（shiftVisible 只是隐藏按钮，清不掉状态）。
        val fake = FakeEngine().apply { mode = EngineMode.ENGLISH.value }
        val ctrl = EngineController(fake)
        ctrl.shiftLongPress()
        assertEquals(ShiftState.LOCK, ctrl.shiftState)

        ctrl.switchMode(EngineMode.PINYIN)

        assertEquals(ShiftState.OFF, ctrl.shiftState)
        assertEquals(listOf(true, false), fake.shiftCalls) // 引擎侧同步关 ⇧
    }

    @Test
    fun switchModeClearsSingleShiftToo() {
        val fake = FakeEngine().apply { mode = EngineMode.ENGLISH.value }
        val ctrl = EngineController(fake)
        ctrl.shiftTap()
        assertEquals(ShiftState.SINGLE, ctrl.shiftState)

        ctrl.switchMode(EngineMode.TRADITIONAL)

        assertEquals(ShiftState.OFF, ctrl.shiftState)
        assertEquals(listOf(true, false), fake.shiftCalls)
    }

    @Test
    fun fromIntMapsTraditional() {
        assertEquals(EngineMode.TRADITIONAL, EngineMode.fromInt(4))
        assertEquals(EngineMode.PINYIN, EngineMode.fromInt(99))
    }

    @Test
    fun selectFromPageUsesAbsoluteIndex() {
        val fake = FakeEngine().apply {
            cands = cands(20)
            selectResults[8] = "中"
        }
        val ctrl = EngineController(fake)
        ctrl.nextPage()
        assertEquals("中", ctrl.selectFromPage(0))
        assertEquals("", ctrl.buffer) // 选中即提交，buffer 清空
    }

    // ---- 长按删用户词 ----

    @Test
    fun removeUserWordDeletesCandidateAndRefreshes() {
        val fake = FakeEngine().apply { cands = cands(3) }
        val ctrl = EngineController(fake)

        ctrl.removeUserWord(1)

        assertEquals(listOf("c1"), fake.removed)
        assertEquals(listOf("c0", "c2"), ctrl.candidates) // 候选栏刷新（词已不在）
    }

    @Test
    fun removeUserWordUsesPageIndexNotAbsoluteIndex() {
        // 下标语义与 selectFromPage 一致：第 2 页第 1 项是 c8，不是 c0
        val fake = FakeEngine().apply { cands = cands(20) }
        val ctrl = EngineController(fake)
        ctrl.nextPage()

        ctrl.removeUserWord(0)

        assertEquals(listOf("c8"), fake.removed)
    }

    @Test
    fun removeUserWordOutOfRangeIsNoOp() {
        val fake = FakeEngine().apply { cands = cands(2) }
        val ctrl = EngineController(fake)

        ctrl.removeUserWord(7) // 页内越界
        ctrl.removeUserWord(-1)
        assertEquals(emptyList<String>(), fake.removed)

        fake.cands = null // 无候选时也不许崩（下标取自控制器自己的候选表，故先 refresh 同步）
        ctrl.refresh()
        ctrl.removeUserWord(0)
        assertEquals(emptyList<String>(), fake.removed)

        fake.cands = cands(2)
        ctrl.refresh()
        ctrl.removeUserWord(0)
        assertEquals(listOf("c0"), fake.removed)
    }

    @Test
    fun removeUserWordSwallowsDeadLibraryError() {
        // 新增的 JNI 出口若未被 so 注册（RegisterNatives 缺失）会在调用点抛
        // UnsatisfiedLinkError —— 误触长按不该崩 IME，也不该谎报删除（候选保持原样）
        val fake = FakeEngine().apply {
            cands = cands(2)
            throwOnRemove = UnsatisfiedLinkError("so missing")
        }
        val ctrl = EngineController(fake)

        ctrl.removeUserWord(0) // 不抛

        assertEquals(listOf("c0", "c1"), ctrl.candidates)
    }

    // ---- 学习状态变更通知（落盘时机） ----

    @Test
    fun selectNotifiesLearnedOnlyWhenTextCommitted() {
        val fake = FakeEngine().apply {
            cands = cands(2)
            selectResults[0] = "中"
        }
        var learned = 0
        val ctrl = EngineController(fake) { learned++ }

        ctrl.select(0)
        assertEquals(1, learned)

        ctrl.select(9) // 越界 → 空串：引擎没改学习状态，别排一次无意义的落盘
        assertEquals(1, learned)
    }

    @Test
    fun inputSpaceNotifiesLearnedOnlyWhenTextCommitted() {
        // 拼音模式空格 = 选中首候选，同样改学习状态
        val fake = FakeEngine().apply { spaceResult = "你" }
        var learned = 0
        val ctrl = EngineController(fake) { learned++ }

        ctrl.inputSpace()
        assertEquals(1, learned)

        fake.spaceResult = "" // 英文模式空格只是提交 buffer，不学习
        ctrl.inputSpace()
        assertEquals(1, learned)
    }

    @Test
    fun typingDoesNotNotifyLearned() {
        // 敲键/退格/清空都不改学习状态：每次按键排一次落盘是纯浪费
        val fake = FakeEngine()
        var learned = 0
        val ctrl = EngineController(fake) { learned++ }

        ctrl.input("w")
        ctrl.backspace()
        ctrl.clear()

        assertEquals(0, learned)
    }

    @Test
    fun removeUserWordNotifiesLearned() {
        // 删词也改学习状态：不落盘的话下次启动它又被 import 回来
        val fake = FakeEngine().apply { cands = cands(2) }
        var learned = 0
        val ctrl = EngineController(fake) { learned++ }

        ctrl.removeUserWord(0)

        assertEquals(1, learned)
    }

    @Test
    fun learnerFlagReadsThroughToPetMood() {
        // 学习开关是宠物的表情输入：候选栏待命位的小欧靠它区分 IDLE / SLEEPY。
        // 设置页关掉开关后（同一个 Rust 单例），IME 下一次 refresh 必须跟着变 ——
        // 停在 IDLE 就是「明明不记事却笑嘻嘻」，正是 OpiPet 头注说要避免的撒谎。
        val fake = FakeEngine()
        val ctrl = EngineController(fake)
        assertTrue(ctrl.learnerEnabled)
        assertEquals(PetMood.IDLE, petMood(ctrl.buffer, ctrl.candidates.size, ctrl.learnerEnabled))

        fake.learner = false
        ctrl.refresh()
        assertEquals(PetMood.SLEEPY, petMood(ctrl.buffer, ctrl.candidates.size, ctrl.learnerEnabled))
    }
}
