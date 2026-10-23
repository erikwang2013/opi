// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · **CI 已真编译**（apple.yml 的 iOS job，2026-10-15）· **从未上机运行**
//
// 本文件在 Linux 上编写。**已过 `swiftc ios/*.swift`（-typecheck + -emit-library）** ——
// 2026-10-15 那次全目录真编译（同一轮还当场抓出一处闭包里的 `continue`，见 RarePanel.swift）；
// 但从未在设备或模拟器上跑过：**行为仍未验证**，别把「编过」读成「能用」。
//
// 键面渲染的**尺寸算法**（`[ModeRow]` → 视图树）。单开一个文件的理由：
// 它与 `InputModes.swift` 的键位表是一对（改表就要动行高/列宽权的理解），但那段
// 已经从 `KeyboardLayout.swift` 搬过一次、`InputModes.swift` 也因此顶到了 500 行
// 源码上限的边上（拆文件比删注释好，本仓的既有取舍）。
//
// **触摸接线与视图树装配仍在 `KeyboardLayout`**（`wire` 闭包由它传进来）——
// 这里只造按钮 + 定尺寸，不注册任何手势、不判层、不碰引擎。

import UIKit

/// 把 `[ModeRow]` 画进一个竖排 stack。
///
/// 尺寸算式与 `KeyboardLayout` 的 26 键行**同一套**（曾在那里，现收口到这里）：
/// 每个格子的宽 = 行宽 × 比例 − 本格分摊的空隙，各格求和 = 行宽 − 空隙总数；
/// 行高同理（相对 stack 高）。26 键/14 键/笔画的行高权重恒 1 ⇒ 对它们是恒等变换。
enum ModeKeyRenderer {

    /// 渲染。`stack` 的 `distribution` 必须是 `.fill`（行高由本函数给的约束决定，
    /// `.fillEqually` 会与之冲突）；`wire` 在**每个**按钮造好后立刻回调一次。
    static func render(
        _ rows: [ModeRow],
        into stack: UIStackView,
        gap: CGFloat,
        wire: (KeyButton) -> Void
    ) {
        let totalUnits = rows.reduce(0) { $0 + $1.heightUnits }
        let rowGaps = stack.spacing * CGFloat(max(0, rows.count - 1))
        for model in rows {
            let row = UIStackView()
            row.axis = .horizontal
            row.spacing = gap
            // .fill（不是 .fillProportionally）：宽度由下面的显式比例约束决定。
            // fillProportionally 只看 intrinsicContentSize，会无视 widthUnits。
            row.distribution = .fill
            let total = model.cells.reduce(0) { $0 + $1.widthUnits }
            // 键间空隙要从总宽里扣掉，否则「各键比例之和 = 1」再加上空隙会超出行宽，
            // 结果是约束冲突（布局崩 + 控制台刷警告）。
            let gaps = gap * CGFloat(max(0, model.cells.count - 1))
            for cell in model.cells {
                let view = makeView(cell, gap: gap, wire: wire)
                row.addArrangedSubview(view)
                // v.width = row.width × 比例 − 本格分摊的空隙
                let frac = cell.widthUnits / total
                view.widthAnchor.constraint(
                    equalTo: row.widthAnchor, multiplier: frac, constant: -gaps * frac
                ).isActive = true
            }
            stack.addArrangedSubview(row)
            // 行高同上：row.height = stack.height × 比例 − 本行分摊的行距。
            let rfrac = model.heightUnits / totalUnits
            row.heightAnchor.constraint(
                equalTo: stack.heightAnchor, multiplier: rfrac, constant: -rowGaps * rfrac
            ).isActive = true
        }
    }

    /// 一个格子 → 一个视图（单键 = 按钮；列 = 竖排 stack，列内等高）。
    private static func makeView(_ cell: ModeCell, gap: CGFloat, wire: (KeyButton) -> Void) -> UIView {
        switch cell {
        case .key(let spec):
            let b = KeyButton(spec: spec)
            wire(b)
            return b
        case .column(let specs):
            let col = UIStackView()
            col.axis = .vertical
            col.spacing = gap
            // 列内等高（Android：标点列五格、右列三格各占等分）。
            col.distribution = .fillEqually
            for spec in specs {
                let b = KeyButton(spec: spec)
                wire(b)
                col.addArrangedSubview(b)
            }
            return col
        case .scrollingColumn(let specs):
            return makeScrollingColumn(specs, gap: gap, wire: wire)
        }
    }

    /// 可竖向滚动的一列（9 键左标点列，2026-10-24）。
    ///
    /// 返回一个**容器**（而不是 UIScrollView 本身）：读数浮层要贴在容器上、
    /// 不随列内容滚动（`installReadingColumn` 的 4 条约束按容器算）。
    /// 每格固定为容器高的 1/5 —— 六格内容 = 1.2 倍高，多出来的一格靠滚动露出。
    private static func makeScrollingColumn(
        _ specs: [KeySpec], gap: CGFloat, wire: (KeyButton) -> Void
    ) -> UIView {
        let container = UIView()
        let scroll = UIScrollView()
        scroll.showsVerticalScrollIndicator = false
        // 键盘上的点击要立刻生效；纵向拖动仍由滚动手势接管。
        scroll.delaysContentTouches = false
        scroll.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(scroll)

        let col = UIStackView()
        col.axis = .vertical
        col.spacing = gap
        col.distribution = .fill
        col.translatesAutoresizingMaskIntoConstraints = false
        scroll.addSubview(col)
        NSLayoutConstraint.activate([
            scroll.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            scroll.topAnchor.constraint(equalTo: container.topAnchor),
            scroll.bottomAnchor.constraint(equalTo: container.bottomAnchor),
            col.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor),
            col.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor),
            col.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor),
            col.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor),
            col.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor),
        ])
        for spec in specs {
            let b = KeyButton(spec: spec)
            wire(b)
            col.addArrangedSubview(b)
            b.widthAnchor.constraint(equalTo: col.widthAnchor).isActive = true
            b.heightAnchor.constraint(equalTo: container.heightAnchor, multiplier: 0.2).isActive = true
        }
        return container
    }
}
