// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 键面浮层（2026-10-24）：长按/上滑的按键浮层 + 9 键左列的自定义符号选择器。
//
// 浮层是**只显示、不接触摸**的视图：手指仍留在原来的按键上，选哪一项由
// `KeyboardTouch.swift` 的手势按横向位移算 —— 这样抬手事件不会从按键上丢走，
// 常规点击（未触发浮层）仍由 UIButton 自己处理。键盘扩展里没有额外窗口，
// 所以浮层直接挂在 `KeyboardLayout` 上（不会离开键盘区域，也就不会被系统收回）。
//
// ⚠️ 本机无 UIKit，只在 Apple CI 真编译；浮层坐标/手感从未上机验证。

import UIKit

/// 一个按键的浮层：条目字号**按声明次序递减**，默认项高亮。
final class KeyPopup: UIView {

    private let items: [KeyPopupItem]
    private var labels: [UILabel] = []
    private(set) var selectedIndex = 0
    private let palette: KeyboardPalette

    /// 当前选中项的提交值（走普通按键进引擎，见 `KeyboardTouch.commitKeyPopup`）。
    var selectedValue: String {
        items.indices.contains(selectedIndex) ? items[selectedIndex].value : ""
    }

    /// true = 该项直接原样上屏（大写/数字/符号）；false = 进引擎匹配词库。
    var selectedDirect: Bool {
        items.indices.contains(selectedIndex) ? items[selectedIndex].direct : false
    }

    /// 条目数与初选下标（`KeyboardTouch` 按它们算宽度、把横向拖动换算成选项）。
    var itemCount: Int { items.count }
    let defaultIndex: Int

    /// 来源键（手势判断浮层属于哪颗键；弱持有，键面重建时不拦销毁）。
    weak var source: KeyButton?
    /// 开浮层时手指的 x（按键坐标）：选中项按**相对位移**算，按在键边也不会跳项。
    var anchorX: CGFloat = 0

    init(items: [KeyPopupItem], defaultIndex: Int, palette: KeyboardPalette, fontScale: CGFloat) {
        self.items = items
        self.defaultIndex = defaultIndex
        self.palette = palette
        super.init(frame: .zero)
        isUserInteractionEnabled = false
        backgroundColor = palette.keyBg
        layer.cornerRadius = 8
        layer.borderWidth = 1
        layer.borderColor = palette.accent.withAlphaComponent(0.7).cgColor

        let row = UIStackView()
        row.axis = .horizontal
        row.distribution = .fillEqually
        row.translatesAutoresizingMaskIntoConstraints = false
        addSubview(row)
        NSLayoutConstraint.activate([
            row.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 4),
            row.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -4),
            row.topAnchor.constraint(equalTo: topAnchor, constant: 4),
            row.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -4),
        ])
        var scale: CGFloat = 1
        for item in items {
            let label = UILabel()
            label.text = item.label
            label.textAlignment = .center
            label.font = .systemFont(ofSize: max(10, 22 * fontScale * scale))
            label.adjustsFontSizeToFitWidth = true
            label.minimumScaleFactor = 0.6
            row.addArrangedSubview(label)
            labels.append(label)
            scale *= 0.8
        }
        setSelected(defaultIndex)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 浮层全代码构建") }

    /// 选中第 index 项（夹到两端）。高亮 = 强调色 + 粗体。
    func setSelected(_ index: Int) {
        guard !items.isEmpty else { return }
        let index = min(items.count - 1, max(0, index))
        selectedIndex = index
        for (i, label) in labels.enumerated() {
            let on = i == index
            label.textColor = on ? palette.accent : palette.keyFg
            label.font = .systemFont(
                ofSize: label.font.pointSize, weight: on ? .bold : .regular
            )
        }
    }

}

/// 常用符号选择器（9 键左标点列末尾的自定义键打开）。
/// 数据由控制器现取（`OpiEngine.commonSymbols()`，空结果用内置兜底表）。
final class PunctuationPicker: UIView {

    var onPick: ((String) -> Void)?
    var onClose: (() -> Void)?

    private let header = KeyboardPanelHeader(
        title: "常用符号", hint: "点一个符号，设到左列末尾的自定义键"
    )
    private let scroll = UIScrollView()
    private let grid = UIStackView()
    private var buttons: [UIButton] = []
    private var symbols: [String] = []
    private var palette: KeyboardPalette = .light
    private var fontScale: CGFloat = 1

    /// 引擎没装载时 `commonSymbols()` 是空的：给一组标点兜底，别让选择器空白。
    private static let fallback = ["，", "。", "、", "？", "！", "…", "；", "：", "“", "”"]

    init() {
        super.init(frame: .zero)
        backgroundColor = palette.panelBg
        header.onBack = { [weak self] in self?.onClose?() }
        header.translatesAutoresizingMaskIntoConstraints = false
        scroll.showsVerticalScrollIndicator = false
        scroll.delaysContentTouches = false
        scroll.translatesAutoresizingMaskIntoConstraints = false
        grid.axis = .vertical
        grid.spacing = 4
        grid.translatesAutoresizingMaskIntoConstraints = false
        scroll.addSubview(grid)
        addSubview(header)
        addSubview(scroll)
        NSLayoutConstraint.activate([
            header.leadingAnchor.constraint(equalTo: leadingAnchor),
            header.trailingAnchor.constraint(equalTo: trailingAnchor),
            header.topAnchor.constraint(equalTo: topAnchor),
            scroll.leadingAnchor.constraint(equalTo: leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: trailingAnchor),
            scroll.topAnchor.constraint(equalTo: header.bottomAnchor),
            scroll.bottomAnchor.constraint(equalTo: bottomAnchor),
            grid.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 8),
            grid.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -8),
            grid.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor, constant: 4),
            grid.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -8),
            grid.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor, constant: -16),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 选择器全代码构建") }

    /// 换一批符号（同一个列表不重建）。
    func show(_ list: [String]) {
        let data = list.isEmpty ? Self.fallback : list
        guard data != symbols else { return }
        symbols = data
        rebuild()
    }

    func applyTheme(_ p: KeyboardPalette, fontScale: CGFloat) {
        palette = p
        self.fontScale = fontScale
        backgroundColor = p.panelBg
        header.applyTheme(p)
        for button in buttons {
            button.setTitleColor(p.panelFg, for: .normal)
            button.backgroundColor = p.keyBg
            button.titleLabel?.font = .systemFont(ofSize: 20 * fontScale)
        }
    }

    private func rebuild() {
        for row in grid.arrangedSubviews {
            row.removeFromSuperview()
        }
        buttons = []
        let perRow = 7
        for start in stride(from: 0, to: symbols.count, by: perRow) {
            let row = UIStackView()
            row.axis = .horizontal
            row.distribution = .fillEqually
            row.spacing = 4
            row.heightAnchor.constraint(equalToConstant: 40).isActive = true
            let end = min(start + perRow, symbols.count)
            for i in start..<end {
                let button = UIButton(type: .system)
                button.setTitle(symbols[i], for: .normal)
                button.titleLabel?.font = .systemFont(ofSize: 20 * fontScale)
                button.setTitleColor(palette.panelFg, for: .normal)
                button.backgroundColor = palette.keyBg
                button.layer.cornerRadius = 6
                button.tag = i
                button.addTarget(self, action: #selector(tap(_:)), for: .touchUpInside)
                row.addArrangedSubview(button)
                buttons.append(button)
            }
            // 末行补空位，格子不会因为行余额而变宽。
            for _ in end..<(start + perRow) {
                row.addArrangedSubview(UIView())
            }
            grid.addArrangedSubview(row)
        }
    }

    @objc private func tap(_ sender: UIButton) {
        guard symbols.indices.contains(sender.tag) else { return }
        onPick?(symbols[sender.tag])
    }
}
