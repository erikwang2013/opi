// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 顶部**宠物条**（2026-10-23 用户报障「iOS 键盘顶部没有随机多形态的项目宠物」）。
// 位置 = 拼音行那条 38pt 槽位**空闲时**的内容（输入中仍是拼音行，与 Android
// `ime/PetStrip.kt` / 鸿蒙 `TopBar.idlePetStrip` 同一条产品语义）。
//
// 与 Android 的差异（如实记）：Android 用矢量小欧 + 每枚独立动效；iOS 这里用
// **静态 PNG** 轮换整组（2.8s 一次、随机 1…8 枚互不重复）。原因：键盘扩展里同时
// 解码多张 10 帧 GIF 会成倍抬内存（本端启动已常驻 37MB 词库），而工具条最左那枚
// 单宠仍是 GIF 动效，动态感没有全丢。`isReduceMotionEnabled` 时本类行为不变
// （本来就不做逐枚动画）。
//
// 纯装饰：读屏跳过；不接手势。⚠️ 本机无 UIKit，只在 Apple CI 真编译。

import UIKit

final class PetStripBar: UIView {

    /// 与 Android `PET_FORM_HOLD_MS` / iOS 工具条同节奏。
    private static let hold: TimeInterval = 2.8
    /// 同时最多显示几枚：条高 38pt，8 枚已足够密；也限制解码张数。
    private static let maxForms = 8
    /// 固定 8 个格位，只换图/显隐 —— 不在每轮里增删 arrangedSubviews
    /// （2026-10-23 真机 .ips 的教训：UIStackView 里频繁拆子视图会踩 NSISEngine）。
    private let cells: [UIImageView] = (0..<PetStripBar.maxForms).map { _ in
        let v = UIImageView()
        // 2026-10-25：`scaleAspectFill` + 等高裁切 —— 多枚时横向铺满、不留大空隙；
        // 贴纸 PNG 四周有 ~11% 透明边，裁掉的正是透明区，不会切到宠物本体。
        v.contentMode = .scaleAspectFill
        v.clipsToBounds = true
        v.translatesAutoresizingMaskIntoConstraints = false
        return v
    }
    private var cellWidths: [NSLayoutConstraint] = []
    private let stack = UIStackView()
    private var timer: Timer?
    private var active = false
    private var visibleCount = 0
    private var palette: KeyboardPalette = .light
    /// sticker 名 → 已解码 PNG（避免每 2.8s 重新 base64 解码同一张）。
    private var cache: [String: UIImage] = [:]

    init() {
        super.init(frame: .zero)
        isHidden = true
        isAccessibilityElement = false
        accessibilityElementsHidden = true
        backgroundColor = .clear
        stack.axis = .horizontal
        // 固定格宽 + 居中：格宽在 layoutSubviews 按「条宽 / 枚数」算，多枚时铺满、
        // 少枚时居中（不再用 fillEqually 把单枚拉成巨宽再被 aspectFit 缩回去）。
        stack.distribution = .fill
        stack.alignment = .fill
        stack.spacing = 2
        stack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(stack)
        NSLayoutConstraint.activate([
            stack.centerXAnchor.constraint(equalTo: centerXAnchor),
            stack.topAnchor.constraint(equalTo: topAnchor, constant: 2),
            stack.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -2),
        ])
        for v in cells {
            stack.addArrangedSubview(v)
            let w = v.widthAnchor.constraint(equalToConstant: 0)
            w.isActive = true
            cellWidths.append(w)
        }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    deinit { timer?.invalidate() }

    /// 显示/隐藏（空闲态才转）。隐藏时停表，不在面板里空转。
    func setActive(_ on: Bool) {
        guard on != active else { return }
        active = on
        isHidden = !on
        if on {
            refresh()
            startTimer()
        } else {
            stopTimer()
        }
    }

    func applyTheme(_ p: KeyboardPalette, fontScale: CGFloat) {
        palette = p
        backgroundColor = p.barBg
    }

    private func startTimer() {
        stopTimer()
        let t = Timer(timeInterval: Self.hold, repeats: true) { [weak self] _ in
            self?.refresh()
        }
        // 与 iOS 工具条一致：滚动/触摸跟踪时暂停。
        RunLoop.current.add(t, forMode: .default)
        timer = t
    }

    private func stopTimer() {
        timer?.invalidate()
        timer = nil
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        applyCellWidths()
    }

    /// 枚数 → 格宽：`min(条高 × 1.2, 可用宽 / 枚数 - 间距)`。8 枚时约等于条宽/8，
    /// 横向铺满；1–2 枚时被 1.2×条高封顶，居中显示而不是拉成宽条。
    private func applyCellWidths() {
        let n = max(1, visibleCount)
        guard !cellWidths.isEmpty else { return }
        let available = bounds.width - 12
        let spacing = CGFloat(n - 1) * 2
        let share = (available - spacing) / CGFloat(n)
        let width = max(0, min(bounds.height * 1.2, share))
        for c in cellWidths where abs(c.constant - width) > 0.5 {
            c.constant = width
        }
    }

    /// 随机 1…8 枚互不重复的形态，顺序也随机（对齐 Android `petStripSelection`）。
    private func refresh() {
        let forms = PetEmojiTable.forms
        guard !forms.isEmpty else { return }
        let count = Int.random(in: 1...min(Self.maxForms, forms.count))
        let picked = forms.shuffled().prefix(count)
        visibleCount = picked.count
        for (i, cell) in cells.enumerated() {
            guard i < picked.count else {
                cell.isHidden = true
                continue
            }
            let name = picked[picked.startIndex + i].sticker
            cell.image = image(named: name)
            cell.isHidden = false
        }
        applyCellWidths()
        setNeedsLayout()
    }

    private func image(named name: String) -> UIImage? {
        if let hit = cache[name] { return hit }
        guard let img = PetStickerImages.image(named: name) else { return nil }
        cache[name] = img
        return img
    }
}
