// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import xyz.erik.opi.candidate.CANDIDATE_BAR_HEIGHT_DP
import xyz.erik.opi.candidate.CandidateBar
import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.engine.EngineMode
import xyz.erik.opi.engine.ShiftState
import xyz.erik.opi.keyboard.DarkPalette
import xyz.erik.opi.keyboard.ElderPalette
import xyz.erik.opi.keyboard.SkinPanel
import xyz.erik.opi.keyboard.FourteenKeyKeyboard
import xyz.erik.opi.keyboard.LightPalette
import xyz.erik.opi.keyboard.LocalKeyboardPalette
import xyz.erik.opi.keyboard.NineKeyKeyboard
import xyz.erik.opi.keyboard.NumberPad
import xyz.erik.opi.keyboard.QwertyKeyboard
import xyz.erik.opi.keyboard.RarePanel
import xyz.erik.opi.keyboard.StrokeKeyboard
import xyz.erik.opi.keyboard.SymbolCatalog
import xyz.erik.opi.keyboard.SymbolPanel
import xyz.erik.opi.pet.petMood

/**
 * IME 面板根：工具条（图标入口）+ 候选栏 + 键盘；数字/符号/生僻字面板。
 *
 * 2026-10-09 起按 **输入模式 × 键盘模式** 组合渲染（对齐搜狗移动端）：
 *   · 输入模式决定画哪套键面（26/9/14/大9/笔画）+ 引擎 scheme（[ImeState.selectInputMode]）；
 *   · 键盘模式是外观/窗口层（暗光/长辈换色板、单手偏移、悬浮/游戏改窗口，见
 *     [KeyboardModes] 的注释）。
 * 两者都在 [ImeState]（单一状态源），本组件只做渲染与转发。
 *
 * [rareIndex] 是**取数函数**（懒加载 1.1MB 的部首索引）：只在真正打开生僻字面板时
 * 被调用 —— 别把它换成启动时读好的 Map（那会进服务启动路径）。
 */
@Composable
fun ImeScreen(
    state: ImeState,
    controller: EngineController,
    router: KeyRouter,
    catalog: SymbolCatalog,
    rareIndex: () -> Map<Int, Map<Int, List<String>>> = { emptyMap() },
) {
    val modes = state.keyboardModes
    // 优先级：长辈 > 暗光 > 皮肤（皮肤是「底」，模式是临时覆盖；见 SkinOption 的说明）。
    val palette = when {
        modes.elder -> ElderPalette
        modes.dark -> DarkPalette
        else -> state.skin.palette
    }
    // 长辈模式：字号整体 +18%（配 ElderPalette 的高对比）。改 LocalDensity 只动字号，
    // 不动像素尺寸（键高仍由窗口高度均分），所以不会把键挤出屏幕。
    val baseDensity = LocalDensity.current
    val density = if (modes.elder) {
        Density(baseDensity.density, baseDensity.fontScale * 1.18f)
    } else {
        baseDensity
    }

    CompositionLocalProvider(
        LocalKeyboardPalette provides palette,
        LocalDensity provides density,
    ) {
        Column(modifier = Modifier.fillMaxSize()) {
            // ---- 工具条：图标入口（表情 / 输入模式 / 键盘模式）----
            ImeToolbar(
                currentMode = state.inputMode,
                keyboardModes = modes,
                traditional = controller.mode == EngineMode.TRADITIONAL,
                petMood = petMood(
                    buffer = controller.buffer,
                    candidateCount = controller.candidates.size,
                    learnerEnabled = controller.learnerEnabled,
                    dictionaryDegraded = controller.dictionaryDegraded,
                ),
                onOpenEmoji = state::openEmoji,
                onOpenSkin = state::openSkin,
                onSelectInputMode = state::selectInputMode,
                onToggleTraditional = {
                    // 与「中/繁/英」键同一条路：只切引擎模式，方案（9/14/键布局）不动
                    controller.switchMode(
                        if (controller.mode == EngineMode.TRADITIONAL) {
                            EngineMode.PINYIN
                        } else {
                            EngineMode.TRADITIONAL
                        },
                    )
                },
                onUpdateKeyboardModes = state::updateKeyboardModes,
            )
            // 候选栏位置**固定预留** CANDIDATE_BAR_HEIGHT_DP：它出现/消失极其频繁
            // （打第一个字母出现、选词后消失），而窗口高度是常量，条件挂载就等于让 4 行键
            // 在"有没有栏"之间伸缩 —— 本机实测键高 117px ↔ 89px、整块键盘下沉 116px，
            // 每个词都跳两次。槽位常驻后键区高度只由 OpiImeService.keyboardHeight 决定。
            // 代价：多占 44dp（english 模式是模式条，pinyin 空缓冲是小欧的待命位，见下）。
            if (state.view == ImeState.View.QWERTY || state.view == ImeState.View.RARE) {
                Box(
                    modifier = Modifier.fillMaxWidth().height(CANDIDATE_BAR_HEIGHT_DP.dp),
                    // 待命位（下面 else 分支）的小欧靠这个贴左边；候选栏自身填满整槽，不受影响
                    contentAlignment = Alignment.CenterStart,
                ) {
                    if (state.view == ImeState.View.RARE) {
                        // 生僻字面板是浏览式查字：候选栏槽位停用小欧（它不参与查找），
                        // 保持空槽以免键区伸缩（同上面那条理由）。
                    } else if (controller.mode == EngineMode.ENGLISH ||
                        controller.buffer.isNotEmpty() ||
                        controller.candidates.isNotEmpty()
                    ) {
                        // 长按候选 = 删除该自造词（下标语义同点击：页内下标）
                        CandidateBar(
                            controller = controller,
                            onTap = router::handleCandidate,
                            onLongPress = controller::removeUserWord,
                        )
                    }
                    // else：槽位空着。小欧 2026-10-09 已并入工具条（用户裁决
                    // 「入口要和 logo 图标在一行」）—— 待命位不再画它，槽位只做
                    // 「不让键区伸缩」的占位（见上）。
                }
            }
            // ---- 键盘区：单手模式整体偏移 + 收窄（纯布局层）----
            val keyArea = Modifier.weight(1f).fillMaxWidth().let {
                when (modes.oneHand) {
                    KeyboardModes.OneHand.OFF -> it
                    KeyboardModes.OneHand.LEFT -> it.fillMaxWidth(0.76f)
                    KeyboardModes.OneHand.RIGHT -> it.fillMaxWidth(0.76f)
                }
            }
            Row(modifier = keyArea) {
                if (modes.oneHand == KeyboardModes.OneHand.RIGHT) {
                    Box(modifier = Modifier.weight(0.24f)) // 右侧单手：左边留空
                }
                KeyboardArea(state, controller, router, catalog, rareIndex, modifier = Modifier.weight(1f))
                if (modes.oneHand == KeyboardModes.OneHand.LEFT) {
                    Box(modifier = Modifier.weight(0.24f)) // 左侧单手：右边留空
                }
            }
        }
    }
}

/** 键盘区（面板分发）：数字/符号/生僻字面板与输入模式对应的字母键面。 */
@Composable
private fun KeyboardArea(
    state: ImeState,
    controller: EngineController,
    router: KeyRouter,
    catalog: SymbolCatalog,
    rareIndex: () -> Map<Int, Map<Int, List<String>>>,
    modifier: Modifier = Modifier,
) {
    val shiftVisible = controller.mode == EngineMode.ENGLISH
    when (state.view) {
        ImeState.View.RARE -> RarePanel(
            index = rareIndex(),
            onCommit = router::commitText,
            onClose = state::backToLetters,
            modifier = modifier,
        )
        ImeState.View.SKIN -> SkinPanel(
            selected = state.skin,
            onSelect = state::selectSkin,
            onClose = state::backToLetters,
            modifier = modifier,
        )
        ImeState.View.NUMBER -> NumberPad(
            modifier = modifier,
            // 数字/标点走引擎（键码 = 字符）：引擎层标点表与全角开关在面板上同样生效
            // —— 旧路径 onKey = router::commitText 是 UI 直提，绕开引擎。
            onKey = router::handleKey,
            onSymbol = state::openSymbol,
            onLetters = state::backToLetters,
            onSpace = router::handleSpace,
            onBackspace = router::handleBackspace,
            onEnter = router::handleEnter,
        )
        ImeState.View.SYMBOL -> Column(modifier = modifier) {
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
                    initialTab = state.symbolInitialTab,
                )
            }
            if (state.searchActive) {
                // 96dp = 4 行 × 24dp（Material 最小触控目标）。
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
                    onModeSwitch = state::cycleMode,
                    onNumber = state::openNumber,  // 同字母盘：进数字面板
                    onShift = null,
                    onShiftLongPress = null,
                    shiftState = ShiftState.OFF,
                    modeLabel = modeLabelOf(controller.mode),
                )
            }
        }
        ImeState.View.QWERTY -> {
            // 输入模式分发（生僻字是面板，已在上面分支）。
            val label = modeLabelOf(controller.mode)
            when (state.inputMode) {
                InputMode.PINYIN9, InputMode.BIG9 -> NineKeyKeyboard(
                    modifier = modifier,
                    onKey = router::handleKey,
                    onSpace = router::handleSpace,
                    onBackspace = router::handleBackspace,
                    onEnter = router::handleEnter,
                    onModeSwitch = state::cycleMode,
                    onSymbol = state::openSymbol,
                    modeLabel = label,
                    big = state.inputMode == InputMode.BIG9,
                )
                InputMode.PINYIN14 -> FourteenKeyKeyboard(
                    modifier = modifier,
                    onKey = router::handleKey,
                    onSpace = router::handleSpace,
                    onBackspace = router::handleBackspace,
                    onEnter = router::handleEnter,
                    onModeSwitch = state::cycleMode,
                    onSymbol = state::openSymbol,
                    modeLabel = label,
                )
                InputMode.STROKE -> StrokeKeyboard(
                    modifier = modifier,
                    onKey = router::handleKey,
                    onSpace = router::handleSpace,
                    onBackspace = router::handleBackspace,
                    onEnter = router::handleEnter,
                    onModeSwitch = state::cycleMode,
                    onSymbol = state::openSymbol,
                    onRare = state::openRare,
                    modeLabel = label,
                )
                // PINYIN26 / ENGLISH26 / RARE（RARE 走上面分支，这里只为穷尽）
                else -> QwertyKeyboard(
                    modifier = modifier,
                    onKey = router::handleKey,
                    onSpace = router::handleSpace,
                    onBackspace = router::handleBackspace,
                    onEnter = router::handleEnter,
                    onModeSwitch = state::cycleMode,
                    onNumber = state::openNumber,
                    // pinyin 模式 ⇧ 无意义且残留状态会泄漏进 English：传 null 禁用
                    onShift = if (shiftVisible) controller::shiftTap else null,
                    onShiftLongPress = if (shiftVisible) controller::shiftLongPress else null,
                    shiftState = if (shiftVisible) controller.shiftState else ShiftState.OFF,
                    modeLabel = label,
                )
            }
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
