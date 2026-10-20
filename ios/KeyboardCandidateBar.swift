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
        // 44pt = 工具条同高（Android `IME_TOOLBAR_HEIGHT_DP`）：两者共用一个槽位，
        // 高度不同会让键区在「空闲 ↔ 输入」之间上下跳。
        candidateBar.translatesAutoresizingMaskIntoConstraints = false
        addSubview(candidateBar)
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

        candidateBar.addSubview(prev)
        candidateBar.addSubview(candidateScroll)
        candidateBar.addSubview(next)
        candidateBar.addSubview(chineseButton)
        candidateScroll.addSubview(candidateStack)

        // 「中」键隐藏时不能留 26pt 空位（用户要求候选栏占满整条）。两条互斥的
        // trailing 约束由 `showChineseButton(_:)` 切换 —— `isHidden` 本身不会
        // 撤销手写约束，原实现因此在中文候选栏右端留了一条空白。
        nextToChinese = next.trailingAnchor.constraint(
            equalTo: chineseButton.leadingAnchor, constant: -2
        )
        nextToBar = next.trailingAnchor.constraint(
            equalTo: candidateBar.trailingAnchor, constant: -4
        )

        // ⚠️ 未编译：以下约束按常见写法写，Mac 上大概率要调数值/优先级。
        NSLayoutConstraint.activate([
            candidateBar.leadingAnchor.constraint(equalTo: leadingAnchor),
            candidateBar.trailingAnchor.constraint(equalTo: trailingAnchor),
            candidateBar.topAnchor.constraint(equalTo: topAnchor),
            candidateBar.heightAnchor.constraint(equalToConstant: 44),

            prev.leadingAnchor.constraint(equalTo: candidateBar.leadingAnchor, constant: 4),
            prev.centerYAnchor.constraint(equalTo: candidateScroll.centerYAnchor),
            prev.widthAnchor.constraint(equalToConstant: 28),

            chineseButton.trailingAnchor.constraint(equalTo: candidateBar.trailingAnchor, constant: -4),
            chineseButton.centerYAnchor.constraint(equalTo: candidateScroll.centerYAnchor),
            chineseButton.widthAnchor.constraint(equalToConstant: 26),
            chineseButton.heightAnchor.constraint(equalToConstant: 26),

            next.centerYAnchor.constraint(equalTo: candidateScroll.centerYAnchor),
            next.widthAnchor.constraint(equalToConstant: 28),

            candidateScroll.leadingAnchor.constraint(equalTo: prev.trailingAnchor, constant: 4),
            candidateScroll.trailingAnchor.constraint(equalTo: next.leadingAnchor, constant: -4),
            candidateScroll.topAnchor.constraint(equalTo: candidateBar.topAnchor),
            candidateScroll.heightAnchor.constraint(equalToConstant: 44),

            candidateStack.leadingAnchor.constraint(equalTo: candidateScroll.contentLayoutGuide.leadingAnchor),
            candidateStack.trailingAnchor.constraint(equalTo: candidateScroll.contentLayoutGuide.trailingAnchor),
            candidateStack.topAnchor.constraint(equalTo: candidateScroll.contentLayoutGuide.topAnchor),
            candidateStack.bottomAnchor.constraint(equalTo: candidateScroll.contentLayoutGuide.bottomAnchor),
            candidateStack.heightAnchor.constraint(equalTo: candidateScroll.frameLayoutGuide.heightAnchor),
        ])
        // 初始中文态：只有 next→bar 这条生效。
        nextToBar?.isActive = true
    }

    /// 显示/隐藏候选栏右侧的低存在感「中」键。隐藏时把「›」直接贴到栏尾，
    /// 不留空位（`isHidden` 不会撤销约束，必须显式换约束）。
    func showChineseButton(_ show: Bool) {
        chineseButton.isHidden = !show
        if show {
            nextToBar?.isActive = false
            nextToChinese?.isActive = true
        } else {
            nextToChinese?.isActive = false
            nextToBar?.isActive = true
        }
    }

    private func makeBarButton(_ title: String, onTap: @escaping () -> Void) -> UIButton {
        let b = BarButton(title: title, onTap: onTap)
        b.translatesAutoresizingMaskIntoConstraints = false
        return b
    }

    /// 隐藏「中」键：复用中/繁/英循环的 `.cycleMode`。英文档由控制器走
    /// `applyInputMode(lastChineseInputMode)`（清缓冲 + 回上次中文方案，与 Android
    /// 隐藏键的英→中腿同义），不新造委托方法。
    @objc private func tapChinese() {
        delegate?.keyboardLayout(self, didTapFunction: .cycleMode)
    }
}
