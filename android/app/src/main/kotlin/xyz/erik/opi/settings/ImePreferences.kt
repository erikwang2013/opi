// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.settings

import android.content.Context
import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.ime.InputMode
import xyz.erik.opi.ime.KeyboardModes
import xyz.erik.opi.jni.OpiEngine
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

    /**
     * 快捷条条目（2026-10-17；`QuickPhrases.SEP` 分隔）。未存过 → 内置常用语；
     * **存过空串 = 用户清空过**（不回种子 —— 与「删过不复活」同一条纪律）。
     * 损坏值原样返回，由 `QuickPhrases.sanitize` 清洗。
     */
    /** 快捷条与 ImeState 的接线（服务构造期调一次）：恢复上次条目 + 变更即落盘。 */
    fun bindQuickPhrases(state: xyz.erik.opi.ime.ImeState) {
        state.quickPhrases.replaceWith(recentPhrases)
        state.quickPhrases.onChanged = { recentPhrases = it }
    }

    var recentPhrases: List<String>
        get() = store.getString(KEY_QUICK_PHRASES)
            ?.split(xyz.erik.opi.ime.QuickPhrases.SEP)
            ?.filter { it.isNotEmpty() }
            ?: xyz.erik.opi.ime.QuickPhrases.DEFAULT
        set(v) = store.put(KEY_QUICK_PHRASES, v.joinToString(xyz.erik.opi.ime.QuickPhrases.SEP))

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
        // 缺省 false = 关（2026-10-15 用户裁决「默认关闭模糊音」）；纠错/简拼仍缺省开。
        get() = store.getBoolean(KEY_FUZZY, false)
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

    /**
     * 双拼布局（2026-10-13）：存**方案名**（本文件对枚举的既有约定 —— 序数会随
     * 表序增删漂；名字表见 [SHUANGPIN_LAYOUTS_ANDROID]）。未设置/认不得 → 表首
     * （自然码，最通行的一套）。推进引擎时用 [shuangpinLayoutIndexOf] 转下标。
     */
    var shuangpinLayout: String
        get() = store.getString(KEY_SHUANGPIN_LAYOUT)
            ?.takeIf { it in SHUANGPIN_LAYOUTS_ANDROID }
            ?: SHUANGPIN_LAYOUTS_ANDROID.first()
        set(v) = store.put(KEY_SHUANGPIN_LAYOUT, v)

    /**
     * 停用词库名单（2026-10-14，M3 词库管理）：逗号分隔的词库名，**空串 = 全部启用**
     * （与桌面 `DisabledDicts` 同一份契约；解析/写回见 `DictStore.parseDisabled` /
     * `DictStore.withDisabled`）。
     *
     * 为什么必须落盘：引擎里导入层是**内存态**，每次启动装载（`DictStore.loadAll`）
     * 都会把目录里的词库置回启用 —— 停用配置得由宿主存着、在装载之后**重放**
     * （引擎 `api/dict.rs` 裁决 2 把重放划给宿主）。不存 ⇒ 开关在重启后静默复活。
     */
    var disabledDicts: String
        get() = store.getString(KEY_DISABLED_DICTS) ?: ""
        set(v) = store.put(KEY_DISABLED_DICTS, v)

    /**
     * 五笔六键（2026-10-14，M4 第二波）：方案 / 风格 / 混输进入方式 / 候选个数 /
     * 回车 / 自动上屏。存**名字/档位**（与双拼布局同惯例）；缺省/认不得 → 表首
     * （= 引擎缺省，见 `WubiNames.kt` 的三个名字表）。推进引擎走
     * [applyWubiSettings]（名字→Int 的换算在用它的地方）。
     */
    var wubiPlan: String
        get() = store.getString(KEY_WUBI_PLAN)?.takeIf { it in WUBI_PLAN_NAMES }
            ?: WUBI_PLAN_NAMES.first()
        set(v) = store.put(KEY_WUBI_PLAN, v)

    var wubiStyle: String
        get() = store.getString(KEY_WUBI_STYLE)?.takeIf { it in WUBI_STYLE_NAMES }
            ?: WUBI_STYLE_NAMES.first()
        set(v) = store.put(KEY_WUBI_STYLE, v)

    var wubiMixedEntry: String
        get() = store.getString(KEY_WUBI_MIXED_ENTRY)?.takeIf { it in WUBI_MIXED_ENTRY_NAMES }
            ?: WUBI_MIXED_ENTRY_NAMES.first()
        set(v) = store.put(KEY_WUBI_MIXED_ENTRY, v)

    /**
     * 候选个数档位（3..9；缺省 = 引擎 [WUBI_CANDIDATE_LIMIT_DEFAULT]）。
     * **按字符串存**（`PrefStore` 只有 string/bool 两种，不为一个数字扩接口 ——
     * 与双拼布局的「存名字」同一条：多一种存储类型就多一处会漂的读写）。
     * 坏值/缺键 → 缺省（与其余键同一条容错）。
     */
    var wubiCandidateLimit: Int
        get() = store.getString(KEY_WUBI_CANDIDATE_LIMIT)?.toIntOrNull()
            ?.takeIf { it in WUBI_CANDIDATE_LIMIT_CHOICES }
            ?: WUBI_CANDIDATE_LIMIT_DEFAULT
        set(v) = store.put(KEY_WUBI_CANDIDATE_LIMIT, v.toString())

    var wubiReturn: String
        get() = store.getString(KEY_WUBI_RETURN)?.takeIf { it in WUBI_RETURN_NAMES }
            ?: WUBI_RETURN_NAMES.first()
        set(v) = store.put(KEY_WUBI_RETURN, v)

    var wubiAutoCommit: Boolean
        get() = store.getBoolean(KEY_WUBI_AUTO_COMMIT, true)
        set(v) = store.put(KEY_WUBI_AUTO_COMMIT, v)

    /**
     * 把五笔五键推给引擎（方案不在这里 —— 它决定**装哪张表**，归 `EngineLoader`）。
     * 名字→Int 的换算在 [WubiNames.kt] 的两个 index 函数（**唯一一份**；名字表与
     * 引擎枚举同序，锁步门禁 `WubiNamesLockstepTest` 现读 `engine_wubi.rs` 比对）。
     * 与 [applyEngineSwitches] 分开是刻意的：那一族是「拼音旋钮」，五笔是另一个面。
     */
    fun applyWubiSettings() {
        OpiEngine.setWubiStyle(wubiStyleIndexOf(wubiStyle))
        OpiEngine.setWubiMixedEntry(wubiMixedEntryIndexOf(wubiMixedEntry))
        OpiEngine.setWubiCandidateLimit(wubiCandidateLimit)
        OpiEngine.setWubiReturnStyle(wubiReturnIndexOf(wubiReturn))
        OpiEngine.setWubiAutoCommit(wubiAutoCommit)
    }

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
        // 双拼布局（2026-10-13）：与方案正交 —— 无论当前是不是双拼都推进去（引擎侧
        // 只在双拼方案下用它解码），这样切到双拼时布局已经是对的。
        controller.setShuangpinLayout(shuangpinLayoutIndexOf(shuangpinLayout))
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
        const val KEY_SHUANGPIN_LAYOUT = "engine_shuangpin_layout"
        // 五笔六键（2026-10-14，M4 第二波）
        const val KEY_WUBI_PLAN = "wubi_plan"
        const val KEY_WUBI_STYLE = "wubi_style"
        const val KEY_WUBI_MIXED_ENTRY = "wubi_mixed_entry"
        const val KEY_WUBI_CANDIDATE_LIMIT = "wubi_candidate_limit"
        const val KEY_WUBI_RETURN = "wubi_return"
        const val KEY_WUBI_AUTO_COMMIT = "wubi_auto_commit"
        const val KEY_DISABLED_DICTS = "dicts_disabled"
        const val KEY_QUICK_PHRASES = "quick_phrases"

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

/**
 * 双拼方案名表（顺序 = 引擎 `engine-core/src/shuangpin_tables.rs` 的 `SHUANGPIN_TABLES`
 * 表序 = FFI 的下标口径）。**锁步门禁**：`ShuangpinLayoutsLockstepTest` 文本读那张
 * 生成表逐项比对 —— 那边加方案这里不加会红。
 */
internal val SHUANGPIN_LAYOUTS_ANDROID = listOf(
    "自然码", "微软", "小鹤", "智能ABC", "紫光", "拼音加加",
)

/** 方案名 → FFI 下标（认不得 → 0 = 自然码，与引擎的钳制口径一致）。 */
internal fun shuangpinLayoutIndexOf(name: String): Int =
    SHUANGPIN_LAYOUTS_ANDROID.indexOf(name).coerceAtLeast(0)
