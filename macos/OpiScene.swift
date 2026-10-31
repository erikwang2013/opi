// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 动态插画皮肤（2026-10-31 用户裁决：动态背景就是皮肤，全端同一套）。
//
// 绘制在 `shared/scene`（Kotlin Compose），由 `SceneArtGeneratorTest` 渲成
// `shared/scene/art/scene-<id>.png`；`package-macos.sh` 拷进
// `Contents/Resources/scenes/`。本端只显示图片 + 用主题默认纯色皮肤取色。
// id 与 Android/桌面/iOS/鸿蒙的持久化字面值一致。

import AppKit

enum OpiSceneTheme: String, CaseIterable {
    case meadow, forest, garden, night, city, mountain, sea, pond, autumn, winter
    case mist, vegetable, sunset, study, kitchen, countryside, streetlight, dogs, cats

    var title: String {
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

    /// 与 Android/桌面/iOS `SceneTheme.defaultSkin` 同一张表。
    var defaultSkin: OpiSkin {
        switch self {
        case .meadow: return .paper
        case .forest: return .pine
        case .garden, .pond, .vegetable, .countryside: return .fresh
        case .night, .streetlight: return .starlight
        case .city, .mountain, .sea: return .azure
        case .autumn, .sunset: return .sunset
        case .winter: return .light
        case .mist, .study, .kitchen: return .paper
        case .dogs: return .lemon
        case .cats: return .wisteria
        }
    }

    var image: NSImage? {
        if let cached = OpiSceneTheme.cache[self] { return cached }
        guard let url = Bundle.main.url(
            forResource: "scene-\(rawValue)", withExtension: "png", subdirectory: "scenes"
        ), let img = NSImage(contentsOf: url) else { return nil }
        OpiSceneTheme.cache[self] = img
        return img
    }

    private static var cache: [OpiSceneTheme: NSImage] = [:]
}

/// 皮肤列表的一项：纯色或动态插画。设置页与状态栏 logo 循环共用同一张列表。
enum OpiSkinChoice: Hashable {
    case solid(OpiSkin)
    case scene(OpiSceneTheme)

    var title: String {
        switch self {
        case .solid(let s): return s.title
        case .scene(let t): return t.title
        }
    }

    /// 11 款纯色（不含旧「不改动」）+ 19 套动态，顺序与 Android/桌面一致。
    static let all: [OpiSkinChoice] =
        [.solid(.fresh)]
        + OpiSkin.allCases.filter { $0 != .fresh && $0 != .keep }.map { .solid($0) }
        + OpiSceneTheme.allCases.map { .scene($0) }
}
