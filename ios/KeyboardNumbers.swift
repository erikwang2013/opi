// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 数字层（123）的两条专门逻辑（2026-10-23 从 `KeyboardViewController.swift` 搬出，
// 本文件为压 500 行门禁；与 `InputModeApply.swift` 同一套先例）。
//
// 背景：拼音 9 键方案下 `2`–`9` 是 T9 **输入键**（`router_digit.rs`），数字层若把
// 2–9 照常送进引擎，会被吞成拼音缓冲，宿主一个数字都收不到 —— 用户报障
// 「点 123 后数字没输入」。数字层是唯一「键面 = 字面文本」的层，必须直提。
//
// ⚠️ 本机无 UIKit，只在 Apple CI 真编译。

import Foundation

extension KeyboardViewController {

    /// 数字层直提：当前层是数字层、且这次按键是**单个 ASCII 数字** ⇒ 直接插入文本。
    /// 返回 true = 已处理（调用方不再走引擎）。
    ///
    /// 全角跟随引擎开关（`punctuation::ascii_fullwidth_wide` / Android `widenAscii`
    /// 同一条规则）；标点/空格/回车不在此列，仍走引擎（中文标点映射不变）。
    func insertNumbersLayerDigitIfNeeded(_ fallback: KeyFallback) -> Bool {
        guard layout.keyLayer == .numbers,
              let digit = fallback.numbersLayerDigit(fullwidth: engine.fullwidthState())
        else { return false }
        insert(digit)
        return true
    }

    /// 进数字层前收掉当前拼音组合（Android `openNumber` 切 Number 模式 ⇒ Rust
    /// `switch_mode` 清缓冲，同一语义）。不提交：9 键的缓冲是按键编码，直提会把
    /// 数字/半截拼音写进字段（2026-10-11 裁决）。清空后旧组合不会在回字母层时「复活」。
    func leaveCompositionForNumbers() {
        guard !engine.buffer().isEmpty else { return }
        engine.clear()
        refresh()
    }
}
