package io.opi.input.engine

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

    override fun inputKey(ch: String): String {
        inputCalls += ch
        buf += ch
        return ""
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
}
