// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 表情页分类条（2026-10-23 从 `EmojiPanel.swift` 机械搬出）。
// 搬出的理由与同轮其它拆分相同：主文件顶着 500 行源码门禁，而本轮要往里加
// 响应式降档与面板释放。这里全是分类条自己的视图逻辑（分组表 → 按钮 →
// 滚动高亮），数据入口与工具条仍在主文件。
//
// 跨文件扩展要读写 `EmojiPanel` 的几处状态/视图，主文件里相应成员已放开为
// internal（见那边的注释）。⚠️ 本机无 UIKit，只在 Apple CI 真编译。

import UIKit

extension EmojiPanel {

    /// 当前子页的分类条（自有没有分类条；Emoji 只用引擎组，**不再加偏移**）。
    private func groupsFor(_ sub: SubTab) -> [OpiEmojiGroup] {
        switch sub {
        case .pet:
            return []
        case .emoji:
            return emojiGroups
        case .kaomoji:
            return KaomojiData.categories.map {
                OpiEmojiGroup(name: $0.label, start: $0.start, count: $0.items.count)
            }
        }
    }

    func rebuildGroupBar() {
        groups = groupsFor(sub)
        groupButtons = groups.map { g in
            BarButton(title: g.name) { [weak self] in
                self?.grid.scrollTo(start: g.start)
            }
        }
        groupStack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        for b in groupButtons {
            groupStack.addArrangedSubview(b)
        }
        groupScroll.setContentOffset(.zero, animated: false)
        selectedGroup = -1
        updateCategoryHighlight()
        updateBarVisibility()
    }

    /// 分类条只在浏览态且**有分类**时出现；它藏起来时网格要吃掉这一条的高度。
    func updateBarVisibility() {
        let visible = !isSearching && !groups.isEmpty
        groupScroll.isHidden = !visible
        // 先撤旧约束再上新约束：两条同时 active 的那一瞬间是互相矛盾的底边。
        if visible {
            gridBottomToBottom.isActive = false
            gridBottomToBar.isActive = true
        } else {
            gridBottomToBar.isActive = false
            gridBottomToBottom.isActive = true
        }
    }

    /// 网格下标 → 分类条要亮第几项（同 Android `SymbolCatalog.groupIndexOf`）。
    /// 分组连续覆盖全表（引擎门禁保证），取「最后一个 start ≤ index 的组」即可。
    private static func groupIndexOf(_ groups: [OpiEmojiGroup], _ index: Int) -> Int {
        var found = -1
        for (i, g) in groups.enumerated() where g.start <= index {
            found = i
        }
        return found
    }

    func updateCategoryHighlight() {
        guard !groups.isEmpty else { return }
        let index = Self.groupIndexOf(groups, grid.firstVisibleItemIndex())
        guard index != selectedGroup else { return }
        selectedGroup = index
        for (i, b) in groupButtons.enumerated() {
            styleCategory(b, selected: i == index)
        }
    }

    private func styleCategory(_ b: BarButton, selected: Bool) {
        b.titleLabel?.font = .systemFont(ofSize: 14, weight: selected ? .bold : .regular)
        b.setTitleColor(selected ? .systemBlue : .label, for: .normal)
        // 选中 = 药丸底（同 Android 的 keyBg 底 + accent 字）。
        b.backgroundColor = selected ? .secondarySystemBackground : .clear
        b.layer.cornerRadius = 8
        let name = b.title(for: .normal) ?? ""
        b.accessibilityLabel = "分类 \(name)" + (selected ? "，已选中" : "")
        if selected {
            b.accessibilityTraits.insert(.selected)
        } else {
            b.accessibilityTraits.remove(.selected)
        }
    }
}
