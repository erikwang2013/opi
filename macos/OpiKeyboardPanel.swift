// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// OpiKeyboardPanel.swift —— 自绘虚拟键盘（非激活浮动 NSPanel）。
//
// Linux 桌面的虚拟键盘（VirtualKeyboard.kt）是 26 键 + 底行「符/空格/回车/退格」，
// 按键用 X11/TSF 注入系统键。macOS **不注入系统键**：点击直接把键值送进
// `OpiClientHub.virtualKey` → 引擎 → 当前宿主客户端。这样
//   * 不需要辅助功能授权；
//   * 面板不抢焦点（`.nonactivatingPanel`），硬件键盘输入完全不受影响；
//   * 上屏仍然经过引擎的标点/全角/候选逻辑，和真实按键同一条路。
//
// 键面：26 / 9 / 14 / 笔画 四档（9/14/笔画取自 Android 对应方案，Linux 桌面只有
// 26 键）。9 键的 123 会进数字面板；「重输」= `opi_clear`。
//
// ⚠️ 平台限制（README 同记）：空缓冲时按 ⌫ 不能删除宿主文档里的字符 ——
// InputMethodKit 没有「向前删除客户端文本」的出口。这里只删输入缓冲，并给出提示。
// 回车在空缓冲时提交 "\n"，也不会触发宿主窗口的默认按钮。

import AppKit

enum OpiKeyboardLayout: Int, CaseIterable {
    case qwerty = 0
    case nine
    case fourteen
    case stroke
    case numbers

    var title: String {
        switch self {
        case .qwerty: return "26"
        case .nine: return "9"
        case .fourteen: return "14"
        case .stroke: return "笔画"
        case .numbers: return "123"
        }
    }

    /// 头部只列四档；numbers 只经 9 键底行的 123 进入。
    static let picker: [OpiKeyboardLayout] = [.qwerty, .nine, .fourteen, .stroke]

    static func forScheme(_ scheme: OpiScheme) -> OpiKeyboardLayout {
        switch scheme {
        case .nine: return .nine
        case .fourteen: return .fourteen
        case .stroke: return .stroke
        default: return .qwerty
        }
    }
}

private enum OpiKeyAction {
    case char(String)
    case backspace
    case space
    case enter
    case segment
    case retype
    case symbols
    case mode
    case english
    case numbers
}

private struct OpiKeySpec {
    let label: String
    let detail: String?
    let weight: CGFloat
    let action: OpiKeyAction

    init(_ label: String, _ action: OpiKeyAction,
         detail: String? = nil, weight: CGFloat = 1) {
        self.label = label
        self.detail = detail
        self.weight = weight
        self.action = action
    }

    /// NSButton 单行标题；9 键数字键写成「2 abc」，不赌多行 attributedTitle 的排版。
    var title: String { detail.map { "\(label) \($0)" } ?? label }
}

/// 同 OpiStatusBar：selector 观察/动作要求 NSObject。
final class OpiKeyboardPanel: NSObject {
    static let shared = OpiKeyboardPanel()

    private var panel: NSPanel?
    private var keys: [OpiHoverButton] = []
    private var specs: [OpiKeySpec] = []
    private var noticeLabel: NSTextField?
    private var current: OpiKeyboardLayout = .qwerty
    private var previous: OpiKeyboardLayout = .qwerty
    private var noticeText: String?
    private var noticeTimer: Timer?

    private let width: CGFloat = 540
    private let margin: CGFloat = 8
    private let gap: CGFloat = 4
    private let headerHeight: CGFloat = 30
    private let rowHeight: CGFloat = 42
    private let noticeHeight: CGFloat = 16

    private override init() {
        super.init()
        NotificationCenter.default.addObserver(
            self, selector: #selector(stateChanged(_:)),
            name: .opiThemeChanged, object: nil)
        NotificationCenter.default.addObserver(
            self, selector: #selector(stateChanged(_:)),
            name: .opiStateChanged, object: nil)
        NotificationCenter.default.addObserver(
            self, selector: #selector(stateChanged(_:)),
            name: .opiSettingsChanged, object: nil)
    }

    var isVisible: Bool { panel?.isVisible ?? false }

    // ---------- 显示 / 隐藏 ----------

    func toggle() { isVisible ? hide() : show() }

    func show() {
        if panel == nil {
            current = OpiKeyboardLayout.forScheme(OpiEngine.shared.currentScheme())
            previous = current == .stroke ? .qwerty : current
            buildPanel()
        }
        rebuild()
        position()
        panel?.orderFrontRegardless()
        OpiStatusBar.shared.refresh()
    }

    func hide() {
        panel?.orderOut(nil)
        OpiStatusBar.shared.refresh()
    }

    @objc private func stateChanged(_ note: Notification) {
        guard isVisible else { return }
        rebuild()
        position()
    }

    private func buildPanel() {
        let p = NSPanel(contentRect: NSRect(x: 0, y: 0, width: width, height: 200),
                        styleMask: [.borderless, .nonactivatingPanel],
                        backing: .buffered, defer: false)
        p.title = "小欧输入法 虚拟键盘"
        p.isOpaque = false
        p.backgroundColor = .clear
        p.hasShadow = true
        p.level = .floating
        p.isFloatingPanel = true
        p.hidesOnDeactivate = false
        p.isReleasedWhenClosed = false
        p.becomesKeyOnlyIfNeeded = true
        p.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
        let root = OpiFlippedView(frame: .zero)
        p.contentView = root
        panel = p
    }

    /// Linux 键盘在屏幕底部居中（`KB_HEIGHT + 60`）；macOS 状态栏也在底部，
    /// 这里把键盘抬到状态栏上方，避免状态栏按钮被键盘盖住。
    private func position() {
        guard let panel, let screen = NSScreen.main ?? NSScreen.screens.first else { return }
        let vf = screen.visibleFrame
        let below = OpiBarSettings.load().hidden ? 60.0 : 180.0
        let x = vf.midX - panel.frame.width / 2
        let y = min(max(vf.minY + CGFloat(below), vf.minY + 8),
                    vf.maxY - panel.frame.height - 8)
        panel.setFrameOrigin(NSPoint(x: x, y: y))
    }

    // ---------- 重建内容 ----------

    private func rebuild() {
        guard let panel, let root = panel.contentView as? OpiFlippedView else { return }
        root.subviews.forEach { $0.removeFromSuperview() }
        keys = []
        specs = []

        let palette = OpiTheme.palette
        let traditional = OpiEngine.shared.mode() == .traditional
        let rows = OpiKeyboardPanel.rows(for: current, traditional: traditional)
        let height = margin * 2 + headerHeight + 6
            + CGFloat(rows.count) * rowHeight + CGFloat(max(0, rows.count - 1)) * gap
            + noticeHeight
        panel.setContentSize(NSSize(width: width, height: height))
        root.frame = NSRect(x: 0, y: 0, width: width, height: height)
        OpiTheme.style(content: root, palette: palette)

        // 头部：标题 + 四档键面 + 关闭。
        let title = NSTextField(labelWithString: "小欧虚拟键盘")
        title.font = .systemFont(ofSize: 11)
        title.textColor = palette.dim
        title.frame = NSRect(x: margin, y: margin, width: 100, height: headerHeight)
        root.addSubview(title)

        var x = margin + 108
        for (index, layout) in OpiKeyboardLayout.picker.enumerated() {
            let b = headerButton(layout.title, palette: palette)
            b.tag = index
            b.action = #selector(layoutClicked(_:))
            b.state = layout == current ? .on : .off
            b.frame = NSRect(x: x, y: margin, width: 54, height: headerHeight)
            root.addSubview(b)
            x += 58
        }
        let close = headerButton("×", palette: palette)
        close.action = #selector(closeClicked)
        close.toolTip = "关闭虚拟键盘"
        close.frame = NSRect(x: width - margin - 30, y: margin, width: 30, height: headerHeight)
        root.addSubview(close)

        // 键区。
        var y = margin + headerHeight + 6
        for row in rows {
            let available = width - margin * 2
            let total = row.reduce(CGFloat(0)) { $0 + $1.weight }
            var kx = margin
            for spec in row {
                let kw = (available - gap * CGFloat(row.count - 1)) * spec.weight / total
                let b = OpiHoverButton(frame: NSRect(x: kx, y: y, width: kw, height: rowHeight))
                b.highlightWhenOn = false
                b.baseFontSize = spec.detail == nil ? 15 : 12
                b.normalColor = palette.fg
                b.apply(palette)
                b.setThemedTitle(spec.title, color: palette.fg,
                                 size: spec.detail == nil ? 15 : 12,
                                 bold: spec.detail == nil)
                b.tag = specs.count
                b.target = self
                b.action = #selector(keyClicked(_:))
                b.toolTip = toolTip(spec)
                root.addSubview(b)
                keys.append(b)
                specs.append(spec)
                kx += kw + gap
            }
            y += rowHeight + gap
        }

        let notice = NSTextField(labelWithString: "")
        notice.font = .systemFont(ofSize: 11)
        notice.alignment = .center
        notice.textColor = palette.accent
        notice.frame = NSRect(x: margin, y: y, width: width - margin * 2,
                             height: noticeHeight)
        root.addSubview(notice)
        noticeLabel = notice
        updateNotice()
    }

    private func headerButton(_ text: String, palette: OpiPalette) -> OpiHoverButton {
        let b = OpiHoverButton(frame: .zero)
        b.highlightWhenOn = true
        b.baseFontSize = 12
        b.normalColor = palette.fg
        b.apply(palette)
        b.setThemedTitle(text, color: palette.fg, size: 12)
        b.target = self
        return b
    }

    private func toolTip(_ spec: OpiKeySpec) -> String? {
        switch spec.action {
        case .char(let s): return "输入 \(s)"
        case .backspace: return "退格（空缓冲时只能删输入缓冲，不能删宿主文本）"
        case .space: return "空格"
        case .enter: return "回车"
        case .segment: return "分词：插入音节边界 '（需先有拼音）"
        case .retype: return "重输：清空当前组合"
        case .symbols: return "符号与表情"
        case .mode: return "简繁切换"
        case .english: return "英文 26 键"
        case .numbers: return "数字键盘"
        }
    }

    // ---------- 键面数据 ----------

    private static func rows(for layout: OpiKeyboardLayout,
                             traditional: Bool) -> [[OpiKeySpec]] {
        let mode = traditional ? "简" : "繁"
        switch layout {
        case .qwerty:
            let digits = "1234567890".map { OpiKeySpec(String($0), .char(String($0))) }
            let r1 = "qwertyuiop".map { OpiKeySpec(String($0), .char(String($0))) }
            let r2 = "asdfghjkl".map { OpiKeySpec(String($0), .char(String($0))) }
            let r3 = "zxcvbnm".map { OpiKeySpec(String($0), .char(String($0))) }
            return [digits, r1, r2, r3, [
                OpiKeySpec("符", .symbols),
                OpiKeySpec("空格", .space, weight: 5),
                OpiKeySpec("回车", .enter, weight: 2),
                OpiKeySpec("⌫", .backspace, weight: 2),
            ]]
        case .nine:
            return [
                [OpiKeySpec("，", .char(",")), OpiKeySpec("分词", .segment),
                 OpiKeySpec("2", .char("2"), detail: "abc"),
                 OpiKeySpec("3", .char("3"), detail: "def"),
                 OpiKeySpec("⌫", .backspace)],
                [OpiKeySpec("。", .char(".")),
                 OpiKeySpec("4", .char("4"), detail: "ghi"),
                 OpiKeySpec("5", .char("5"), detail: "jkl"),
                 OpiKeySpec("6", .char("6"), detail: "mno"),
                 OpiKeySpec("重输", .retype)],
                [OpiKeySpec("、", .char("\\")),
                 OpiKeySpec("7", .char("7"), detail: "pqrs"),
                 OpiKeySpec("8", .char("8"), detail: "tuv"),
                 OpiKeySpec("9", .char("9"), detail: "wxyz"),
                 OpiKeySpec("空格", .space)],
                [OpiKeySpec("？", .char("?")), OpiKeySpec("！", .char("!")),
                 OpiKeySpec("符", .symbols), OpiKeySpec(mode, .mode),
                 OpiKeySpec("123", .numbers), OpiKeySpec("英文", .english),
                 OpiKeySpec("↵", .enter)],
            ]
        case .fourteen:
            let r1 = ["qw", "er", "ty", "ui", "op"].map {
                OpiKeySpec($0, .char(String($0.prefix(1))))
            }
            let r2 = ["as", "df", "gh", "jk", "l"].map {
                OpiKeySpec($0, .char(String($0.prefix(1))))
            }
            let r3 = ["zx", "cv", "bn", "m"].map {
                OpiKeySpec($0, .char(String($0.prefix(1))))
            }
            return [r1, r2, r3, [
                OpiKeySpec("符", .symbols), OpiKeySpec(mode, .mode),
                OpiKeySpec("英文", .english),
                OpiKeySpec("空格", .space, weight: 2),
                OpiKeySpec("⌫", .backspace), OpiKeySpec("↵", .enter),
            ]]
        case .stroke:
            let strokes: [(String, String, String)] = [
                ("一", "h", "横"), ("丨", "s", "竖"), ("丿", "p", "撇"),
                ("丶", "n", "捺"), ("乙", "z", "折"),
            ]
            return [strokes.map { OpiKeySpec($0.0, .char($0.1), detail: $0.2) }, [
                OpiKeySpec("符", .symbols), OpiKeySpec(mode, .mode),
                OpiKeySpec("英文", .english),
                OpiKeySpec("空格", .space, weight: 2),
                OpiKeySpec("⌫", .backspace), OpiKeySpec("↵", .enter),
            ]]
        case .numbers:
            let digits = "1234567890".map { OpiKeySpec(String($0), .char(String($0))) }
            return [digits, [
                OpiKeySpec("符", .symbols), OpiKeySpec(mode, .mode),
                OpiKeySpec("英文", .english),
                OpiKeySpec("空格", .space, weight: 5),
                OpiKeySpec("⌫", .backspace), OpiKeySpec("↵", .enter),
            ]]
        }
    }

    // ---------- 动作 ----------

    @objc private func layoutClicked(_ sender: NSButton) {
        guard OpiKeyboardLayout.picker.indices.contains(sender.tag) else { return }
        choose(OpiKeyboardLayout.picker[sender.tag])
    }

    @objc private func closeClicked() { hide() }

    @objc private func keyClicked(_ sender: NSButton) {
        guard specs.indices.contains(sender.tag) else { return }
        perform(specs[sender.tag].action)
    }

    private func perform(_ action: OpiKeyAction) {
        let engine = OpiEngine.shared
        switch action {
        case .char(let s):
            type(s)
        case .backspace:
            if !OpiClientHub.shared.virtualKey(keyval: OpiKeySpace.backspace, states: 0) {
                showNotice("输入缓冲已空 —— macOS/IMK 不能删除宿主文档里的字符")
            }
        case .space:
            type(" ")
        case .enter:
            if !OpiClientHub.shared.virtualKey(keyval: OpiKeySpace.ret, states: 0) {
                showNotice("回车未被引擎处理")
            }
        case .segment:
            if engine.buffer().isEmpty {
                showNotice("缓冲为空：分词键要先有拼音")
            } else {
                type("'")
            }
        case .retype:
            engine.clear()
            OpiClientHub.shared.refreshState()
        case .symbols:
            OpiSymbolPanel.shared.show()
        case .mode:
            if current == .numbers { choose(previous) }
            engine.switchMode(engine.mode() == .traditional ? .pinyin : .traditional)
            OpiClientHub.shared.refreshState()
            rebuild()
        case .english:
            choose(.qwerty)
            engine.switchMode(.english)
            OpiClientHub.shared.refreshState()
            rebuild()
        case .numbers:
            previous = current == .numbers ? previous : (current == .stroke ? .qwerty : current)
            current = .numbers
            engine.switchMode(.number)
            OpiClientHub.shared.refreshState()
            rebuild()
        }
    }

    private func type(_ s: String) {
        guard let scalar = s.unicodeScalars.first else { return }
        if !OpiClientHub.shared.virtualKey(keyval: scalar.value, states: 0) {
            showNotice("引擎未处理该键")
        }
    }

    private func choose(_ layout: OpiKeyboardLayout) {
        guard layout != .numbers else {
            perform(.numbers)
            return
        }
        previous = layout
        current = layout
        let engine = OpiEngine.shared
        switch layout {
        case .nine:
            OpiSettingsStore.applyScheme(.nine)
        case .fourteen:
            OpiSettingsStore.applyScheme(.fourteen)
        case .stroke:
            OpiSettingsStore.applyScheme(.stroke)
            if !engine.isSchemeAvailable(.stroke) {
                showNotice("笔画词典未装载 —— 按键会进缓冲，但没有候选")
            }
        case .qwerty:
            if [.nine, .fourteen, .stroke].contains(engine.currentScheme()) {
                OpiSettingsStore.applyScheme(.full)
            }
        case .numbers:
            break
        }
        if engine.mode() == .number || (engine.mode() == .english && layout != .qwerty) {
            engine.switchMode(.pinyin)
        }
        OpiClientHub.shared.refreshState()
        rebuild()
    }

    // ---------- 提示 ----------

    private func showNotice(_ text: String) {
        noticeText = text
        noticeTimer?.invalidate()
        noticeTimer = Timer.scheduledTimer(withTimeInterval: 4, repeats: false) { [weak self] _ in
            self?.noticeText = nil
            self?.updateNotice()
        }
        updateNotice()
    }

    private func updateNotice() {
        noticeLabel?.stringValue = noticeText ?? ""
        noticeLabel?.textColor = OpiTheme.palette.accent
        noticeLabel?.isHidden = noticeText == nil
    }
}
