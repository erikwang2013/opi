// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · **CI 已真编译**（apple.yml 的 iOS job，2026-10-15）· **从未上机运行**
//
// 本文件在 Linux 上编写。**已过 `swiftc ios/*.swift`（-typecheck + -emit-library）** ——
// 2026-10-15 那次全目录真编译（同一轮还当场抓出一处闭包里的 `continue`，见 RarePanel.swift）；
// 但从未在设备或模拟器上跑过：**行为仍未验证**，别把「编过」读成「能用」。
// UIKit 是 Apple 独有框架，本机 `import UIKit` 就会断。
//
// 输入模式**菜单**（2026-10-14）：九项模式的选择面板。参照实现是 Android
// `ime/ImeToolbar.kt` 的 `InputModePanel`（表头「输入模式 / 选中即换」+ 一排 chip），
// 抄语义与次序，不抄 Compose 语法。
//
// 与 `SymbolPanel` / `EmojiPanel` 同一个面板惯用法（**别再发明第三套**）：
// 一个 UIView 子类 + 回调解包，摆在 `KeyboardLayout` 里与 `rowsStack` **同位置、
// 互斥显示**，点击通过既有的 `didTapFunction` 单入口上报。
//
// ⚠️ 本面板**不碰引擎也不碰键盘层**：`onSelect` 只把「用户选了哪一项」报上去，
// 换方案/换键面/切层全在 `KeyboardViewController.applyInputMode(_:)`
// （`InputModes.swift`）—— 面板只管画与报。

import UIKit

/// 输入模式菜单。**选中即换**（Android 表头就这么写）：点一下 chip 立刻生效，
/// 不留「确认」按钮 —— 换模式是可逆的低风险动作，多一步确认只是把常用路径变长。
final class InputModePanel: UIView {

    /// 选中一项（控制器用它走 `applyInputMode`）。
    ///
    /// ⚠️ Android 的 `InputModePanel` **点完不关面板**（其 `PanelHeader` 注释：
    /// 键盘模式常常连开两三个）。这里**关**（iOS 侧由控制器在 `applyInputMode` 里
    /// 切回字母层）—— 理由不同端不同：Android 那个面板是**叠加在键盘上方**的横条，
    /// 收起与否不影响打字；本端面板**占掉整个键面**（与符号/表情面板同位置），
    /// 不收起用户就没法接着打字。这是有意的分歧，不是漏抄。
    var onSelect: ((InputMode) -> Void)?
    /// 「返回」→ 回字母层（由 `KeyboardLayout` 转成 `.showLetters`）。
    var onClose: (() -> Void)?

    /// 每行几个 chip（9 项 → 3 行）。
    private static let perRow = 3
    private static let chipHeight: CGFloat = 40

    private let titleLabel = UILabel()
    private let backButton = BarButton(title: "‹ 返回") {}
    private let gridStack = UIStackView()
    /// 九颗 chip（建一次；`show(current:)` 只改选中样式 —— 9 个静态项不值得重建）。
    private var chips: [InputMode: BarButton] = [:]

    init() {
        super.init(frame: .zero)
        // 表头：Android 是「输入模式」+ 副标题「选中即换」两行；本端并成一行。
        titleLabel.text = "输入模式 · 选中即换"
        titleLabel.font = .systemFont(ofSize: 14)
        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        backButton.addTarget(self, action: #selector(tappedBack), for: .touchUpInside)
        gridStack.axis = .vertical
        gridStack.spacing = 6
        gridStack.translatesAutoresizingMaskIntoConstraints = false

        addSubview(titleLabel)
        addSubview(backButton)
        addSubview(gridStack)
        NSLayoutConstraint.activate([
            backButton.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 4),
            backButton.topAnchor.constraint(equalTo: topAnchor, constant: 4),
            titleLabel.leadingAnchor.constraint(equalTo: backButton.trailingAnchor, constant: 8),
            titleLabel.centerYAnchor.constraint(equalTo: backButton.centerYAnchor),
            titleLabel.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor, constant: -8),
            // 表头高 40：与面包屑行（`RarePanel`）同一节奏。
            gridStack.topAnchor.constraint(equalTo: backButton.bottomAnchor, constant: 4),
            gridStack.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 3),
            gridStack.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -3),
        ])
        buildChips()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    /// 九项按**声明次序**（= Android `InputMode.entries` 的次序）铺 3×N 格。
    private func buildChips() {
        let all = InputMode.allCases
        for start in stride(from: 0, to: all.count, by: Self.perRow) {
            let end = min(start + Self.perRow, all.count)
            let row = UIStackView()
            row.axis = .horizontal
            row.spacing = 5
            row.distribution = .fillEqually
            for mode in all[start..<end] {
                let chip = BarButton(title: mode.label) { [weak self] in
                    self?.onSelect?(mode)
                }
                // 模式名最长 5 字（「拼音大9键」「生僻字键盘」）；BarButton 默认 18pt
                // 会在 1/3 宽里挤成省略号 —— 收到 15。
                chip.titleLabel?.font = .systemFont(ofSize: 15)
                chips[mode] = chip
                row.addArrangedSubview(chip)
            }
            row.heightAnchor.constraint(equalToConstant: Self.chipHeight).isActive = true
            gridStack.addArrangedSubview(row)
        }
    }

    /// 同步「当前是哪一项」的高亮。**读侧只有这一个来源**：控制器在切层前把
    /// `layout.inputMode` 传进来（别在这里自己问引擎 —— 引擎只有 scheme+mode，
    /// 反推不出大 9 键与生僻字这两种面板态，见 `InputMode.kt` 的同一条注释）。
    func show(current: InputMode) {
        for (mode, chip) in chips {
            let selected = (mode == current)
            chip.backgroundColor = selected ? .systemBlue : .secondarySystemBackground
            chip.setTitleColor(selected ? .white : .label, for: .normal)
        }
    }

    @objc private func tappedBack() { onClose?() }
}
