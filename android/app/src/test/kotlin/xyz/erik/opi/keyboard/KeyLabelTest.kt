// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.keyboard

import xyz.erik.opi.candidate.composingAnnouncement
import xyz.erik.opi.engine.EngineMode
import xyz.erik.opi.ime.modeLabelOf
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
        // 选字后 buffer 清空而候选仍在：不能播成「拼音 ，3 个候选」
        assertEquals("3 个候选", composingAnnouncement("", 3))
    }

    @Test
    fun modeLabelFollowsActualMode() {
        // 符号搜索叠盘的模式键原先硬编码 "中"：实际是英文/繁体时键面撒谎
        // （读屏还会照着这个字念「中英切换」）
        assertEquals("中", modeLabelOf(EngineMode.PINYIN))
        assertEquals("繁", modeLabelOf(EngineMode.TRADITIONAL))
        assertEquals("英", modeLabelOf(EngineMode.ENGLISH))
        assertEquals("英", modeLabelOf(EngineMode.NUMBER)) // 非拼音类一律按英文显示
    }
}
