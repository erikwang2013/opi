// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 顶部候选栏：左右翻页键 + 横向滚动的候选 + 最右的英文隐藏「中」键。
//
// 2026-10-20 从 KeyboardLayout.swift 拆出（加隐藏「中」键后那边越过 500 行门禁；
// 按职责拆：顶部栏 vs 键位/面板摆放）。三枚子视图是 KeyboardLayout 的属性，
// 故那边去掉了 `private`（Swift 的 private 只跨同文件扩展可见）。
//
// ⚠️ 未编译：本文件与其它 iOS 源文件一样只在 Apple CI 上真编译，本机无 UIKit。

import UIKit

extension KeyboardLayout {

    /// 建候选栏：页翻键走 `opi_key_event` 的 PAGE_UP/PAGE_DOWN（页码钳制在 router.rs）。
    func buildCandidateBar() {
        candidateScroll.showsHorizontalScrollIndicator = false
        candidateScroll.translatesAutoresizingMaskIntoConstraints = false
        candidateStack.axis = .horizontal
        candidateStack.spacing = 8
        candidateStack.translatesAutoresizingMaskIntoConstraints = false

        let prev = makeBarButton("‹") { [weak self] in
            guard let self else { return }
            self.delegate?.keyboardLayout(self, didRequestPage: -1)
        }
        let next = makeBarButton("›") { [weak self] in
            guard let self else { return }
            self.delegate?.keyboardLayout(self, didRequestPage: 1)
        }

        // 隐藏「中」键：小号低对比度，按下时系统高亮让它可见。
        chineseButton.setTitle("中", for: .normal)
        chineseButton.titleLabel?.font = .systemFont(ofSize: 13)
        chineseButton.setTitleColor(
            UIColor.secondaryLabel.withAlphaComponent(0.4),
            for: .normal
        )
        chineseButton.setTitleColor(.label, for: .highlighted)
        chineseButton.translatesAutoresizingMaskIntoConstraints = false
        chineseButton.isHidden = true
        chineseButton.addTarget(self, action: #selector(tapChinese), for: .touchUpInside)

        addSubview(prev)
        addSubview(candidateScroll)
        addSubview(next)
        addSubview(chineseButton)
        candidateScroll.addSubview(candidateStack)

        // ⚠️ 未编译：以下约束按常见写法写，Mac 上大概率要调数值/优先级。
        NSLayoutConstraint.activate([
            prev.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 4),
            prev.centerYAnchor.constraint(equalTo: candidateScroll.centerYAnchor),
            prev.widthAnchor.constraint(equalToConstant: 28),

            chineseButton.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -4),
            chineseButton.centerYAnchor.constraint(equalTo: candidateScroll.centerYAnchor),
            chineseButton.widthAnchor.constraint(equalToConstant: 26),
            chineseButton.heightAnchor.constraint(equalToConstant: 26),

            next.trailingAnchor.constraint(equalTo: chineseButton.leadingAnchor, constant: -2),
            next.centerYAnchor.constraint(equalTo: candidateScroll.centerYAnchor),
            next.widthAnchor.constraint(equalToConstant: 28),

            candidateScroll.leadingAnchor.constraint(equalTo: prev.trailingAnchor, constant: 4),
            candidateScroll.trailingAnchor.constraint(equalTo: next.leadingAnchor, constant: -4),
            candidateScroll.topAnchor.constraint(equalTo: topAnchor),
            candidateScroll.heightAnchor.constraint(equalToConstant: 40),

            candidateStack.leadingAnchor.constraint(equalTo: candidateScroll.contentLayoutGuide.leadingAnchor),
            candidateStack.trailingAnchor.constraint(equalTo: candidateScroll.contentLayoutGuide.trailingAnchor),
            candidateStack.topAnchor.constraint(equalTo: candidateScroll.contentLayoutGuide.topAnchor),
            candidateStack.bottomAnchor.constraint(equalTo: candidateScroll.contentLayoutGuide.bottomAnchor),
            candidateStack.heightAnchor.constraint(equalTo: candidateScroll.frameLayoutGuide.heightAnchor),
        ])
    }

    private func makeBarButton(_ title: String, onTap: @escaping () -> Void) -> UIButton {
        let b = BarButton(title: title, onTap: onTap)
        b.translatesAutoresizingMaskIntoConstraints = false
        return b
    }

    /// 隐藏「中」键：复用中/繁/英循环的 `.cycleMode`（英文档 = 清缓冲 + 回拼音，
    /// 与 Android 隐藏键走 `cycleMode()` 的英→中腿同义），不新造委托方法。
    @objc private func tapChinese() {
        delegate?.keyboardLayout(self, didTapFunction: .cycleMode)
    }
}
