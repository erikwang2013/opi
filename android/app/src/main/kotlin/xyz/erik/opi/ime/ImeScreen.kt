// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import xyz.erik.opi.candidate.CANDIDATE_BAR_HEIGHT_DP
import xyz.erik.opi.candidate.CandidateBar
import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.engine.EngineMode
import xyz.erik.opi.engine.ShiftState
import xyz.erik.opi.keyboard.NumberPad
import xyz.erik.opi.keyboard.QwertyKeyboard
import xyz.erik.opi.keyboard.SymbolCatalog
import xyz.erik.opi.keyboard.SymbolPanel
import xyz.erik.opi.pet.OpiPet
import xyz.erik.opi.pet.petMood

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
        // 候选栏位置**固定预留** CANDIDATE_BAR_HEIGHT_DP：它出现/消失极其频繁
        // （打第一个字母出现、选词后消失），而窗口高度是常量，条件挂载就等于让 4 行键
        // 在"有没有栏"之间伸缩 —— 本机实测键高 117px ↔ 89px、整块键盘下沉 116px，
        // 每个词都跳两次。槽位常驻后键区高度只由 OpiImeService.keyboardHeight 决定。
        // 代价：多占 44dp（english 模式是模式条，pinyin 空缓冲是小欧的待命位，见下）。
        if (state.view == ImeState.View.QWERTY) {
            Box(
                modifier = Modifier.fillMaxWidth().height(CANDIDATE_BAR_HEIGHT_DP.dp),
                // 待命位（下面 else 分支）的小欧靠这个贴左边；候选栏自身填满整槽，不受影响
                contentAlignment = Alignment.CenterStart,
            ) {
                // 面板视图不显；pinyin 有 buffer/候选才显；english 恒显模式条
                if (controller.mode == EngineMode.ENGLISH ||
                    controller.buffer.isNotEmpty() ||
                    controller.candidates.isNotEmpty()
                ) {
                    // 长按候选 = 删除该自造词（下标语义同点击：页内下标）
                    CandidateBar(
                        controller = controller,
                        onTap = router::handleCandidate,
                        onLongPress = controller::removeUserWord,
                    )
                } else {
                    // 槽位常驻是为了不让键区伸缩，但 pinyin/traditional 空缓冲时这里原本
                    // 什么都不画 —— 44dp 死白的代价（见上方注释）等于白付。让小欧站这儿：
                    // 它就是「安静待命」的样子，打完第一个字母自然让位给候选栏。
                    // 情绪照旧由引擎状态推导（设置页关了学习 → 它睡着，不是笑嘻嘻）。
                    OpiPet(
                        mood = petMood(
                            buffer = controller.buffer,
                            candidateCount = controller.candidates.size,
                            learnerEnabled = controller.learnerEnabled,
                        ),
                        size = 30.dp,
                        modifier = Modifier.padding(start = 12.dp),
                    )
                }
            }
        }
        when (state.view) {
            ImeState.View.NUMBER -> NumberPad(
                modifier = Modifier.weight(1f),
                // 数字/标点走引擎（键码 = 字符）：引擎层标点表与全角开关在面板上同样生效
                // —— 旧路径 onKey = router::commitText 是 UI 直提，绕开引擎。
                onKey = router::handleKey,
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
                    // 叠盘底部功能行与字母盘同义（原先 中/123/↵ 三个键都接 closeSearch：
                    // 「中」键面写着一个模式字、读屏念「中英切换」，行为却是关搜索；⇧ 传空
                    // lambda，既不动也不被标成已停用）。这里每个键给语义正确的行为，
                    // ⇧ 在搜索串上没有意义 → 传 null，由 KeyButton 标成「已停用」。
                    QwertyKeyboard(
                        modifier = Modifier.height(96.dp),
                        onKey = state::searchKey,
                        onSpace = state::searchSpace,
                        onBackspace = state::searchBackspace,
                        onEnter = state::closeSearch, // ↵ 收起叠盘（搜索串与结果保留）
                        onModeSwitch = ::toggleMode,   // 同字母盘：中→繁→英
                        onNumber = state::openNumber,  // 同字母盘：进数字面板
                        onShift = null,
                        onShiftLongPress = null,
                        shiftState = ShiftState.OFF,
                        modeLabel = modeLabelOf(controller.mode),
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
                modeLabel = modeLabelOf(controller.mode),
            )
        }
    }
}

/**
 * 模式键键面字（字母盘与符号搜索叠盘共用一张表）。
 * 键面只有一个字，读屏靠 [xyz.erik.opi.keyboard.spokenKeyName] 念成「中英切换」，
 * 所以这个字必须说实话 —— 叠盘里曾经硬编码「中」。
 */
internal fun modeLabelOf(mode: EngineMode): String = when (mode) {
    EngineMode.PINYIN -> "中"
    EngineMode.TRADITIONAL -> "繁"
    else -> "英"
}
