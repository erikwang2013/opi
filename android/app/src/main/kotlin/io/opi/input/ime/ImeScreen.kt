package io.opi.input.ime

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import io.opi.input.candidate.CandidateBar
import io.opi.input.engine.EngineController
import io.opi.input.engine.EngineMode
import io.opi.input.engine.ShiftState
import io.opi.input.keyboard.NumberPad
import io.opi.input.keyboard.QwertyKeyboard
import io.opi.input.keyboard.SymbolCatalog
import io.opi.input.keyboard.SymbolPanel

/**
 * IME 面板根：候选栏（qwerty 视图）+ 键盘；数字/符号面板（A4）。
 * 模式切换/⇧ 可见性对齐 flutter ime_main.dart；面板切换与搜索态在 ImeState。
 */
@Composable
fun ImeScreen(
    state: ImeState,
    controller: EngineController,
    router: KeyRouter,
    catalog: SymbolCatalog,
) {
    // 切中/英：切英文前清残留拼音，防止残留拼音被空格/回车意外提交
    fun toggleMode() {
        if (controller.mode == EngineMode.PINYIN) {
            controller.clear()
            controller.switchMode(EngineMode.ENGLISH)
        } else {
            controller.switchMode(EngineMode.PINYIN)
        }
    }
    val shiftVisible = controller.mode == EngineMode.ENGLISH
    Column(modifier = Modifier.fillMaxSize()) {
        // 候选栏：pinyin 有 buffer/候选才显；english 恒显模式条；面板视图不显
        if (state.view == ImeState.View.QWERTY &&
            (controller.mode == EngineMode.ENGLISH ||
                controller.buffer.isNotEmpty() ||
                controller.candidates.isNotEmpty())
        ) {
            CandidateBar(controller = controller, onTap = router::handleCandidate)
        }
        when (state.view) {
            ImeState.View.NUMBER -> NumberPad(
                modifier = Modifier.weight(1f),
                onKey = router::commitText,
                onSymbol = state::openSymbol,
                onLetters = state::backToLetters,
                onSpace = router::handleSpace,
                onBackspace = router::handleBackspace,
                onEnter = router::handleEnter,
            )
            ImeState.View.SYMBOL -> Column(modifier = Modifier.weight(1f).fillMaxWidth()) {
                // 面板恒挂载：搜索态下搜索框+结果网格保持可见；焦点时下方叠 qwerty 搜索盘
                Box(modifier = Modifier.weight(3f).fillMaxWidth()) {
                    SymbolPanel(
                        catalog = catalog,
                        searchText = state.searchText,
                        searchActive = state.searchActive,
                        searchQuery = state.searchQuery,
                        onSearchText = state::updateSearchText,
                        onSearchFocus = state::onSearchFocus,
                        onCommit = router::commitText,
                        onClose = state::backToLetters,
                        onBackToNumber = state::openNumber,
                    )
                }
                if (state.searchActive) {
                    // 固定 176dp（4 行 × 44dp）：Expanded 均分在 IME 短窗下每行仅 ~22dp，
                    // 低于 18dp 触控 slop；面板侧网格可滚动、能吸收挤压。
                    QwertyKeyboard(
                        modifier = Modifier.height(176.dp),
                        onKey = state::searchKey,
                        onSpace = state::searchSpace,
                        onBackspace = state::searchBackspace,
                        onEnter = state::closeSearch,
                        onModeSwitch = state::closeSearch,
                        onNumber = state::closeSearch,
                        onShift = {},
                        onShiftLongPress = {},
                        shiftState = ShiftState.OFF,
                    )
                }
            }
            ImeState.View.QWERTY -> QwertyKeyboard(
                modifier = Modifier.weight(1f),
                onKey = router::handleKey,
                onSpace = router::handleSpace,
                onBackspace = router::handleBackspace,
                onEnter = router::handleEnter,
                onModeSwitch = ::toggleMode,
                onNumber = state::openNumber,
                // pinyin 模式 ⇧ 无意义且残留状态会泄漏进 English：传 null 禁用
                onShift = if (shiftVisible) controller::shiftTap else null,
                onShiftLongPress = if (shiftVisible) controller::shiftLongPress else null,
                shiftState = if (shiftVisible) controller.shiftState else ShiftState.OFF,
                modeLabel = if (controller.mode == EngineMode.PINYIN) "中" else "英",
            )
        }
    }
}
