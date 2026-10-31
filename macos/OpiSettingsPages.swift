// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// OpiSettingsPages.swift —— 设置窗的常用 / 按键 / 外观 / 状态栏 / 符号与表情页。

import AppKit

extension OpiSettingsWindow {

    /// 设置页 popup 的显示顺序（与 `OpiScheme` 的线上编码无关）。
    /// 计算属性：Swift 扩展不能加存储属性。
    static var schemeChoices: [OpiScheme] {
        [.full, .shuangpin, .nine, .fourteen, .stroke, .wubi]
    }

    // ---------- 常用 ----------

    func buildCommon() {
        setPageHeight(560)
        let engine = OpiEngine.shared
        label("常用", y: 8, size: 18, bold: true)
        note("输入偏好：改动立即生效；重启后从 UserDefaults 自动套用。默认值真源在 Rust 引擎，不在此处。",
             y: 34)

        checkbox("学习（记住选词）", y: 82, on: engine.learnerEnabled(),
                 action: #selector(learnChanged(_:)))
        let punctBox = checkbox("中文标点（。，；：“”……）", y: 108,
                                on: engine.chinesePunctEnabled(),
                                action: #selector(punctChanged(_:)))
        punctBox.frame.size.width = 240
        checkbox("全角字符", x: 260, y: 108, on: engine.fullwidthState(),
                 action: #selector(fullwidthChanged(_:)))
        checkbox("模糊音", y: 134, on: engine.fuzzyEnabled(),
                 action: #selector(fuzzyMasterChanged(_:)))

        let names = engine.fuzzyPairNames()
        if names.isEmpty {
            note("（模糊音组名不可用：引擎未装载）", x: 24, y: 164, height: 20)
        } else {
            let mask = Int(engine.fuzzyPairsMask())
            for (i, name) in names.enumerated() {
                let b = checkbox(name, x: 24 + CGFloat(i % 2) * 220,
                                 y: 160 + CGFloat(i / 2) * 26,
                                 on: mask & (1 << i) != 0,
                                 action: #selector(fuzzyPairChanged(_:)), tag: i)
                b.frame.size.width = 200
            }
        }

        let after = 160 + CGFloat(max(1, (names.count + 1) / 2)) * 26 + 6
        checkbox("简拼", x: 24, y: after, on: engine.jianpinEnabled(),
                 action: #selector(jianpinChanged(_:)))
        checkbox("拼音纠错", x: 260, y: after, on: engine.correctionEnabled(),
                 action: #selector(correctionChanged(_:)))
        let assoc = checkbox("智能联想（提交后联想常用续接词句；默认关闭）", y: after + 26,
                             on: OpiSettingsStore.association,
                             action: #selector(associationChanged(_:)))
        assoc.frame.size.width = 360
        let brackets = checkbox("括号自动配对（默认开启）", x: 380, y: after + 26,
                                on: OpiSettingsStore.bracketPairing,
                                action: #selector(bracketPairingChanged(_:)))
        brackets.frame.size.width = 320
        brackets.toolTip = "打 ( 自动补 )，光标停在中间；宿主不支持光标回移时自动退回普通输入。"

        let schemeY = after + 68
        label("输入方案", x: 0, y: schemeY, w: 90, size: 12)
        let current = engine.currentScheme()
        let idx = Self.schemeChoices.firstIndex(of: current) ?? 0
        let scheme = popup(Self.schemeChoices.map { $0.title }, x: 96, y: schemeY - 3,
                           w: 150, selected: idx, action: #selector(schemeChanged(_:)))
        for (i, value) in Self.schemeChoices.enumerated() where !engine.isSchemeAvailable(value) {
            scheme.menu?.item(at: i)?.isEnabled = false
            scheme.menu?.item(at: i)?.toolTip = "\(value.title)词典未装载"
        }

        label("双拼布局", x: 270, y: schemeY, w: 80, size: 12)
        popup(OpiShuangpinLayout.names, x: 356, y: schemeY - 3, w: 160,
              selected: engine.shuangpinLayout(), action: #selector(layoutChanged(_:)))
        note("9/14 键与笔画属于输入方案（不是皮肤）；五笔码表未装载时菜单与这里都禁用。逐对模糊音会覆盖总开关，全不选 = 关闭。",
             y: schemeY + 38)
    }

    // ---------- 按键 ----------

    func buildKeys() {
        setPageHeight(520)
        label("按键", y: 8, size: 18, bold: true)
        note("硬件快捷键固定在 InputController.swift（与 Linux/Windows 同一张表）；本页只读。",
             y: 34)
        let rows = [
            "⌃ '        英文 ⇄ 拼音",
            "⌃ \\        拼音 ⇄ 符号模式",
            "⇧ Space   全角 ⇄ 半角",
            "⌃ ⇧ F      简体 ⇄ 繁体",
            "⌃ ⇧ Z      打开符号面板",
        ]
        for (i, text) in rows.enumerated() {
            label(text, y: 78 + CGFloat(i) * 28, size: 13)
        }

        label("虚拟键盘", y: 78 + CGFloat(rows.count) * 28 + 24, size: 14, bold: true)
        note("自绘 NSPanel，点键直接把键值喂给引擎（不注入系统按键）：不需要辅助功能授权，不抢焦点，硬件键盘不受影响。",
             y: 78 + CGFloat(rows.count) * 28 + 50, height: 34)
        button("打开虚拟键盘…", y: 78 + CGFloat(rows.count) * 28 + 90, w: 150,
               action: #selector(openKeyboard))
        note("布局：26 / 9 / 14 / 笔画；9 键含 123 数字面板。空缓冲时 ⌫ 只能删输入缓冲，不能删宿主文档字符（IMK 无删除出口）。",
             y: 78 + CGFloat(rows.count) * 28 + 128, height: 34)
        note("「Shift 轻按切换中英文」（ShiftToggle）：已接（2026-10-28，经 recognizedEvents 申领 flagsChanged）；iTerm / VS Code 这类不送修饰键事件的宿主仍请用 ⌃ '。",
             y: 78 + CGFloat(rows.count) * 28 + 166, height: 40)
    }

    @objc func openKeyboard() {
        OpiKeyboardPanel.shared.show()
    }

    // ---------- 外观 ----------

    func buildAppearance() {
        setPageHeight(560)
        label("外观", y: 8, size: 18, bold: true)
        note("11 款纯色 + 19 套动态插画：纯色只换配色，动态主题插画与配色一起换。只作用于 OPI 自绘面板（状态栏、虚拟键盘、表情/贴纸）；系统候选窗与设置窗本身保持 macOS 外观。",
             y: 34, height: 34)

        label("皮肤", y: 84, size: 13)
        let all = OpiSkinChoice.all
        popup(all.map { $0.title }, x: 96, y: 79, w: 180,
              selected: all.firstIndex(of: OpiTheme.currentChoice) ?? 0,
              action: #selector(skinChanged(_:)))
        label("当前：\(OpiTheme.currentChoice.title)", x: 290, y: 84, w: 160, size: 12,
              color: .secondaryLabelColor)
        if let scene = OpiTheme.scene, let img = scene.image {
            let iv = NSImageView(frame: NSRect(x: 470, y: 62, width: 220, height: 124))
            iv.image = img
            iv.imageScaling = .scaleProportionallyUpOrDown
            iv.wantsLayer = true
            iv.layer?.cornerRadius = 6
            iv.layer?.masksToBounds = true
            iv.layer?.borderWidth = 1
            iv.layer?.borderColor = NSColor.separatorColor.cgColor
            host?.addSubview(iv)
        }

        let palette = OpiTheme.palette
        let colors = [palette.bg, palette.fg, palette.hover, palette.line, palette.accent]
        let names = ["底色", "文字", "悬停", "边线", "强调"]
        for (i, color) in colors.enumerated() {
            let box = NSView(frame: NSRect(x: 96 + CGFloat(i) * 44, y: 118, width: 34, height: 34))
            box.wantsLayer = true
            box.layer?.backgroundColor = color.cgColor
            box.layer?.borderColor = NSColor.separatorColor.cgColor
            box.layer?.borderWidth = 1
            box.layer?.cornerRadius = 5
            host?.addSubview(box)
            label(names[i], x: 96 + CGFloat(i) * 44, y: 154, w: 40, size: 10,
                  color: .secondaryLabelColor)
        }

        checkbox("暗光模式（皮肤为「不改动」时用深色面板）", y: 194, on: OpiTheme.darkPanels,
                 action: #selector(darkChanged(_:)))
        checkbox("长辈模式（高对比黑白面板，优先级最高）", y: 220, on: OpiTheme.elderPanels,
                 action: #selector(elderChanged(_:)))
        note("暗光/长辈是 macOS 端本地显示模式（Android/Harmony 同源）；Linux 桌面没有这两项。",
             y: 252, height: 34)
        note("候选字号：Linux 由 classicui 的 Font 控制；macOS 的 IMKCandidates 不接受字号/皮肤设置，标记为不可用。",
             y: 294, height: 40)
        button("在状态栏预览", y: 348, w: 150, action: #selector(previewStatusBar))
        note("皮肤也可点状态栏 LOGO 循环切换（同 Linux）。", y: 382, height: 20)
    }

    @objc func skinChanged(_ sender: NSPopUpButton) {
        let all = OpiSkinChoice.all
        guard all.indices.contains(sender.indexOfSelectedItem) else { return }
        OpiTheme.select(all[sender.indexOfSelectedItem])
        OpiTheme.notifyChanged()
        rebuildPage()
    }

    @objc func darkChanged(_ sender: NSButton) {
        OpiTheme.darkPanels = sender.state == .on
        OpiTheme.notifyChanged()
        rebuildPage()
    }

    @objc func elderChanged(_ sender: NSButton) {
        OpiTheme.elderPanels = sender.state == .on
        OpiTheme.notifyChanged()
        rebuildPage()
    }

    @objc func previewStatusBar() {
        if OpiStatusBar.shared.isActive {
            OpiStatusBar.shared.refresh()
        } else {
            alert("状态栏只在 opi 输入源激活时显示", info: "切换到小欧输入法后再点一次；这是 2026-10-21 的明确要求。")
        }
    }

    // ---------- 状态栏 ----------

    func buildStatusBar() {
        setPageHeight(560)
        let bar = OpiBarSettings.load()
        label("状态栏", y: 8, size: 18, bold: true)
        note("悬浮状态栏只在 opi 输入源激活时出现（IMK Activate/Deactivate）。菜单「显示状态栏」与这里同源。",
             y: 34, height: 34)

        checkbox("显示状态栏", y: 82, on: !bar.hidden,
                 action: #selector(statusMasterChanged(_:)))

        label("常驻按钮", y: 118, size: 13, bold: true)
        let choices = OpiBarButton.allCases
        for (i, choice) in choices.enumerated() {
            let x = CGFloat(i % 2) * 250
            let y = 148 + CGFloat(i / 2) * 30
            let b = checkbox(choice.label, x: x, y: y, on: choice.resident(bar),
                             action: #selector(statusButtonChanged(_:)), tag: choice.rawValue)
            b.frame.size.width = 220
            b.toolTip = choice.hint
            if choice == .mode {
                b.isEnabled = false
                b.state = .on
                b.toolTip = "模式按钮恒常驻（Linux 无 StatusBarMode= 键）"
            }
            if choice == .enpro {
                b.toolTip = "englishpro 在 macOS 无控制面：按钮会显示但动作禁用，可在这里隐藏"
            }
        }

        let scaleY = 148 + CGFloat((choices.count + 1) / 2) * 30 + 12
        label("缩放", x: 0, y: scaleY, w: 60, size: 12)
        let scaleIndex = OpiBarMetrics.scaleChoices.firstIndex(of: bar.scale) ?? 2
        popup(OpiBarMetrics.scaleChoices.map { "\($0)%" }, x: 64, y: scaleY - 3,
              w: 100, selected: scaleIndex, action: #selector(statusScaleChanged(_:)))
        note("ENPRO：macOS 没有 fcitx5 的 englishpro 控制面 —— 不伪造该功能，按钮禁用并给出说明。VK 默认关（同 Linux StatusBarVk=False）。",
             y: scaleY + 38, height: 40)
    }

    @objc func statusMasterChanged(_ sender: NSButton) {
        var s = OpiBarSettings.load()
        s.hidden = sender.state == .off
        s.save()
        notifySettingsChanged()
    }

    @objc func statusButtonChanged(_ sender: NSButton) {
        guard let button = OpiBarButton(rawValue: sender.tag) else { return }
        var s = OpiBarSettings.load()
        s.setResident(button, sender.state == .on)
        s.save()
        notifySettingsChanged()
    }

    @objc func statusScaleChanged(_ sender: NSPopUpButton) {
        let choices = OpiBarMetrics.scaleChoices
        guard choices.indices.contains(sender.indexOfSelectedItem) else { return }
        var s = OpiBarSettings.load()
        s.scale = choices[sender.indexOfSelectedItem]
        s.save()
        notifySettingsChanged()
    }

    // ---------- 符号与表情 ----------

    func buildSymbols() {
        setPageHeight(500)
        label("符号与表情", y: 8, size: 18, bold: true)
        label(OpiSymbolPanel.catalogSummary(), y: 42)
        label(OpiEmojiPanel.catalogSummary(), y: 66)

        button("打开符号面板…", y: 104, w: 140, action: #selector(openSymbols))
        button("打开表情与贴纸…", x: 150, y: 104, w: 150, action: #selector(openEmoji))
        note("符号：22 类竖条 + 12×6 网格 + 搜索，点符号直接上屏并保持面板打开。表情：中文分组/搜索，12×6；颜文字 6×6。",
             y: 144, height: 40)

        let showOwn = checkbox("显示「贴纸」页（ShowOwn，默认关）", y: 196,
                               on: OpiSettingsStore.showOwn,
                               action: #selector(showOwnChanged(_:)))
        showOwn.frame.size.width = 300
        note("贴纸页来自 shared/pet（PNG+GIF）。点贴纸把 PNG+TIFF 与 GIF 原文件写进剪贴板，提示按 ⌘V 粘贴；不合成按键、不要求辅助功能授权。",
             y: 228, height: 40)
        note("表情/颜文字数据随包（data/raw/emoji.tsv、kaomoji.tsv）；贴纸映射随包（pet-map.tsv + stickers/、stickers-gif/）。缺文件时面板如实显示为空并写日志。",
             y: 276, height: 40)
        note("符号面板的控件是系统外观，不随皮肤改变；状态栏/虚拟键盘/表情贴纸面板随皮肤。",
             y: 324, height: 34)
    }

    @objc func showOwnChanged(_ sender: NSButton) {
        OpiSettingsStore.showOwn = sender.state == .on
        notifySettingsChanged()
        rebuildPage()
    }

    @objc func openSymbols() { OpiSymbolPanel.shared.show() }

    @objc func openEmoji() { OpiEmojiPanel.shared.show() }

    // ---------- 常用页动作 ----------

    @objc func learnChanged(_ sender: NSButton) {
        let on = sender.state == .on
        OpiEngine.shared.setLearner(on)
        OpiSettingsStore.set("Learn", on)
    }

    @objc func punctChanged(_ sender: NSButton) {
        let on = sender.state == .on
        OpiEngine.shared.setChinesePunct(on)
        OpiSettingsStore.set("ChinesePunct", on)
        notifySettingsChanged()
    }

    /// 全角只有 toggle 出口（没有 setter），按当前状态决定是否翻。
    @objc func fullwidthChanged(_ sender: NSButton) {
        let want = sender.state == .on
        if OpiEngine.shared.fullwidthState() != want {
            _ = OpiEngine.shared.toggleFullwidth()
        }
        OpiClientHub.shared.refreshState()
    }

    @objc func fuzzyMasterChanged(_ sender: NSButton) {
        let on = sender.state == .on
        OpiEngine.shared.setFuzzy(on)
        OpiSettingsStore.set("Fuzzy", on)
        // 总开关优先：清掉逐对覆盖，否则下次启动逐对值会把总开关盖回去。
        OpiSettingsStore.remove("FuzzyPairs")
        rebuildPage()
    }

    @objc func fuzzyPairChanged(_ sender: NSButton) {
        guard sender.tag >= 0, sender.tag < 32 else { return }
        let engine = OpiEngine.shared
        let names = engine.fuzzyPairNames()
        let mask = Int(engine.fuzzyPairsMask()) ^ (1 << sender.tag)
        let selected = names.enumerated().compactMap {
            mask & (1 << $0.offset) != 0 ? $0.element : nil
        }
        engine.setFuzzyPairs(selected)
        OpiSettingsStore.set("FuzzyPairs", selected.joined(separator: ","))
        OpiSettingsStore.set("Fuzzy", !selected.isEmpty)
        rebuildPage()
    }

    @objc func jianpinChanged(_ sender: NSButton) {
        let on = sender.state == .on
        OpiEngine.shared.setJianpin(on)
        OpiSettingsStore.set("Jianpin", on)
    }

    @objc func correctionChanged(_ sender: NSButton) {
        let on = sender.state == .on
        OpiEngine.shared.setCorrection(on)
        OpiSettingsStore.set("Correction", on)
    }

    @objc func associationChanged(_ sender: NSButton) {
        OpiSettingsStore.association = sender.state == .on
        OpiClientHub.shared.refreshState()
    }

    @objc func bracketPairingChanged(_ sender: NSButton) {
        OpiSettingsStore.bracketPairing = sender.state == .on
        BracketPairing.shared.reset() // 关掉后不再跟踪已有的那一对
    }

    @objc func schemeChanged(_ sender: NSPopUpButton) {
        let index = sender.indexOfSelectedItem
        guard Self.schemeChoices.indices.contains(index) else { return }
        let scheme = Self.schemeChoices[index]
        guard OpiEngine.shared.isSchemeAvailable(scheme) else {
            alert("\(scheme.title)词典未装载", info: "把对应 .opid 放进包的 Resources/dicts/ 或 ~/Library/Application Support/opi/。")
            rebuildPage()
            return
        }
        OpiSettingsStore.applyScheme(scheme)
        OpiClientHub.shared.refreshState()
        rebuildPage()
    }

    @objc func layoutChanged(_ sender: NSPopUpButton) {
        let index = sender.indexOfSelectedItem
        guard OpiShuangpinLayout.names.indices.contains(index) else { return }
        OpiEngine.shared.setShuangpinLayout(index)
        OpiSettingsStore.set("ShuangpinLayout", index)
    }
}
