// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import xyz.erik.opi.candidate.barBufferText
import xyz.erik.opi.candidate.composingAnnouncement
import xyz.erik.opi.engine.EngineMode
import xyz.erik.opi.engine.InputScheme
import xyz.erik.opi.ime.modeKeyLabelOf
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * 读屏播报文案（纯函数，JVM 可测）。
 * Compose 语义本身（liveRegion / stateDescription 是否真被 TalkBack 念出来）要真机 + 读屏软件，
 * 本机无设备测不了 —— 手工验收步骤见交付说明。
 */
class KeyLabelTest {

    @Test
    fun glyphKeysGetReadableNames() {
        assertEquals("大写", spokenKeyName("⇧"))
        assertEquals("退格", spokenKeyName("⌫"))
        assertEquals("回车", spokenKeyName("↵"))
        assertEquals("空格", spokenKeyName("空格"))
        assertEquals("中英切换", spokenKeyName("中"))
        assertEquals("中英切换", spokenKeyName("繁"))
        assertEquals("中英切换", spokenKeyName("英"))
        assertEquals("数字与符号", spokenKeyName("123"))
        assertEquals("数字与符号", spokenKeyName("?123"))
        assertEquals("字母", spokenKeyName("ABC"))
    }

    @Test
    fun letterKeysGetSuffix() {
        assertEquals("a 键", spokenKeyName("a"))
        assertEquals("m 键", spokenKeyName("m"))
    }

    @Test
    fun digitsAndPunctuationReadAsIs() {
        assertEquals("1", spokenKeyName("1"))
        assertEquals("逗号", spokenKeyName(","))
        assertEquals("句号", spokenKeyName("."))
    }

    @Test
    fun composingAnnouncementCountsCandidates() {
        assertEquals("拼音 nihao，5 个候选", composingAnnouncement("nihao", 5))
        assertEquals("拼音 nihao，无候选", composingAnnouncement("nihao", 0))
        // 2026-10-14：五笔态那格放的是码串 —— 念成「编码」，别把码串说成拼音。
        assertEquals("编码 wqvb，3 个候选", composingAnnouncement("wqvb", 3, label = "编码"))
    }

    /**
     * 9 键的缓冲格**恒空**（2026-10-09 用户二次裁决：「输入无匹配的时候，候选出现
     * logo 图标，logo 图标前就不要显示数字了」）—— 别把「无候选时显示缓冲原文」
     * 加回来（曾被当作反馈修复，用户明确不要）。
     */
    @Test
    fun nineKeyBufferCellStaysEmpty() {
        // 9 键：有候选 / 无候选 / 空缓冲 —— 三种都不显示数字串
        assertEquals("", barBufferText(InputScheme.NINE, "64"))
        assertEquals("", barBufferText(InputScheme.NINE, "99999"))
        assertEquals("", barBufferText(InputScheme.NINE, ""))
        // 其余方案（26/14/笔画）显示实际缓冲（字母串本来就是拼音）
        assertEquals("ni", barBufferText(InputScheme.FULL, "ni"))
        assertEquals("hh", barBufferText(InputScheme.STROKE, "hh"))
        // 选字后 buffer 清空而候选仍在：不能播成「拼音 ，3 个候选」
        assertEquals("3 个候选", composingAnnouncement("", 3))
    }

    @Test
    fun modeKeyLabelShowsTheTargetNotTheCurrentMode() {
        // 2026-10-12 晚用户裁决：「点击英文按钮切换到英文键盘，这时为何在英文键盘还显示
        // 英的按钮，点击了是中文键盘，按钮是中才对」——键面写字面目标（与 9 键底行
        // 「英文」键同一条语义）。逐条对齐 cycleMode() 的次序：中→繁、繁→英、英→回中文。
        assertEquals("繁", modeKeyLabelOf(EngineMode.PINYIN, lastChineseIsTrad = false))
        assertEquals("英", modeKeyLabelOf(EngineMode.TRADITIONAL, lastChineseIsTrad = false))
        assertEquals("中", modeKeyLabelOf(EngineMode.ENGLISH, lastChineseIsTrad = false))
        // 数字面板也走 cycleMode 的 else 档（回上次的中文方案）⇒ 键面是「中」不是「英」。
        // ⚠️ 我第一版把这条写成了「英」——是照抄旧语义（当前态一律非拼音按英文）写的，
        // 实现是对的、期望错了，跑出来红在这条（expected 英 but was 中）。
        assertEquals("中", modeKeyLabelOf(EngineMode.NUMBER, lastChineseIsTrad = false))
        // 「英→回上次的中文方案」：上次是繁体就回繁体（键面同说实话）
        assertEquals("繁", modeKeyLabelOf(EngineMode.ENGLISH, lastChineseIsTrad = true))
        assertEquals("繁", modeKeyLabelOf(EngineMode.NUMBER, lastChineseIsTrad = true))
    }
}
