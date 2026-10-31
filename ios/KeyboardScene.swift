// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 动态插画背景的 UIKit 接线（2026-10-31）。图片由 `package-ios.sh` 从
// `shared/scene/art/` 拷进 appex 的 `scenes/`；本文件只负责铺在键面后，
// 与 Android `KeyboardScene.kt` 同一条：插画从键缝/边缘透出，键帽本身保持不透明。

import UIKit

private let sceneBackgroundTag = 0x5CE6E
private let sceneScrimTag = 0x5CE6F

extension KeyboardLayout {
    /// 换主题图；nil = 纯色皮肤（隐藏插画）。
    func applyScene(_ theme: SceneTheme?) {
        let bg: UIImageView
        let scrim: UIView
        if let existing = viewWithTag(sceneBackgroundTag) as? UIImageView,
           let existingScrim = viewWithTag(sceneScrimTag) {
            bg = existing
            scrim = existingScrim
        } else {
            bg = UIImageView()
            bg.tag = sceneBackgroundTag
            bg.contentMode = .scaleAspectFill
            bg.clipsToBounds = true
            bg.isUserInteractionEnabled = false
            bg.translatesAutoresizingMaskIntoConstraints = false
            insertSubview(bg, at: 0)
            scrim = UIView()
            scrim.tag = sceneScrimTag
            scrim.isUserInteractionEnabled = false
            scrim.translatesAutoresizingMaskIntoConstraints = false
            insertSubview(scrim, aboveSubview: bg)
            NSLayoutConstraint.activate([
                bg.leadingAnchor.constraint(equalTo: leadingAnchor),
                bg.trailingAnchor.constraint(equalTo: trailingAnchor),
                bg.topAnchor.constraint(equalTo: topAnchor),
                bg.bottomAnchor.constraint(equalTo: bottomAnchor),
                scrim.leadingAnchor.constraint(equalTo: leadingAnchor),
                scrim.trailingAnchor.constraint(equalTo: trailingAnchor),
                scrim.topAnchor.constraint(equalTo: topAnchor),
                scrim.bottomAnchor.constraint(equalTo: bottomAnchor),
            ])
        }
        bg.image = theme?.image
        bg.accessibilityIdentifier = theme?.rawValue
        bg.isHidden = theme == nil
        scrim.isHidden = theme == nil
        // 一层很薄的压暗：浅色插画上键面仍然有对比，深色插画也不至于发灰。
        scrim.backgroundColor = UIColor.black.withAlphaComponent(0.10)
    }
}

extension KeyboardViewController {
    /// 选中一套动态主题：配色跟主题的默认纯色皮肤走（与 Android/桌面同一条）。
    func selectScene(_ theme: SceneTheme) {
        skin = theme.defaultSkin
        KeyboardPreferences.skin = skin
        KeyboardPreferences.scene = theme
        layout.setSkin(skin, scene: theme)
        applyTheme()
    }
}
