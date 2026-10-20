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
// `ime/ImeToolbar.kt` 的 `InputModePanel`（表头「输入模式 / 选中即换」+ 可滚动的
// chip 区），抄语义与次序，不抄 Compose 语法。
//
// 2026-10-21 用户报障「输入模式页面错乱」后重排：**固定 3 列网格 + 整页竖向滚动**
// （窄屏/横屏/长辈字号下都不裁字），选中样式改成 Android `PanelChip` 同款
// （键底色 + 强调色描边 + 粗体 + 行尾 ✓），色值全部走当前色板、不再写系统色。
// 表头沿用 `KeyboardPanelHeader`：返回箭头单独一行（2026-10-21 用户裁决）。
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
    private static let margin: CGFloat = 8

    private let header = KeyboardPanelHeader(title: "输入模式", hint: "选中即换")
    private let scroll = UIScrollView()
    private let rows = UIStackView()
    /// 九颗 chip（建一次；`show(current:)` 只改选中样式 —— 9 个静态项不值得重建）。
    private var chips: [InputMode: BarButton] = [:]
    private var current: InputMode = .pinyin26
    private var palette: KeyboardPalette = .light

    init() {
        super.init(frame: .zero)
        backgroundColor = .light.panelBg
        // 表头统一为「返回箭头单独一行 + 标题/提示下一行」（2026-10-21 用户裁决）。
        header.onBack = { [weak self] in self?.onClose?() }

        // 滚动容器：横屏（键区只有 ~150pt 高）时 3 行 chip 放不下，必须能滚。
        // `delaysContentTouches = false` 让点击立刻落到 chip 上（默认值会把单击
        // 压到 150ms 的 pan 判定之后，连续点选会显得「慢/没响应」）。
        scroll.translatesAutoresizingMaskIntoConstraints = false
        scroll.alwaysBounceVertical = false
        scroll.showsVerticalScrollIndicator = true
        scroll.delaysContentTouches = false

        rows.axis = .vertical
        rows.spacing = 6
        rows.translatesAutoresizingMaskIntoConstraints = false
        scroll.addSubview(rows)

        for v in [header, scroll] {
            v.translatesAutoresizingMaskIntoConstraints = false
            addSubview(v)
        }
        NSLayoutConstraint.activate([
            header.leadingAnchor.constraint(equalTo: leadingAnchor),
            header.trailingAnchor.constraint(equalTo: trailingAnchor),
            header.topAnchor.constraint(equalTo: topAnchor),
            scroll.leadingAnchor.constraint(equalTo: leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: trailingAnchor),
            scroll.topAnchor.constraint(equalTo: header.bottomAnchor, constant: 4),
            scroll.bottomAnchor.constraint(equalTo: bottomAnchor),

            rows.leadingAnchor.constraint(
                equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: Self.margin
            ),
            rows.trailingAnchor.constraint(
                equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -Self.margin
            ),
            rows.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor),
            rows.bottomAnchor.constraint(
                equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -4
            ),
            rows.widthAnchor.constraint(
                equalTo: scroll.frameLayoutGuide.widthAnchor, constant: -2 * Self.margin
            ),
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
            row.spacing = 6
            row.distribution = .fillEqually
            row.heightAnchor.constraint(equalToConstant: Self.chipHeight).isActive = true
            for mode in all[start..<end] {
                let chip = BarButton(title: mode.label) { [weak self] in
                    self?.onSelect?(mode)
                }
                chip.titleLabel?.adjustsFontSizeToFitWidth = true
                chip.titleLabel?.minimumScaleFactor = 0.7
                chip.titleLabel?.lineBreakMode = .byClipping
                // BarButton 默认 18pt + 左右各 8 的内边距，在 1/3 宽（320pt 屏约
                // 95pt）里会把「生僻字键盘」+ ✓ 挤成省略号；收到 14pt + 4 内边距。
                chip.contentEdgeInsets = UIEdgeInsets(top: 4, left: 4, bottom: 4, right: 4)
                chip.layer.cornerRadius = 8
                chip.clipsToBounds = true
                chips[mode] = chip
                row.addArrangedSubview(chip)
            }
            rows.addArrangedSubview(row)
        }
    }

    /// 同步「当前是哪一项」的高亮。**读侧只有这一个来源**：控制器在切层前把
    /// `layout.inputMode` 传进来（别在这里自己问引擎 —— 引擎只有 scheme+mode，
    /// 反推不出大 9 键与生僻字这两种面板态，见 `InputMode.kt` 的同一条注释）。
    func show(current: InputMode) {
        self.current = current
        for (mode, chip) in chips {
            let selected = (mode == current)
            chip.setTitle(selected ? "\(mode.label) ✓" : mode.label, for: .normal)
            chip.titleLabel?.font = .systemFont(
                ofSize: 14, weight: selected ? .bold : .regular
            )
            chip.setTitleColor(selected ? palette.accent : palette.panelFg, for: .normal)
            chip.backgroundColor = palette.keyBg
            chip.layer.borderWidth = selected ? 2 : 1
            chip.layer.borderColor = (selected
                ? palette.accent
                : palette.barDimFg.withAlphaComponent(0.35)).cgColor
            chip.accessibilityTraits = selected ? [.button, .selected] : .button
            chip.accessibilityLabel = "输入模式 \(mode.label)" + (selected ? "，已选中" : "")
        }
    }

    func applyTheme(_ p: KeyboardPalette) {
        palette = p
        backgroundColor = p.panelBg
        header.applyTheme(p)
        // 色板换了要把 chip 的边框/字色重刷一遍（`show` 是唯一的样式入口）。
        show(current: current)
    }
}
