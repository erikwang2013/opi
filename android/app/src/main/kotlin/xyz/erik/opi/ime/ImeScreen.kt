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
import xyz.erik.opi.keyboard.KeyAlternative
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
import xyz.erik.opi.pet.PetMood

/**
 * 单手模式：键盘整体占屏宽的 76%，其余 24% 留在另一侧。
 *
 * 注意它与 [oneHandGapWeight] 的分工：键区外层已经 `fillMaxWidth(0.76f)`，行内的
 * 留白权重是相对**键区**分配的，所以留白要占 `(1-0.76)/0.76 ≈ 0.316`，而不是 0.24。
 * 原实现用 0.24 → 键只拿到 0.76×0.76 = 58% 屏宽，右手模式还会离右边缘 18%
 * （320dp 屏上九宫格中列每格只剩约 37dp，提示行放不下）。
 */
private const val ONE_HAND_WIDTH_RATIO = 0.76f

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
    /** 颜文字分类（懒读 `assets/kaomoji.tsv`）：只在表情页挂载时才取。 */
    kaomojiCategories: () -> List<xyz.erik.opi.keyboard.KaomojiCategories.Category> = { emptyList() },
    /** 点「自有」宠物形态：服务层注入图片插入（commitPetSticker）；默认空 = JVM 测试不感知。 */
    onPickPet: (PetMood) -> Unit = {},
    /** 收起键盘（工具栏最右图标）：服务层注入 requestHideSelf；默认空 = JVM 测试不感知。 */
    onCloseKeyboard: () -> Unit = {},
    /** 键盘内设置面板「清除用户词库」：服务层注入共享两半清理（引擎内存 + 落盘）。 */
    onClearUserWords: () -> Unit = {},
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
        LocalSceneActive provides (state.scene != null),
    ) {
        // 根列**铺工具条底色**（2026-10-17 用户裁决「红线区域内输入法的其他界面都覆盖了，
        // 只有默认界面没有，默认界面也要全覆盖」）：顶部信息行槽此前透明 ⇒ 窗口
        // 露白，与工具条之间出现一条白色空带；其他视图（皮肤/符号/关于…）整窗 panelBg
        // 铺满所以覆盖得住。这里给根列工具条色 —— 空槽与工具条连成一片，视觉上就是
        // 工具条向上延伸（布局/高度一个字没动）。2026-10-09 那条「候选词没词的时候不
        // 显示背景」管的是**候选槽**（现在候选槽与工具条共用一格、恒有内容），不冲突；
        // 各分片仍照旧给自己铺底（工具条 toolbarBg、候选栏 barBg、键区半透明 barBg 罩在插画上）。
        Column(modifier = Modifier.fillMaxSize().opiSkinBackground(state.scene)) {
            // ---- 拼音行（2026-10-15 用户裁决「移动端的拼音在候选词上方显示，不要在
            // 输入框里」）：槽位常驻（空时不铺底色），画的是 `composingText()`
            // （9 键 = 分词读数 `ni'hao`、其余方案 = 原字母）+ 五笔编码提示。
            // 读屏播报也挂在这一行（原先挂在候选栏的缓冲首格，那格已随裁决撤掉）。
            // 平铺候选视图也保留它（参考图 2 的拼音就在最顶上）。----
            val englishIdle = controller.mode == EngineMode.ENGLISH &&
                controller.buffer.isEmpty() && controller.candidates.isEmpty()
            if (state.view == ImeState.View.QWERTY || state.view == ImeState.View.RARE ||
                state.view == ImeState.View.CANDIDATES_TILED
            ) {
                // 顶部共用槽位：输入中画拼音行、空闲画**小欧宠物条**。
                // 判据在 composingRowActive（缓冲/候选也算输入中 —— 有候选的那一拍
                // 绝不能让拼音行被宠物条顶掉；9 键无匹读数空时同理）。
                if (composingRowActive(
                        controller.composingText(),
                        controller.wubiHint,
                        controller.buffer,
                        controller.candidates.size,
                    )
                ) {
                    ComposingBar(controller)
                } else {
                    PetStrip()
                }
            }
            if (state.view == ImeState.View.CANDIDATES_TILED) {
                // 平铺候选视图：占满拼音行以下的整块。
                CandidateTiledView(
                    state,
                    controller,
                    router,
                    modifier = Modifier.weight(1f).fillMaxWidth(),
                )
            } else {
                Box(
                    modifier = Modifier.fillMaxWidth().height(IME_TOOLBAR_HEIGHT_DP.dp),
                    contentAlignment = Alignment.CenterStart,
                ) {
                    if (state.view == ImeState.View.QWERTY && state.candidatesShowable && !englishIdle) {
                        // 候选栏仍替换工具条；只有英文**空闲**（无缓冲、无候选）时显示工具条。
                        CandidateBar(
                            controller = controller,
                            onTap = router::handleCandidate,
                            onLongPress = controller::removeUserWord,
                            onExpand = state::expandCandidates,
                            inEnglish = controller.mode == EngineMode.ENGLISH,
                            onSwitchToChinese = state::cycleMode,
                        )
                    } else {
                        // ---- 工具条：中文空闲 / 英文空闲 / 面板视图都显示 ----
                        ImeToolbar(
                            currentMode = state.inputMode,
                            keyboardModes = modes,
                            petMood = petMood(
                                buffer = controller.buffer,
                                candidateCount = controller.candidates.size,
                                learnerEnabled = controller.learnerEnabled,
                                dictionaryDegraded = controller.dictionaryDegraded,
                            ),
                            inEnglish = controller.mode == EngineMode.ENGLISH,
                            onSwitchToChinese = state::cycleMode,
                            onOpenEmoji = state::openEmoji,
                            onOpenSkin = state::openSkin,
                            onOpenInputMode = state::openInputMode,
                            onOpenKeyboardMode = state::openKeyboardMode,
                            // 齿轮（2026-10-22 用户裁决）：不再跳 app 设置页，开键盘内设置面板。
                            onOpenSettings = state::openSettings,
                            onOpenAbout = state::openAbout,
                            onCloseKeyboard = onCloseKeyboard,
                        )
                    }
                }
                // ---- 键盘区：单手模式收窄到 76%，贴所选手一侧（纯布局层）----
                // 键区自己刷底色（根列已透明）：键与键之间的缝是这一层 —— 换深色皮肤时
                // 不刷会露出窗口底色（2026-10-09「整体统一」那条的落点）。
                // 权重分摊的是**整行宽**：键区 0.76 + 空白 0.24 = 屏宽 76/24。行本身必须满宽，
                // 否则右手模式的键区停在 76% 处、右侧反而多出一条空白（不是贴边）。
                Row(
                    modifier = Modifier.weight(1f).fillMaxWidth().background(palette.barBg.copy(alpha = if (state.scene != null) KEY_AREA_SCENE_SCRIM_ALPHA else KEY_AREA_SCRIM_ALPHA)),
                ) {
                    if (modes.oneHand == KeyboardModes.OneHand.RIGHT) {
                        Box(modifier = Modifier.weight(1f - ONE_HAND_WIDTH_RATIO)) // 右手：左边留空
                    }
                    KeyboardArea(
                        state,
                        controller,
                        router,
                        catalog,
                        rareIndex,
                        symbolCategories,
                        kaomojiCategories,
                        onPickPet,
                        onClearUserWords,
                        modifier = Modifier.weight(
                            if (modes.oneHand == KeyboardModes.OneHand.OFF) 1f
                            else ONE_HAND_WIDTH_RATIO,
                        ),
                    )
                    if (modes.oneHand == KeyboardModes.OneHand.LEFT) {
                        Box(modifier = Modifier.weight(1f - ONE_HAND_WIDTH_RATIO)) // 左手：右边留空
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
    kaomojiCategories: () -> List<xyz.erik.opi.keyboard.KaomojiCategories.Category>,
    onPickPet: (PetMood) -> Unit,
    onClearUserWords: () -> Unit,
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
            initialCategory = state.symbolPageInitialCategory,
        )
        ImeState.View.SKIN -> SkinPanel(
            state.scene, state.skin, state::selectScene, state::selectSolidSkin,
            state::backToLetters, modifier,
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
        // 设置面板（2026-10-22 用户裁决）：齿轮直达，开关即时生效并落盘。
        ImeState.View.SETTINGS -> SettingsPanel(
            controller = controller,
            onClose = state::backToLetters,
            onClearUserWords = onClearUserWords,
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
                    onPickPet = onPickPet,
                    onClose = state::backToLetters,
                    onBackToNumber = state::openNumber,
                    initialTab = state.symbolInitialTab,
                    kaomojiCategories = kaomojiCategories,
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
                // density > 2.39 必现。折中取 96dp：结果网格约 46dp（可见可滚），
                // 搜索盘键高降到 24dp；根治需改 IME 窗口高度，本机无设备未实测
                //（复核：`adb shell wm density` ≥2.4 时看结果区是否可见）。
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
            // 9 键 / 笔画 的模式键**只切简/繁**（2026-10-15 用户裁决）：
            // 键面字不走 [modeKeyLabel] 的三档循环，改用 [jianFanModeKeyLabelOf]。
            // 26 键仍走三档循环（它没有独立的英文键，cycleMode 那条腿不能摘）。
            val jianFanLabel = jianFanModeKeyLabelOf(controller.mode)
            when (state.inputMode) {
                InputMode.PINYIN9, InputMode.BIG9 -> NineKeyKeyboard(
                    modifier = modifier,
                    // 弹层序号 1（分词格）：Nine 协议只收 2–9，引擎对 '1' 是空操作 ⇒ 直提。
                    onKey = { ch -> if (ch == "1") router.commitText("1") else router.handleKey(ch) },
                    onSpace = router::handleSpace,
                    onBackspace = router::handleBackspace,
                    onEnter = router::handleEnter,
                    // 9 键模式键只切简/繁（2026-10-15 裁决；笔画同）：不是 26 键/搜索叠盘
                    // 那条 cycleMode 中→繁→英循环。
                    onModeSwitch = state::toggleTraditional,
                    onSymbol = state::openSymbolPage,
                    // 重输（2026-10-10 用户裁决：去分词键、加「重输」）：清掉当前组合重来；
                    // 空缓冲是空操作（clear 幂等）。
                    onRetype = controller::clear,
                    // 分词键（2026-10-15；2026-10-23 订正）：有拼音 ⇒ 发 `'` 插音节边界；
                    // 空缓冲的 `'` 是引号不是分词符 ⇒ 打一个「1」（engine_punct.rs）。
                    onSegment = {
                        if (controller.buffer.isEmpty()) {
                            router.commitText("1")
                        } else {
                            router.handleKey("'")
                        }
                    },
                    // 数字键盘（2026-10-10 用户裁决：底行新增）——与 26 键的 123 同路。
                    onNumber = state::openNumber,
                    // 英文键盘：先按拼音上屏 pending、再切英文 26 键
                    //（2026-10-10 用户裁决「切英文默认是拼音，不要是候选词」）。
                    onEnglish = state::switchToEnglishKeyboard,
                    // 左侧一列：**只有读得到拼音时才替换**（2026-10-09 裁决「不输入、
                    // 回退、确定就重置」）——读数空就回标点列，不回退显示数字串
                    // （上一版的自作主张，用户明确不要）。
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
                    customPunct = state.customPunct,
                    onPickCustomPunct = state::selectCustomPunct,
                    commonSymbols = { catalog.common },
                    onCommitText = router::commitText,
                    onAlternative = { a -> if (a.direct) router.commitText(a.value) else router.handleKey(a.value) },
                )
                InputMode.PINYIN14 -> FourteenKeyKeyboard(
                    modifier = modifier,
                    onKey = router::handleKey,
                    onSpace = router::handleSpace,
                    onBackspace = router::handleBackspace,
                    onEnter = router::handleEnter,
                    // 底行「中英」：中文 ⇄ 英文（2026-10-24 版式；英文恒 26 键布局）。
                    onModeSwitch = state::toggleChineseEnglish,
                    onModeLongPress = state::openInputMode,
                    onSymbol = state::openSymbolPage,
                    onNumber = state::openNumber,
                    onShift = controller::shiftTap,
                    onShiftLongPress = controller::shiftLongPress,
                    shiftState = controller.shiftState,
                    onAlternative = { a -> if (a.direct) router.commitText(a.value) else router.handleKey(a.value) },
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
                // PINYIN26 / ENGLISH26 / SHUANGPIN / WUBI（RARE 走上面分支，这里只为穷尽）
                else -> QwertyKeyboard(
                    modifier = modifier,
                    onKey = router::handleKey,
                    onSpace = router::handleSpace,
                    onBackspace = router::handleBackspace,
                    onEnter = router::handleEnter,
                    // 26 键「英/中」一键直达（2026-10-25）；繁体走长按输入模式面板。
                    onModeSwitch = state::toggleChineseEnglish,
                    onNumber = state::openNumber,
                    // pinyin 模式 ⇧ 无意义且残留状态会泄漏进 English：传 null 禁用
                    onShift = if (shiftVisible) controller::shiftTap else null,
                    onShiftLongPress = if (shiftVisible) controller::shiftLongPress else null,
                    shiftState = if (shiftVisible) controller.shiftState else ShiftState.OFF,
                    modeLabel = label,
                    // 英文 26 键：顶部数字行 + 底行 123 → pro（2026-10-19）；搜索叠盘关闭。
                    showNumberRow = controller.mode == EngineMode.ENGLISH,
                    showProKey = controller.mode == EngineMode.ENGLISH,
                    proActive = state.englishPro,
                    onProToggle = state::toggleEnglishPro,
                    // 英文底行 `!?#`：进符号页（英文符号在符号页 ASCII 分类）。
                    onSymbol = state::openSymbolPage,
                    // 五笔 26 键（ads.jpg）：最左「拼」切拼音 26、空格键面「五笔」。
                    wubi = state.inputMode == InputMode.WUBI,
                    onSwitchToPinyin = { state.selectInputMode(InputMode.PINYIN26) },
                    onAlternative = { a -> if (a.direct) router.commitText(a.value) else router.handleKey(a.value) },
                    onModeLongPress = state::openInputMode,
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
 * **9 键 / 笔画**底行模式键的键面字（2026-10-15 用户裁决「简繁按钮就只切换简繁」，
 * 笔画同日裁决「统一成只切简繁」；14 键 2026-10-24 改版后底行是「中英」，不再用它）。
 * 与 [modeKeyLabelOf] 同一条语义 —— 键面 = **点它会切到哪** —— 只是把「英」那条腿
 * 摘掉：这两盘上它只在 中/繁 之间切（[ImeState.toggleTraditional]），进英文走各自
 * 底行那枚独立的 [英文] 键。故：拼音态 →「繁」、繁体态 →「中」。
 *
 * 兜底给「繁」：这两盘按 `state.inputMode` 分发，理论上只会拿到中文态；不返回空/不抛。
 * 纯函数，单测钉住（`KeyLabelTest`）。
 */
internal fun jianFanModeKeyLabelOf(mode: EngineMode): String =
    if (mode == EngineMode.TRADITIONAL) "中" else "繁"
