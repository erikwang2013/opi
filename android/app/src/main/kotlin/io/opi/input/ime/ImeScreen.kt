package io.opi.input.ime

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import io.opi.input.candidate.CandidateBar
import io.opi.input.engine.EngineController
import io.opi.input.engine.EngineMode
import io.opi.input.engine.ShiftState
import io.opi.input.keyboard.QwertyKeyboard

/**
 * IME 面板根：候选栏（qwerty 视图）+ 键盘；数字/符号面板 A4 接入。
 * 模式切换/⇧ 可见性对齐 flutter ime_main.dart。
 */
@Composable
fun ImeScreen(state: ImeState, controller: EngineController, router: KeyRouter) {
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
        QwertyKeyboard(
            modifier = Modifier.weight(1f),
            onKey = router::handleKey,
            onSpace = router::handleSpace,
            onBackspace = router::handleBackspace,
            onEnter = router::handleEnter,
            onModeSwitch = ::toggleMode,
            onNumber = { state.switchView(ImeState.View.NUMBER) }, // A4 数字面板
            // pinyin 模式 ⇧ 无意义且残留状态会泄漏进 English：传 null 禁用
            onShift = if (shiftVisible) controller::shiftTap else null,
            onShiftLongPress = if (shiftVisible) controller::shiftLongPress else null,
            shiftState = if (shiftVisible) controller.shiftState else ShiftState.OFF,
            modeLabel = if (controller.mode == EngineMode.PINYIN) "中" else "英",
        )
    }
}
