// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 面板装配（2026-10-20 从 `KeyboardLayout.swift` 机械搬出 + 新增三面板）。
// 拆出的理由与 `KeyboardCandidateBar.swift` 相同：主文件压 500 行门禁。
//
// 八个面板（符号/表情/输入模式/生僻字/皮肤/键盘模式/关于/设置）**同位置、互斥显示**，
// 都占满键区；顶部 44pt 槽位在面板态恒为工具条（Android 同）。平铺候选另占满
// **整个键盘**（连顶部槽位一起盖住），见 `buildCandidateTiledPanel`。本文件只管
// 摆位与回调接线，数据与画法留在各面板自己的文件里。
//
// ⚠️ 本机无 UIKit，只在 Apple CI 真编译；从未上机运行。

import UIKit

extension KeyboardLayout {

    // MARK: - 工具条

    func buildToolbar() {
        toolbar.translatesAutoresizingMaskIntoConstraints = false
        addSubview(toolbar)
        NSLayoutConstraint.activate([
            toolbar.leadingAnchor.constraint(equalTo: leadingAnchor),
            toolbar.trailingAnchor.constraint(equalTo: trailingAnchor),
            toolbar.topAnchor.constraint(equalTo: composingBar.bottomAnchor),
            toolbar.heightAnchor.constraint(equalToConstant: 44),
        ])
        toolbar.onOpenEmoji = { [weak self] in self?.tap(.showEmoji) }
        toolbar.onOpenSkin = { [weak self] in self?.tap(.showSkin) }
        toolbar.onOpenInputMode = { [weak self] in self?.tap(.showModes) }
        toolbar.onOpenKeyboardMode = { [weak self] in self?.tap(.showKeyboardModes) }
        toolbar.onOpenAbout = { [weak self] in self?.tap(.showAbout) }
        toolbar.onOpenSettings = { [weak self] in self?.tap(.showSettings) }
        toolbar.onCloseKeyboard = { [weak self] in self?.tap(.closeKeyboard) }
        toolbar.onSwitchToChinese = { [weak self] in self?.tap(.cycleMode) }
    }

    private func tap(_ fn: KeyboardFunctionKey) {
        delegate?.keyboardLayout(self, didTapFunction: fn)
    }

    // MARK: - 响应式 / 释放

    /// 窄屏/矮屏降档（2026-10-23）：各面板按实际键区尺寸降列数/行高/表头高。
    /// 控制器在 `viewWillLayoutSubviews` 里按 `compact` 调用（见 KeyboardResponsive）。
    func setCompact(_ compact: Bool) {
        symbolPanel.setCompact(compact)
        emojiPanel.setCompact(compact)
        rarePanel.setCompact(compact)
        skinPanel.setCompact(compact)
        for view in descendants(of: self) {
            (view as? KeyboardPanelHeader)?.setCompact(compact)
        }
    }

    /// 离开面板层前的收尾（`returnToCurrentInputMode` 调）：表情页如果还在
    /// 搜索态，先退出搜索 —— 否则下次打开浮着搜索框，防抖还会往隐藏网格里写。
    func prepareToLeavePanels() {
        emojiPanel.exitSearchForClose()
    }

    /// 系统内存告警：隐藏着的面板把大数组/子视图全放掉（表情页 689 颗按钮是热点）。
    /// 可见的那个面板不动 —— 用户正看着，清掉会闪白。
    func releaseHeavyContentOnMemoryWarning() {
        if emojiPanel.isHidden { emojiPanel.releaseContent() }
        if symbolPanel.isHidden { symbolPanel.releaseContent() }
        if rarePanel.isHidden { rarePanel.releaseContent() }
    }

    // MARK: - 面板装配

    func buildPanels() {
        buildSymbolPanel()
        buildEmojiPanel()
        buildModePanel()
        buildRarePanel()
        buildSkinPanel()
        buildKeyboardModePanel()
        buildAboutPanel()
        buildSettingsPanel()
        buildCandidateTiledPanel()
        buildPunctuationPicker()
    }

    // MARK: - 面板数据转发

    /// 交给符号层的符号（来自 `OpiEngine.commonSymbols()`，由控制器在切到符号层时取）。
    /// 只做转发 —— 网格怎么画、空列表怎么提示，都在 `SymbolPanel`（那边有理由）。
    func setSymbols(_ list: [String]) {
        symbolPanel.setSymbols(list)
    }

    /// 交给表情层的**原始**数据（来自 `OpiEngine.emojiSymbols()` / `emojiGroups()`，
    /// 由控制器在切到表情层时取）。字形过滤与分类下标重算都在 `EmojiPanel` 里做
    /// （那边有理由），本层一个字都不判。
    func setEmojiBrowse(_ list: [String], groups: [OpiEmojiGroup]) {
        emojiPanel.showBrowse(list, groups: groups)
    }

    /// 交给表情层的搜索结果（来自 `OpiEngine.searchSymbols(keyword:)`）。
    /// `query` 传**去空白后**的查询词（控制器 trim）—— 空查询时面板画浏览内容。
    func setEmojiResults(_ results: [String], query: String) {
        emojiPanel.showResults(results, query: query)
    }

    /// 9 键左列自定义键的符号选择器（2026-10-24）：盖住键区，点一项就收。
    /// 选中只换键面并持久化（`setCustomPunctuation`）；上屏由自定义键自己直提。
    private func buildPunctuationPicker() {
        punctuationPicker.translatesAutoresizingMaskIntoConstraints = false
        punctuationPicker.isHidden = true
        punctuationPicker.onPick = { [weak self] text in
            // 只换键面 + 持久化（Android `onPickCustomPunct` 同）：选完不自动上屏，
            // 之后点该格才直提（长按可再换）。
            self?.setCustomPunctuation(text)
        }
        punctuationPicker.onClose = { [weak self] in
            self?.punctuationPicker.isHidden = true
        }
        keyArea.addSubview(punctuationPicker)
        NSLayoutConstraint.activate([
            punctuationPicker.leadingAnchor.constraint(equalTo: keyArea.leadingAnchor),
            punctuationPicker.trailingAnchor.constraint(equalTo: keyArea.trailingAnchor),
            punctuationPicker.topAnchor.constraint(equalTo: keyArea.topAnchor),
            punctuationPicker.bottomAnchor.constraint(equalTo: keyArea.bottomAnchor),
        ])
    }

    /// 控制器打开选择器时调（数据现取，空列表由选择器兜底）。
    func showPunctuationPicker(_ symbols: [String]) {
        punctuationPicker.show(symbols)
        punctuationPicker.isHidden = false
    }

    /// 平铺候选（2026-10-21）：左列钉读音、中间选词、右列四个动作。
    /// 全部转成控制器功能键 —— 本文件只管摆位与接线。
    private func buildCandidateTiledPanel() {
        candidateTiledPanel.onSelect = { [weak self] index in
            guard let self else { return }
            self.delegate?.keyboardLayout(self, didSelectCandidateAt: index)
        }
        candidateTiledPanel.onBack = { [weak self] in self?.tap(.hideTiledCandidates) }
        candidateTiledPanel.onBackspace = { [weak self] in self?.tap(.tiledBackspace) }
        candidateTiledPanel.onBackspaceRepeat = { [weak self] in self?.tap(.tiledBackspaceRepeat) }
        candidateTiledPanel.onRepeatEnded = { [weak self] in self?.tap(.tiledRepeatEnded) }
        candidateTiledPanel.onRetype = { [weak self] in self?.tap(.retype) }
        candidateTiledPanel.onSpace = { [weak self] in self?.tap(.tiledSpace) }
        candidateTiledPanel.onPinReading = { [weak self] reading in
            self?.tap(.pinReading(reading))
        }
        addPanel(candidateTiledPanel)
    }

    /// 面板与键位行同位置（占满键区）。`rebuildKeys` 负责谁亮谁灭。
    private func addPanel(_ panel: UIView) {
        panel.translatesAutoresizingMaskIntoConstraints = false
        keyArea.addSubview(panel)
        NSLayoutConstraint.activate([
            panel.leadingAnchor.constraint(equalTo: keyArea.leadingAnchor),
            panel.trailingAnchor.constraint(equalTo: keyArea.trailingAnchor),
            panel.topAnchor.constraint(equalTo: keyArea.topAnchor),
            panel.bottomAnchor.constraint(equalTo: keyArea.bottomAnchor),
        ])
        panel.isHidden = true
    }

    /// 符号层：点击**直提**（`.commitText`，不过引擎）；返回回当前模式键面。
    private func buildSymbolPanel() {
        symbolPanel.onPick = { [weak self] text in self?.tap(.commitText(text)) }
        symbolPanel.onClose = { [weak self] in self?.tap(.showLetters) }
        addPanel(symbolPanel)
    }

    /// 表情层。`onPick` → `.commitText`；`onBack` → `.showLetters`；
    /// `onSearch` → 既有的搜索词委托（防抖与查询归控制器）。
    private func buildEmojiPanel() {
        emojiPanel.onPick = { [weak self] text in self?.tap(.commitText(text)) }
        emojiPanel.onBack = { [weak self] in self?.tap(.showLetters) }
        emojiPanel.onSearch = { [weak self] query in
            guard let self else { return }
            self.delegate?.keyboardLayout(self, didSearchEmoji: query)
        }
        addPanel(emojiPanel)
    }

    /// 输入模式菜单：选中由控制器走 `applyInputMode`（换引擎 + 换键面 + 关面板）。
    private func buildModePanel() {
        modePanel.onSelect = { [weak self] mode in self?.tap(.selectInputMode(mode)) }
        modePanel.onClose = { [weak self] in self?.tap(.showLetters) }
        addPanel(modePanel)
    }

    /// 生僻字面板：直提；关闭回键面。
    private func buildRarePanel() {
        rarePanel.onCommit = { [weak self] ch in self?.tap(.commitText(ch)) }
        rarePanel.onClose = { [weak self] in self?.tap(.showLetters) }
        addPanel(rarePanel)
    }

    /// 皮肤：**选中不关面板**（Android 同：方便连着换着比）。
    private func buildSkinPanel() {
        skinPanel.onSelect = { [weak self] s in self?.tap(.selectSkin(s)) }
        skinPanel.onClose = { [weak self] in self?.tap(.showLetters) }
        addPanel(skinPanel)
    }

    /// 键盘模式：点选项也不关面板。
    private func buildKeyboardModePanel() {
        keyboardModePanel.onToggleTraditional = { [weak self] in self?.tap(.toggleTraditional) }
        keyboardModePanel.onUpdate = { [weak self] m in self?.tap(.updateKeyboardModes(m)) }
        keyboardModePanel.onClose = { [weak self] in self?.tap(.showLetters) }
        addPanel(keyboardModePanel)
    }

    /// 关于：链接由控制器复制 + 尝试打开（扩展环境可能不允许直接开）。
    private func buildAboutPanel() {
        aboutPanel.onOpenURL = { [weak self] url in self?.tap(.openURL(url)) }
        aboutPanel.onClose = { [weak self] in self?.tap(.showLetters) }
        addPanel(aboutPanel)
    }

    /// 设置（2026-10-21）：引擎开关立即回传控制器；皮肤/键盘模式/关于用跳转，
    /// 不重复画一套（三块面板各有自己的状态与返回路径）。
    private func buildSettingsPanel() {
        settingsPanel.onClose = { [weak self] in self?.tap(.showLetters) }
        settingsPanel.onChange = { [weak self] s in self?.tap(.updateSettings(s)) }
        settingsPanel.onUpdateModes = { [weak self] m in self?.tap(.updateKeyboardModes(m)) }
        settingsPanel.onOpenSkin = { [weak self] in self?.tap(.showSkin) }
        settingsPanel.onOpenKeyboardModes = { [weak self] in self?.tap(.showKeyboardModes) }
        settingsPanel.onOpenAbout = { [weak self] in self?.tap(.showAbout) }
        settingsPanel.onOpenURL = { [weak self] url in self?.tap(.openURL(url)) }
        // 用户词库（2026-10-23）：面板只负责确认与提示，真清理由控制器做
        // （引擎单例在控制器手里，布局层碰不到）。
        settingsPanel.onClearUserWords = { [weak self] in self?.tap(.clearUserWords) }
        addPanel(settingsPanel)
    }

    // MARK: - 顶部槽位 / 皮肤 / 单手

    /// 顶部 44pt 槽位：候选栏替换工具条的条件与 Android `ImeScreen` 同一句
    /// （`candidatesShowable && !englishIdle`，化简后 = 有缓冲或有候选）。
    /// 面板层恒显示工具条。
    func setTopBar(candidatesVisible: Bool, english: Bool) {
        wantsCandidateBar = candidatesVisible
        self.english = english
        updateTopBarVisibility()
    }

    /// `rebuildKeys` 与 `setTopBar` 共用一处可见性（别在两处各判一遍）。
    func updateTopBarVisibility() {
        // 平铺候选盖住整个键盘（含顶部 44pt 槽位）—— 与 Android
        // `CandidateTiledView` 一致；右列「返回」是平铺态唯一收拢入口。
        let tiled = keyLayer == .candidatesTiled
        let showCandidates = !tiled && wantsCandidateBar && !isPanelLayer
        candidateBar.isHidden = !showCandidates
        toolbar.isHidden = tiled || showCandidates
        toolbar.setActive(!toolbar.isHidden)
        toolbar.setEnglish(english)
        showChineseButton(english && showCandidates)
        // ⌄ 只在键面候选栏出现；平铺态用右列「返回」，不重复入口。
        setExpandVisible(!candidateTexts.isEmpty && showCandidates)
        updatePetStripVisibility()
    }

    /// 顶部宠物条只在**空闲键面**转：不在面板/平铺里、没有候选、没有组合串。
    /// 与 Android `PetStrip` 的位置判据（`idlePetStrip()`）同义。
    func updatePetStripVisibility() {
        // 只在字母键面空闲态出现：数字层（123）与面板态都不画（Android `idlePetStrip`
        // 的 view === QWERTY 同义）。
        petStrip.setActive(
            keyLayer == .letters && !isPanelLayer
                && candidateTexts.isEmpty && lastComposingText.isEmpty
        )
    }

    func setSkin(_ s: SkinOption) {
        skin = s
        if keyLayer == .skins { skinPanel.show(current: s) }
    }

    func setKeyboardModes(_ m: KeyboardModes, traditional: Bool) {
        keyboardModes = m
        self.traditional = traditional
        if keyLayer == .keyboardModes {
            keyboardModePanel.show(traditional: traditional, modes: m)
        }
        if keyLayer == .settings { settingsPanel.updateModes(m) }
    }

    /// 换肤 + 长辈字号 + 单手布局。控制器在启动、切皮肤、切键盘模式后调。
    func applyTheme(_ p: KeyboardPalette, fontScale: CGFloat) {
        palette = p
        self.fontScale = fontScale
        backgroundColor = p.toolbarBg
        // 深色皮肤下让系统控件（旧面板的 label/背景）也切深色 —— 否则候选/表情页
        // 会是「深底黑字」。本端旧面板没有逐控件接色板，这是最小的补救。
        overrideUserInterfaceStyle = p.isDark ? .dark : .light
        candidateBar.backgroundColor = p.barBg
        keyArea.backgroundColor = p.barBg
        composingBar.applyTheme(p, fontScale: fontScale)
        petStrip.applyTheme(p, fontScale: fontScale)
        toolbar.applyTheme(p)
        skinPanel.applyTheme(p)
        keyboardModePanel.applyTheme(p)
        aboutPanel.applyTheme(p)
        modePanel.applyTheme(p)
        settingsPanel.applyTheme(p)
        // 符号面板带自己的表头，接完整色板；其余旧面板只统一底色，
        // 内部控件仍用系统色（README 记为已知差异）。
        symbolPanel.applyTheme(p)
        emojiPanel.backgroundColor = p.panelBg
        modePanel.backgroundColor = p.panelBg
        rarePanel.backgroundColor = p.panelBg
        chineseButton.setTitleColor(p.barDimFg.withAlphaComponent(0.6), for: .normal)
        readingColumn.applyTheme(p, fontScale: fontScale)
        wordColumn.applyTheme(p, fontScale: fontScale)
        punctuationPicker.applyTheme(p, fontScale: fontScale)
        candidateTiledPanel.applyTheme(p, fontScale: fontScale)
        // 面板态下键位行与顶栏都隐藏：别为一次换肤递归走几十个不可见的 KeyButton。
        if !isPanelLayer { applyKeyTheme() }
        applyOneHand(keyboardModes.oneHand)
    }

    /// 单手模式：键区收窄到 0.76 并靠左/靠右（Android `fillMaxWidth(0.76f)` 同值）。
    /// 顶部槽位不收窄 —— 与 Android 的「只有键区偏移」一致。
    func applyOneHand(_ hand: KeyboardModes.OneHand) {
        keyAreaLeading?.isActive = false
        keyAreaTrailing?.isActive = false
        keyAreaWidth?.isActive = false
        // 平铺态键区顶到键盘顶部（顶部 44pt 槽位被平铺盖住）。
        let fullHeight = keyLayer == .candidatesTiled
        keyAreaTop?.isActive = !fullHeight
        keyAreaTopFull?.isActive = fullHeight
        // 面板（皮肤/输入模式/键盘模式/关于…）占满整块键区；单手只收窄**键面**。
        // 此前 panel 层也被收窄 → 单手开启后页面错乱（2026-10-21 用户报障）。
        switch isPanelLayer ? .off : hand {
        case .off:
            keyAreaLeading?.isActive = true
            keyAreaTrailing?.isActive = true
        case .left:
            keyAreaLeading?.isActive = true
            keyAreaWidth?.isActive = true
        case .right:
            keyAreaTrailing?.isActive = true
            keyAreaWidth?.isActive = true
        }
        setNeedsLayout()
    }

    /// 把当前色板刷到键与候选栏按钮上（新重建的键默认是系统色）。
    func applyKeyTheme() {
        for view in descendants(of: rowsStack) {
            (view as? KeyButton)?.applyTheme(palette, fontScale: fontScale)
        }
        for view in descendants(of: candidateBar) {
            (view as? BarButton)?.applyTheme(palette, fontScale: fontScale)
        }
    }

    private func descendants(of view: UIView) -> [UIView] {
        view.subviews + view.subviews.flatMap { descendants(of: $0) }
    }
}
