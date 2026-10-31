// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 顶部工具条（2026-10-20）：对齐 Android `ime/ImeToolbar.kt` 的**空闲态**。
//
// Android 的顶部 44dp 槽位是「候选栏 | 工具条」二选一：
// 有候选/有缓冲 → 候选栏；中文空闲、英文空闲、任何面板 → 工具条。
// iOS 此前恒画候选栏，于是空闲时既没有皮肤/输入模式入口，也没有收起键盘键。
// 本类只画图标与回调，状态判断在 `KeyboardLayout.setTopBar(candidatesVisible:english:)`。
//
// 2026-10-21 用户裁决：项目宠物放在工具条**最前面**（Android `ImeToolbar` 同位置）。
// 2026-10-22 恢复多形态随机显示：对齐 Android `PetStrip`，每 2.8s 换一个随机
// 形态（同形态不连出）；键盘每次出现时重抽（不是每次候选栏收起都重抽，避免
// 每提交一个词就解一次 GIF）。减弱动态时只留一张静态图，不轮播；26pt 固定框
// 只换 `image`，不触发布局。
//
// ⚠️ 与 Android 的已知差别（如实列出，不伪造）：
//   · iOS 扩展不能控制键盘窗口 ⇒ 没有悬浮/游戏快捷入口（面板里有禁用项）；
//   · 设置入口是本端新增的齿轮（Android 的设置页是独立 Activity）。
//
// 宽度预算（320pt 最窄屏，英文空闲时「中」键也出现）：
//   图标次序：表情 / 皮肤 / 输入模式 / 键盘模式 / 剪贴板 / 齿轮 / 关于 / 收起；
//   pet 26 + 图标 8 个 + 中 28 + 左右边距 8。图标宽在 30…44 间按可用宽均分，
//   320pt 下 ≈32.2 —— 宽屏放大触摸区，窄屏不溢出（30 仅为最窄屏兜底）。
//
// ⚠️ 本机无 UIKit，只在 Apple CI 真编译；从未上机运行。

import UIKit

final class KeyboardToolbar: UIView {

    var onOpenEmoji: (() -> Void)?
    var onOpenSkin: (() -> Void)?
    var onOpenInputMode: (() -> Void)?
    var onOpenKeyboardMode: (() -> Void)?
    var onOpenAbout: (() -> Void)?
    /// 设置面板（2026-10-21）：Android 是独立设置页，iOS 扩展里做成齿轮面板。
    var onOpenSettings: (() -> Void)?
    var onCloseKeyboard: (() -> Void)?
    var onSwitchToChinese: (() -> Void)?
    /// 「发送剪贴板」：用户主动点才读 UIPasteboard（iOS 降级版，见 KeyboardClipboard.swift）。
    var onSendClipboard: (() -> Void)?

    /// 英文空闲时出现的低存在感「中」键（Android `ChineseHiddenButton` 同款）。
    private let chineseButton = UIButton(type: .system)
    /// 工具条最左的项目宠物（Android `ImeToolbar` 的 `OpiPet` 同位置，2026-10-21）。
    private let petView = UIImageView(image: PetStickerImages.image(named: "idle"))
    private var iconButtons: [UIButton] = []
    /// 图标宽约束（随键盘宽度在 36…44 间调，见 `updateIconWidths`）。
    private var iconWidthConstraints: [NSLayoutConstraint] = []
    private var petTimer: Timer?
    private var petFormIndex = -1
    /// 工具条此刻是否真的可见（空闲态）。隐藏时停表，不让宠物在后台空转。
    private var petActive = false
    /// 当前色板，供 `applyTheme` 反复调用（切皮肤时控件已存在）。
    private var palette: KeyboardPalette = .light
    /// iOS 26 默认皮肤：条底交回系统（见 `KeyboardLayout.nativeBackdrop`）。
    private var nativeBackdrop = false

    init() {
        super.init(frame: .zero)
        // 图标按钮：SF Symbols 名字在运行时可能不存在（系统版本差异），
        // 取不到就退回文字 —— 不静默丢按钮。
        let emoji = makeIcon("face.smiling", fallback: "☺", label: "表情") { [weak self] in
            self?.onOpenEmoji?()
        }
        let skin = makeIcon("paintpalette", fallback: "肤", label: "皮肤") { [weak self] in
            self?.onOpenSkin?()
        }
        let input = makeIcon("keyboard", fallback: "模", label: "输入模式") { [weak self] in
            self?.onOpenInputMode?()
        }
        let modes = makeIcon("slider.horizontal.3", fallback: "盘", label: "键盘模式") { [weak self] in
            self?.onOpenKeyboardMode?()
        }
        let clip = makeIcon("doc.on.clipboard", fallback: "📋", label: "发送剪贴板") { [weak self] in
            self?.onSendClipboard?()
        }
        // 齿轮：SF Symbols 5 才有 `gearshape`，老系统退回文字「⚙」。
        let settings = makeIcon("gearshape", fallback: "⚙", label: "设置") { [weak self] in
            self?.onOpenSettings?()
        }
        let about = makeIcon("info.circle", fallback: "i", label: "关于") { [weak self] in
            self?.onOpenAbout?()
        }
        let close = makeIcon(
            "keyboard.chevron.compact.down", fallback: "⌄", label: "收起键盘"
        ) { [weak self] in
            self?.onCloseKeyboard?()
        }
        iconButtons = [emoji, skin, input, modes, clip, settings, about, close]

        chineseButton.setTitle("中", for: .normal)
        chineseButton.titleLabel?.font = .systemFont(ofSize: 13)
        chineseButton.accessibilityLabel = "切换到中文"
        chineseButton.addTarget(self, action: #selector(tapChinese), for: .touchUpInside)
        chineseButton.isHidden = true

        petView.contentMode = .scaleAspectFit
        petView.isUserInteractionEnabled = false
        petView.isAccessibilityElement = false
        petView.accessibilityElementsHidden = true
        petView.translatesAutoresizingMaskIntoConstraints = false

        let spacer = UIView()
        spacer.setContentHuggingPriority(.defaultLow, for: .horizontal)

        let stack = UIStackView(arrangedSubviews: [
            petView, emoji, skin, input, modes, clip, settings, about, spacer, chineseButton, close,
        ])
        stack.axis = .horizontal
        stack.alignment = .fill
        stack.spacing = 0
        stack.translatesAutoresizingMaskIntoConstraints = false
        // 隐藏态必须让 stack 能把它压到 0（required 会与 stack 的折叠约束打架）。
        let chineseWidth = chineseButton.widthAnchor.constraint(equalToConstant: 28)
        chineseWidth.priority = .defaultHigh
        addSubview(stack)
        NSLayoutConstraint.activate([
            // 横屏刘海：图标内容让开安全区，背景仍铺满整条。
            stack.leadingAnchor.constraint(equalTo: safeAreaLayoutGuide.leadingAnchor, constant: 4),
            stack.trailingAnchor.constraint(equalTo: safeAreaLayoutGuide.trailingAnchor, constant: -4),
            stack.topAnchor.constraint(equalTo: topAnchor),
            stack.bottomAnchor.constraint(equalTo: bottomAnchor),
            chineseWidth,
            // 与顶部宠物条的每枚同尺寸（`PetStripBar.petSide`；用户裁决
            // 「多形态宠物要和工具条 logo 一样大」）。
            petView.widthAnchor.constraint(equalToConstant: PetStripBar.petSide),
            // 不锁高度：stack 是 .fill（44pt 高），再锁 28 会与 stack 的约束打架。
            // `.scaleAspectFit` 会自己按 26 宽保持比例。
        ])
        for b in iconButtons {
            let width = b.widthAnchor.constraint(equalToConstant: 36)
            width.isActive = true
            iconWidthConstraints.append(width)
            b.setContentHuggingPriority(.required, for: .horizontal)
            b.setContentCompressionResistancePriority(.required, for: .horizontal)
        }
        petView.setContentHuggingPriority(.required, for: .horizontal)
        petView.setContentCompressionResistancePriority(.required, for: .horizontal)
        applyTheme(palette)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    deinit { petTimer?.invalidate() }

    override func didMoveToWindow() {
        super.didMoveToWindow()
        // 键盘收起后别在后台空转。
        guard window != nil else {
            stopPetRotation()
            return
        }
        // 键盘每次出现都重抽一形态（不只首次）；对齐 Android `PetStrip` 的「出现即换」。
        showRandomPet()
        // 候选栏占位时不轮播；等工具条回到空闲态再启动。
        if petActive { startPetRotation() }
    }

    /// 英文空闲 = 显示隐藏「中」键（英文有候选时这个键在候选栏，不在这里）。
    func setEnglish(_ english: Bool) {
        chineseButton.isHidden = !english
    }

    /// 工具条显隐（只在空闲态转宠物）。由 `KeyboardLayout.updateTopBarVisibility` 调。
    func setActive(_ active: Bool) {
        guard active != petActive else { return }
        petActive = active
        if active { startPetRotation() } else { stopPetRotation() }
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        updateIconWidths()
    }

    /// 图标宽在 30…44 间按可用宽均分：宽屏给足触摸区，320pt 也不溢出。
    private func updateIconWidths() {
        let safe = safeAreaInsets
        let usable = bounds.width - safe.left - safe.right
        guard usable > 0, !iconWidthConstraints.isEmpty else { return }
        // 固定占用 = pet 26 + 左右边距 8 + 英文「中」28（隐藏时也按它预留，留余量）。
        let perIcon = min(44, max(30, (usable - 62) / CGFloat(iconButtons.count)))
        for c in iconWidthConstraints where abs(c.constant - perIcon) > 0.5 {
            c.constant = perIcon
        }
    }

    // MARK: - 宠物多形态随机轮播（Android `PetStrip` 同节奏）

    private func startPetRotation() {
        stopPetRotation()
        // 首帧兜底：窗口出现前就被判为 active 时也得有图（正常由 didMoveToWindow 抽）。
        if petFormIndex < 0 { showRandomPet() }
        // 减弱动态：只留这一张静态图，不启动轮播。
        guard !UIAccessibility.isReduceMotionEnabled else { return }
        let timer = Timer(timeInterval: 2.8, repeats: true) { [weak self] _ in
            self?.tickPet()
        }
        // 用 default：滚动/触摸跟踪时暂停，别在面板滑动时解 GIF。
        RunLoop.current.add(timer, forMode: .default)
        petTimer = timer
    }

    private func stopPetRotation() {
        petTimer?.invalidate()
        petTimer = nil
    }

    private func tickPet() {
        // 键盘还活着时用户可能刚打开减弱动态：下一拍停表并换成静态图。
        guard !UIAccessibility.isReduceMotionEnabled else {
            stopPetRotation()
            showRandomPet()
            return
        }
        showRandomPet()
    }

    private func showRandomPet() {
        let forms = PetEmojiTable.forms
        guard !forms.isEmpty else { return }
        var next = Int.random(in: 0..<forms.count)
        // 同形态不连出（Android 是整组重抽，单枚这里只做这一步）。
        if forms.count > 1, next == petFormIndex {
            next = (next + 1) % forms.count
        }
        petFormIndex = next
        let name = forms[next].sticker
        if UIAccessibility.isReduceMotionEnabled {
            petView.image = PetStickerImages.image(named: name)
                ?? PetStickerImages.image(named: "idle")
        } else {
            petView.image = PetStickerGifs.animatedImage(named: name)
                ?? PetStickerImages.image(named: name)
                ?? PetStickerImages.image(named: "idle")
        }
    }

    /// 见 `KeyboardLayout.nativeBackdrop`。
    func setNativeBackdrop(_ on: Bool) {
        nativeBackdrop = on
        backgroundColor = on ? .clear : palette.toolbarBg
        isOpaque = !on
    }

    func applyTheme(_ p: KeyboardPalette) {
        palette = p
        backgroundColor = nativeBackdrop ? .clear : p.toolbarBg
        tintColor = p.toolbarFg
        for b in iconButtons {
            b.tintColor = p.toolbarFg
            b.setTitleColor(p.toolbarFg, for: .normal)
        }
        chineseButton.setTitleColor(p.toolbarFg.withAlphaComponent(0.35), for: .normal)
        chineseButton.setTitleColor(p.toolbarFg, for: .highlighted)
    }

    private func makeIcon(
        _ symbol: String, fallback: String, label: String, onTap: @escaping () -> Void
    ) -> UIButton {
        let b = UIButton(type: .system)
        if let image = UIImage(systemName: symbol) {
            b.setImage(image, for: .normal)
            b.imageView?.contentMode = .scaleAspectFit
        } else {
            b.setTitle(fallback, for: .normal)
            b.titleLabel?.font = .systemFont(ofSize: 18)
        }
        b.accessibilityLabel = label
        b.addAction(UIAction { _ in onTap() }, for: .touchUpInside)
        return b
    }

    @objc private func tapChinese() {
        onSwitchToChinese?()
    }
}
