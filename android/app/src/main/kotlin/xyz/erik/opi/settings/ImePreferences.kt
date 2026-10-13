// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.settings

import android.content.Context
import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.ime.InputMode
import xyz.erik.opi.ime.KeyboardModes
import xyz.erik.opi.keyboard.SkinOption

/**
 * 键值存储抽象：生产 = SharedPreferences（[AndroidPrefStore]），JVM 测试注入内存实现
 * —— 与 [xyz.erik.opi.jni.EngineLoader.FileOps] 同一套「把 android.* 挡在纯 JVM 之外」
 * 的做法。
 */
interface PrefStore {
    fun getString(key: String): String?
    fun getBoolean(key: String, def: Boolean): Boolean
    fun put(key: String, value: String)
    fun put(key: String, value: Boolean)
}

/** 生产实现：MODE_PRIVATE 的 SharedPreferences。 */
class AndroidPrefStore(context: Context) : PrefStore {
    private val sp = context.getSharedPreferences(FILE_NAME, Context.MODE_PRIVATE)

    override fun getString(key: String): String? = sp.getString(key, null)
    override fun getBoolean(key: String, def: Boolean): Boolean = sp.getBoolean(key, def)
    override fun put(key: String, value: String) {
        sp.edit().putString(key, value).apply()
    }

    override fun put(key: String, value: Boolean) {
        sp.edit().putBoolean(key, value).apply()
    }

    companion object {
        const val FILE_NAME = "opi_ime_prefs"
    }
}

/**
 * IME 偏好（2026-10-09）。
 *
 * 存的是**三类**：
 *   · 输入模式（[InputMode]，7 选 1）；
 *   · 键盘模式（[KeyboardModes]：悬浮/游戏/单手/暗光/长辈 —— 繁体在引擎 mode 上，不存这里）；
 *   · 皮肤（[SkinOption]，2026-10-09 起；五款与桌面/配置面同名）；
 *   · 设置页的三个引擎开关（学习/模糊音/简拼）—— ⚠️ **这三个此前不落盘**（进程重启
 *     回默认，docs/install/android.md §4.5 记着的既有缺口）；本次顺手关掉：存的是
 *     开关的**当前值**，恢复时设置页与服务各读一次。
 *
 * 枚举一律按 **name** 持久化（序数会随枚举增删漂）。未知值/损坏 → 回默认，不抛
 * （与设置页读侧同一条容错约定）。**中文标点开关不在内**：设置页没有它的开关，
 * 默认开是引擎行为。
 */
class ImePreferences(private val store: PrefStore) {

    /** 默认 **9 键**（2026-10-11 用户裁决「移动端默认初始 9 键」）；已存过选择的用户读自己的值，不被改写。 */
    var inputMode: InputMode
        get() = store.getString(KEY_INPUT_MODE)
            ?.let { name -> InputMode.entries.firstOrNull { it.name == name } }
            ?: InputMode.PINYIN9
        set(v) = store.put(KEY_INPUT_MODE, v.name)

    var keyboardModes: KeyboardModes
        get() = KeyboardModes(
            floating = store.getBoolean(KEY_FLOATING, false),
            game = store.getBoolean(KEY_GAME, false),
            oneHand = store.getString(KEY_ONE_HAND)
                ?.let { name -> KeyboardModes.OneHand.entries.firstOrNull { it.name == name } }
                ?: KeyboardModes.OneHand.OFF,
            dark = store.getBoolean(KEY_DARK, false),
            elder = store.getBoolean(KEY_ELDER, false),
        )
        set(v) {
            store.put(KEY_FLOATING, v.floating)
            store.put(KEY_GAME, v.game)
            store.put(KEY_ONE_HAND, v.oneHand.name)
            store.put(KEY_DARK, v.dark)
            store.put(KEY_ELDER, v.elder)
        }

    /** 皮肤（[SkinOption] 的 id 字符串；未知值回默认，同其它偏好）。 */
    var skin: SkinOption
        get() = SkinOption.fromId(store.getString(KEY_SKIN))
        set(v) = store.put(KEY_SKIN, v.id)

    var learn: Boolean
        get() = store.getBoolean(KEY_LEARN, true)
        set(v) = store.put(KEY_LEARN, v)

    var fuzzy: Boolean
        get() = store.getBoolean(KEY_FUZZY, true)
        set(v) = store.put(KEY_FUZZY, v)

    var jianpin: Boolean
        get() = store.getBoolean(KEY_JIANPIN, true)
        set(v) = store.put(KEY_JIANPIN, v)

    /** 拼音纠错（2026-10-13；缺省开）。 */
    var correction: Boolean
        get() = store.getBoolean(KEY_CORRECTION, true)
        set(v) = store.put(KEY_CORRECTION, v)

    /**
     * 模糊音**逐对**列表（2026-10-13 用户裁决「可多选类似 z=zh」）：
     * 逗号分隔组名（[FUZZY_PAIRS_ANDROID] 的条目）。**空串 = 未设置 ⇒ 回落
     * [fuzzy] 布尔**（老装机行为一字不变）；非空 ⇒ 以列表为准。
     */
    var fuzzyPairs: String
        get() = store.getString(KEY_FUZZY_PAIRS) ?: ""
        set(v) = store.put(KEY_FUZZY_PAIRS, v)

    /** 把三个引擎开关推给引擎（服务启动与设置页保存共用一处，防两处漂）。 */
    fun applyEngineSwitches(controller: EngineController) {
        controller.setLearner(learn)
        // 逐对列表非空 ⇒ 以它为准；空 ⇒ 回落旧布尔（合同与 TSF/桌面/fcitx5 一字不差）。
        val pairs = fuzzyPairs
        if (pairs.isEmpty()) {
            controller.setFuzzy(fuzzy)
        } else {
            controller.setFuzzyPairs(fuzzyMaskOf(pairs))
        }
        controller.setJianpin(jianpin)
        controller.setCorrection(correction)
    }

    companion object {
        const val KEY_INPUT_MODE = "input_mode"
        const val KEY_FLOATING = "kb_floating"
        const val KEY_GAME = "kb_game"
        const val KEY_ONE_HAND = "kb_one_hand"
        const val KEY_DARK = "kb_dark"
        const val KEY_ELDER = "kb_elder"
        const val KEY_SKIN = "kb_skin"
        const val KEY_LEARN = "engine_learn"
        const val KEY_FUZZY = "engine_fuzzy"
        const val KEY_FUZZY_PAIRS = "engine_fuzzy_pairs"
        const val KEY_JIANPIN = "engine_jianpin"
        const val KEY_CORRECTION = "engine_correction"

        fun from(context: Context): ImePreferences = ImePreferences(AndroidPrefStore(context))
    }
}

/**
 * 模糊音组名表（顺序 = 引擎 `engine-core/src/fuzzy.rs` 的 `FUZZY_PAIRS` 数组序 =
 * 掩码位序）。**锁步门禁**：`FuzzyPairsLockstepTest` 文本读 `fuzzy.rs` 逐项比对
 * —— 那边加组这里不加会红。展示与解析都用规范名 `zh/z`（与引擎 `pair_names()` 同形）。
 */
internal val FUZZY_PAIRS_ANDROID = listOf(
    "zh/z", "ch/c", "sh/s", "an/ang", "en/eng", "in/ing", "n/l", "f/h",
)

/**
 * 组名串 → 掩码（位序 = [FUZZY_PAIRS_ANDROID] 的下标）。容错口径与引擎的
 * `fuzzy::mask_from_names` 一致：逗号切、去空白、认不得的整条忽略。
 */
internal fun fuzzyMaskOf(names: String): Int =
    names.split(',').fold(0) { acc, raw ->
        val i = FUZZY_PAIRS_ANDROID.indexOf(raw.trim())
        if (i >= 0) acc or (1 shl i) else acc
    }
