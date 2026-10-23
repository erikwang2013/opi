// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 候选左列的两处落地（2026-10-21）：
//   1. 9 键左列：输入中把标点列整列换成**拼音读数**（`opi_candidate_readings()`），
//      点一行 = 钉住该读音（`opi_pin_reading`），不上屏；
//   2. 英文 26 键左列：显示候选词，点词 = 直接上屏（与候选栏同一条选词路）。
//
// 两处都用 `CandidateSideColumn`。9 键那块是**浮层**：贴在第一行标点列的
// 竖 stack 上，不参与 `ModeKeyRenderer` 的列宽算式（读数列与标点列同宽），
// 读数空时隐藏、标点键原样露出 —— 与 Android「空闲标点列 / 输入读数态」同义。
//
// 英文列则必须**真占宽**：QWERTY 第一列是 q/a/z，浮层会挡住按键；所以出现时
// 把 `rowsStack` 左锚移到词列右边，键面整体收窄。320pt 下这是可见的代价，
// 已记入交付风险。
//
// ⚠️ 本机无 UIKit，只在 Apple CI 真编译；从未上机运行。

import UIKit

extension KeyboardLayout {

    /// 9 键/大 9 键才取读数。控制器据此决定要不要调 `opi_candidate_readings()`。
    var wantsReadings: Bool { inputMode == .pinyin9 || inputMode == .big9 }

    /// 平铺入口/自动收拢共用的「还有候选吗」。
    var hasCandidates: Bool { !candidateTexts.isEmpty }

    /// 建英文词列（9 键读数浮层在 `installReadingColumn()` 里贴到标点列上）。
    func buildSideColumns() {
        wordColumn.translatesAutoresizingMaskIntoConstraints = false
        wordColumn.isHidden = true
        wordColumn.onPick = { [weak self] index in
            guard let self, self.candidateTexts.indices.contains(index) else { return }
            self.delegate?.keyboardLayout(self, didSelectCandidateAt: index)
        }
        keyArea.addSubview(wordColumn)
        // 0.22 在 iPad 上会到 160pt+：词列封顶 120，手机上（≤93pt）不受影响。
        let wordWidth = wordColumn.widthAnchor.constraint(
            equalTo: keyArea.widthAnchor, multiplier: 0.22
        )
        wordWidth.priority = .defaultHigh
        NSLayoutConstraint.activate([
            wordColumn.leadingAnchor.constraint(equalTo: keyArea.leadingAnchor),
            wordColumn.topAnchor.constraint(equalTo: keyArea.topAnchor),
            wordColumn.bottomAnchor.constraint(equalTo: keyArea.bottomAnchor),
            wordWidth,
            wordColumn.widthAnchor.constraint(lessThanOrEqualToConstant: 120),
        ])
        rowsStackToWord = rowsStack.leadingAnchor.constraint(
            equalTo: wordColumn.trailingAnchor, constant: 4
        )

        readingColumn.onPick = { [weak self] index in
            guard let self,
                  self.candidateReadingTexts.indices.contains(index)
            else { return }
            self.delegate?.keyboardLayout(
                self, didTapFunction: .pinReading(self.candidateReadingTexts[index])
            )
        }
    }

    /// 英文 26 键词列的显隐 + 键面左锚切换。层/模式/候选变化都要调。
    func applySideColumns() {
        let showWords = keyLayer == .letters
            && inputMode == .english26
            && !candidateTexts.isEmpty
        wordColumn.isHidden = !showWords
        rowsStackLeading?.isActive = false
        rowsStackToWord?.isActive = false
        if showWords {
            rowsStackToWord?.isActive = true
            // 配色由 `KeyboardPanelHost.applyTheme` 统一下发；这里每次击键都重刷
            // 一遍色板/字体会让英文输入多走一整列按钮的样式重算，不必。
            wordColumn.setEntries(candidateTexts, pinned: "", resetKey: candidateResetKey)
        } else {
            rowsStackLeading?.isActive = true
        }
        // 英文词列是「真占宽」的：出现/消失都会改变键宽，必须让 Auto Layout 重排。
        setNeedsLayout()
    }

    /// 9 键读数浮层：贴到第一行第一列（标点竖列）上。行重建后必须重贴。
    func installReadingColumn() {
        let show = keyLayer == .letters && wantsReadings && !candidateReadingTexts.isEmpty
        guard show,
              let firstRow = rowsStack.arrangedSubviews.first as? UIStackView,
              let firstColumn = firstRow.arrangedSubviews.first
        else {
            detachReadingColumn()
            return
        }
        if readingColumn.superview !== firstColumn {
            // **先停用旧约束再换父视图**（2026-10-23 修 NSISEngine abort）：否则旧列
            // 被拆除时会把旧约束（属于旧布局引擎）也交给移除流程。
            detachReadingColumn()
            readingColumn.translatesAutoresizingMaskIntoConstraints = false
            firstColumn.addSubview(readingColumn)
            readingColumn.hostConstraints = [
                readingColumn.leadingAnchor.constraint(equalTo: firstColumn.leadingAnchor),
                readingColumn.trailingAnchor.constraint(equalTo: firstColumn.trailingAnchor),
                readingColumn.topAnchor.constraint(equalTo: firstColumn.topAnchor),
                readingColumn.bottomAnchor.constraint(equalTo: firstColumn.bottomAnchor),
            ]
            NSLayoutConstraint.activate(readingColumn.hostConstraints)
        }
        readingColumn.isHidden = false
        // 同 `applySideColumns`：配色只在换肤/长辈模式时下发，不在每次击键时重刷。
        readingColumn.setEntries(
            candidateReadingTexts, pinned: pinnedReading, resetKey: candidateResetKey
        )
    }

    /// 把读数浮层从当前父视图上摘下来，并**显式停用**它的 4 条约束。
    /// 幂等；`rebuildKeys`（拆行前）与 `installReadingColumn`（换列前）都走这条。
    func detachReadingColumn() {
        NSLayoutConstraint.deactivate(readingColumn.hostConstraints)
        readingColumn.hostConstraints.removeAll()
        readingColumn.removeFromSuperview()
    }
}
