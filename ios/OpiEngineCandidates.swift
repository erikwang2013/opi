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

    /// **显示用**拼音（`opi_display_buffer()`）：9/14 键与双拼由引擎出读数（9/14 键
    /// 分词成 `ni'hao`，14 键的 `bu` 是按键符号，不能直接当拼音显示）；其余方案就是
    /// 字母缓冲。已确认未提交的逐字片段在前（`中guo`）。
    ///
    /// ⚠️ 与 `buffer()` 是**两个出口，别合并**（2026-10-09 血泪，见
    /// `api/mod.rs` 的注释）：`buffer()` 被宿主当「缓冲非空？」的**行为判据**，
    /// 9 键无候选时显示串恰为空串。**显示是显示，判定是判定** —— 本包装只给
    /// 拼音行用，路由/回退/平铺收拢仍一律读 `buffer()`。
    ///
    /// 为什么放在本文件而不是 `OpiEngine.swift`：那个文件已在 500 行门禁的
    /// 498 行（本仓 500 行只管源码，判据是物理行）。本文件本就是「候选/读音的
    /// C ABI 包装」，显示读数与候选同源（同一次排名的缓存），归这里更顺。
    func displayBuffer() -> String {
        OpiEngine.takeString(opi_display_buffer())
    }

    /// 全局候选（从第 0 条起，最多 `limit` 条）。
    /// 候选栏只取前几项，平铺视图按 `limit` 全量铺页（Android 平铺同样吃全量候选）。
    /// `limit` 由调用方决定（槽位数 / 平铺整批）；不设默认 —— 默认值会冒充产品口径。
    func allCandidates(limit: Int) -> [String] {
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

    /// 已确认但**尚未提交**的逐字片段（`opi_pending_text`，2026-10-28；JNI 同源）。
    /// 换方案/模式前必须上屏 —— `switchMode` 会静默丢掉它们；显示串仍走
    /// `displayBuffer()`（已前置这段）。**不**含打了一半的未确认拼音。
    func pendingText() -> String {
        OpiEngine.takeString(opi_pending_text())
    }

    /// 引擎版本（`opi_engine_version`，workspace 版本）。App 版本来自 appex 的
    /// `CFBundleShortVersionString`，两者同一次打包写入，不一致时关于页两个都显示。
    static func engineVersion() -> String {
        takeString(opi_engine_version())
    }
}
