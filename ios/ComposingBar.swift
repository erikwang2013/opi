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
    static let height: CGFloat = 24

    private let label = UILabel()
    private var palette: KeyboardPalette = .light

    init() {
        super.init(frame: .zero)
        backgroundColor = .clear
        label.textAlignment = .left
        label.numberOfLines = 1
        // 长缓冲（五笔/双拼码串）以省略号收尾，不把行撑宽。
        label.lineBreakMode = .byTruncatingTail
        label.translatesAutoresizingMaskIntoConstraints = false
        // 显示串由引擎给，不再在 Swift 里二次分词（`'` 的切分规则只有引擎一份）。
        addSubview(label)
        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 10),
            label.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -10),
            label.centerYAnchor.constraint(equalTo: centerYAnchor),
        ])
        setText("")
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    /// 显示串（已由引擎分词）。空串 = 无组合输入：清空文字并撤掉底色带，
    /// 但**槽位高度保留**（见文件头注）。
    func setText(_ text: String) {
        label.text = text
        label.isHidden = text.isEmpty
        backgroundColor = text.isEmpty ? .clear : palette.barBg
    }

    func applyTheme(_ p: KeyboardPalette, fontScale: CGFloat) {
        palette = p
        label.textColor = p.barBufferFg
        label.font = .systemFont(ofSize: 15 * fontScale)
        // 保持当前文字/底色与新色板一致（重开面板/切皮肤时）。
        setText(label.text ?? "")
    }
}
