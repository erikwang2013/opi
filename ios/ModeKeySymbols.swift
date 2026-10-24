// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 26/14 键的符号提示与浮层条目（2026-10-24）。
//
// 单列一个文件的理由：`InputModes.swift` 顶着 500 行门禁，而这里只是
// **参考图 aa/cc/bb 的逐键数据** + 两个小构造器，不含布局/引擎判断。
//
// ⚠️ 本机无 UIKit，只在 Apple CI 真编译；提示表是手抄参考图，从未上机核对。

import UIKit

extension ModeKeyTables {

    /// 26 键符号提示：空格分隔的「键+提示」对（两者都是单字符）。
    /// 提示取自参考图 aa（英文）/ cc（拼音）；拼音只覆盖 h/j 两个引号。
    /// 别拆成三个数组按行拼 —— 键位表一改就会静默漂。
    static let symbolHints = "q` w= e+ r$ t… y\" u^ i[ o] p| a~ s! d@ f# g% h' j& k* l? z( x) c- v- b: n; m/"

    /// 拼音 26 键的引号提示（英文 26 键仍是 ' 与 &，见参考图 aa）。
    static let pinyinHintOverrides: [Character: String] = ["h": "“", "j": "”"]

    /// 某个字母键的符号提示。找不到 → 空串（浮层里不摆空项）。
    static func symbolHint(for key: Character, english: Bool) -> String {
        if !english, let override = pinyinHintOverrides[key] { return override }
        for token in symbolHints.split(separator: " ") {
            let chars = Array(token)
            if chars.count == 2, chars[0] == key { return String(chars[1]) }
        }
        return ""
    }

    /// 26 键字母键：`badge` = 右上角数字（拼音顶行 Q–P）；其余字母头顶放符号提示。
    ///
    /// 发送的始终是**小写**码点（大小写归引擎的 ⇧ 状态机，见 `letterRows` 注释）。
    /// 浮层 = [大写, 符号/数字, 小写]，与键面提示一致；**默认中间项**
    /// （符号提示 / 拼音顶行的数字 —— Android `qwertyAlternatives` 同序）。
    static func letterSpec(_ letter: Character, english: Bool, badge: String? = nil) -> KeySpec {
        let lower = String(letter).lowercased()
        let upper = lower.uppercased()
        let display = english ? lower : upper
        let hint = symbolHint(for: letter, english: english)
        // 拼音顶行有数字角标时不再画符号提示（参考图 cc；与 Android cornerHint 同）。
        let head = badge == nil ? hint : ""
        let label = head.isEmpty ? display : "\(head)\n\(display)"
        let scales: [CGFloat] = head.isEmpty ? [1] : [0.62, 1]
        let kv = lower.unicodeScalars.first.map { OpiKey.printable($0) } ?? 0
        let middle = badge ?? hint
        let items = middle.isEmpty
            ? [KeyPopupItem(upper, direct: true), KeyPopupItem(lower)]
            : [KeyPopupItem(upper, direct: true), KeyPopupItem(middle, direct: true), KeyPopupItem(lower)]
        return KeySpec(
            label, keyval: kv, fallback: .text(lower),
            fontSize: 22, lineScales: scales, cornerHint: badge,
            popupItems: items, popupDefaultIndex: items.count == 3 ? 1 : 0
        )
    }

    /// 14 键两字母键（参考图 bb）：`badge`（数字）或 `symbol`（符号）显示在字母上方。
    /// 键面字母大写；发送**首个字母的小写码点**（协议约束见 `fourteenKeyRows`）。
    /// 浮层**所有键都有**（2026-10-25 用户裁决，Android `fourteenAlternatives` 同）：
    /// [大写对, 数字或符号, 小写对]，**默认中间项**；提交值大小写两项是协议首字母。
    static func fourteenPair(
        _ pair: String, badge: String? = nil, symbol: String? = nil
    ) -> KeySpec {
        let upper = pair.uppercased()
        let lower = pair.lowercased()
        let head = badge ?? symbol
        let label = head.map { "\($0)\n\(upper)" } ?? upper
        let scales: [CGFloat] = head == nil ? [1] : [0.62, 1]
        let kv = lower.unicodeScalars.first.map { OpiKey.printable($0) } ?? 0
        var items: [KeyPopupItem] = []
        if let head {
            items = [
                // 大写项直接原样上屏：显示 `QW` 就上屏 `QW`。
                KeyPopupItem(upper, direct: true),
                KeyPopupItem(head, direct: true),
                KeyPopupItem(lower, value: String(lower.prefix(1))),
            ]
        }
        return KeySpec(
            label, keyval: kv, fallback: .text(pair),
            fontSize: 22, lineScales: scales,
            popupItems: items, popupDefaultIndex: items.isEmpty ? 0 : 1
        )
    }
}
