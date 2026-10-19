// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.settings

import org.junit.Assert.assertEquals
import org.junit.Test
import xyz.erik.opi.ime.InputMode
import xyz.erik.opi.ime.KeyboardModes
import xyz.erik.opi.keyboard.SkinOption

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
        // 默认 9 键（2026-10-11 用户裁决「移动端默认初始 9 键」）
        assertEquals(InputMode.PINYIN9, p.inputMode)
        assertEquals(KeyboardModes(), p.keyboardModes)
        assertEquals(true, p.learn)
        // 模糊音缺省**关**（2026-10-15 用户裁决「默认关闭模糊音、开启拼音纠错、简拼」；
        // 引擎侧 `Engine` 初值同步为 0）。
        assertEquals(false, p.fuzzy)
        assertEquals(true, p.jianpin)
        // 自有表情 PUA 缺省关（默认仍插图片；2026-10-19）。
        assertEquals(false, p.puaEmoji)
        assertEquals(SkinOption.FOLLOW, p.skin)
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
        p.puaEmoji = true
        p.skin = SkinOption.PAPER
        val q = ImePreferences(store)
        assertEquals(InputMode.PINYIN14, q.inputMode)
        assertEquals(
            KeyboardModes(true, false, KeyboardModes.OneHand.LEFT, true, false),
            q.keyboardModes,
        )
        assertEquals(false, q.learn)
        assertEquals(false, q.fuzzy)
        assertEquals(true, q.jianpin)
        assertEquals(true, q.puaEmoji)
        assertEquals(SkinOption.PAPER, q.skin)
    }

    @Test
    fun unknownEnumNamesFallBackToDefault() {
        val store = FakeStore()
        store.put(ImePreferences.KEY_INPUT_MODE, "SOMETHING_ELSE")
        store.put(ImePreferences.KEY_ONE_HAND, "MIDDLE")
        store.put(ImePreferences.KEY_SKIN, "RAINBOW")
        store.put(ImePreferences.KEY_SHUANGPIN_LAYOUT, "打错了") // 双拼布局也走同一条容错
        val p = ImePreferences(store)
        assertEquals(InputMode.PINYIN9, p.inputMode)
        assertEquals(KeyboardModes.OneHand.OFF, p.keyboardModes.oneHand)
        assertEquals(SkinOption.FOLLOW, p.skin)
        assertEquals(SHUANGPIN_LAYOUTS_ANDROID.first(), p.shuangpinLayout)
    }

    /** 双拼布局（2026-10-13）：存名字、读回同名；缺省 = 表首（自然码）。 */
    @Test
    fun shuangpinLayoutRoundTripsByName() {
        val store = FakeStore()
        val p = ImePreferences(store)
        assertEquals("缺省 = 自然码（表首）", "自然码", p.shuangpinLayout)
        p.shuangpinLayout = "小鹤"
        assertEquals("小鹤", ImePreferences(store).shuangpinLayout)
        assertEquals(2, shuangpinLayoutIndexOf("小鹤"))
    }

    /** 五笔六键（2026-10-14，M4 第二波）：往返 + 坏值回缺省（与其余键同一条容错）。 */
    @Test
    fun wubiKeysRoundTripAndFallBack() {
        val store = FakeStore()
        val p = ImePreferences(store)
        // 缺省 = 表首/引擎缺省。
        assertEquals("86", p.wubiPlan)
        assertEquals("纯五笔", p.wubiStyle)
        assertEquals("直接字母", p.wubiMixedEntry)
        assertEquals(WUBI_CANDIDATE_LIMIT_DEFAULT, p.wubiCandidateLimit)
        assertEquals("清空", p.wubiReturn)
        assertEquals(true, p.wubiAutoCommit)
        p.wubiPlan = "98"
        p.wubiStyle = "五笔拼音"
        p.wubiMixedEntry = "z键"
        p.wubiCandidateLimit = 7
        p.wubiReturn = "上屏原文"
        p.wubiAutoCommit = false
        val q = ImePreferences(store)
        assertEquals("98", q.wubiPlan)
        assertEquals("五笔拼音", q.wubiStyle)
        assertEquals("z键", q.wubiMixedEntry)
        assertEquals(7, q.wubiCandidateLimit)
        assertEquals("上屏原文", q.wubiReturn)
        assertEquals(false, q.wubiAutoCommit)
        // 坏值/认不得 ⇒ 回缺省（不抛）。
        store.put(ImePreferences.KEY_WUBI_PLAN, "06")
        store.put(ImePreferences.KEY_WUBI_STYLE, "打错了")
        store.put(ImePreferences.KEY_WUBI_CANDIDATE_LIMIT, "99")
        val r = ImePreferences(store)
        assertEquals("86", r.wubiPlan)
        assertEquals("纯五笔", r.wubiStyle)
        assertEquals(WUBI_CANDIDATE_LIMIT_DEFAULT, r.wubiCandidateLimit)
    }
}
