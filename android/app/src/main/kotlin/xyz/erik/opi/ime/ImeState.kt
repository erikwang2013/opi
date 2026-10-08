// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import android.os.Handler
import android.os.Looper
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import xyz.erik.opi.engine.EngineController
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
    enum class View { QWERTY, NUMBER, SYMBOL, RARE }

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

    /** 当前输入模式（工具条与键盘布局按它画）。 */
    var inputMode by mutableStateOf(InputMode.PINYIN26)
        private set

    /** 键盘模式（悬浮/游戏/单手/暗光/长辈）。繁体不在这里 —— 它在引擎 mode 上。 */
    var keyboardModes by mutableStateOf(KeyboardModes())
        private set

    /** 符号面板的起始 Tab（工具条表情入口直达 Emoji）。 */
    var symbolInitialTab by mutableStateOf(xyz.erik.opi.keyboard.SymbolTab.Common)
        private set

    /**
     * 偏好落盘钩子（OpiImeService 注入；默认空 = JVM 测试不感知）。
     * 与 [commit] 同一套「服务层注入副作用」的做法。
     */
    var persist: () -> Unit = {}

    /** 上次的中文侧输入模式（9 键 ↔ 英文 往返时恢复用）。 */
    private var lastChineseMode = InputMode.PINYIN26

    /**
     * 选一个输入模式（工具条菜单）。生僻字（[InputMode.panel]）**只切面板**，
     * 不动引擎的 scheme/mode —— 它是浏览式查字，不是输入方案。
     */
    fun selectInputMode(m: InputMode) {
        commitPendingBuffer()
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
     * 中→繁只切**引擎模式**（方案不动，9 键仍是 9 键）；繁→英走 [selectInputMode]
     * 的英文项（方案回全键盘 —— 9 键上打英文没有候选路）；英→中回**上次的中文方案**。
     */
    fun cycleMode() {
        when (controller.mode) {
            EngineMode.PINYIN -> {
                commitPendingBuffer()
                controller.switchMode(EngineMode.TRADITIONAL)
            }
            EngineMode.TRADITIONAL -> selectInputMode(InputMode.ENGLISH26)
            else -> selectInputMode(lastChineseMode)
        }
    }

    /** 键盘模式变更（工具条菜单）→ 立即生效 + 落盘。 */
    fun updateKeyboardModes(m: KeyboardModes) {
        keyboardModes = m
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
     * 引擎侧恢复偏好（服务启动时调一次）：方案 + 开关 + 输入模式。
     * 引擎是权威，UI 的 [inputMode]/[keyboardModes] 由服务从偏好装载后经
     * [selectInputMode] 推一遍（引擎装载成功才调，见 OpiImeService）。
     */
    fun restore(inputMode: InputMode, modes: KeyboardModes) {
        keyboardModes = modes
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
