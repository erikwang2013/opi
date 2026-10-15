// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import xyz.erik.opi.engine.pendingCommitText
import android.os.Handler
import android.os.Looper
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.keyboard.SkinOption
import xyz.erik.opi.engine.EngineMode

/** 防抖调度抽象：生产用主线程 Handler，JVM 测试注入假时钟（确定性，无 Thread.sleep）。 */
fun interface Debouncer {
    /** delayMs 后执行 action；返回取消函数。 */
    fun schedule(delayMs: Long, action: () -> Unit): () -> Unit
}

/** 生产实现：主线程 Handler.postDelayed（复用单个 Handler，不在每次调度时新建）。 */
class HandlerDebouncer : Debouncer {
    private val handler = Handler(Looper.getMainLooper())

    override fun schedule(delayMs: Long, action: () -> Unit): () -> Unit {
        val r = Runnable { action() }
        handler.postDelayed(r, delayMs)
        return { handler.removeCallbacks(r) }
    }
}

/**
 * 输入连接暂不可用时的延迟重试（OpiImeService.commitWithRetry 的排程部分）。
 *
 * 投递与撤销必须落在**同一处**，所以调度器由字段持有：原先两边都现取
 * `window.window.decorView`，而 IME 窗口在旋转/配置变化时会被整棵重建
 * （见 OpiImeService.setInputView 的长注释）—— 撤销时取到的 decorView 已经不是
 * 投递时那个，`removeCallbacks` 撤不掉挂在旧 root 上的 action，50ms 后它照样执行，
 * 把上一次的候选写进了**新的**编辑框。字段持有的 Handler 不随窗口重建而改变。
 */
class PendingCommit(
    private val debouncer: Debouncer = HandlerDebouncer(),
) {
    private var cancel: (() -> Unit)? = null

    /** 排一次重试；已有排队中的先撤（两次重试叠加 = 同一个词被提交两遍）。 */
    fun schedule(retry: () -> Unit) {
        cancel()
        cancel = debouncer.schedule(RETRY_MS) {
            cancel = null
            retry()
        }
    }

    fun cancel() {
        cancel?.invoke()
        cancel = null
    }

    companion object {
        /** 重试延迟：给窗口拿到 IC 的时间，短到用户察觉不到。 */
        const val RETRY_MS = 50L
    }
}

/**
 * 面板状态机（对齐 flutter ime_main.dart _ImeScreenState）：
 * - pending buffer 提交：开面板前有候选选第一个提交，无候选清掉（防面板往返残留噪音）
 * - 符号搜索 250ms 防抖：searchText 实时、searchQuery 防抖后生效（驱动结果）
 * - 搜索焦点联动 qwerty 叠盘（IME 窗口内 TextField 无系统键盘）
 * - editorChanged 全量重置：清搜索、回 qwerty（组合串由服务层 clear）
 */
class ImeState(
    val controller: EngineController,
    /** IME 提交通道（OpiImeService 在 onCreateInputView 注入 commitWithRetry）。 */
    var commit: (String) -> Unit = {},
    private val debouncer: Debouncer = HandlerDebouncer(),
) {
    enum class View { QWERTY, NUMBER, SYMBOL, RARE, SKIN, INPUT_MODE, KEYBOARD_MODE, SYMBOL_PAGE }

    companion object {
        /** 搜索防抖窗口（对齐 flutter 250ms）。 */
        const val SEARCH_DEBOUNCE_MS = 250L
    }

    var view by mutableStateOf(View.QWERTY)
        private set

    /** 搜索框焦点：true 时下方叠 qwerty 搜索盘。 */
    var searchActive by mutableStateOf(false)
        private set

    /** 搜索框实时文本（绑定 TextField）。 */
    var searchText by mutableStateOf("")
        private set

    /** 防抖后生效的查询（trim；面板据此出结果）。 */
    var searchQuery by mutableStateOf("")
        private set

    private var pendingDebounce: (() -> Unit)? = null

    /**
     * 进数字面板前的字母模式（拼音/繁体/英文）。面板期间引擎切到 NUMBER，离开时恢复：
     * 面板的键现在是**引擎键码**（数字进缓冲、`,` `.` 由标点表出文本），引擎不在
     * Number 模式它们就没有归宿（拼音模式收不了数字 → 整块面板变死键）。
     */
    private var letterMode = EngineMode.PINYIN

    // ---- 输入模式 / 键盘模式（2026-10-09）----

    /** 当前输入模式（工具条与键盘布局按它画）。初始值 = 偏好默认的 9 键，restore 后以偏好为准。 */
    var inputMode by mutableStateOf(InputMode.PINYIN9)
        private set

    /** 键盘模式（悬浮/游戏/单手/暗光/长辈）。繁体不在这里 —— 它在引擎 mode 上。 */
    var keyboardModes by mutableStateOf(KeyboardModes())
        private set

    /**
     * 皮肤（工具条「皮肤」面板的当前选择）。与键盘模式的优先级见 [SkinOption]：
     * 长辈 > 暗光 > 皮肤 —— 皮肤是「底」，模式是临时覆盖。
     */
    var skin by mutableStateOf(xyz.erik.opi.keyboard.SkinOption.FOLLOW)
        private set

    /** 符号面板的起始 Tab（工具条表情入口直达 Emoji）。 */
    var symbolInitialTab by mutableStateOf(xyz.erik.opi.keyboard.SymbolTab.Common)
        private set

    /**
     * 偏好落盘钩子（OpiImeService 注入；默认空 = JVM 测试不感知）。
     * 与 [commit] 同一套「服务层注入副作用」的做法。
     */
    var persist: () -> Unit = {}

    /** 上次的中文侧输入模式（9 键 ↔ 英文 往返时恢复用）。初始同偏好默认（9 键）。 */
    private var lastChineseMode = InputMode.PINYIN9

    /** 「密码字段临时英文」进入前的中文模式；null = 不在该态。**不落盘**（见下）。 */
    private var transientEnglishFrom: InputMode? = null

    /**
     * 密码类字段自动切英文（2026-10-10 用户报障「弹框里没法输入」的第二来源）：
     * Wi-Fi 密码/登录框要的是英文数字**直出**，拼音候选在这儿帮倒忙——搜狗同场景
     * 自动切英文，我们照做。**临时且不落盘**（与 [selectInputMode] 的区别：那条是
     * 用户的明确选择、要 persist 这条是场景适配、离开字段即还原）。
     * 已经在英文（用户自己切的）⇒ 空操作，也不记状态（leave 才不会乱还原）。
     */
    fun enterTransientEnglish() {
        if (controller.mode == EngineMode.ENGLISH) return
        transientEnglishFrom = inputMode
        inputMode = InputMode.ENGLISH26
        controller.switchLanguage(EngineMode.ENGLISH, InputMode.ENGLISH26.scheme)
        view = View.QWERTY
    }

    /** 离开密码字段：还原进入前的中文模式（没进过 = 空操作）。 */
    fun leaveTransientEnglish() {
        val from = transientEnglishFrom ?: return
        transientEnglishFrom = null
        inputMode = from
        controller.switchLanguage(from.engineMode, from.scheme)
        view = View.QWERTY
    }

    /**
     * 选一个输入模式（工具条菜单）。生僻字（[InputMode.panel]）**只切面板**，
     * 不动引擎的 scheme/mode —— 它是浏览式查字，不是输入方案。
     */
    fun selectInputMode(m: InputMode) {
        // 切到英文 ⇒ pending **按拼音原样上屏**（2026-10-10 用户裁决「拼音输入的时候切换
        // 英文，就默认是拼音，不要是候选词」；与 [英文] 底行键、中繁→英循环腿同语义——
        // 三处共用 [commitPendingAsPinyin]）。其余模式照旧「有候选选首候选」：那是输入模式
        // 面板「明确换布局」的既有语义，用户裁决只点名了英文。
        if (m.engineMode == EngineMode.ENGLISH) {
            commitPendingAsPinyin()
        } else {
            commitPendingBuffer()
        }
        inputMode = m
        if (m.panel) {
            view = View.RARE
        } else {
            if (m.engineMode == EngineMode.PINYIN || m.engineMode == EngineMode.TRADITIONAL) {
                lastChineseMode = m
            }
            controller.switchScheme(m.scheme)
            if (controller.mode != m.engineMode) controller.switchMode(m.engineMode)
            view = View.QWERTY
        }
        persist()
    }

    /**
     * 「中/繁/英」一键循环（既有键位，2026-10-09 起与方案/布局接线对齐）：
     * 中→繁只切**引擎模式**（方案不动，9 键仍是 9 键）；英那条会换到全键盘方案
     * （9 键上打英文没有候选路）；英→中回**上次的中文方案**。
     *
     * **二期（同日晚）：三条腿全部保留缓冲** —— 用户裁决「中文输入的时候切换英文，
     * 拼音保留可继续使用」。走 [EngineController.switchLanguage]（引擎侧换模式/方案
     * 但不清缓冲），不再 commit pending；英文模式下字母照常直传（见 KeyRouter），
     * 保留的拼音切回中文后继续打/接着选。⚠️ 「输入模式」面板那条（[selectInputMode]）
     * 仍先 commit 再换 —— 那是明确换布局的动作，判据不同，别顺手统一。
     */
    /**
     * 26 键底行模式键的**键面字 = 点它会切到哪**（用户裁决 2026-10-12 晚：
     * 「点击英文按钮切换到英文键盘，这时为何在英文键盘还显示英的按钮，点击了是中文键盘，
     * 按钮是中才对」）。与 9 键底行「英文」键**同一条语义** —— 键面写字面目标（那是动作），
     * 不写当前态；当前态由候选栏/键盘布局本身表达（英文态是 26 键盘）。
     *
     * 目标 = [cycleMode] 的下一档：中→繁、繁→英、英→（回上次的中文方案）。
     * ⚠️ 改 [cycleMode] 的次序时**必须同改这里** —— 键面说谎比没有键面更坏。
     */
    val modeKeyLabel: String
        get() = modeKeyLabelOf(
            controller.mode,
            lastChineseMode.engineMode == EngineMode.TRADITIONAL,
        )

    fun cycleMode() {
        when (controller.mode) {
            EngineMode.PINYIN ->
                controller.switchLanguage(EngineMode.TRADITIONAL, controller.inputScheme)
            // 切英文前把 pending 按**拼音**上屏（2026-10-10 用户裁决「拼音输入的时候
            // 切换英文，就默认是拼音，不要是候选词」——打了一半英文切键盘，字母原样留下；
            // 此前是「保留缓冲」，但保留下来的拼音字段里看不见，切回才复活，用户判为异常）。
            EngineMode.TRADITIONAL -> {
                commitPendingAsPinyin()
                switchLanguageKeepingBuffer(InputMode.ENGLISH26)
            }
            else -> switchLanguageKeepingBuffer(lastChineseMode)
        }
    }

    /**
     * 「英文」键盘按钮（9 键底行；2026-10-10 新增）：**先按拼音上屏 pending、再切英文 26 键**。
     * 语义单一来源 = [selectInputMode] 的英文档（2026-10-10 深夜裁决统一：面板的
     * 「英文26键」也走同一条——三处英文入口共用 [commitPendingAsPinyin]）。落盘照旧。
     */
    fun switchToEnglishKeyboard() = selectInputMode(InputMode.ENGLISH26)

    /**
     * 9 键底行模式键的动作（2026-10-15 用户裁决「简繁按钮就只切换简繁」）：
     * **只切简/繁**（PINYIN ⇄ TRADITIONAL），不再循环进英文 —— 9 键进英文走底行那枚
     * 独立的 [英文] 键（[switchToEnglishKeyboard]）。
     *
     * 走 [EngineController.switchLanguage]（与 [cycleMode] 的「中→繁」腿**同一条**）：
     * 方案不动（9 键仍是 9 键）、缓冲保留（2026-10-09 裁决「拼音保留可继续使用」）。
     * 键面字 = 点它会切到哪（[nineKeyModeLabelOf]，ImeScreen.kt）。
     */
    fun toggleTraditional() {
        val next =
            if (controller.mode == EngineMode.TRADITIONAL) EngineMode.PINYIN else EngineMode.TRADITIONAL
        controller.switchLanguage(next, controller.inputScheme)
    }

    /**
     * 把 pending 缓冲**按拼音原样上屏**（2026-10-10 用户裁决，切英文专用）：
     * 文本取 [EngineController.pendingCommitText]（读数优先；26 键无候选回退原字母；
     * ⚠️ **不是** [EngineController.composingText] —— 那个带待编键的「·」显示记号
     * ——「字母串原样留下」；**9 键读数空不回退数字** —— 那是按键编码不是文本，
     * 2026-10-11 修「9 键没匹配的时候会变成数字输入」的第三扇门）。空文本 = 只清
     * 不上屏；空缓冲 = 空操作。
     */
    private fun commitPendingAsPinyin() {
        if (controller.buffer.isEmpty()) return
        val text = controller.pendingCommitText()
        controller.clear()
        if (text.isNotEmpty()) commit(text)
    }

    /**
     * 语言切换（中↔英）：换方案 + 模式，但**不提交、不清**缓冲（[cycleMode] 的两条腿）。
     * 与 [selectInputMode] 的区别：那条是「输入模式」面板的明确换布局 —— 先 commit
     * pending 再换；这条服从 2026-10-09 裁决「中文输入的时候切换英文，拼音保留可继续
     * 使用」，缓冲原样留在引擎里，切回中文接着打/接着选。
     */
    private fun switchLanguageKeepingBuffer(m: InputMode) {
        inputMode = m
        if (m.engineMode == EngineMode.PINYIN || m.engineMode == EngineMode.TRADITIONAL) {
            lastChineseMode = m
        }
        controller.switchLanguage(m.engineMode, m.scheme)
        view = View.QWERTY
        persist()
    }

    /** 键盘模式变更（工具条菜单）→ 立即生效 + 落盘。 */
    fun updateKeyboardModes(m: KeyboardModes) {
        keyboardModes = m
        persist()
    }

    /** 皮肤选择（工具条「皮肤」面板）→ 立即生效 + 落盘。面板**不自动关**（可连着换着看）。 */
    fun selectSkin(s: xyz.erik.opi.keyboard.SkinOption) {
        skin = s
        persist()
    }

    /** 表情入口：开符号面板并直达表情页。 */
    fun openEmoji() {
        symbolInitialTab = xyz.erik.opi.keyboard.SymbolTab.Emoji
        openSymbol()
    }

    /** 生僻字面板（工具条/笔画键盘的入口）。 */
    fun openRare() {
        commitPendingBuffer()
        resetSearch()
        view = View.RARE
    }

    /** 皮肤面板（工具条入口）。只换观感，不碰引擎 —— 与输入模式那类切换不同，无缓冲语义。 */
    fun openSkin() {
        resetSearch()
        view = View.SKIN
    }

    /**
     * 输入模式面板（工具条入口；2026-10-09 用户裁决「做成像皮肤那样的面板，不要弹框」）。
     * 面板里选中即走 [selectInputMode]（它自己收尾回键面/生僻字面板）；只开面板不动引擎，
     * 中途关掉也不会丢正在打的缓冲。
     */
    fun openInputMode() {
        resetSearch()
        view = View.INPUT_MODE
    }

    /**
     * 键盘模式面板（同上）。面板**不自动关**：单手/暗光/长辈常要连开两三个
     * （与皮肤面板同一条理由，见 ImeToolbar 的注释）。
     */
    fun openKeyboardMode() {
        resetSearch()
        view = View.KEYBOARD_MODE
    }

    // ---- 面板切换（开面板前提交 pending buffer） ----

    fun openNumber() {
        commitPendingBuffer()
        resetSearch()
        // 符号面板回数字面板（已是 NUMBER）时不覆盖记录，否则记下的会是 NUMBER 自己
        if (controller.mode != EngineMode.NUMBER) letterMode = controller.mode
        controller.switchMode(EngineMode.NUMBER)
        view = View.NUMBER
    }

    fun openSymbol() {
        commitPendingBuffer()
        resetSearch()
        view = View.SYMBOL
    }

    /**
     * 符号页（9 键左下角「符」；26/14/笔画同键也走它 —— 2026-10-09 用户裁决
     * 「左下角符按钮点进去应该是个新的页面」）。收起旧符号面板那条路不变（表情图标
     * 仍走 [openSymbol] 的表情 Tab）。开页面前先提交 pending buffer（同 openSymbol）。
     */
    fun openSymbolPage() {
        commitPendingBuffer()
        resetSearch()
        view = View.SYMBOL_PAGE
    }

    /**
     * 引擎侧恢复偏好（服务启动时调一次）：方案 + 开关 + 输入模式。
     * 引擎是权威，UI 的 [inputMode]/[keyboardModes] 由服务从偏好装载后经
     * [selectInputMode] 推一遍（引擎装载成功才调，见 OpiImeService）。
     */
    fun restore(inputMode: InputMode, modes: KeyboardModes, skin: SkinOption = SkinOption.FOLLOW) {
        keyboardModes = modes
        this.skin = skin
        selectInputMode(inputMode)
    }

    /** 回 qwerty：失焦搜索并清空（对齐 flutter _backToLetters）。 */
    fun backToLetters() {
        // 数字面板里打了一半的数字要先上屏（引擎 Number 模式缓冲就是待上屏文本），
        // 否则下面的模式切换会连缓冲一起清掉 —— 用户看得见的输入被静默吞掉
        commitPendingBuffer()
        resetSearch()
        view = View.QWERTY
        restoreLetterMode()
    }

    /** 仅关闭搜索叠盘（对齐 flutter _closeSearch：失焦但保留输入）。 */
    fun closeSearch() {
        searchActive = false
    }

    /**
     * 全量重置搜索态。**面板切换时必须调用**：符号面板的 TextField 随 view 变化被
     * 移出组合，`onFocusChanged(false)` 不会回调，`searchActive` 会卡在 true ——
     * 再切回符号面板时搜索框带旧文本、结果网格直接进搜索态、叠盘自动展开，但那个
     * 框其实没有焦点；离开期间挂起的 250ms 防抖也仍会触发写入 `searchQuery`。
     */
    private fun resetSearch() {
        cancelDebounce()
        searchActive = false
        searchText = ""
        searchQuery = ""
    }

    // ---- 搜索态 qwerty 路由（叠盘键位） ----

    /** 搜索文本统一入口：TextField 键入与 qwerty 搜索盘共用，防抖由此触发。 */
    fun updateSearchText(text: String) {
        searchText = text
        cancelDebounce()
        pendingDebounce = debouncer.schedule(SEARCH_DEBOUNCE_MS) {
            searchQuery = searchText.trim()
        }
    }

    fun searchKey(ch: String) = updateSearchText(searchText + ch)

    fun searchSpace() = searchKey(" ")

    /** 按码点删除（emoji 等代理对不拆半）。 */
    fun searchBackspace() {
        val t = searchText
        if (t.isEmpty()) return
        val cp = t.codePointBefore(t.length)
        updateSearchText(t.dropLast(Character.charCount(cp)))
    }

    /** 搜索框焦点联动（TextField onFocusChanged）。 */
    fun onSearchFocus(focused: Boolean) {
        searchActive = focused
    }

    /** 输入目标切换/输入视图结束：清搜索、清 ⇧、回 qwerty。 */
    fun onEditorChanged() {
        resetSearch()
        controller.resetShift() // 锁定态不得跨输入目标残留（换 app 后开键盘仍全大写）
        view = View.QWERTY
        // 数字面板期间换编辑框：模式必须回字母，否则整个字母盘没有键有归宿
        restoreLetterMode()
    }

    /** 离开数字面板：引擎模式回到进入前的字母模式（只在面板里才切，别动字母流程）。 */
    private fun restoreLetterMode() {
        if (controller.mode == EngineMode.NUMBER) controller.switchMode(letterMode)
    }

    private fun cancelDebounce() {
        pendingDebounce?.invoke()
        pendingDebounce = null
    }

    /** 打开面板前提交 pending 拼音：有候选选第一个提交；无候选的乱码缓冲（如 abc）清掉。 */
    private fun commitPendingBuffer() {
        if (controller.buffer.isEmpty()) return
        // 数字模式的缓冲**就是要上屏的文本本身**（引擎 Number 模式：数字进缓冲、收尾时
        // 原样提交）。走下面的「有候选选第一个」会落进「无候选就清掉」那支 —— 数字串
        // 没有候选，等于把用户刚打的 123 吃掉。inputSpace 是引擎里 Number 的那条收尾。
        if (controller.mode == EngineMode.NUMBER) {
            val text = controller.inputSpace()
            if (text.isNotEmpty()) commit(text)
            return
        }
        if (controller.candidates.isEmpty()) {
            controller.clear()
            return
        }
        val text = controller.select(0)
        if (text.isNotEmpty()) commit(text)
        else controller.clear() // select 返回空（引擎异常）：清掉 buffer 不留残留
    }
}
