// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 词库装载（2026-10-21 从 `KeyboardViewController.swift` 搬出，本文件为压 500 行门禁）。
// 与 `OpiEngineSwitches.swift` 同思路：只搬家，不加行为。
//
// ⚠️ 本机无 Swift 编译器，只在 Apple CI 真编译。

import Foundation

extension KeyboardViewController {

    /// 装载主词库。
    ///
    /// 词库文件来自**扩展自己的 bundle**（不是宿主 App、不是 App Group）——
    /// 与 Android 把 luna.opid / trad.opid 放进 assets 同一思路。
    /// 文件从哪来见 ios/README.md「构建集成」。
    func loadDictionaries() {
        if let p = Bundle.main.path(forResource: "luna", ofType: "opid") {
            dictionaryLoaded = engine.load(path: p)
        } else {
            // 没打包词库文件 → 退化用编译进二进制的内置回退词库
            // （opi_load 对空路径的语义）。可用但候选很少，UI 上要提示。
            dictionaryLoaded = engine.load(path: "")
        }
        // 繁体库可选：失败**不是**错误 —— 繁体模式会回退简体库（Rust 侧语义）。
        if let t = Bundle.main.path(forResource: "trad", ofType: "opid") {
            engine.loadTrad(path: t)
        }
        if !dictionaryLoaded {
            // 不静默。候选栏里会显示「词库未装载」。
            NSLog("[OPI] 词库装载失败 —— 引擎将退化为空操作")
        }
    }

    /// 可选增强词库：笔画与五笔。
    ///
    /// 文件名与 Android `EngineLoader` 的常量**逐字对齐**
    /// （`ASSET_NAME_STROKE` = `stroke.opid`、`ASSET_NAME_WUBI86` = `wubi86.opid`）。
    /// `scripts/apple/package-ios.sh` 会把它们从 `android/app/src/main/assets/`
    /// 拷进 appex 根目录（`Bundle.main` 就是扩展自己）。用 Xcode 集成时要手工加进
    /// 扩展 target 的 Copy Bundle Resources，**文件名一个字都别改**。
    ///
    /// 三条失败语义与 Android `EngineLoader.loadStrokeAsset/loadWubiAsset` 相同：
    /// 缺文件 / 装载返回 false → **只降级**（那一个方案回退普通路径），不动 luna 与 trad。
    func loadBundledDictionaries() {
        if let p = Bundle.main.path(forResource: "stroke", ofType: "opid") {
            if !engine.loadStroke(path: p) {
                NSLog("[OPI] stroke.opid 装载失败 —— 笔画键盘回退普通路径（无候选）")
            }
        } else {
            NSLog("[OPI] bundle 里没有 stroke.opid —— 笔画键盘无候选（见 ios/README.md）")
        }
        // 五笔码表：**只装一张**（86/98 的码空间重叠、权重不可比，两张同时进引擎 =
        // 两套体系打架；Android `EngineLoader` 与 fcitx5 的 `opi_wubi.h` 同一条裁决）。
        // `package-ios.sh` 只拷 86，本端也没有「选 86/98」的设置项（设置面板里没有，
        // 因为 98 的码表没进 bundle）⇒ 恒装 86（Android 缺省也是 86）。
        if let p = Bundle.main.path(forResource: "wubi86", ofType: "opid") {
            if !engine.loadWubi(path: p) {
                NSLog("[OPI] wubi86.opid 装载失败 —— 五笔回退普通路径（无候选）")
            }
        } else {
            NSLog("[OPI] bundle 里没有 wubi86.opid —— 五笔无候选（见 ios/README.md）")
        }
        // ⚠️ 装完**不换方案**（`opi_load_wubi` / `opi_load_stroke` 的契约：装载只管码表）：
        // 要用它们得用户从输入模式菜单选「五笔」/「笔画键盘」——那时 `applyInputMode`
        // 才会 `setScheme`。所以「装了没换成方案」不是 bug，是这两条出口的分工。
    }
}
