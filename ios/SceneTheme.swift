// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 动态插画皮肤（2026-10-31 用户裁决：动态背景就是皮肤，全端同一套）。
//
// 绘制代码在 `shared/scene`（Kotlin Compose），由 `SceneArtGeneratorTest` 按每套
// 主题的默认皮肤渲成 `shared/scene/art/scene-<id>.png`；本端只负责显示同一份
// 图片。id 与 Android/桌面/鸿蒙的持久化字面值一致，改这里要先改那边。

import UIKit

enum SceneTheme: String, CaseIterable {
    case meadow, forest, garden, night, city, mountain, sea, pond, autumn, winter
    case mist, vegetable, sunset, study, kitchen, countryside, streetlight, dogs, cats

    var label: String {
        switch self {
        case .meadow: return "田园"
        case .forest: return "森林"
        case .garden: return "花园"
        case .night: return "星夜"
        case .city: return "城市"
        case .mountain: return "山川"
        case .sea: return "大海"
        case .pond: return "池塘"
        case .autumn: return "秋色"
        case .winter: return "冬雪"
        case .mist: return "迷雾"
        case .vegetable: return "菜地"
        case .sunset: return "落日"
        case .study: return "书房"
        case .kitchen: return "灶台"
        case .countryside: return "乡村"
        case .streetlight: return "路灯"
        case .dogs: return "狗群"
        case .cats: return "猫群"
        }
    }

    /// 每套主题自带的默认纯色皮肤（与 Android/桌面 `ThemeSkin.kt` 同一张表）。
    var defaultSkin: SkinOption {
        switch self {
        case .meadow: return .paper
        case .forest: return .pine
        case .garden: return .fresh
        case .night: return .starlight
        case .city, .mountain, .sea: return .azure
        case .pond, .vegetable, .countryside: return .fresh
        case .autumn, .sunset: return .sunset
        case .winter: return .light
        case .mist, .study, .kitchen: return .paper
        case .streetlight: return .starlight
        case .dogs: return .lemon
        case .cats: return .wisteria
        }
    }

    /// 图片由 `package-ios.sh` 拷进 appex 的 `scenes/`；读不到返回 nil（卡片画纯色底）。
    var image: UIImage? {
        if let cached = SceneTheme.imageCache[self] { return cached }
        guard let path = Bundle.main.path(
            forResource: "scene-\(rawValue)", ofType: "png", inDirectory: "scenes"
        ), let img = UIImage(contentsOfFile: path) else { return nil }
        SceneTheme.imageCache[self] = img
        return img
    }

    /// 键帽纹理（预压主题底色的 256×256 裁片，包内 scenes/keycap-<id>.png）。
    var keycapImage: UIImage? {
        if let cached = SceneTheme.keycapCache[self] { return cached }
        guard let path = Bundle.main.path(
            forResource: "keycap-\(rawValue)", ofType: "png", inDirectory: "scenes"
        ), let img = UIImage(contentsOfFile: path) else { return nil }
        SceneTheme.keycapCache[self] = img
        return img
    }

    private static var imageCache: [SceneTheme: UIImage] = [:]
    private static var keycapCache: [SceneTheme: UIImage] = [:]
}
