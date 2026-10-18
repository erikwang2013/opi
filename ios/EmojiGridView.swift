// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · **未编译**（2026-10-18 新增；本机没有 swiftc，待 CI apple.yml 的 iOS job）
//
// 表情页的网格：竖向滚动、固定列数、每行显式行高。
//
// 为什么单独一个文件：`EmojiPanel.swift` 现在装「面板状态 + 两套工具条 + 分类条」，
// 再加网格渲染就撞上本仓「源码文件 500 行」的规矩。拆法按**概念**切：
// 网格只管「画行、发点击、报滚动位置」，分类条高亮仍由面板算（它才知道分类表）。
//
// 姿态与 `SymbolPanel.swift` 相同：只画与发射点击，**不上屏、不碰引擎**。

import UIKit

/// 表情/颜文字网格。
final class EmojiGridView: UIView, UIScrollViewDelegate {

    /// 点中一格（提交什么由面板/控制器决定）。
    var onPick: ((String) -> Void)?
    /// 滚动位置变了（分类条高亮跟着反推）。
    var onScroll: (() -> Void)?

    /// 行高。滚动视图里的行**必须**有明确高度（容器 `.fillEqually` 在滚动视图里
    /// 会退化成 0 高，见 `SymbolPanel.init` 那条注释）。
    static let rowHeight: CGFloat = 40
    /// 行间距。与 `SymbolPanel` 的 `stack.spacing = 6` 是同一个数。
    static let gridSpacing: CGFloat = 6

    private let scroll = UIScrollView()
    private let stack = UIStackView()
    private var itemCount = 0
    private var columns = 8

    init() {
        super.init(frame: .zero)
        scroll.showsVerticalScrollIndicator = true
        scroll.delegate = self
        scroll.translatesAutoresizingMaskIntoConstraints = false
        stack.axis = .vertical
        stack.spacing = Self.gridSpacing
        stack.translatesAutoresizingMaskIntoConstraints = false
        scroll.addSubview(stack)
        addSubview(scroll)

        // 约束写法与 `SymbolPanel` 逐条同构（contentLayoutGuide 定内容、
        // 宽度钉 frameLayoutGuide 防横向滚动）。
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
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    /// Emoji 子页：`自有`（emoji 字形 + 小标签）在前，其后是引擎表情表。
    func showEmoji(pets: [PetEmojiTable.Form], emojis: [String], columns: Int) {
        self.columns = columns
        let total = pets.count + emojis.count
        itemCount = total
        reset()
        guard total > 0 else {
            // 空列表**不休假成空白**（同 SymbolPanel 的理由）；正常不会走到，
            // 因为 `自有` 恒有 12 格。
            addEmpty("（表情库未就绪）")
            return
        }
        for start in stride(from: 0, to: total, by: columns) {
            let end = min(start + columns, total)
            let row = makeRow()
            for i in start..<end {
                if i < pets.count {
                    let form = pets[i]
                    row.addArrangedSubview(
                        PetEmojiCell(label: form.label, emoji: form.emoji) { [weak self] in
                            self?.onPick?(form.emoji)
                        }
                    )
                } else {
                    let text = emojis[i - pets.count]
                    row.addArrangedSubview(BarButton(title: text) { [weak self] in
                        self?.onPick?(text)
                    })
                }
            }
            padRow(row, filled: end - start, columns: columns)
            stack.addArrangedSubview(row)
        }
        // 换了一批就把滚动位置收回顶部 —— 否则上一批的偏移会留着，面板看起来像
        // 「打开就是空的」（内容其实在视口上方）。同 `SymbolPanel.setSymbols`。
        scroll.setContentOffset(.zero, animated: false)
    }

    /// 纯文本网格（颜文字 / 搜索结果）。
    func showTexts(_ texts: [String], columns: Int, empty: String, shrink: Bool) {
        self.columns = columns
        itemCount = texts.count
        reset()
        guard !texts.isEmpty else {
            addEmpty(empty)
            return
        }
        for start in stride(from: 0, to: texts.count, by: columns) {
            let end = min(start + columns, texts.count)
            let row = makeRow()
            for text in texts[start..<end] {
                let button = BarButton(title: text) { [weak self] in
                    self?.onPick?(text)
                }
                if shrink {
                    // 颜文字比单个 emoji 宽得多：允许缩字，少截成省略号（Android 14sp 同义）。
                    button.titleLabel?.adjustsFontSizeToFitWidth = true
                    button.titleLabel?.minimumScaleFactor = 0.5
                }
                row.addArrangedSubview(button)
            }
            padRow(row, filled: end - start, columns: columns)
            stack.addArrangedSubview(row)
        }
        scroll.setContentOffset(.zero, animated: false)
    }

    /// 滚到第 `start` 条所在的行（分类条点一下的去向）。
    ///
    /// 偏移 = 行号 × (行高 + 行距)：行高是显式约束、行距是 `stack.spacing`，
    /// 两者都不由内容决定，所以这个换算与真实布局一致（不像内容自适应那样会漂）。
    func scrollTo(start: Int) {
        let row = start / columns
        let y = CGFloat(row) * (Self.rowHeight + Self.gridSpacing)
        scroll.setContentOffset(CGPoint(x: 0, y: y), animated: true)
    }

    /// 首个可见项的下标（分类条高亮反推的输入）。滚动偏移是显式行高 + 行距拼的，可逆。
    func firstVisibleItemIndex() -> Int {
        guard itemCount > 0 else { return 0 }
        let stride = Self.rowHeight + Self.gridSpacing
        let row = Int(max(0, scroll.contentOffset.y) / stride)
        return min(row * columns, itemCount - 1)
    }

    func scrollViewDidScroll(_ scrollView: UIScrollView) {
        onScroll?()
    }

    private func reset() {
        stack.arrangedSubviews.forEach { $0.removeFromSuperview() }
    }

    private func makeRow() -> UIStackView {
        let row = UIStackView()
        row.axis = .horizontal
        row.spacing = 5
        row.distribution = .fillEqually
        row.heightAnchor.constraint(equalToConstant: Self.rowHeight).isActive = true
        return row
    }

    /// 补齐一行到固定列数：否则最后一行的两三格会被 `.fillEqually` 拉宽，
    /// 与上面几行的列宽对不上（Android 的网格是固定列）。
    private func padRow(_ row: UIStackView, filled: Int, columns: Int) {
        guard filled < columns else { return }
        for _ in filled..<columns {
            row.addArrangedSubview(UIView())
        }
    }

    private func addEmpty(_ text: String) {
        stack.addArrangedSubview(BarButton(title: text) {})
    }
}

/// 「自有」一格：emoji 字形 + 小标签（例：😊 笑）。
///
/// iOS 草案**没有小欧矢量绘制**（见 `PetEmojiTable.swift` 头注），所以这里用
/// **emoji 选择器**表达；点击提交的是映射到的 emoji，不是这格画的东西。
private final class PetEmojiCell: UIButton {
    private let onTap: () -> Void

    init(label: String, emoji: String, onTap: @escaping () -> Void) {
        self.onTap = onTap
        super.init(frame: .zero)
        let glyph = UILabel()
        glyph.text = emoji
        glyph.font = .systemFont(ofSize: 20)
        glyph.textAlignment = .center
        let caption = UILabel()
        caption.text = label
        caption.font = .systemFont(ofSize: 9)
        caption.textColor = .secondaryLabel
        caption.textAlignment = .center
        let stack = UIStackView(arrangedSubviews: [glyph, caption])
        stack.axis = .vertical
        stack.alignment = .fill
        stack.spacing = 0
        stack.isUserInteractionEnabled = false
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        NSLayoutConstraint.activate([
            stack.centerXAnchor.constraint(equalTo: centerXAnchor),
            stack.centerYAnchor.constraint(equalTo: centerYAnchor),
            stack.leadingAnchor.constraint(greaterThanOrEqualTo: leadingAnchor),
            stack.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor),
        ])
        // 读屏文案同 Android `EmojiBrowse` 的 contentDescription。
        accessibilityLabel = "小欧\(label)，提交 \(emoji)"
        addTarget(self, action: #selector(tapped), for: .touchUpInside)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    @objc private func tapped() { onTap() }
}
