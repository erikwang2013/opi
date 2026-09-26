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
    // 切模式：中→繁→英→中 三态循环；离开拼音类模式清残留拼音，防止被空格/回车意外提交
    fun toggleMode() {
        when (controller.mode) {
            EngineMode.PINYIN -> {
                controller.clear()
                controller.switchMode(EngineMode.TRADITIONAL)
            }
            EngineMode.TRADITIONAL -> {
                controller.clear()
                controller.switchMode(EngineMode.ENGLISH)
            }
            else -> controller.switchMode(EngineMode.PINYIN)
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
                    // 96dp = 4 行 × 24dp（Material 最小触控目标）。
                    //
                    // 原为固定 176dp（4 × 44dp），理由是「均分后每行仅 ~22dp 低于触控
                    // slop」。但 176dp 在主流机型上会把面板挤没：IME 窗口高
                    // (0.42×min(w,h)+168)px，1080p 机型 ≈621px，density 2.75 → 仅 226dp；
                    // 而面板固定头 48dp + Tab 36dp + 叠盘 176dp = 260dp > 226dp，
                    // 于是 weight(3f) 的 Box 只剩 50dp、面板内部结果网格 Box(weight(1f))
                    // 直接塌成 0dp —— 点搜索框后结果区完全不可见，Tab 行也被叠盘盖住。
                    // density > 2.39 必现（近十年手机基本都在此列）。
                    //
                    // 折中取 96dp：结果网格回到约 46dp（可见、可滚），代价是搜索盘键高
                    // 从 44dp 降到 24dp。这是缓解不是根治 —— 根治要让 IME 窗口在
                    // 「符号面板 + 搜索态」下变高（改 OpiImeService.keyboardHeight），
                    // 那会影响所有面板，须真机核对。**本机无设备，此改动未实测**：
                    // 复核请 `adb shell wm density` 确认 ≥2.4，再看搜索结果区是否可见。
                    QwertyKeyboard(
                        modifier = Modifier.height(96.dp),
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
                modeLabel = when (controller.mode) {
                    EngineMode.PINYIN -> "中"
                    EngineMode.TRADITIONAL -> "繁"
                    else -> "英"
                },
            )
        }
    }
}
