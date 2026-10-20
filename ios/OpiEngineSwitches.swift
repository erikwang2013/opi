// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 设置面板可用的引擎开关（C ABI 的薄包装，2026-10-21）。
//
// 与 `OpiEngine.swift` 分文件：那边 ~498 行，贴着 500 行源码门禁。这里只做
// 「Swift 名字 → `opi_*`」的一对一转发，**不判业务**（缺省值/回落规则在
// `KeyboardSettings` 与调用方）。
//
// 只转发 C ABI 真实存在、且本文件之前**没有**包装的出口（`macos/OpiFFI.h`）：
// 模糊音（总开关 + 逐组名）、简拼、纠错、中文标点。双拼布局与五笔五旋钮的包装
// 在 `OpiEngineScheme.swift` / `OpiEngineWubi.swift`（同名重复声明会编不过）。
// `opi_fuzzy_pair_names()` 是逐组名字的**唯一名源**（Android `FUZZY_PAIRS_ANDROID`
// 是同一份契约的镜像，不要在这里再写一张表）。
//
// ⚠️ 本机无 UIKit/Swift；只在 Apple CI 真编译。

import Foundation

extension OpiEngine {

    // MARK: - 模糊音

    var fuzzyEnabled: Bool { opi_fuzzy_enabled() }

    func setFuzzy(_ on: Bool) { opi_set_fuzzy(on) }

    /// 引擎的规范组名（逗号分隔，如 `zh/z,ch/c,...`）。
    /// 未装载时 C 侧返回空串 —— 设置面板据此显示「逐对暂不可用」，不伪造名字。
    func fuzzyPairNames() -> [String] {
        let raw = OpiEngine.takeString(opi_fuzzy_pair_names())
        return raw.split(separator: ",").map {
            String($0).trimmingCharacters(in: .whitespaces)
        }.filter { !$0.isEmpty }
    }

    /// 按**组名**设置（空串 = 清空；未知名字由引擎忽略，不做本地校验）。
    func setFuzzyPairs(_ names: String) {
        OpiEngine.withUTF16(names) { p, n in opi_set_fuzzy_pairs_names(p, n) }
    }

    // MARK: - 简拼 / 纠错 / 中文标点

    var jianpinEnabled: Bool { opi_jianpin_enabled() }
    func setJianpin(_ on: Bool) { opi_set_jianpin(on) }

    var correctionEnabled: Bool { opi_correction_enabled() }
    func setCorrection(_ on: Bool) { opi_set_correction(on) }

    var chinesePunct: Bool { opi_chinese_punct() }
    func setChinesePunct(_ on: Bool) { opi_set_chinese_punct(on) }

    // MARK: - 双拼布局 / 五笔
    //
    // 这两族的底层包装**已经存在**：`OpiEngineScheme.swift`（双拼布局，收 Int）、
    // `OpiEngineWubi.swift`（五个五笔旋钮，收 Int32）。这里**不再重复声明** ——
    // 同名 `var` 与零参 `func` 是非法重定义，同名 setter 多一层 `Int` 重载只会让
    // 调用点产生歧义。设置面板的 `Int` / 名字串在 `applySettings` 里换算。

    // MARK: - 一次性应用

    /// 把用户偏好整体推给引擎。**幂等**，启动装载词库后与面板每次改动后各调一次。
    ///
    /// 模糊音的顺序固定：组名串非空就用组名（覆盖总开关），否则用布尔总开关
    /// —— 与 Android `fuzzyPairs.isEmpty -> setFuzzy(fuzzy)` 的合同同序。
    func applySettings(_ s: KeyboardSettings) {
        learnerEnabled = s.learn
        if s.fuzzyPairs.isEmpty {
            setFuzzy(s.fuzzy)
        } else {
            setFuzzyPairs(s.fuzzyPairs)
        }
        setJianpin(s.jianpin)
        setCorrection(s.correction)
        setChinesePunct(s.chinesePunct)
        // 名字 → 下标：认不得的名字回落到自然码（0）；下标越界由引擎钳制。
        setShuangpinLayout(SHUANGPIN_LAYOUT_NAMES.firstIndex(of: s.shuangpinLayout) ?? 0)
        // 五笔旋钮：偏好里是 Int，既有包装收 Int32 裸值（值域 0..=1 / 3..=9）。
        setWubiStyle(Int32(s.wubiStyle))
        setWubiMixedEntry(Int32(s.wubiMixedEntry))
        setWubiCandidateLimit(Int32(s.wubiCandidateLimit))
        setWubiReturnStyle(Int32(s.wubiReturn))
        setWubiAutoCommit(s.wubiAutoCommit)
    }
}
