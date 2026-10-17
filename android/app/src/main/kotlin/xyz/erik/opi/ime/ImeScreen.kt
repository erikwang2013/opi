// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

package xyz.erik.opi.ime

import androidx.compose.foundation.background
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
import xyz.erik.opi.candidate.CandidateBar
import xyz.erik.opi.engine.EngineController
import xyz.erik.opi.engine.composingText
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
import xyz.erik.opi.keyboard.SymbolPage
import xyz.erik.opi.keyboard.SymbolPanel
import xyz.erik.opi.pet.petMood

/**
 * IME 面板根：工具条（图标入口）+ 候选栏 + 键盘；数字/符号/生僻字面板。
 *
 * 2026-10-09 起按 **输入模式 × 键盘模式** 组合渲染（对齐主流移动输入法）：
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
    /** 符号页类目（懒读资产）：只在真正打开符号页时才取。 */
    symbolCategories: () -> List<xyz.erik.opi.keyboard.SymbolCategories.Category> = { emptyList() },
    /** 收起键盘（工具栏最右图标）：服务层注入 requestHideSelf；默认空 = JVM 测试不感知。 */
    onCloseKeyboard: () -> Unit = {},
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
        // 根列**铺工具条底色**（2026-10-17 用户裁决「红线区域内输入法的其他界面都覆盖了，
        // 只有默认界面没有，默认界面也要全覆盖」）：拼音行空槽（38dp）此前透明 ⇒ 窗口
        // 露白，与工具条之间出现一条白色空带；其他视图（皮肤/符号/关于…）整窗 panelBg
        // 铺满所以覆盖得住。这里给根列工具条色 —— 空槽与工具条连成一片，视觉上就是
        // 工具条向上延伸（布局/高度一个字没动）。2026-10-09 那条「候选词没词的时候不
        // 显示背景」管的是**候选槽**（现在候选槽与工具条共用一格、恒有内容），不冲突；
        // 各分片仍照旧给自己铺底（工具条 toolbarBg、候选栏 barBg、键区 barBg）。
        Column(modifier = Modifier.fillMaxSize().background(palette.toolbarBg)) {
            // ---- 拼音行（2026-10-15 用户裁决「移动端的拼音在候选词上方显示，不要在
            // 输入框里」）：槽位常驻（空时不铺底色），画的是 `composingText()`
            // （9 键 = 分词读数 `ni'hao`、其余方案 = 原字母）+ 五笔编码提示。
            // 读屏播报也挂在这一行（原先挂在候选栏的缓冲首格，那格已随裁决撤掉）。
            // 平铺候选视图也保留它（参考图 2 的拼音就在最顶上）。----
            if (state.view == ImeState.View.QWERTY || state.view == ImeState.View.RARE ||
                state.view == ImeState.View.CANDIDATES_TILED
            ) {
                // 顶部共用槽位（2026-10-17 裁决「默认顶部好空」）：输入中画拼音行、
                // 空闲画快捷条（最近上屏的词/短语；点=上屏、长按=删除）。同一 38dp
                // 槽位两种内容 —— 与「候选|工具条共用一格」同一套哲学；高度不变。
                if (controller.composingText().isEmpty() && controller.wubiHint.isEmpty()) {
                    QuickPhraseBar(
                        phrases = state.quickPhrases.items,
                        onTap = router::commitText,
                        onRemove = state.quickPhrases::remove,
                    )
                } else {
                    ComposingBar(controller)
                }
            }
            if (state.view == ImeState.View.CANDIDATES_TILED) {
                // ---- 平铺候选视图（2026-10-16 裁决「替换键盘平铺出来」）----
                // 它占满「共用格 + 键盘区」的整块（拼音行以下全给网格与两侧列）。
                CandidateTiledView(
                    state,
                    controller,
                    router,
                    modifier = Modifier.weight(1f).fillMaxWidth(),
                )
            } else {
                // ---- 候选｜工具条**共用一格**（2026-10-16 用户裁决「候选词在功能按钮
                // 区域显示（替换掉功能区域）」）----
                // 原先是两行（候选栏一行 + 工具条一行），现一格两用：输入中有候选
                // （含英文提示条、含「无匹配」）画候选、否则画工具条。判据**单一来源** =
                // [ImeState.candidatesShowable]（平铺态的自动收拢用同一支）。格位常驻
                // （工具条那条 44dp）—— 「槽位常驻、条件挂载会让键区伸缩」那条 2026-10-09
                // 实测教训继续成立；窗口里少掉的正是原候选行那一行（见
                // OpiImeService.keyboardHeight 的新账）。
                Box(
                    modifier = Modifier.fillMaxWidth().height(IME_TOOLBAR_HEIGHT_DP.dp),
                    contentAlignment = Alignment.CenterStart,
                ) {
                    if (state.view == ImeState.View.QWERTY && state.candidatesShowable) {
                        // 长按候选 = 删除该自造词（下标语义同点击：页内下标）
                        CandidateBar(
                            controller = controller,
                            onTap = router::handleCandidate,
                            onLongPress = controller::removeUserWord,
                            onExpand = state::expandCandidates,
                        )
                    } else {
                        // ---- 工具条：图标入口（小欧 logo + 表情 / 皮肤 / 输入模式 / 键盘模式）----
                        ImeToolbar(
                            currentMode = state.inputMode,
                            keyboardModes = modes,
                            petMood = petMood(
                                buffer = controller.buffer,
                                candidateCount = controller.candidates.size,
                                learnerEnabled = controller.learnerEnabled,
                                dictionaryDegraded = controller.dictionaryDegraded,
                            ),
                            onOpenEmoji = state::openEmoji,
                            onOpenSkin = state::openSkin,
                            onOpenInputMode = state::openInputMode,
                            onOpenKeyboardMode = state::openKeyboardMode,
                            onOpenAbout = state::openAbout,
                            onCloseKeyboard = onCloseKeyboard,
                        )
                    }
                }
                // ---- 键盘区：单手模式整体偏移 + 收窄（纯布局层）----
                // 键区自己刷底色（根列已透明）：键与键之间的缝是这一层 —— 换深色皮肤时
                // 不刷会露出窗口底色（2026-10-09「整体统一」那条的落点）。
                val keyArea = Modifier.weight(1f).fillMaxWidth().background(palette.barBg).let {
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
                    KeyboardArea(state, controller, router, catalog, rareIndex, symbolCategories, modifier = Modifier.weight(1f))
                    if (modes.oneHand == KeyboardModes.OneHand.LEFT) {
                        Box(modifier = Modifier.weight(0.24f)) // 左侧单手：右边留空
                    }
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
    symbolCategories: () -> List<xyz.erik.opi.keyboard.SymbolCategories.Category>,
    modifier: Modifier = Modifier,
) {
    val shiftVisible = controller.mode == EngineMode.ENGLISH
    when (state.view) {
        // 平铺候选视图（2026-10-16 裁决）：顶层已分流（它连「共用格」一起占，根本
        // 不进键区）—— 本分支只为 when 穷尽，正常到不了。
        ImeState.View.CANDIDATES_TILED -> Box(modifier = modifier)
        ImeState.View.RARE -> RarePanel(
            index = rareIndex(),
            onCommit = router::commitText,
            onClose = state::backToLetters,
            modifier = modifier,
        )
        ImeState.View.SYMBOL_PAGE -> SymbolPage(
            categories = symbolCategories(),
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
        // 输入模式/键盘模式面板（2026-10-09 用户裁决「做成像皮肤那样的面板，不要弹框」）：
        // 形态与皮肤同一套；选中/开关的收尾走各自既有路径（selectInputMode 自己回键面）。
        ImeState.View.INPUT_MODE -> InputModePanel(
            current = state.inputMode,
            onSelect = state::selectInputMode,
            onClose = state::backToLetters,
            modifier = modifier,
        )
        ImeState.View.KEYBOARD_MODE -> KeyboardModePanel(
            traditional = controller.mode == EngineMode.TRADITIONAL,
            keyboardModes = state.keyboardModes,
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
            onUpdate = state::updateKeyboardModes,
            onClose = state::backToLetters,
            modifier = modifier,
        )
        // 关于面板（2026-10-15 用户裁决）：工具条入口直达，内容复用 settings 侧的常量。
        ImeState.View.ABOUT -> AboutPanel(
            onClose = state::backToLetters,
            modifier = modifier,
        )
        ImeState.View.NUMBER -> NumberPad(
            modifier = modifier,
            // 数字/标点走引擎（键码 = 字符）：引擎层标点表与全角开关在面板上同样生效
            // —— 旧路径 onKey = router::commitText 是 UI 直提，绕开引擎。
            onKey = router::handleKey,
            onSymbol = state::openSymbolPage,
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
                    modeLabel = state.modeKeyLabel,
                )
            }
        }
        ImeState.View.QWERTY -> {
            // 输入模式分发（生僻字是面板，已在上面分支）。
            val label = state.modeKeyLabel
            // 9 键 / 14 键 / 笔画 的模式键**只切简/繁**（2026-10-15 用户裁决，三盘统一）：
            // 键面字不走 [modeKeyLabel] 的三档循环，改用 [jianFanModeKeyLabelOf]。
            // 26 键仍走三档循环（它没有独立的英文键，cycleMode 那条腿不能摘）。
            val jianFanLabel = jianFanModeKeyLabelOf(controller.mode)
            when (state.inputMode) {
                InputMode.PINYIN9, InputMode.BIG9 -> NineKeyKeyboard(
                    modifier = modifier,
                    onKey = router::handleKey,
                    onSpace = router::handleSpace,
                    onBackspace = router::handleBackspace,
                    onEnter = router::handleEnter,
                    // 9 键的模式键**只切简/繁**（2026-10-15 用户裁决「简繁按钮就只切换
                    // 简繁」）—— 不是 26 键那条中→繁→英循环（cycleMode 只在 26 键与
                    // 搜索叠盘上留着；14/笔画同日裁决后也归到本条）。
                    onModeSwitch = state::toggleTraditional,
                    onSymbol = state::openSymbolPage,
                    // 重输（2026-10-10 用户裁决：去分词键、加「重输」）：清掉当前组合重来；
                    // 空缓冲是空操作（clear 幂等）。
                    onRetype = controller::clear,
                    // 分词键（2026-10-15 用户裁决放回主区域第一格）：缓冲非空才发 `'`
                    // —— 空缓冲时它在引擎里是引号不是分词符（engine_punct.rs），
                    // 所以宿主拦住；无匹配的中文分词匹配仍由引擎自动回退承担（engine-seg）。
                    onSegment = {
                        if (controller.buffer.isNotEmpty()) router.handleKey("'")
                    },
                    // 数字键盘（2026-10-10 用户裁决：底行新增）——与 26 键的 123 同路。
                    onNumber = state::openNumber,
                    // 英文键盘：先按拼音上屏 pending、再切英文 26 键
                    //（2026-10-10 用户裁决「切英文默认是拼音，不要是候选词」）。
                    onEnglish = state::switchToEnglishKeyboard,
                    // 左侧一列：**只有读得到拼音时才替换**（主流同款，2026-10-09
                    // 用户裁决「不输入、回退、确定，就重置到初始化的一列」）——
                    // 读数空（没候选/已提交/退空）就回到标点列；不回退显示数字串
                    // （那是上一版的自作主张，用户明确不要）。
                    readings = controller.candidateReadings,
                    onPickReading = { i ->
                        // 点拼音行 = **钉读音**（2026-10-09 用户裁决「不要直接确认」）：
                        // 候选收窄到读数恰等的条目、不上屏；同一行再点 = 解除。
                        // 「恰等」是 2026-10-16 裁决（此前是「以它开头」，联想词组会漏进来）。
                        controller.candidateReadings.getOrNull(i)?.let(controller::togglePinnedReading)
                    },
                    pinnedReading = controller.pinnedReading,
                    scrollKey = controller.buffer,
                    modeLabel = jianFanLabel,
                    big = state.inputMode == InputMode.BIG9,
                )
                InputMode.PINYIN14 -> FourteenKeyKeyboard(
                    modifier = modifier,
                    onKey = router::handleKey,
                    onSpace = router::handleSpace,
                    onBackspace = router::handleBackspace,
                    onEnter = router::handleEnter,
                    // 只切简/繁（2026-10-15 用户裁决，与 9 键统一）
                    onModeSwitch = state::toggleTraditional,
                    onSymbol = state::openSymbolPage,
                    // 英文入口必须独立留一枚（与 9 键同）：本盘原先靠模式键循环进英文，
                    // 摘掉那条腿后不补这枚，英文在本盘**就不可达了**（2026-10-15 实测
                    // 全仓只有 9 键与 cycleMode 两条进英文的路）。
                    onEnglish = state::switchToEnglishKeyboard,
                    modeLabel = jianFanLabel,
                )
                InputMode.STROKE -> StrokeKeyboard(
                    modifier = modifier,
                    onKey = router::handleKey,
                    onSpace = router::handleSpace,
                    onBackspace = router::handleBackspace,
                    onEnter = router::handleEnter,
                    // 只切简/繁（2026-10-15 用户裁决，与 9 键统一）
                    onModeSwitch = state::toggleTraditional,
                    onSymbol = state::openSymbolPage,
                    onRare = state::openRare,
                    // 同上：14 键那枚 [英文] 的理由逐条适用（本盘原先也没有独立入口）
                    onEnglish = state::switchToEnglishKeyboard,
                    modeLabel = jianFanLabel,
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
 * 模式键键面字 = **点它会切到哪**（不是当前模式；用户裁决 2026-10-12 晚）。
 * 纯函数，单测钉住（[xyz.erik.opi.ime.ImeState.modeKeyLabel] 是它的读侧）。
 *
 * 与 `cycleMode()` 的次序**逐条对应**：中→繁、繁→英、英→回上次的中文方案
 * （`lastChineseIsTrad` = 上次的中文方案是不是繁体）。改次序必须同改这里。
 *
 * ⚠️ 历史：此前这里返回的是**当前**模式（中/繁/英）—— 用户实测报「切到英文键盘后
 * 键面还写着英，点下去却是中文键盘，键面该是「中」才对」。键面写动作、当前态由键盘
 * 布局本身表达（英文态就是 26 键盘），与 9 键底行「英文」键同一条语义。
 */
internal fun modeKeyLabelOf(mode: EngineMode, lastChineseIsTrad: Boolean): String = when (mode) {
    EngineMode.PINYIN -> "繁"
    EngineMode.TRADITIONAL -> "英"
    else -> if (lastChineseIsTrad) "繁" else "中"
}

/**
 * **9 键 / 14 键 / 笔画**底行模式键的键面字（2026-10-15 用户裁决「简繁按钮就只切换简繁」，
 * 后两盘同日裁决「统一成只切简繁」）。
 * 与 [modeKeyLabelOf] 同一条语义 —— 键面 = **点它会切到哪** —— 只是把「英」那条腿
 * 摘掉：这三盘上它只在 中/繁 之间切（[ImeState.toggleTraditional]），进英文走各自
 * 底行那枚独立的 [英文] 键。故：拼音态 →「繁」、繁体态 →「中」。
 *
 * 兜底给「繁」：这三盘按 `state.inputMode` 分发，理论上只会拿到中文态；不返回空/不抛。
 * 纯函数，单测钉住（`KeyLabelTest`）。
 */
internal fun jianFanModeKeyLabelOf(mode: EngineMode): String =
    if (mode == EngineMode.TRADITIONAL) "中" else "繁"
