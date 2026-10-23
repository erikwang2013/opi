// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 顶部候选栏（2026-10-21 重写）：固定槽位 + 最右 ⌄，不再横向滚动/翻页。
//
// 用户裁决（2026-10-21）：候选栏「不再横向滚动或翻页；固定显示若干候选（溢出截断），
// 最右是 v/⌄」。所以：
//   · ‹ / › 两枚翻页键整段下线；
//   · 候选槽位等宽、按宽度自适应（320pt 5 个，iPad 最多 9 个），超出的候选不画，
//     长词由 `UIButton` 尾部截断；
//   · 「中」仍在英文态显示（上一轮裁决，低存在感），⌄ 永远在最右。
//
// ⌄ 进平铺候选（`CandidateTiledPanel`），不在这里直接画网格。
//
// ⚠️ 未编译：本文件与其它 iOS 源文件一样只在 Apple CI 上真编译，本机无 UIKit。

import UIKit

extension KeyboardLayout {

    /// 按键盘宽算槽位数：每格（含 4pt 间距）尽量 ≥44pt，320pt 下仍是 5 格，
    /// iPad 封顶 9 格。由控制器在 `viewWillLayoutSubviews` 调；格数没变就不重建。
    func updateCandidateSlots(for width: CGFloat) {
        guard width > 0 else { return }
        // 预留左右边距 10 + 英文「中」26 + ⌄ 30 + 间距 2 = 68；+4 是槽间距的修正。
        let slots = min(9, max(5, Int((width - 68) / 48)))
        guard slots != candidateSlotCount else { return }
        candidateSlotCount = slots
        updateCandidateBar(candidateTexts)
    }

    func buildCandidateBar() {
        // 拼音行在最上（24pt 常驻），候选栏/工具条接在它下沿 —— 见 `buildComposingBar`。
        buildComposingBar()
        // 44pt = 工具条同高（Android `IME_TOOLBAR_HEIGHT_DP`）：两者共用一个槽位，
        // 高度不同会让键区在「空闲 ↔ 输入」之间上下跳。
        candidateBar.translatesAutoresizingMaskIntoConstraints = false
        addSubview(candidateBar)

        candidateStack.axis = .horizontal
        candidateStack.spacing = 4
        candidateStack.distribution = .fillEqually
        candidateStack.translatesAutoresizingMaskIntoConstraints = false

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

        expandButton.setTitle("⌄", for: .normal)
        expandButton.accessibilityLabel = "展开候选"
        expandButton.translatesAutoresizingMaskIntoConstraints = false

        let trailingStack = UIStackView(arrangedSubviews: [chineseButton, expandButton])
        trailingStack.axis = .horizontal
        trailingStack.alignment = .center
        trailingStack.spacing = 2
        trailingStack.translatesAutoresizingMaskIntoConstraints = false

        candidateBar.addSubview(candidateStack)
        candidateBar.addSubview(trailingStack)

        // ⚠️ 未编译：以下约束按常见写法写，Mac 上大概率要调数值/优先级。
        NSLayoutConstraint.activate([
            candidateBar.leadingAnchor.constraint(equalTo: leadingAnchor),
            candidateBar.trailingAnchor.constraint(equalTo: trailingAnchor),
            candidateBar.topAnchor.constraint(equalTo: composingBar.bottomAnchor),
            candidateBar.heightAnchor.constraint(equalToConstant: 44),

            // 横屏刘海：候选与「中」/⌄ 让开安全区，栏背景仍铺满。
            candidateStack.leadingAnchor.constraint(
                equalTo: candidateBar.safeAreaLayoutGuide.leadingAnchor, constant: 6
            ),
            candidateStack.trailingAnchor.constraint(
                equalTo: trailingStack.leadingAnchor, constant: -4
            ),
            candidateStack.topAnchor.constraint(equalTo: candidateBar.topAnchor, constant: 2),
            candidateStack.bottomAnchor.constraint(equalTo: candidateBar.bottomAnchor, constant: -2),

            trailingStack.trailingAnchor.constraint(
                equalTo: candidateBar.safeAreaLayoutGuide.trailingAnchor, constant: -4
            ),
            trailingStack.centerYAnchor.constraint(equalTo: candidateBar.centerYAnchor),
        ])
        // 隐藏项在 UIStackView 里会被压成 0：显式尺寸降到 defaultHigh，避免与
        // stack 的折叠约束（required）冲突。「中」与 ⌄ 都会按模式/候选隐藏。
        for c in [
            chineseButton.widthAnchor.constraint(equalToConstant: 26),
            chineseButton.heightAnchor.constraint(equalToConstant: 26),
            expandButton.widthAnchor.constraint(equalToConstant: 30),
            expandButton.heightAnchor.constraint(equalToConstant: 30),
        ] {
            c.priority = .defaultHigh
            c.isActive = true
        }
    }

    /// 候选栏重建。文本来自 `opi_candidates(limit)`，槽位固定、超出丢弃。
    func updateCandidateBar(_ texts: [String]) {
        candidateStack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        for i in 0..<candidateSlotCount {
            if i < texts.count {
                let b = BarButton(title: texts[i]) { [weak self] in
                    guard let self else { return }
                    self.delegate?.keyboardLayout(self, didSelectCandidateAt: i)
                }
                b.titleLabel?.lineBreakMode = .byTruncatingTail
                b.titleLabel?.numberOfLines = 1
                b.contentEdgeInsets = UIEdgeInsets(top: 2, left: 2, bottom: 2, right: 2)
                b.applyTheme(palette, fontScale: fontScale)
                candidateStack.addArrangedSubview(b)
            } else {
                // 固定槽位：候选不足时留空，整条栏的格子宽度不随候选数跳。
                candidateStack.addArrangedSubview(UIView())
            }
        }
    }

    /// 显示/隐藏候选栏右侧的低存在感「中」键。
    /// `isHidden` 在 `UIStackView` 里会自动让出宽度，不再需要两套 trailing 约束。
    func showChineseButton(_ show: Bool) {
        chineseButton.isHidden = !show
    }

    /// ⌄：只在有候选且不在平铺态时出现（平铺态有右列「返回」）。
    func setExpandVisible(_ show: Bool) {
        expandButton.isHidden = !show
    }

    /// 隐藏「中」键：复用中/繁/英循环的 `.cycleMode`。英文档由控制器走
    /// `applyInputMode(lastChineseInputMode)`（清缓冲 + 回上次中文方案，与 Android
    /// 隐藏键的英→中腿同义），不新造委托方法。
    @objc private func tapChinese() {
        delegate?.keyboardLayout(self, didTapFunction: .cycleMode)
    }

    // MARK: - 拼音行（2026-10-23）

    /// 顶部拼音行：键盘最上一条 24pt 常驻槽，候选栏与工具条都在它下面。
    /// 平铺候选态键区顶锚改接 `composingBar.bottom`（见 `buildRows`），所以平铺
    /// 盖住 44pt 槽位、但**不盖**拼音行 —— 组合中的拼音在平铺态同样可见。
    func buildComposingBar() {
        composingBar.translatesAutoresizingMaskIntoConstraints = false
        addSubview(composingBar)
        petStrip.translatesAutoresizingMaskIntoConstraints = false
        addSubview(petStrip)
        NSLayoutConstraint.activate([
            composingBar.leadingAnchor.constraint(equalTo: leadingAnchor),
            composingBar.trailingAnchor.constraint(equalTo: trailingAnchor),
            composingBar.topAnchor.constraint(equalTo: topAnchor),
            composingBar.heightAnchor.constraint(equalToConstant: ComposingBar.height),
            // 宠物条与拼音行**同一槽位**：空闲时盖住空行，组合中由 setComposingText 隐藏。
            petStrip.leadingAnchor.constraint(equalTo: composingBar.leadingAnchor),
            petStrip.trailingAnchor.constraint(equalTo: composingBar.trailingAnchor),
            petStrip.topAnchor.constraint(equalTo: composingBar.topAnchor),
            petStrip.bottomAnchor.constraint(equalTo: composingBar.bottomAnchor),
        ])
    }

    /// 刷新拼音行。文本通常来自 `OpiEngine.displayBuffer()`（引擎已分词）；
    /// 9 键无匹（display 空）时**留空**，不回退数字键串（判据见
    /// `KeyboardCandidates.refresh`）。
    func setComposingText(_ text: String) {
        composingBar.setText(text)
        lastComposingText = text
        updatePetStripVisibility()
    }
}
