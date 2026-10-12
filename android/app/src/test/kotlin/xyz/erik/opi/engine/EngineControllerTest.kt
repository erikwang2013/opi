// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.engine

import xyz.erik.opi.pet.PetMood
import xyz.erik.opi.pet.petMood
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

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
        assertEquals(
            PetMood.IDLE,
            petMood(ctrl.buffer, ctrl.candidates.size, ctrl.learnerEnabled, ctrl.dictionaryDegraded),
        )

        fake.learnerOn = false
        ctrl.refresh()
        assertEquals(
            PetMood.SLEEPY,
            petMood(ctrl.buffer, ctrl.candidates.size, ctrl.learnerEnabled, ctrl.dictionaryDegraded),
        )
    }

    @Test
    fun dictionaryLoadFailureReadsThroughToPetMood() {
        // 词库降级的**唯一**真值是 EngineLoader.load(context) 的返回值（false = 回退内置
        // 35 词库）：引擎没有对应的 JNI 出口，宿主不回报的话 controller 的 degraded 恒为
        // 默认 false，petMood 的 DEGRADED 分支永远到不了（断天线就是死代码）。
        val fake = FakeEngine()
        val ctrl = EngineController(fake)
        assertFalse(ctrl.dictionaryDegraded)
        assertEquals(
            PetMood.IDLE,
            petMood(ctrl.buffer, ctrl.candidates.size, ctrl.learnerEnabled, ctrl.dictionaryDegraded),
        )

        ctrl.reportDictionaryLoad(false)
        assertTrue(ctrl.dictionaryDegraded)
        assertEquals(
            PetMood.DEGRADED,
            petMood(ctrl.buffer, ctrl.candidates.size, ctrl.learnerEnabled, ctrl.dictionaryDegraded),
        )

        // refresh 不许把它抹掉：真值只在装载那一刻拿得到，refresh 没有可重读的源；
        // 而每次按键都走一次 refresh —— 抹掉的话断天线只闪一帧，用户根本看不见。
        ctrl.refresh()
        assertTrue(ctrl.dictionaryDegraded)

        // 反向：后来的装载成功了，判定要跟着回到正常（以最近一次为准）
        ctrl.reportDictionaryLoad(true)
        assertFalse(ctrl.dictionaryDegraded)
        assertEquals(
            PetMood.IDLE,
            petMood(ctrl.buffer, ctrl.candidates.size, ctrl.learnerEnabled, ctrl.dictionaryDegraded),
        )
    }

    @Test
    fun switchSchemeWritesEngineAndRereads() {
        // 输入模式（2026-10-09）：控制器只做两件事 —— 写引擎（引擎侧清缓冲）+
        // 重读把真值同步回来（与 switchMode 同一条「单一状态源」约定）。
        val fake = FakeEngine()
        val ctrl = EngineController(fake)
        assertEquals(InputScheme.FULL, ctrl.inputScheme)
        ctrl.switchScheme(InputScheme.NINE)
        assertEquals(listOf(InputScheme.NINE.value), fake.schemeCalls)
        assertEquals(InputScheme.NINE, ctrl.inputScheme)
        ctrl.switchScheme(InputScheme.FULL)
        assertEquals(
            listOf(InputScheme.NINE.value, InputScheme.FULL.value),
            fake.schemeCalls,
        )
        assertEquals(InputScheme.FULL, ctrl.inputScheme)
    }

    @Test
    fun readingAndBufferAreSeparateExports() {
        // 两个出口各司其职（2026-10-09）：buffer = 缓冲本体（行为判定），reading = 显示
        // 读数（9 键 = 带分词读音）。9 键无候选时 reading 为空而 buffer 是数字串 ——
        // 谁也别拿对方当自己。
        val fake = FakeEngine().apply { buf = "999"; displayBuf = "ni'hao" }
        val ctrl = EngineController(fake)
        assertEquals("999", ctrl.buffer)
        assertEquals("ni'hao", ctrl.reading)
        fake.displayBuf = "" // 无候选
        fake.buf = "999"
        ctrl.refresh()
        assertEquals("", ctrl.reading)
        assertEquals("999", ctrl.buffer)
    }

    @Test
    fun composingTextNeverLeaksNineKeyDigits() {
        // preedit 取值（2026-10-11 修「9 键没匹配的时候会变成数字输入」）：读数优先；
        // 读数空时 26 键回退缓冲（旧行为不动）、9 键**不落数字** —— 待编键改用「·」
        // 标记（2026-10-12 晚：无匹时字段一字不动，用户判为「键盘死了」）。
        val fake = FakeEngine().apply { buf = "94642"; displayBuf = "" }
        val ctrl = EngineController(fake)
        assertEquals("26 键：读数空回退缓冲（旧行为）", "94642", ctrl.composingText())
        fake.schemeValue = InputScheme.NINE.value
        ctrl.refresh()
        assertEquals(
            "9 键：读数空不落数字，但每个待编键要有个记号（5 键 = 5 个 ·）",
            "·····",
            ctrl.composingText(),
        )
        assertFalse("· 标记里不许出现数字", ctrl.composingText().any { it.isDigit() })
        fake.displayBuf = "xiang'gang"
        ctrl.refresh()
        assertEquals(
            "有读数时以读数为准；读数覆盖 9 个字母键（' 不占键位）够盖住 5 键缓冲 ⇒ 无补记",
            "xiang'gang",
            ctrl.composingText(),
        )
        // 上屏文本与显示文本同源但不含「·」（见 pendingCommitText）
        assertEquals("xiang'gang", ctrl.pendingCommitText())
    }

    @Test
    fun composingTextMarksKeysNotCoveredByTheReading() {
        // 9 键点候选确认后残留的尾巴（用户报障的核心形态）：读数只覆盖「最长可匹配
        // 前缀」，没被覆盖的键以前完全不显示 ⇒ 再按新键时读数与候选一字不变。
        // 实测：连点 6 次 6 → 点候选确认 → 缓冲剩 `66`（读数 mo'mo 覆盖 4 键）⇒ `mo'mo··`。
        val fake = FakeEngine().apply {
            schemeValue = InputScheme.NINE.value
            buf = "666666"
            displayBuf = "mo'mo"
        }
        val ctrl = EngineController(fake)
        assertEquals("mo'mo··", ctrl.composingText())
        // 再按一键（缓冲 7 位）：标记跟着长一位 —— 「键被收到了」看得见
        fake.buf = "6666665"
        fake.displayBuf = "mo'mo"
        ctrl.refresh()
        assertEquals("mo'mo···", ctrl.composingText())
        // 读数覆盖满（无待编键）⇒ 不加标记
        fake.buf = "6666"
        ctrl.refresh()
        assertEquals("mo'mo", ctrl.composingText())
    }

    @Test
    fun candidateReadingsComeThroughRefresh() {
        // 读数（9 键左列的数据源，2026-10-09 用户裁决）：与 candidates 同一次刷新
        // 读出（**去重保序在引擎出口**做，本类只透传）；JNI 返回 null 时退空列表
        // （不崩、不当成一条空读数）。
        val fake = FakeEngine().apply {
            cands = arrayOf("你", "你好")
            candReadings = arrayOf("ni", "ni'hao")
        }
        val ctrl = EngineController(fake)
        assertEquals(listOf("你", "你好"), ctrl.candidates)
        assertEquals(listOf("ni", "ni'hao"), ctrl.candidateReadings)
        fake.candReadings = null
        ctrl.refresh()
        assertEquals(emptyList<String>(), ctrl.candidateReadings)
    }

    @Test
    fun pinnedReadingTogglesAndDiesWithTheBuffer() {
        // 钉读音（点左列拼音行，2026-10-09 用户裁决「不要直接确认」）：同值再点 = 解除；
        // 缓冲一变由 refresh 的看守清掉（引擎侧同样按缓冲校验，双保险）。
        val fake = FakeEngine().apply { buf = "64" }
        val ctrl = EngineController(fake)
        ctrl.togglePinnedReading("niu")
        assertEquals("niu", ctrl.pinnedReading)
        assertEquals(listOf("niu"), fake.pinCalls)
        ctrl.togglePinnedReading("niu") // 同一读音再点 = 解除
        assertEquals("", ctrl.pinnedReading)
        assertEquals(listOf("niu", ""), fake.pinCalls)
        ctrl.togglePinnedReading("ni")
        fake.buf = "6" // 缓冲一变
        ctrl.refresh()
        assertEquals("", ctrl.pinnedReading)
        assertEquals("", fake.pinCalls.last())
    }

    @Test
    fun readingReflectsTheLatestCandidatesFetchNotAStaleOne() {
        // 读数必须**最后读**（refresh 的读序契约）：真引擎的读数是 candidates() 的
        // 副产物、且与算它时的缓冲绑存（FakeEngine.readingModel 按此模拟）。先读
        // 读数再取候选 = 拿到空串或上一条缓冲的旧值 —— 2026-10-09 用户报障「确定
        // 候选词后左列不恢复标点」的 Kotlin 侧根因，这条把次序钉死。
        val fake = FakeEngine().apply { readingModel = { b -> "读($b)" } }
        val ctrl = EngineController(fake)
        ctrl.input("6")
        ctrl.input("4")
        assertEquals("组字中：读数与当前缓冲一致（不许慢一拍）", "读(64)", ctrl.reading)
        fake.selectResults[0] = "命"
        ctrl.select(0) // 提交：缓冲清空
        assertEquals("", ctrl.buffer)
        assertEquals("提交后读数必须立即归零（左列回标点）", "", ctrl.reading)
    }

    @Test
    fun bufferSyncedCallbackFiresAfterEveryRefresh() {
        // 组合串进字段（2026-10-10）的同步点：每次 refresh 收尾都要回调服务层
        //（服务层据此 setComposingText/finishComposingText）。回调次数 ≥ 动作次数即可，
        // 重点是从不调（那会让字段里的组合串原地冻结）。
        var synced = 0
        val ctrl = EngineController(FakeEngine(), onBufferSynced = { synced++ })
        ctrl.input("n")
        assertTrue("input 后应触发同步回调", synced > 0)
        val afterInput = synced
        ctrl.backspace()
        assertTrue("backspace 后也要触发（退格到空时要收口组合区）", synced > afterInput)
    }
}
