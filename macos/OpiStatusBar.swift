// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// OpiStatusBar.swift —— 可显示/隐藏的悬浮状态栏（Linux 状态栏的 AppKit 等价物）。
//
// 按钮顺序**逐项**对齐 desktop/…/SkinAndStatus.kt 的 `StatusBarButton`：
//   LOGO / 中英 / 标点 / 简繁 / ENPRO / 模式 / 全角 / 软键盘 / 符号 / 设置
// 面也照 Linux：中/英、。，/.,、简/繁、拼/五/en、月相（●/◐）、键盘、Ω、四格。
// LOGO 是项目宠物贴纸（透明底 idle.png），单击循环下一款皮肤（同 Linux）。
//
// ENPRO（fcitx5 的 englishpro 控制面）在 macOS **没有 C ABI，也没有 InputMethodKit
// 等价物**：本文件不伪造它 —— 按钮照位置显示但禁用（灰、tooltip 说明），设置里可
// 把它整个隐藏（默认仍常驻，与 Linux 的 `StatusBarEnPro=True` 同缺省）。
//
// VK 可做：自绘 NSPanel（OpiKeyboardPanel）把点击直接喂给引擎，不注入系统按键、
// 不需要辅助功能授权、不影响硬件键盘。Linux 的 VK 默认关（StatusBarVk=False），
// 这里同缺省。
//
// 显示条件：master 未隐藏 + 至少一个按钮常驻 + **opi 输入源正处于激活态**
// （InputController 的 activateServer/deactivateServer 驱动）。这最后一条是用户
// 2026-10-21 的明确要求，Linux 桌面版对 Windows 也是 requireActive。
// 不使用定时轮询：macOS 的设置窗/菜单/热键都在同一进程，改动后直接通知刷新。

import AppKit

// ---------- 按钮模型 ----------

enum OpiBarButton: Int, CaseIterable {
    case logo = 0
    case lang
    case punct
    case trad
    case enpro
    case mode
    case fullwidth
    case vk
    case symbols
    case settings

    var label: String {
        switch self {
        case .logo: return "logo"
        case .lang: return "中英文切换"
        case .punct: return "中英标点"
        case .trad: return "简繁转换"
        case .enpro: return "英文专业"
        case .mode: return "模式"
        case .fullwidth: return "全半角"
        case .vk: return "软键盘"
        case .symbols: return "符号大全"
        case .settings: return "设置"
        }
    }

    var hint: String {
        switch self {
        case .logo: return "品牌标识兼皮肤切换；单击换下一款皮肤"
        case .lang: return "中文 ⇄ 英文（与 ⌃' 同判）"
        case .punct: return "中文标点（。，）⇄ 英文标点（.,）"
        case .trad: return "简体 ⇄ 繁体（与 ⌃⇧F 同判）"
        case .enpro: return "英文普通 ⇄ 专业（macOS 没有 englishpro 控制面，已禁用）"
        case .mode: return "拼音 → 五笔 → 英文26键 循环"
        case .fullwidth: return "全角 ⇄ 半角"
        case .vk: return "显示/隐藏自绘虚拟键盘"
        case .symbols: return "打开符号与表情面板"
        case .settings: return "打开 OPI 设置"
        }
    }

    /// MODE 恒常驻（Linux 没有 `StatusBarMode=` 键）；ENPRO 的常驻位可拨但动作禁用。
    func resident(_ s: OpiBarSettings) -> Bool {
        switch self {
        case .logo: return s.logo
        case .lang: return s.lang
        case .punct: return s.punct
        case .trad: return s.trad
        case .enpro: return s.enpro
        case .mode: return true
        case .fullwidth: return s.fullwidth
        case .vk: return s.vk
        case .symbols: return s.symbols
        case .settings: return s.settings
        }
    }

    static func visible(_ s: OpiBarSettings) -> [OpiBarButton] {
        guard !s.hidden else { return [] }
        return allCases.filter { $0.resident(s) }
    }
}

extension OpiBarSettings {
    mutating func setResident(_ button: OpiBarButton, _ on: Bool) {
        switch button {
        case .logo: logo = on
        case .lang: lang = on
        case .punct: punct = on
        case .trad: trad = on
        case .enpro: enpro = on
        case .mode: break
        case .fullwidth: fullwidth = on
        case .vk: vk = on
        case .symbols: symbols = on
        case .settings: settings = on
        }
    }
}

// ---------- 窗口 ----------

/// 状态栏不抢焦点（Linux `focusable=false` 的同义）：点击不应把用户的输入框弄丢焦点。
final class OpiStatusPanel: NSPanel {
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}

/// 必须继承 NSObject：NotificationCenter 的 selector 观察与 `#selector` 动作
/// 都要求观察者/目标能被 ObjC 运行时看见。纯 Swift 类会在编译期就被拦下。
final class OpiStatusBar: NSObject {
    static let shared = OpiStatusBar()

    private var panel: OpiStatusPanel?
    private var logoButton: OpiHoverButton?
    private var noticeLabel: NSTextField?
    private var buttons: [OpiBarButton: OpiHoverButton] = [:]
    private var active = false
    private var signature = ""
    private var noticeText: String?
    private var noticeTimer: Timer?

    private override init() {
        super.init()
        NotificationCenter.default.addObserver(
            self, selector: #selector(externalRefresh(_:)),
            name: .opiSettingsChanged, object: nil)
        NotificationCenter.default.addObserver(
            self, selector: #selector(externalRefresh(_:)),
            name: .opiThemeChanged, object: nil)
        NotificationCenter.default.addObserver(
            self, selector: #selector(screenChanged(_:)),
            name: NSApplication.didChangeScreenParametersNotification, object: nil)
    }

    var isActive: Bool { active }

    // ---------- 公开入口 ----------

    /// IMK 生命周期驱动：只有 opi 输入源激活时才显示。
    func setActive(_ on: Bool) {
        onMain {
            self.active = on
            if on {
                self.refreshOnMain()
            } else {
                self.panel?.orderOut(nil)
            }
        }
    }

    func refresh() {
        onMain { self.refreshOnMain() }
    }

    /// 菜单「显示状态栏」与设置页 master 开关共用。
    func setHidden(_ hidden: Bool) {
        var s = OpiBarSettings.load()
        s.hidden = hidden
        s.save()
        NotificationCenter.default.post(name: .opiSettingsChanged, object: nil)
        refresh()
    }

    func toggleHidden() {
        setHidden(!OpiBarSettings.load().hidden)
    }

    // NotificationCenter 的 selector 形态要求带一个 Notification 参数。
    @objc private func externalRefresh(_ note: Notification) { refresh() }
    @objc private func screenChanged(_ note: Notification) { refresh() }

    // ---------- 刷新 ----------

    private func onMain(_ body: @escaping () -> Void) {
        if Thread.isMainThread { body() } else { DispatchQueue.main.async(execute: body) }
    }

    private func refreshOnMain() {
        let settings = OpiBarSettings.load()
        let shown = OpiBarButton.visible(settings)
        guard active, !shown.isEmpty else {
            panel?.orderOut(nil)
            return
        }
        let palette = OpiTheme.palette
        let sig = shown.map { String($0.rawValue) }.joined(separator: ",")
            + "|\(settings.scale)|\(OpiTheme.skin.rawValue)|\(OpiTheme.scene?.rawValue ?? "")"
            + "|\(OpiTheme.darkPanels)|\(OpiTheme.elderPanels)"

        if panel == nil || sig != signature {
            rebuild(shown: shown, settings: settings, palette: palette, signature: sig)
        } else {
            applyTheme(palette)
            updateButtonTitles(scale: CGFloat(settings.scale) / 100.0)
        }
        layout(shown: shown, scale: settings.scale)
        position()
        updateNotice(scale: CGFloat(settings.scale) / 100.0)
        panel?.orderFrontRegardless()
    }

    private func rebuild(shown: [OpiBarButton], settings: OpiBarSettings,
                         palette: OpiPalette, signature: String) {
        let m = OpiBarMetrics.make(scale: settings.scale)
        let p = ensurePanel(width: m.width(buttonCount: shown.count), height: m.height)
        guard let root = p.contentView as? OpiFlippedView else { return }
        root.subviews.forEach { $0.removeFromSuperview() }

        buttons = [:]
        for button in shown {
            let b = OpiHoverButton(frame: .zero)
            b.tag = button.rawValue
            b.target = self
            b.action = #selector(buttonClicked(_:))
            b.highlightWhenOn = (button == .vk)
            b.toolTip = "\(button.label)：\(button.hint)"
            b.baseFontSize = 14 * m.scale
            b.normalColor = palette.fg
            b.apply(palette)
            if button == .logo {
                // 透明底的宠物贴纸；找不到时退回一个「欧」字（面板照常可用）。
                if let url = petLogoURL(), let image = NSImage(contentsOf: url) {
                    b.image = image
                    b.imagePosition = .imageOnly
                    b.imageScaling = .scaleProportionallyUpOrDown
                } else {
                    b.setThemedTitle("欧", color: palette.accent,
                                     size: 15 * m.scale, bold: true)
                }
                b.toolTip = "小欧：单击换下一款皮肤"
                logoButton = b
            }
            root.addSubview(b)
            buttons[button] = b
        }

        let notice = NSTextField(labelWithString: "")
        notice.frame = .zero
        notice.alignment = .center
        notice.font = .systemFont(ofSize: 11 * m.scale)
        notice.textColor = palette.accent
        notice.isHidden = true
        root.addSubview(notice)
        noticeLabel = notice

        self.signature = signature
        applyTheme(palette)
        updateButtonTitles(scale: m.scale)
    }

    private func ensurePanel(width: CGFloat, height: CGFloat) -> OpiStatusPanel {
        if let panel {
            panel.setContentSize(NSSize(width: width, height: height))
            return panel
        }
        let p = OpiStatusPanel(contentRect: NSRect(x: 0, y: 0, width: width, height: height),
                               styleMask: [.borderless, .nonactivatingPanel],
                               backing: .buffered, defer: false)
        p.title = "小欧输入法 状态栏"
        p.isOpaque = false
        p.backgroundColor = .clear
        p.hasShadow = true
        p.level = .floating
        p.isFloatingPanel = true
        p.hidesOnDeactivate = false
        p.isReleasedWhenClosed = false
        p.becomesKeyOnlyIfNeeded = true
        p.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        let root = OpiFlippedView(frame: NSRect(x: 0, y: 0, width: width, height: height))
        root.autoresizingMask = [.width, .height]
        p.contentView = root
        panel = p
        return p
    }

    private func applyTheme(_ palette: OpiPalette) {
        if let root = panel?.contentView { OpiTheme.style(content: root, palette: palette) }
        for b in buttons.values { b.apply(palette) }
        logoButton?.apply(palette)
        noticeLabel?.textColor = palette.accent
    }

    private func updateButtonTitles(scale: CGFloat) {
        let engine = OpiEngine.shared
        let mode = engine.mode()
        let palette = OpiTheme.palette
        let buttonSide = 30 * scale // OpiBarMetrics.btn；面内容由 OpiBarIcons 收到 0.6。

        /// 图标按钮：清掉标题、换成模板图（颜色随皮肤）。
        func useImage(_ b: OpiHoverButton, _ image: NSImage) {
            b.setThemedTitle("")
            b.image = image
            b.imagePosition = .imageOnly
            b.imageScaling = .scaleNone
        }
        func useText(_ b: OpiHoverButton) {
            b.image = nil
            b.imagePosition = .noImage
        }

        for (kind, b) in buttons {
            guard kind != .logo else { continue }
            switch kind {
            case .punct:
                useImage(b, OpiBarIcons.punctPair(chinese: engine.chinesePunctEnabled(),
                                                  buttonSide: buttonSide, color: palette.fg))
            case .fullwidth:
                useImage(b, OpiBarIcons.moon(fullwidth: engine.fullwidthState(),
                                             buttonSide: buttonSide, color: palette.fg))
            case .vk:
                useImage(b, OpiBarIcons.keyboard(buttonSide: buttonSide, color: palette.fg))
            case .settings:
                useImage(b, OpiBarIcons.settings(buttonSide: buttonSide, color: palette.fg))
            case .enpro:
                // 与 Linux 同面（en 主字 + pro 角标），但动作禁用；设置里可隐藏。
                useImage(b, OpiBarIcons.modeFace(main: "en", corner: "pro",
                                                 buttonSide: buttonSide, color: palette.fg))
            case .mode where mode == .english:
                // Linux 的 en26 键帽：主字 en + 右下角 26。
                useImage(b, OpiBarIcons.modeFace(main: "en", corner: "26",
                                                 buttonSide: buttonSide, color: palette.fg))
            default:
                useText(b)
                let title: String
                switch kind {
                case .lang: title = mode == .english ? "英" : "中"
                case .trad: title = mode == .traditional ? "繁" : "简"
                case .mode:
                    title = engine.schemeCode() == OpiScheme.wubi.rawValue ? "五" : "拼"
                case .symbols: title = "Ω"
                default: title = ""
                }
                b.setThemedTitle(title, color: palette.fg,
                                 size: (kind == .lang || kind == .trad || kind == .mode)
                                    ? 14 * scale : 16 * scale,
                                 bold: kind == .mode)
            }
            b.isEnabled = kind != .enpro
            b.alphaValue = kind == .enpro ? 0.45 : 1.0
            b.highlightWhenOn = (kind == .vk)
            b.state = (kind == .vk && OpiKeyboardPanel.shared.isVisible) ? .on : .off
        }
        logoButton?.apply(palette)
    }

    private func layout(shown: [OpiBarButton], scale: Int) {
        guard let panel, let root = panel.contentView as? OpiFlippedView else { return }
        let m = OpiBarMetrics.make(scale: scale)
        let width = m.width(buttonCount: shown.count)
        panel.setContentSize(NSSize(width: width, height: m.height))
        root.frame = NSRect(x: 0, y: 0, width: width, height: m.height)
        for (i, kind) in shown.enumerated() {
            let x = m.pad + CGFloat(i) * (m.btn + m.gap)
            let y = (m.height - m.btn) / 2
            buttons[kind]?.frame = NSRect(x: x, y: y, width: m.btn, height: m.btn)
        }
        noticeLabel?.frame = NSRect(x: m.pad, y: 0, width: width - 2 * m.pad, height: m.height)
    }

    /// Linux 位置：屏幕底部居中、距底 128（`barPosition` 的 `screen.height - 128`）。
    private func position() {
        guard let panel else { return }
        guard let screen = NSScreen.main ?? NSScreen.screens.first else { return }
        let vf = screen.visibleFrame
        let x = vf.midX - panel.frame.width / 2
        let y = vf.minY + 128
        panel.setFrameOrigin(NSPoint(x: x, y: y))
    }

    private func updateNotice(scale: CGFloat) {
        let hasNotice = noticeText != nil
        for b in buttons.values { b.isHidden = hasNotice }
        logoButton?.isHidden = hasNotice
        noticeLabel?.isHidden = !hasNotice
        noticeLabel?.stringValue = noticeText ?? ""
        noticeLabel?.font = .systemFont(ofSize: 11 * scale)
        noticeLabel?.textColor = OpiTheme.palette.accent
    }

    /// 失败必须看得见（Linux 的 notice 行同义）。4 秒后回到按钮列。
    private func showNotice(_ text: String) {
        noticeText = text
        noticeTimer?.invalidate()
        noticeTimer = Timer.scheduledTimer(withTimeInterval: 4, repeats: false) { [weak self] _ in
            self?.noticeText = nil
            self?.refresh()
        }
        refresh()
    }

    // ---------- 动作 ----------

    @objc private func buttonClicked(_ sender: NSButton) {
        guard let button = OpiBarButton(rawValue: sender.tag) else { return }
        perform(button)
    }

    func perform(_ button: OpiBarButton) {
        let engine = OpiEngine.shared
        switch button {
        case .logo:
            OpiTheme.cycleSkin()
        case .lang:
            OpiClientHub.shared.commitPendingBeforeModeSwitch()
            engine.switchMode(engine.mode() == .english ? .pinyin : .english)
            OpiClientHub.shared.refreshState()
        case .punct:
            engine.toggleChinesePunct()
        case .trad:
            // 成对切换走保缓冲的 `opi_switch_language`（同 iOS `toggleTraditional`）。
            engine.switchLanguage(engine.mode() == .traditional ? .pinyin : .traditional,
                                  engine.currentScheme())
            OpiClientHub.shared.refreshState()
        case .enpro:
            showNotice("英文专业（englishpro）在 macOS 没有控制面 —— 本键已禁用")
            return
        case .mode:
            cycleMode()
        case .fullwidth:
            engine.toggleFullwidth()
        case .vk:
            OpiKeyboardPanel.shared.toggle()
        case .symbols:
            OpiSymbolPanel.shared.show()
        case .settings:
            OpiSettingsWindow.shared.show(page: .common)
        }
        refresh()
        NotificationCenter.default.post(name: .opiStateChanged, object: nil)
    }

    /// 拼音 → 五笔 → 英文26键 → 拼音（Linux `cycleMode`）。五笔码表没装载时
    /// **如实提示并保持原档**，不把「切了但没候选」画成成功。
    private func cycleMode() {
        let engine = OpiEngine.shared
        if engine.mode() == .english {
            OpiSettingsStore.applyScheme(.full)
            OpiClientHub.shared.commitPendingBeforeModeSwitch()
            engine.switchMode(.pinyin)
            OpiClientHub.shared.refreshState()
            return
        }
        if engine.schemeCode() == OpiScheme.wubi.rawValue {
            OpiClientHub.shared.commitPendingBeforeModeSwitch()
            engine.switchMode(.english)
            OpiClientHub.shared.refreshState()
            return
        }
        guard engine.isSchemeAvailable(.wubi) else {
            showNotice("五笔码表未装载 —— 模式停在拼音；请把 wubi86.opid 放进词库目录")
            return
        }
        OpiClientHub.shared.commitPendingBeforeModeSwitch()
        engine.switchMode(.pinyin)
        OpiSettingsStore.applyScheme(.wubi)
        OpiClientHub.shared.refreshState()
    }

    private func petLogoURL() -> URL? {
        Bundle.main.url(forResource: "idle", withExtension: "png",
                        subdirectory: "pet/stickers")
    }
}
