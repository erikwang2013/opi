// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 26 键的字母层与功能行（2026-10-24 从 `InputModes.swift` 拆出：那边 482 行顶着
// 500 行门禁，本轮五笔键面（参考图 ads）又要改这两张表）。仍是 `ModeKeyTables` 的
// 扩展，只有数据，不含引擎判断。
//
// ⚠️ 本机无 UIKit，只在 Apple CI 真编译；从未上机运行。

import UIKit

extension ModeKeyTables {

    // MARK: 26 键（字母层 / 数字层）

    /// 26 键字母层（2026-10-24 重排，对齐参考图 aa/cc；五笔见 ads）：
    /// - 拼音 26 键：**没有独立数字行**，Q–P 头上是小号数字徽标；大写键面。
    /// - 英文 26 键：独立数字行 + 小写键面；每个字母头上是符号提示。
    /// - 五笔：键面同拼音，但第三行最左是「拼」（回拼音 26 键），空格键面写「五笔」。
    /// 其余两行（A–L、Z–M）头上都是符号提示；提示表在 `ModeKeySymbols.swift`。
    ///
    /// 字母键一律送**小写码点 + states 0**，不自己做大写化：keys.rs 的约定是
    /// 「大小写由码点携带」是给**物理键盘**的（平台已应用 ⇧）；而软键盘的 ⇧ 走
    /// `handle_shift` 维护的引擎内状态机，由它决定英文模式的 Input 与拼音
    /// composer 的大小写。这里若再自己转一次大写，就是第五份大小写逻辑。
    static func letterRows(english: Bool = false, wubi: Bool = false) -> [ModeRow] {
        let digits = Array("1234567890")
        let top = Array("qwertyuiop").enumerated().map { i, c in
            letterSpec(c, english: english, badge: english ? nil : String(digits[i]))
        }
        let mid = "asdfghjkl".map { letterSpec($0, english: english) }
        var bottom: [KeySpec] = [
            // ⇧：单击/长按的**语义**（Off→Single→Lock）全在 Rust 的 handle_shift，
            // 这里只把「长按」翻成 stateLongPressed 位。states 不送 KEY_STATE_SHIFT：
            // handle_shift 不读它。
            // 五笔（参考图 ads）第三行最左是「拼」：与菜单里的拼音 26 键**同一条路**
            // （`.selectInputMode` → `applyInputMode`，含换方案/清缓冲）。
            wubi
                ? KeySpec("拼", function: .selectInputMode(.pinyin26), widthUnits: 1.4)
                : KeySpec("⇧", keyval: OpiKey.shift, widthUnits: 1.4)
        ]
        bottom += "zxcvbnm".map { letterSpec($0, english: english) }
        bottom.append(
            KeySpec("⌫", keyval: OpiKey.backspace, fallback: .deleteBackward, longPressRepeatsBackspace: true, widthUnits: 1.4)
        )
        var rows: [ModeRow] = []
        // 英文 26 键的独立数字行（参考图 aa）；拼音 26 键的数字在 Q–P 的徽标里。
        if english { rows.append(ModeRow.plain(digits.map { KeySpec.char($0) })) }
        rows += [ModeRow.plain(top), ModeRow.plain(mid), ModeRow.plain(bottom)]
        rows.append(ModeRow.plain(funcRow26(english: english, wubi: wubi)))
        return rows
    }

    /// 26 键功能行（参考图 aa/cc 的底行）：底行两侧标点夹着宽空格，最右换行。
    ///
    /// 中文态 `,`/`。` 走**引擎**（中文标点表会把 `,` 转成 `，`）；英文态直提字面
    /// 标点 —— 英文模式不该被中文标点表改写（2026-10-23 裁决，沿用）。
    /// 五笔态空格键面写「五笔」（参考图 ads），发送仍是空格。
    /// 模式键键面按参考图固定写「中英」：26 键没有独立英文键，点它走三态循环。
    private static func funcRow26(english: Bool, wubi: Bool) -> [KeySpec] {
        let symbolLabel = english ? "!?#" : "符"
        let comma = english
            ? KeySpec(",", function: .commitText(","))
            : punctSpec("，", ",")
        let period = english
            ? KeySpec(".", function: .commitText("."))
            : punctSpec("。", ".")
        return [
            systemInputKey,
            KeySpec(symbolLabel, function: .showSymbols),
            KeySpec("123", function: .showNumbers),
            comma,
            KeySpec(wubi ? "五笔" : "空格", keyval: OpiKey.space, fallback: .text(" "), widthUnits: 4.2),
            period,
            KeySpec("中英", function: .cycleMode, longPressFunction: .showModes),
            KeySpec("换行", keyval: OpiKey.ret, fallback: .text("\n"), widthUnits: 1.4),
        ]
    }
}
