// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 平铺候选面板（2026-10-21）。语义对齐 Android `ime/CandidateTiledView.kt`：
//   · 左列 = 读音列（与 9 键左列**同一块** `CandidateSideColumn`，点行 = 钉读音）；
//   · 中间 = 候选按 4 单位/行贪心铺开（≤2 字 1 单位、更长 2 单位），每页 4 行，
//     上下滑动翻页；
//   · 右列 = 返回 / ⌫ / 重输 / 空格（四格等高）。
//
// 平铺态**盖住整个键盘**（含顶部 44pt 槽位，`KeyboardLayout.applyOneHand` 把键区
// 顶锚切到键盘顶部），与 Android 同覆盖范围；右列「返回」是唯一收拢入口。
//
// ⚠️ 本机无 UIKit，只在 Apple CI 真编译；从未上机运行。

import UIKit

final class CandidateTiledPanel: UIView {

    /// 全局候选下标（`opi_candidates(limit)` 的次序 = `opi_select(index)` 的下标）。
    var onSelect: ((Int) -> Void)?
    var onBack: (() -> Void)?
    var onBackspace: (() -> Void)?
    var onRetype: (() -> Void)?
    var onSpace: (() -> Void)?
    var onPinReading: ((String) -> Void)?

    private struct Tile {
        let index: Int
        let units: CGFloat
    }

    private let readingColumn = CandidateSideColumn(cells: 5, fontSize: 20)
    private let pager = UIScrollView()
    private let pageStack = UIStackView()
    private let actionStack = UIStackView()
    private var actionButtons: [UIButton] = []

    private var candidates: [String] = []
    private var readings: [String] = []
    private var pinned = ""
    private var resetKey = ""
    private var dirty = true
    private var palette: KeyboardPalette = .light
    private var fontScale: CGFloat = 1

    private static let unitsPerRow: CGFloat = 4
    private static let rowsPerPage = 4
    private static let gap: CGFloat = 4

    init() {
        super.init(frame: .zero)
        backgroundColor = palette.barBg
        buildReadingColumn()
        buildPager()
        buildActions()
        buildLayout()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 平铺候选全代码构建") }

    private func buildReadingColumn() {
        readingColumn.translatesAutoresizingMaskIntoConstraints = false
        readingColumn.onPick = { [weak self] index in
            guard let self, self.readings.indices.contains(index) else { return }
            self.onPinReading?(self.readings[index])
        }
        addSubview(readingColumn)
    }

    private func buildPager() {
        pager.isPagingEnabled = true
        pager.showsVerticalScrollIndicator = false
        pager.alwaysBounceVertical = false
        // 与左列同理：点格子不延迟；上下滑动仍翻页。
        pager.delaysContentTouches = false
        pager.translatesAutoresizingMaskIntoConstraints = false

        pageStack.axis = .vertical
        pageStack.spacing = 0
        pageStack.translatesAutoresizingMaskIntoConstraints = false

        addSubview(pager)
        pager.addSubview(pageStack)
        NSLayoutConstraint.activate([
            pageStack.leadingAnchor.constraint(equalTo: pager.contentLayoutGuide.leadingAnchor),
            pageStack.trailingAnchor.constraint(equalTo: pager.contentLayoutGuide.trailingAnchor),
            pageStack.topAnchor.constraint(equalTo: pager.contentLayoutGuide.topAnchor),
            pageStack.bottomAnchor.constraint(equalTo: pager.contentLayoutGuide.bottomAnchor),
            pageStack.widthAnchor.constraint(equalTo: pager.frameLayoutGuide.widthAnchor),
        ])
    }

    private func buildActions() {
        actionStack.axis = .vertical
        actionStack.distribution = .fillEqually
        actionStack.spacing = 4
        actionStack.translatesAutoresizingMaskIntoConstraints = false
        let specs: [(String, Selector)] = [
            ("返回", #selector(tapBack)),
            ("⌫", #selector(tapBackspace)),
            ("重输", #selector(tapRetype)),
            ("空格", #selector(tapSpace)),
        ]
        for (title, action) in specs {
            let b = UIButton(type: .system)
            b.setTitle(title, for: .normal)
            b.titleLabel?.numberOfLines = 1
            b.addTarget(self, action: action, for: .touchUpInside)
            actionStack.addArrangedSubview(b)
            actionButtons.append(b)
        }
        addSubview(actionStack)
    }

    private func buildLayout() {
        NSLayoutConstraint.activate([
            readingColumn.leadingAnchor.constraint(equalTo: leadingAnchor),
            readingColumn.topAnchor.constraint(equalTo: topAnchor),
            readingColumn.bottomAnchor.constraint(equalTo: bottomAnchor),
            readingColumn.widthAnchor.constraint(equalTo: widthAnchor, multiplier: 0.2),

            pager.leadingAnchor.constraint(equalTo: readingColumn.trailingAnchor, constant: 4),
            pager.trailingAnchor.constraint(equalTo: actionStack.leadingAnchor, constant: -4),
            pager.topAnchor.constraint(equalTo: topAnchor),
            pager.bottomAnchor.constraint(equalTo: bottomAnchor),

            actionStack.trailingAnchor.constraint(equalTo: trailingAnchor),
            actionStack.topAnchor.constraint(equalTo: topAnchor),
            actionStack.bottomAnchor.constraint(equalTo: bottomAnchor),
            // 与 Android `CandidateTiledView` 的 1:3:1 权重一致（右列 = 20%）。
            actionStack.widthAnchor.constraint(equalTo: widthAnchor, multiplier: 0.2),
        ])
        styleActions()
    }

    // MARK: - 数据

    /// 控制器/布局每次刷新都调这里。面板隐藏时只记脏，不造几十个按钮。
    func update(candidates: [String], readings: [String], pinned: String, resetKey: String) {
        self.candidates = candidates
        self.readings = readings
        self.pinned = pinned
        self.resetKey = resetKey
        dirty = true
        if !isHidden { reload() }
    }

    /// 进入平铺层时由 `rebuildKeys` 调；候选没变就不重造视图。
    func reloadIfNeeded() {
        guard dirty else { return }
        reload()
    }

    func applyTheme(_ p: KeyboardPalette, fontScale: CGFloat) {
        palette = p
        self.fontScale = fontScale
        backgroundColor = p.barBg
        readingColumn.applyTheme(p, fontScale: fontScale)
        styleActions()
        dirty = true
        if !isHidden { reload() }
    }

    private func reload() {
        dirty = false
        readingColumn.setEntries(readings, pinned: pinned, resetKey: resetKey)
        rebuildPages()
    }

    // MARK: - 平铺网格

    private func rebuildPages() {
        pageStack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        let pages = Self.pack(candidates)
        for page in pages {
            let pageView = UIView()
            pageView.translatesAutoresizingMaskIntoConstraints = false
            pageStack.addArrangedSubview(pageView)
            NSLayoutConstraint.activate([
                pageView.widthAnchor.constraint(equalTo: pageStack.widthAnchor),
                pageView.heightAnchor.constraint(equalTo: pager.frameLayoutGuide.heightAnchor),
            ])

            let rows = UIStackView()
            rows.axis = .vertical
            rows.distribution = .fillEqually
            rows.spacing = 0
            rows.translatesAutoresizingMaskIntoConstraints = false
            pageView.addSubview(rows)
            NSLayoutConstraint.activate([
                rows.leadingAnchor.constraint(equalTo: pageView.leadingAnchor),
                rows.trailingAnchor.constraint(equalTo: pageView.trailingAnchor),
                rows.topAnchor.constraint(equalTo: pageView.topAnchor),
                rows.bottomAnchor.constraint(equalTo: pageView.bottomAnchor),
            ])
            for r in 0..<Self.rowsPerPage {
                let row = UIStackView()
                row.axis = .horizontal
                row.distribution = .fill
                row.spacing = Self.gap
                rows.addArrangedSubview(row)
                addRow(r < page.count ? page[r] : [], to: row)
            }
        }
        // 候选整批换了（继续输入 / 钉读音 / 选词后剩尾巴）→ 回第一页，别停在旧页。
        layoutIfNeeded()
        pager.setContentOffset(.zero, animated: false)
    }

    private func addRow(_ tiles: [Tile], to row: UIStackView) {
        guard !tiles.isEmpty else { return }
        var items = tiles
        let used = tiles.reduce(CGFloat.zero) { $0 + $1.units }
        if used < Self.unitsPerRow {
            // 补空位：本行剩余单位用透明视图占掉，否则 `.fill` 会把实格拉宽。
            items.append(Tile(index: -1, units: Self.unitsPerRow - used))
        }
        let gaps = Self.gap * CGFloat(max(0, items.count - 1))
        for tile in items {
            let view: UIView
            if tile.index >= 0, candidates.indices.contains(tile.index) {
                view = makeTile(candidates[tile.index], index: tile.index)
            } else {
                view = UIView()
            }
            row.addArrangedSubview(view)
            let frac = tile.units / Self.unitsPerRow
            view.widthAnchor.constraint(
                equalTo: row.widthAnchor, multiplier: frac, constant: -gaps * frac
            ).isActive = true
        }
    }

    private func makeTile(_ text: String, index: Int) -> UIButton {
        let b = UIButton(type: .system)
        b.setTitle(text, for: .normal)
        b.setTitleColor(palette.keyFg, for: .normal)
        b.backgroundColor = palette.keyBg
        b.layer.cornerRadius = 6
        b.clipsToBounds = true
        b.titleLabel?.font = .systemFont(ofSize: 20 * fontScale)
        // 4 列格子在 320pt 上只有 ~42pt 宽：先缩字保证两字词完整，实在放不下才截断。
        b.titleLabel?.adjustsFontSizeToFitWidth = true
        b.titleLabel?.minimumScaleFactor = 0.7
        b.titleLabel?.lineBreakMode = .byTruncatingTail
        b.titleLabel?.numberOfLines = 1
        b.contentEdgeInsets = UIEdgeInsets(top: 2, left: 3, bottom: 2, right: 3)
        b.accessibilityLabel = text
        b.tag = index
        b.addTarget(self, action: #selector(tapTile(_:)), for: .touchUpInside)
        return b
    }

    /// 候选 → 页 → 行 → 格。贪心**不重排**（次序 = 候选名次）。
    private static func pack(_ texts: [String]) -> [[[Tile]]] {
        guard !texts.isEmpty else { return [] }
        var rows: [[Tile]] = []
        var row: [Tile] = []
        var used: CGFloat = 0
        for (i, text) in texts.enumerated() {
            let units: CGFloat = text.count <= 2 ? 1 : 2
            if used + units > unitsPerRow {
                rows.append(row)
                row = []
                used = 0
            }
            row.append(Tile(index: i, units: units))
            used += units
        }
        if !row.isEmpty { rows.append(row) }
        var pages: [[[Tile]]] = []
        var start = 0
        while start < rows.count {
            let end = min(start + rowsPerPage, rows.count)
            pages.append(Array(rows[start..<end]))
            start = end
        }
        return pages
    }

    // MARK: - 主题与动作

    private func styleActions() {
        for b in actionButtons {
            b.setTitleColor(palette.keyFg, for: .normal)
            b.backgroundColor = palette.keyBg
            b.layer.cornerRadius = 6
            b.clipsToBounds = true
            b.titleLabel?.font = .systemFont(ofSize: 16 * fontScale)
        }
    }

    @objc private func tapTile(_ sender: UIButton) { onSelect?(sender.tag) }
    @objc private func tapBack() { onBack?() }
    @objc private func tapBackspace() { onBackspace?() }
    @objc private func tapRetype() { onRetype?() }
    @objc private func tapSpace() { onSpace?() }
}
