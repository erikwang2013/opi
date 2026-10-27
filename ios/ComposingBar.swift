// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 拼音行（2026-10-23）。语义对齐 Android `ime/ComposingBar.kt`：组合输入时，
// 在候选栏**上方**单独一行显示引擎的**显示串**（`Engine::display_buffer` ——
// 9 键/双拼带分词 `ni'hao`，其余方案是字母缓冲；已确认片段在前，`中guo`）。
//
// 为什么不再只靠输入框里的 marked text：拼音要留在输入法自己的界面里
// （2026-10-15 Android 同一条用户裁决）。iOS 侧本就不把 preedit 写进字段
// （`KeyboardCandidates.syncMarkedText` 只清残留、不 setMarkedText），
// 这行是组合串在键盘内的**唯一可见落点**。
//
// 槽位**常驻**（空串时不铺底色、只留高度）：条件挂载会让键区在「有没有拼音」
// 之间伸缩（Android 实测过键高 117px ↔ 89px）。高度由 `ComposingBar.height`
// 单一常量给出，控制器把它算进键盘总高、候选栏/工具条接在它下沿。
//
// ⚠️ 本机无 UIKit，只在 Apple CI 真编译；从未上机运行。

import UIKit

final class ComposingBar: UIView {

    /// 行高。控制器把它加进 `preferredKeyboardHeight`，这样键区净高不变。
    /// 2026-10-25：24 → 38（对齐 Android 拼音行 38dp）—— 宠物条与拼音行共用这条槽，
    /// 24pt 时 8 枚宠物只能缩到 ~20pt、横向留下大片空隙。
    static let height: CGFloat = 38

    private let label = UILabel()
    private let scroll = UIScrollView()
    private var palette: KeyboardPalette = .light
    /// iOS 26 默认皮肤：本行交回系统背景（见 `KeyboardLayout.nativeBackdrop`）。
    private var nativeBackdrop = false
    /// `setText` 后等下一次布局再把视图滚到最右端（当时 bounds/contentSize 还没更新）。
    private var scrollToEnd = false

    init() {
        super.init(frame: .zero)
        backgroundColor = .clear
        label.textAlignment = .left
        label.numberOfLines = 1
        // 不省略：完整串放在横向滚动里，超宽可拖动查看（2026-10-23 用户裁决）。
        label.lineBreakMode = .byClipping
        label.setContentCompressionResistancePriority(.required, for: .horizontal)
        label.setContentHuggingPriority(.required, for: .horizontal)
        label.translatesAutoresizingMaskIntoConstraints = false
        scroll.translatesAutoresizingMaskIntoConstraints = false
        scroll.showsHorizontalScrollIndicator = false
        scroll.alwaysBounceHorizontal = true
        // 显示串由引擎给，不再在 Swift 里二次分词（`'` 的切分规则只有引擎一份）。
        scroll.addSubview(label)
        addSubview(scroll)
        NSLayoutConstraint.activate([
            scroll.leadingAnchor.constraint(equalTo: leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: trailingAnchor),
            scroll.topAnchor.constraint(equalTo: topAnchor),
            scroll.bottomAnchor.constraint(equalTo: bottomAnchor),
            label.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 10),
            label.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -10),
            label.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor),
            label.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor),
            label.heightAnchor.constraint(equalTo: scroll.frameLayoutGuide.heightAnchor),
        ])
        setText("")
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        guard scrollToEnd else { return }
        scrollToEnd = false
        scroll.layoutIfNeeded()
        let maxX = max(0, scroll.contentSize.width - scroll.bounds.width)
        scroll.setContentOffset(CGPoint(x: maxX, y: 0), animated: false)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    /// 显示串（已由引擎分词）。空串 = 无组合输入：清空文字并撤掉底色带，
    /// 但**槽位高度保留**（见文件头注）。9 键无匹时文本为空——绝不把数字键串
    /// 当拼音画（2026-10-24）。
    func setText(_ text: String) {
        label.text = text
        label.isHidden = text.isEmpty
        label.textColor = palette.barBufferFg
        // 默认皮肤在 iOS 26 上整行透明（系统圆角背景透上来）；其余皮肤/系统保留
        // 组合态的底色带。
        backgroundColor = (text.isEmpty || nativeBackdrop) ? .clear : palette.barBg
        scrollToEnd = !text.isEmpty
        setNeedsLayout()
    }

    /// 见 `KeyboardLayout.nativeBackdrop`。切换后要重算当前底色。
    func setNativeBackdrop(_ on: Bool) {
        nativeBackdrop = on
        setText(label.text ?? "")
    }

    func applyTheme(_ p: KeyboardPalette, fontScale: CGFloat) {
        palette = p
        label.font = .systemFont(ofSize: 15 * fontScale)
        // 保持当前文字/底色与新色板一致（重开面板/切皮肤时）。
        setText(label.text ?? "")
    }
}
