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

    /// 点中一格文本（emoji / 颜文字；提交什么由面板/控制器决定）。
    var onPick: ((String) -> Void)?
    /// 点中一格「自有」贴纸（面板负责复制图片，网格不碰剪贴板）。
    var onSticker: ((PetEmojiTable.Form) -> Void)?
    /// 滚动位置变了（分类条高亮跟着反推）。
    var onScroll: (() -> Void)?

    /// 行高。滚动视图里的行**必须**有明确高度（容器 `.fillEqually` 在滚动视图里
    /// 会退化成 0 高，见 `SymbolPanel.init` 那条注释）。
    static let rowHeight: CGFloat = 40
    /// 贴纸行高：44 图 + 标签（自有子页 4 列，格子比 emoji 大）。
    static let stickerRowHeight: CGFloat = 64
    /// 行间距。与 `SymbolPanel` 的 `stack.spacing = 6` 是同一个数。
    static let gridSpacing: CGFloat = 6

    private let scroll = UIScrollView()
    private let stack = UIStackView()
    private var itemCount = 0
    private var columns = 8
    /// 当前网格的行高（贴纸 / 文本两套；`scrollTo` 与高亮反推都用它）。
    private var rowHeight: CGFloat = EmojiGridView.rowHeight

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

    /// 「自有」子页：4 列网格，格子里是**真实小欧 PNG**（`PetStickerImages` 内嵌）。
    /// 点击只发 `onSticker`（面板负责复制图片），不经过 `onPick`、不上屏。
    func showStickers(_ forms: [PetEmojiTable.Form], columns: Int) {
        self.columns = columns
        rowHeight = Self.stickerRowHeight
        itemCount = forms.count
        reset()
        guard !forms.isEmpty else {
            addEmpty("（贴纸未内嵌）")
            return
        }
        for start in stride(from: 0, to: forms.count, by: columns) {
            let end = min(start + columns, forms.count)
            let row = makeRow()
            for form in forms[start..<end] {
                row.addArrangedSubview(StickerCell(form: form) { [weak self] in
                    self?.onSticker?(form)
                })
            }
            padRow(row, filled: end - start, columns: columns)
            stack.addArrangedSubview(row)
        }
        scroll.setContentOffset(.zero, animated: false)
    }

    /// Emoji 子页：只画引擎表情表（`自有` 已是独立子页，不再混进来）。
    func showEmoji(_ emojis: [String], columns: Int) {
        self.columns = columns
        rowHeight = Self.rowHeight
        itemCount = emojis.count
        reset()
        guard !emojis.isEmpty else {
            // 空列表**不休假成空白**（同 SymbolPanel 的理由）。
            addEmpty("（表情库未就绪）")
            return
        }
        for start in stride(from: 0, to: emojis.count, by: columns) {
            let end = min(start + columns, emojis.count)
            let row = makeRow()
            for text in emojis[start..<end] {
                row.addArrangedSubview(BarButton(title: text) { [weak self] in
                    self?.onPick?(text)
                })
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
        rowHeight = Self.rowHeight
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
        let y = CGFloat(row) * (rowHeight + Self.gridSpacing)
        scroll.setContentOffset(CGPoint(x: 0, y: y), animated: true)
    }

    /// 首个可见项的下标（分类条高亮反推的输入）。滚动偏移是显式行高 + 行距拼的，可逆。
    func firstVisibleItemIndex() -> Int {
        guard itemCount > 0 else { return 0 }
        let stride = rowHeight + Self.gridSpacing
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
        row.heightAnchor.constraint(equalToConstant: rowHeight).isActive = true
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

/// 「自有」一格：小欧动画 + 小标签（例：[图] 笑）。
///
/// 图片优先来自 `PetStickerGifs`（内嵌的 12 张透明底 GIF，源
/// `shared/pet/stickers-gif/`）；GIF 缺失时退回 `PetStickerImages` 的静态 PNG。
/// 点击只把这一格报给面板（面板复制到剪贴板），**不提交任何文本**。
private final class StickerCell: UIButton {
    private let onTap: () -> Void

    init(form: PetEmojiTable.Form, onTap: @escaping () -> Void) {
        self.onTap = onTap
        super.init(frame: .zero)
        let animated = PetStickerGifs.animatedImage(named: form.sticker)
        let imageView = UIImageView(image: animated ?? PetStickerImages.image(named: form.sticker))
        imageView.contentMode = .scaleAspectFit
        imageView.isUserInteractionEnabled = false
        // 12 张图都内嵌在 `PetStickerGifs` / `PetStickerImages`；缺失时只露标签
        //（不静默画成 emoji 字形）。
        let caption = UILabel()
        caption.text = form.label
        caption.font = .systemFont(ofSize: 9)
        caption.textColor = .secondaryLabel
        caption.textAlignment = .center
        let stack = UIStackView(arrangedSubviews: [imageView, caption])
        stack.axis = .vertical
        stack.alignment = .center
        stack.spacing = 0
        stack.isUserInteractionEnabled = false
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        NSLayoutConstraint.activate([
            imageView.widthAnchor.constraint(equalToConstant: 44),
            imageView.heightAnchor.constraint(equalToConstant: 44),
            stack.centerXAnchor.constraint(equalTo: centerXAnchor),
            stack.centerYAnchor.constraint(equalTo: centerYAnchor),
            stack.leadingAnchor.constraint(greaterThanOrEqualTo: leadingAnchor),
            stack.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor),
        ])
        accessibilityLabel = "\(form.label)，复制图片到剪贴板"
        addTarget(self, action: #selector(tapped), for: .touchUpInside)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    @objc private func tapped() { onTap() }
}
