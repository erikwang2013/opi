// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 候选刷新 / 选词 / 钉读音 / 平铺收拢（2026-10-21）。
//
// 从 `KeyboardViewController.swift` 拆出：那一文件在 500 行门禁边上，而本轮
// 「候选全局化 + 读数 + 平铺」只增不减。跨文件扩展仍是控制器的方法（内部可见），
// 硬件键盘路径照旧调 `refresh()`。
//
// 两条顺序纪律：
//   · `allCandidates()` 必须紧接 `candidateReadings()` —— 读数读的是最近一次
//     候选排名留下的缓存（见 `OpiEngineCandidates.swift` 头注）；
//   · 缓冲一变先解钉，再取候选 —— 否则钉住态会把新缓冲的候选错误收窄，
//     且界面高亮会留在一条已经失效的读音上。

import UIKit

extension KeyboardViewController {

    /// 拉一次引擎状态刷新三处候选 UI（栏 / 左列 / 平铺）。
    /// **在这条路径上不要做别的**（不查磁盘、不发请求）。
    /// ⚠️ internal：`KeyboardHardware.swift` 的热键路径要调它。
    func refresh() {
        guard dictionaryLoaded else {
            layout.updateCandidates(
                ["词库未装载"], readings: [], pinned: "", resetKey: engine.buffer()
            )
            layout.setTopBar(candidatesVisible: true, english: false)
            // 未装载 = 没有组合串可显示（`display_buffer` 也随单例退化）。
            layout.setComposingText("")
            syncMarkedText()
            return
        }
        let buffer = engine.buffer()
        if pinBuffer != buffer {
            // 缓冲变了：显式解钉（引擎也会按缓冲配对作废，这里同步 UI 高亮）。
            if !pinnedReading.isEmpty {
                engine.pinReading("")
                pinnedReading = ""
            }
            pinBuffer = buffer
        }
        let texts = engine.allCandidates(limit: 64)
        let readings = layout.wantsReadings ? engine.candidateReadings() : []
        layout.updateCandidates(
            texts, readings: readings, pinned: pinnedReading, resetKey: buffer
        )
        // 顶部槽位：有缓冲/候选 → 候选栏；否则工具条（Android `ImeScreen` 同判）。
        layout.setTopBar(
            candidatesVisible: !texts.isEmpty || !buffer.isEmpty,
            english: engine.mode() == .english
        )
        // 拼音行（2026-10-23）：显示串由引擎给（9 键/双拼带分词 `ni'hao`）。
        // **必须**在 `allCandidates` 之后读：读数与候选同一次排名（见文件头注）。
        // 不拿 `buffer()` 顶替 —— 9 键的缓冲是数字串，无匹时不该落数字。
        layout.setComposingText(engine.displayBuffer())
        syncMarkedText()
    }

    /// ⌫ 长按（2026-10-23）：删掉**全部组合输入**（缓冲 + 逐字待提交片段），
    /// **不上屏**任何文本。
    ///
    /// 空组合时退化为**恰好一次** `deleteBackward` —— 没有循环，也就不存在
    /// 「长按把宿主正文删光」的那条路（这条防线是结构性的，不靠计数）。
    ///
    /// 判「有没有组合输入」不能只看 `buffer()`：9 键逐字态下已确认的片段在
    /// `pending_text` 里，缓冲可能为空而显示串非空（`Engine::clear` 两者都清）。
    func clearComposingInput() {
        guard dictionaryLoaded else {
            // 引擎未装载 ⇒ 没有组合输入可清；按一次普通退格处理（只删一个字）。
            textDocumentProxy.deleteBackward()
            return
        }
        let composing = !engine.buffer().isEmpty || !engine.displayBuffer().isEmpty
        guard composing else {
            // 空组合：**只删一个字**（最多一次 deleteBackward，绝不是循环）。
            textDocumentProxy.deleteBackward()
            return
        }
        engine.clear()
        refresh()
    }

    /// 2026-10-21 用户裁决：输入框里**不显示任何** preedit/marked text，
    /// 只有确认/选词后 `insertText`。本函数保留 `unmarkText()` 清理路径
    /// （否则宿主里可能残留上一版键盘标出的组合区），但**不再 setMarkedText**。
    /// ⚠️ internal：热键路径（`KeyboardHardware.swift`）也会走到 [refresh]。
    func syncMarkedText() {
        textDocumentProxy.unmarkText()
    }

    /// 点 9 键左列/平铺左列的一行读音：钉住（候选收窄到读音恰等）；再点同一行解除。
    /// 只改候选，不上屏。写完必须 refresh（C 出口不替我们刷新）。
    func togglePinnedReading(_ reading: String) {
        guard layout.wantsReadings, !reading.isEmpty else { return }
        let next = pinnedReading == reading ? "" : reading
        engine.pinReading(next)
        pinnedReading = next
        pinBuffer = engine.buffer()
        refresh()
    }

    /// 选词（**全局**下标，`opi_candidates(limit)` 的次序）。选完若在平铺态，
    /// 按用户裁决回键面（不留在平铺网格里继续选）。
    func selectCandidate(at index: Int) {
        let text = engine.select(index: index)
        insert(text)
        // 先回键面再刷新：否则平铺面板会为这一批即将丢弃的候选白重建一次网格。
        if layout.keyLayer == .candidatesTiled {
            returnToCurrentInputMode()
        }
        refresh()
    }

    /// 候选栏 ⌄：进平铺。没有候选时入口本就不该出现（栏里已隐藏）。
    func showTiledCandidates() {
        guard layout.hasCandidates else { return }
        layout.setLayer(.candidatesTiled)
    }

    /// 平铺里 ⌫ / 重输 / 空格之后：中文且缓冲空、候选也没了 ⇒ 自动回键面，
    /// 否则键盘回不来（Android `ImeState.collapseIfNoCandidates` 同判）。
    /// 英文模式不收拢（与 Android 的 `candidatesShowable` 英文恒真同义）。
    func collapseTiledIfNeeded() {
        guard layout.keyLayer == .candidatesTiled else { return }
        if engine.mode() != .english, engine.buffer().isEmpty, !layout.hasCandidates {
            returnToCurrentInputMode()
        }
    }
}
