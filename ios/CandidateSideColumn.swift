// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 候选侧栏（2026-10-21）：9 键左列的拼音读数、英文 26 键左列的候选词、
// 平铺候选视图的左列，共用这一块。
//
// 语义对齐 Android `keyboard/CandidateReadingColumn.kt`：
//   · 一行一项，整列可竖向滚动；
//   · 格高 = 列高 / cells（9 键 5 格，与标点列同节奏）；
//   · 点行只上报**行号**，是「钉读音」还是「选词」由宿主决定（两处语义不同，
//     别在这里按有没有 pinned 猜）。
//
// ⚠️ 本机无 UIKit，只在 Apple CI 真编译；从未上机运行。

import UIKit

final class CandidateSideColumn: UIView {

    private let scroll = UIScrollView()
    private let stack = UIStackView()
    private let cells: CGFloat
    private let baseFontSize: CGFloat

    private var entries: [String] = []
    private var pinned = ""
    private var scrollKey = ""
    private var palette: KeyboardPalette = .light
    private var fontScale: CGFloat = 1
    /// 已建的行按钮。每次刷新**复用**前 N 个（打字时每键都会换一批读数，
    /// 整列重建几十个 UIButton 是没必要的卡顿来源）。
    private var rows: [UIButton] = []

    /// 行号（0 起）。越界/空列不会回调。
    var onPick: ((Int) -> Void)?

    init(cells: Int, fontSize: CGFloat) {
        self.cells = CGFloat(max(1, cells))
        self.baseFontSize = fontSize
        super.init(frame: .zero)
        backgroundColor = palette.barBg

        scroll.showsVerticalScrollIndicator = false
        scroll.alwaysBounceVertical = false
        // 键盘上的点击要立刻生效；拖动仍会被滚动手势接管。
        scroll.delaysContentTouches = false
        scroll.translatesAutoresizingMaskIntoConstraints = false

        stack.axis = .vertical
        stack.spacing = 0
        stack.translatesAutoresizingMaskIntoConstraints = false

        addSubview(scroll)
        scroll.addSubview(stack)
        NSLayoutConstraint.activate([
            scroll.leadingAnchor.constraint(equalTo: leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: trailingAnchor),
            scroll.topAnchor.constraint(equalTo: topAnchor),
            scroll.bottomAnchor.constraint(equalTo: bottomAnchor),

            stack.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor),
            stack.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor),
            stack.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor),
            stack.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 候选侧栏全代码构建") }

    /// 换一批条目。`resetKey` 变化（通常是拼音缓冲）→ 滚回顶部，别停在旧位置。
    func setEntries(_ new: [String], pinned: String, resetKey: String) {
        let resetScroll = scrollKey != resetKey
        scrollKey = resetKey
        entries = new
        self.pinned = pinned
        rebuild()
        if resetScroll {
            // 新条目可能还没布局完；先排一次再归零，避免 offset 被旧 contentSize 钳住。
            layoutIfNeeded()
            scroll.setContentOffset(.zero, animated: false)
        }
    }

    func applyTheme(_ p: KeyboardPalette, fontScale: CGFloat) {
        palette = p
        self.fontScale = fontScale
        backgroundColor = p.barBg
        rebuild()
    }

    private func rebuild() {
        while rows.count > entries.count {
            rows.removeLast().removeFromSuperview()
        }
        let cellHeight = scroll.frameLayoutGuide.heightAnchor
        for (i, title) in entries.enumerated() {
            let b = i < rows.count ? rows[i] : makeRow(cellHeight: cellHeight)
            let selected = !pinned.isEmpty && title == pinned
            b.setTitle(title, for: .normal)
            b.setTitleColor(selected ? palette.accent : palette.barDimFg, for: .normal)
            b.titleLabel?.font = .systemFont(
                ofSize: baseFontSize * fontScale,
                weight: selected ? .bold : .regular
            )
            b.tag = i
        }
    }

    private func makeRow(cellHeight: NSLayoutDimension) -> UIButton {
        let b = UIButton(type: .system)
        b.titleLabel?.lineBreakMode = .byTruncatingTail
        b.titleLabel?.numberOfLines = 1
        // 读数（ni'hao）比候选词窄，优先缩字而不是截断；候选词也会因此更容易看清。
        b.titleLabel?.adjustsFontSizeToFitWidth = true
        b.titleLabel?.minimumScaleFactor = 0.6
        b.contentEdgeInsets = UIEdgeInsets(top: 1, left: 4, bottom: 1, right: 4)
        b.translatesAutoresizingMaskIntoConstraints = false
        b.addTarget(self, action: #selector(tapped(_:)), for: .touchUpInside)
        stack.addArrangedSubview(b)
        NSLayoutConstraint.activate([
            b.widthAnchor.constraint(equalTo: stack.widthAnchor),
            b.heightAnchor.constraint(equalTo: cellHeight, multiplier: 1 / cells),
        ])
        rows.append(b)
        return b
    }

    @objc private func tapped(_ sender: UIButton) {
        let index = sender.tag
        guard entries.indices.contains(index) else { return }
        onPick?(index)
    }
}
