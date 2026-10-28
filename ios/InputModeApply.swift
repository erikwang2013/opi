// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 输入模式的**控制器接线**（2026-10-23 从 `InputModes.swift` 拆出：那边 499 行
// 顶着 500 行门禁，而本轮 `applyInputMode` 要加「可选码表懒加载」与「合并成一次
// 键面重建」）。数据/键位表仍在 `InputModes.swift`；本文件只有「选中一项后对
// 引擎、键面、持久化做什么」。
//
// ⚠️ 本机无 UIKit，只在 Apple CI 真编译；从未上机运行。

import UIKit

extension KeyboardViewController {

    /// 选中输入模式（输入模式面板 `onSelect` 的回调路径）。
    ///
    /// 换模式前先交出**已确认未提交**的逐字片段（`opi_pending_text`，2026-10-28）。
    /// 与 Android 仍有一处能力差异：Android 在「输入模式」面板换布局前会把**未确认
    /// 的缓冲**也提交（英文 `commitPendingAsPinyin`、其余 `commitPendingBuffer`），
    /// 而 C ABI 只有「已确认片段」出口。所以面板换方案时打了一半的拼音仍会丢 ——
    /// 但**绝不**改成「把缓冲当文本直提」：9 键的缓冲是**数字**，直提就是把
    /// `644269` 打进输入框（Android 2026-10-11 修掉的正是这个 bug）。
    /// 简/繁这类成对切换不走这里，改走保缓冲的 `engine.switchLanguage`。
    ///
    /// 面板式（生僻字）**一个字节都不碰引擎**（Android 同）：只切视图。
    /// `commitPending: false` 只给启动恢复用（见 `viewDidLoad`）。
    func applyInputMode(_ mode: InputMode, commitPending: Bool = true) {
        OpiLog.input.info("applyInputMode \(mode.label, privacy: .public)")
        // 记下选择：键面按它画，菜单高亮也按它。面板式在收起后仍保持高亮
        // （Android 的 chip 同样保持），底下的键面回落 26 键（`RARE.scheme == FULL`）。
        InputModePreference.inputMode = mode
        if mode != .english26 && !mode.panel {
            lastChineseInputMode = mode
            InputModePreference.lastChinese = mode
        }
        // 启动恢复：上一实例残留的会话（含已确认未提交片段）不能带进新宿主。
        // 放在面板分支**之前** —— 恢复成生僻字面板时也要清。
        if !commitPending { engine.clear() }
        if mode.panel {
            layout.setInputMode(mode)
            layout.setLayer(.rare)
            return
        }
        if commitPending {
            commitPendingBeforeModeSwitch()
        }
        // 可选码表懒加载（2026-10-23）：启动只装 luna/trad，笔画/五笔首次选到才读
        // （见 `KeyboardDictionaryLoader.loadBundledDictionary(for:)`）。
        loadBundledDictionary(for: mode)
        engine.setScheme(mode.scheme)   // ⚠️ 清缓冲（Rust 语义），见 `OpiEngineScheme.swift`
        if engine.mode() != mode.engineMode {
            // 读一次现状再决定要不要切：`switch_mode` 会顺带复位全角与前端 ⇧ 三态，
            // 白切一次就是白复位一次（Android 那边同样是「需要才切」）。
            engine.switchMode(mode.engineMode)
        }
        // 键面状态先落地、再 refresh：refresh 要按**新模式**决定取不取 9 键读数
        // （`layout.wantsReadings`），顺序反了的话切进/切出 9 键时左列要等下一击键
        // 才对。合并 API 只重建**一次**（原来 setInputMode + updateModeLabel 各重建
        // 一遍整屏按钮，9 键 = 几十个按钮销毁/重建二连）—— 2026-10-23 用户报的
        // 「切英文后跳系统键盘」首要嫌疑就是这条内存尖峰。
        // 菜单选模式时当前层是 `.modes`（面板）：这里只记状态不重建，真正的重建在
        // `returnToCurrentInputMode` 里**一次**完成，且那时候选/读数已是新模式的。
        let engineMode = engine.mode()
        layout.setInputMode(
            mode,
            modeLabel: mode.usesJianFanModeKey
                ? Self.jianFanModeKeyLabel(of: engineMode)
                : Self.modeKeyLabel(of: engineMode),
            english: engineMode == .english
        )
        refresh()
        returnToCurrentInputMode()      // 从菜单/面板回到当前模式键面（不跳系统输入法）
        OpiLog.input.info("applyInputMode done \(mode.label, privacy: .public)")
    }

    /// 换方案/模式前交出**已确认未提交**的逐字片段（`opi_pending_text`，2026-10-28）。
    /// 只交已确认的 —— 未确认缓冲是方案相关的（9 键 = 数字），绝不直提。
    /// 读完必须 `clear()`：`set_scheme` **不清** `pending_segments`，不 clear 会在
    /// 换模式后再提交一次同一段（重复上屏）；`switch_mode` 则会静默丢掉它们。
    func commitPendingBeforeModeSwitch() {
        let pending = engine.pendingText()
        guard !pending.isEmpty else { return }
        engine.clear()
        insert(pending)
    }

    /// 9 键 / 14 键 / 笔画的模式键：**只切简/繁**（Android `toggleTraditional` 同义）。
    /// 键面字 = 点它会到哪（繁 → 中，其余 → 繁）。
    ///
    /// 与 Android `switchLanguage(next, controller.inputScheme)` 同一条：换模式不换
    /// 方案、**保缓冲**；已确认片段也留着（`opi_switch_language`，2026-10-28）。
    func toggleTraditional() {
        OpiLog.input.info("toggleTraditional start")
        let scheme = engine.scheme()
        engine.switchLanguage(engine.mode() == .traditional ? .pinyin : .traditional, scheme)
        // 英文 26 键面下开繁体：键面同形，但 UI 模式要落到中文 26 键，否则菜单
        // 仍高亮「英文26键」，回英文再回中文还会走错键面。
        if layout.inputMode == .english26 {
            lastChineseInputMode = .pinyin26
            InputModePreference.lastChinese = .pinyin26
            InputModePreference.inputMode = .pinyin26
            layout.setInputMode(.pinyin26)
        }
        updateModeLabel()
        layout.setKeyboardModes(modes, traditional: engine.mode() == .traditional)
        refresh()
        OpiLog.input.info("toggleTraditional done")
    }

    /// 同步模式键的键面字（**按当前键面语义**）：
    /// 9/14/笔画只切简繁 ⇒ 「中/繁」；26 键三档循环 ⇒ 「繁/英/中」。
    /// ⚠️ internal：`KeyboardHardware.swift` 的热键路径也要调。
    func updateModeLabel() {
        let mode = engine.mode()
        let label = layout.inputMode.usesJianFanModeKey
            ? Self.jianFanModeKeyLabel(of: mode)
            : Self.modeKeyLabel(of: mode)
        layout.setModeLabel(label, english: mode == .english)
    }

    /// 简繁键的键面字（Android `jianFanModeKeyLabelOf` 逐字同）。
    static func jianFanModeKeyLabel(of mode: OpiMode) -> String {
        mode == .traditional ? "中" : "繁"
    }
}
