package io.opi.input.ime

import android.os.Handler
import android.os.Looper
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import io.opi.input.engine.EngineController

/** 防抖调度抽象：生产用主线程 Handler，JVM 测试注入假时钟（确定性，无 Thread.sleep）。 */
fun interface Debouncer {
    /** delayMs 后执行 action；返回取消函数。 */
    fun schedule(delayMs: Long, action: () -> Unit): () -> Unit
}

/** 生产实现：主线程 Handler.postDelayed。 */
class HandlerDebouncer : Debouncer {
    override fun schedule(delayMs: Long, action: () -> Unit): () -> Unit {
        val handler = Handler(Looper.getMainLooper())
        val r = Runnable { action() }
        handler.postDelayed(r, delayMs)
        return { handler.removeCallbacks(r) }
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
    enum class View { QWERTY, NUMBER, SYMBOL }

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

    fun switchView(v: View) {
        view = v
    }

    // ---- 面板切换（开面板前提交 pending buffer） ----

    fun openNumber() {
        commitPendingBuffer()
        view = View.NUMBER
    }

    fun openSymbol() {
        commitPendingBuffer()
        view = View.SYMBOL
    }

    /** 回 qwerty：失焦搜索并清空（对齐 flutter _backToLetters）。 */
    fun backToLetters() {
        searchActive = false
        cancelDebounce()
        searchText = ""
        searchQuery = ""
        view = View.QWERTY
    }

    /** 仅关闭搜索叠盘（对齐 flutter _closeSearch：失焦但保留输入）。 */
    fun closeSearch() {
        searchActive = false
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

    /** 输入目标切换/输入视图结束：清搜索、回 qwerty。 */
    fun onEditorChanged() {
        cancelDebounce()
        searchActive = false
        searchText = ""
        searchQuery = ""
        view = View.QWERTY
    }

    private fun cancelDebounce() {
        pendingDebounce?.invoke()
        pendingDebounce = null
    }

    /** 打开面板前提交 pending 拼音：有候选选第一个提交；无候选的乱码缓冲（如 abc）清掉。 */
    private fun commitPendingBuffer() {
        if (controller.buffer.isEmpty()) return
        if (controller.candidates.isEmpty()) {
            controller.clear()
            return
        }
        val text = controller.select(0)
        if (text.isNotEmpty()) commit(text)
    }
}
