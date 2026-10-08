// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.settings

import org.junit.Assert.assertEquals
import org.junit.Test
import xyz.erik.opi.ime.InputMode
import xyz.erik.opi.ime.KeyboardModes

/** 偏好读写：默认值、往返、未知枚举名回默认（损坏容错与设置页读侧同一条约定）。 */
class ImePreferencesTest {

    private class FakeStore : PrefStore {
        val strings = mutableMapOf<String, String>()
        val bools = mutableMapOf<String, Boolean>()
        override fun getString(key: String): String? = strings[key]
        override fun getBoolean(key: String, def: Boolean): Boolean = bools[key] ?: def
        override fun put(key: String, value: String) {
            strings[key] = value
        }

        override fun put(key: String, value: Boolean) {
            bools[key] = value
        }
    }

    @Test
    fun defaultsWhenEmpty() {
        val p = ImePreferences(FakeStore())
        assertEquals(InputMode.PINYIN26, p.inputMode)
        assertEquals(KeyboardModes(), p.keyboardModes)
        assertEquals(true, p.learn)
        assertEquals(true, p.fuzzy)
        assertEquals(true, p.jianpin)
    }

    @Test
    fun roundTripsAllFields() {
        val store = FakeStore()
        val p = ImePreferences(store)
        p.inputMode = InputMode.PINYIN14
        p.keyboardModes = KeyboardModes(
            floating = true, game = false,
            oneHand = KeyboardModes.OneHand.LEFT, dark = true, elder = false,
        )
        p.learn = false
        p.fuzzy = false
        p.jianpin = true
        val q = ImePreferences(store)
        assertEquals(InputMode.PINYIN14, q.inputMode)
        assertEquals(
            KeyboardModes(true, false, KeyboardModes.OneHand.LEFT, true, false),
            q.keyboardModes,
        )
        assertEquals(false, q.learn)
        assertEquals(false, q.fuzzy)
        assertEquals(true, q.jianpin)
    }

    @Test
    fun unknownEnumNamesFallBackToDefault() {
        val store = FakeStore()
        store.put(ImePreferences.KEY_INPUT_MODE, "SOMETHING_ELSE")
        store.put(ImePreferences.KEY_ONE_HAND, "MIDDLE")
        val p = ImePreferences(store)
        assertEquals(InputMode.PINYIN26, p.inputMode)
        assertEquals(KeyboardModes.OneHand.OFF, p.keyboardModes.oneHand)
    }
}
