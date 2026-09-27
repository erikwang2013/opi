// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写，未经过任何 Swift 编译器 —— 一个字符都没有编译过。
// 见 ios/README.md「在 Mac 上第一件要做的事」：先让它编译过，再谈功能。
//
// 符号面板：引擎符号库的**展示层**（竖向滚动的等宽网格）。
//
// 为什么单独一个文件：键位行是**编译期常量**（`KeyboardLayout.letterRows`），
// 符号是**运行时数据**（引擎给的常用集），生命周期也不同（键位行建一次，
// 符号每次开面板重取）。顺带把 `KeyboardLayout.swift` 压回本仓的 500 行源码上限内。
//
// 数据来自 `OpiEngine.commonSymbols()` —— 它是 `opi_symbol_blocks()`（只回 common 块）
// 与 `opi_symbols_in_block(id)` 的并集，**不在 Swift 里另抄一份符号表**：
// 本项目已被「同一语义抄多份」坑过，而 Android 侧同样只把符号交给引擎
// （`SymbolCatalog` + `SymbolPanel`，见 `android/app/src/main/kotlin/io/opi/input/keyboard/`）。

import UIKit

/// 符号网格。只负责画与发射点击，**不上屏、不碰引擎**。
final class SymbolPanel: UIView {

    /// 点中一个符号。上屏由控制器做（它才知道 textDocumentProxy）。
    var onPick: ((String) -> Void)?

    /// 每行几个（与 Android `SymbolPanel` 的网格同粒度）。
    private static let perRow = 8
    /// 行高。⚠️ 未编译：40pt 是拍的，真机上按键高调。
    /// 滚动视图里的行**必须**有明确高度 —— `KeyboardLayout` 的字母层用的是
    /// `.fillEqually`（按容器高度平分），那在滚动视图里会退化成 0 高。
    private static let rowHeight: CGFloat = 40

    private let scroll = UIScrollView()
    private let stack = UIStackView()

    init() {
        super.init(frame: .zero)
        scroll.showsVerticalScrollIndicator = true
        scroll.translatesAutoresizingMaskIntoConstraints = false
        stack.axis = .vertical
        stack.spacing = 6
        stack.translatesAutoresizingMaskIntoConstraints = false
        scroll.addSubview(stack)
        addSubview(scroll)

        // ⚠️ 未编译：滚动的约束写法（contentLayoutGuide 定内容、宽度钉 frameLayoutGuide
        // 防横向滚动）是候选栏那套的竖向版本，Mac 上确认一遍。
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

    /// 装入符号（来自 `OpiEngine.commonSymbols()`）。
    ///
    /// 空列表**不休假成空白**：它多半是「引擎还没就绪 / 词库没装上」而不是
    /// 「真的没有符号」，留白会让用户以为键盘坏了 —— 与 fcitx5 轨「词库损坏时
    /// 插件静默全失效」是同一个坑，所以画一行可见的提示。
    ///
    /// ⚠️ 调用方每开一次面板就重取一次、本层**不做缓存**：空结果是「没拿到」，
    /// 缓存它会让面板从此永远空（Android `SymbolCatalog` 踩过这个坑，
    /// 见那里「空结果不写进缓存」的注释）。
    func setSymbols(_ symbols: [String]) {
        stack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        guard !symbols.isEmpty else {
            stack.addArrangedSubview(BarButton(title: "（符号库未就绪）") {})
            return
        }
        for start in stride(from: 0, to: symbols.count, by: Self.perRow) {
            let end = min(start + Self.perRow, symbols.count)
            let row = makeRow(Array(symbols[start..<end]))
            row.heightAnchor.constraint(equalToConstant: Self.rowHeight).isActive = true
            stack.addArrangedSubview(row)
        }
        // 换了一批符号就把滚动位置收回顶部 —— 否则上一批的滚动偏移会留着，
        // 面板看起来像「打开就是空的」（内容其实在视口上方）。
        scroll.setContentOffset(.zero, animated: false)
    }

    private func makeRow(_ texts: [String]) -> UIStackView {
        let row = UIStackView()
        row.axis = .horizontal
        row.spacing = 5
        row.distribution = .fillEqually
        for t in texts {
            row.addArrangedSubview(BarButton(title: t) { [weak self] in
                self?.onPick?(t)
            })
        }
        return row
    }
}
