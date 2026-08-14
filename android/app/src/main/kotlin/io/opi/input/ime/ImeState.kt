package io.opi.input.ime

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

/**
 * 面板状态机（骨架）：当前仅面板视图切换 + 编辑器切换重置。
 * A4 补 pending buffer / 防抖 / 搜索态。
 */
class ImeState {
    enum class View { QWERTY, NUMBER, SYMBOL }

    var view by mutableStateOf(View.QWERTY)
        private set

    fun switchView(v: View) {
        view = v
    }

    /** 输入目标切换/输入视图结束时重置面板状态（清 buffer/回 qwerty）。 */
    fun onEditorChanged() {
        view = View.QWERTY
    }
}
