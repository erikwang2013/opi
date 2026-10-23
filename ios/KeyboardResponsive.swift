// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 响应式尺寸（2026-10-23）。此前键盘高度写死（竖屏 264 / 横屏 200 + 安全区 + 24），
// 键帽字号只有一个值、面板几何全是常量：横屏/小屏/分屏/大字下要么裁字、要么
// 把宿主挤没。这里集中两件事：
//   · `preferredKeyboardHeight`：按屏幕高度封顶（横屏 60%、绝对 340pt），
//     对齐 Android `ImeWindowMetrics` 的三条裁决，且**不改竖屏手机的既有像素**；
//   · `compact` 判定：给键帽字号与面板几何降一档，判据逐字对齐 Android
//     `KeyboardSizing.compactKeys`（宽 < 300 或高 < 170）。
//
// ⚠️ 本机无 UIKit，只在 Apple CI 真编译；iPhone SE / 横屏 / iPad 分屏 / 长辈字号
// 四种形态都**没有上机**验证过，只能静态推演。

import UIKit

extension KeyboardViewController {

    /// 键盘总高（含拼音行与底部安全区）。
    ///
    /// 三条规则来自 Android `ImeWindowMetrics`：
    ///   1. 横屏（宽 > 高）最多占屏幕高 60% —— 否则键区按宽度线性膨胀、把宿主挤没；
    ///   2. 任意朝向绝对上限 340pt —— 平板上键高不随宽度无限放大；
    ///   3. 最后钳到上限内。
    /// 基数保持本端既有的 264pt（竖屏手机算出 288/322，与改前逐点相同）；
    /// 横屏手机由第 1 条压到 60%（Android 同：旧值 77% 偏大）。
    var preferredKeyboardHeight: CGFloat {
        // 朝向看窗口宽高（分屏/Stage Manager 下 UIScreen 是整块屏幕，不是这个键盘
        // 实际能用的区域）；首帧还没有 window 时退回屏幕尺寸。
        let screen = view.window?.bounds.size ?? UIScreen.main.bounds.size
        let landscape = screen.width > screen.height
        let raw = 264 + view.safeAreaInsets.bottom + ComposingBar.height
        let cap = min(landscape ? screen.height * 0.60 : screen.height, 340)
        return max(1, min(raw, cap))
    }

    /// 长辈字号 × compact 降档。0.82 ≈ Android 九宫格 21sp → 16–17sp 的降幅；
    /// 长辈模式在降档后仍比常规大（1.18 × 0.82 ≈ 0.97）。
    var effectiveFontScale: CGFloat {
        KeyboardTheme.fontScale(modes) * (compact ? 0.82 : 1)
    }

    /// keyArea 的**实际可用**尺寸 → compact。判据与 Android `compactKeys` 逐字一致。
    /// 高度算式与 `KeyboardLayout.buildRows` 的约束一致：
    /// 总高 − 拼音行 24 − 顶部槽位 44 − 底部安全区（键区上下各 4 相互抵消）。
    func updateCompactLayout() {
        let safe = view.safeAreaInsets
        let usable = view.bounds.width - safe.left - safe.right
        guard usable > 0 else { return }   // 首帧可能还没布局，下一轮再判
        let width = usable * (modes.oneHand == .off ? 1 : 0.76)
        let height = preferredKeyboardHeight - ComposingBar.height - 44 - safe.bottom
        let next = width < 300 || height < 170
        guard next != compact else { return }
        compact = next
        layout.setCompact(next)
        // 字号随 compact 变：重刷键帽/候选/拼音行。面板不可见时只存快照。
        applyTheme()
    }
}
