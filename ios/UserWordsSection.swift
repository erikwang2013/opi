// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 用户词库一节（2026-10-23）：设置面板里的「清除用户词库」。
//
// 语义对齐 Android 设置页的同名项：清掉引擎里**学习积累**的用户词（选词频次带来
// 的加权），不碰词库文件本身。iOS 端的特别之处必须如实写出来：
//   · 本端没有用户词落盘 —— 不调 `exportUserWords()` / `importUserWords()`，
//     引擎进程结束就没有了；所以「清除」只影响当前运行的引擎实例，
//     不存在要一起清的本地副本（代码里也没有这样的存储）。
//
// 确认用**面板内两步**而不是 `UIAlertController`：键盘扩展没有可依赖的
// presentation 上下文（宿主 App 的 VC 不在本进程），面板内确认在任何宿主里都成立。
//
// 本文件只画与发回调；`opi_clear_user_words()` 由控制器调（引擎在控制器手里）。
// ⚠️ 本机无 UIKit，只在 Apple CI 真编译。

import UIKit

final class UserWordsSection: UIView {

    /// 用户点了「确认清除」。控制器负责 `engine.clearUserWords()` + `refresh()`。
    var onClear: (() -> Void)?

    private let stack = UIStackView()
    private let clearButton = UIButton(type: .system)
    private let confirmRow = UIStackView()
    private let cancelButton = UIButton(type: .system)
    private let confirmButton = UIButton(type: .system)
    private let statusLabel = UILabel()

    init() {
        super.init(frame: .zero)
        stack.axis = .vertical
        stack.spacing = 4
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: trailingAnchor),
            stack.topAnchor.constraint(equalTo: topAnchor),
            stack.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])

        clearButton.setTitle("清除用户词库", for: .normal)
        clearButton.titleLabel?.font = .systemFont(ofSize: 13)
        clearButton.contentEdgeInsets = UIEdgeInsets(top: 6, left: 4, bottom: 6, right: 4)
        clearButton.accessibilityHint = "清除后需要再次确认"
        clearButton.addTarget(self, action: #selector(tapClear), for: .touchUpInside)
        stack.addArrangedSubview(clearButton)
        clearButton.heightAnchor.constraint(equalToConstant: 34).isActive = true

        cancelButton.setTitle("取消", for: .normal)
        cancelButton.titleLabel?.font = .systemFont(ofSize: 13)
        confirmButton.setTitle("确认清除", for: .normal)
        confirmButton.titleLabel?.font = .systemFont(ofSize: 13, weight: .bold)
        cancelButton.addTarget(self, action: #selector(tapCancel), for: .touchUpInside)
        confirmButton.addTarget(self, action: #selector(tapConfirm), for: .touchUpInside)
        confirmRow.axis = .horizontal
        confirmRow.spacing = 6
        confirmRow.distribution = .fillEqually
        confirmRow.addArrangedSubview(cancelButton)
        confirmRow.addArrangedSubview(confirmButton)
        confirmRow.isHidden = true
        stack.addArrangedSubview(confirmRow)
        confirmRow.heightAnchor.constraint(equalToConstant: 34).isActive = true

        statusLabel.text = ""
        statusLabel.font = .systemFont(ofSize: 10)
        statusLabel.numberOfLines = 0
        stack.addArrangedSubview(statusLabel)

        applyTheme(.light)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    /// 面板每次打开时复位：不留着上一次「确认清除」的待确认态。
    func reset() {
        confirmRow.isHidden = true
        clearButton.isHidden = false
        statusLabel.text = ""
    }

    func applyTheme(_ p: KeyboardPalette) {
        statusLabel.textColor = p.barDimFg
        clearButton.setTitleColor(.systemRed, for: .normal)
        confirmButton.setTitleColor(.systemRed, for: .normal)
        cancelButton.setTitleColor(p.panelFg, for: .normal)
        for b in [cancelButton, confirmButton] {
            b.backgroundColor = p.keyBg
            b.layer.cornerRadius = 7
            b.layer.borderWidth = 1
            b.layer.borderColor = p.barDimFg.withAlphaComponent(0.35).cgColor
        }
        clearButton.accessibilityLabel = "清除用户词库"
        clearButton.accessibilityHint = "点一次进入确认，再点确认才会清除"
    }

    // MARK: - 两步确认

    @objc private func tapClear() {
        clearButton.isHidden = true
        confirmRow.isHidden = false
        statusLabel.text = "确定清除全部用户词？学习排序会立即回落，且不可恢复。"
    }

    @objc private func tapCancel() {
        reset()
    }

    @objc private func tapConfirm() {
        onClear?()
        confirmRow.isHidden = true
        clearButton.isHidden = false
        statusLabel.text = "已清除全部用户词。"
    }
}
