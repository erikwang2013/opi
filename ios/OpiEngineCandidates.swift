// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 候选/读音的 C ABI 包装（2026-10-21）。
//
// 与 `candidatesPage()` 的分工：候选栏/平铺/英文左列要的是**同一批全局候选**，
// 所以本轮读 `opi_candidates(limit)`，选词用**全局**下标的 `opi_select(index)`。
// 页内出口仍留在 `OpiEngine.swift` 给旧调用方，但新的候选 UI 不再使用它。
//
// ⚠️ 读数与候选**同一次排名**：`opi_candidate_readings()` 读的是引擎里最近一次
// `candidates()` 留下的缓存。调用方必须**先 allCandidates、紧接着 candidateReadings**
// （见 `KeyboardViewController.refresh()`），顺序反过来会拿到上一批读数。

import Foundation

extension OpiEngine {

    /// 全局候选（从第 0 条起，最多 `limit` 条）。
    /// 候选栏只取前几项，平铺视图按 `limit` 全量铺页（Android 平铺同样吃全量候选）。
    func allCandidates(limit: Int = 64) -> [String] {
        guard limit > 0 else { return [] }
        return OpiEngine.decodeTexts(OpiEngine.takeString(opi_candidates(limit)))
    }

    /// 9 键左列逐候选拼音读数（已去重、短→长）。缓冲已变/非拼音方案 → 空数组。
    func candidateReadings() -> [String] {
        OpiEngine.decodeTexts(OpiEngine.takeString(opi_candidate_readings()))
    }

    /// 钉/解钉读音。空串 = 解除（Rust `read_utf16` 把 null/零长都读成 `Some("")`，
    /// 所以 `withUTF16("")` 传下的 (nil, 0) 正好走解除分支）。
    /// 本出口**不刷新候选**：调用方写完必须再走一次 `refresh()`。
    func pinReading(_ reading: String) {
        OpiEngine.withUTF16(reading) { p, n in
            opi_pin_reading(p, n)
        }
    }
}
