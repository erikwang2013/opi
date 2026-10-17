// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 共享「自有」映射的门禁（shared/pet/PetEmojiMap.kt，2026-10-18）。
//!
//! 桌面表情页与 Android `keyboard/PetEmoji.kt` 引用同一份表 ⇒ 这里钉的是**两端的**
//! 契约：十二形态一个不少、emoji 逐条是用户点名的那枚、顺序 = PetMood 枚举序。
//! SLEEPY / DEGRADED 不进「自有」（它们是输入状态，不是可挑的表情形态）。

package xyz.erik.opi.pet

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class PetEmojiMapTest {

    @Test
    fun `exact emoji for every form`() {
        assertEquals("😊", PetEmojiMap.emojiFor(PetMood.IDLE))
        assertEquals("😄", PetEmojiMap.emojiFor(PetMood.WAITING))
        assertEquals("😞", PetEmojiMap.emojiFor(PetMood.PUZZLED))
        assertEquals("😭", PetEmojiMap.emojiFor(PetMood.CRYING))
        assertEquals("😱", PetEmojiMap.emojiFor(PetMood.TERRIFIED))
        assertEquals("😠", PetEmojiMap.emojiFor(PetMood.ANGRY))
        assertEquals("🥺", PetEmojiMap.emojiFor(PetMood.COQUETTISH))
        assertEquals("😜", PetEmojiMap.emojiFor(PetMood.CUTE))
        assertEquals("🙏", PetEmojiMap.emojiFor(PetMood.PLEASING))
        assertEquals("🔨", PetEmojiMap.emojiFor(PetMood.KNOCKING))
        assertEquals("🤦", PetEmojiMap.emojiFor(PetMood.FACE_PALM))
        assertEquals("😛", PetEmojiMap.emojiFor(PetMood.TONGUE_OUT))
    }

    @Test
    fun `forms follow enum order and cover every non-state mood`() {
        val expected = PetMood.entries.filter {
            it != PetMood.SLEEPY && it != PetMood.DEGRADED
        }
        assertEquals(expected, PetEmojiMap.forms.map { it.mood })
        assertEquals("自有 = 十二形态", 12, PetEmojiMap.forms.size)
        for (f in PetEmojiMap.forms) {
            assertTrue("形态 ${f.mood} 缺无障碍标签", f.label.isNotEmpty())
            assertTrue("形态 ${f.mood} 缺 emoji", f.emoji.isNotEmpty())
        }
    }

    @Test
    fun `name based lookup mirrors the enum table`() {
        assertEquals(PetEmojiMap.forms.size, PetEmojiMap.formsByName.size)
        for (f in PetEmojiMap.forms) {
            assertEquals(f, PetEmojiMap.formsByName[f.mood.name])
            assertEquals(f.emoji, PetEmojiMap.emojiFor(f.mood.name))
        }
        assertNull(PetEmojiMap.emojiFor("SLEEPY"))
        assertNull(PetEmojiMap.emojiFor("NO_SUCH_MOOD"))
    }

    @Test
    fun `sleepy and degraded are input states not forms`() {
        assertNull(PetEmojiMap.emojiFor(PetMood.SLEEPY))
        assertNull(PetEmojiMap.emojiFor(PetMood.DEGRADED))
        val moods = PetEmojiMap.forms.map { it.mood }
        assertTrue(PetMood.SLEEPY !in moods)
        assertTrue(PetMood.DEGRADED !in moods)
    }
}
