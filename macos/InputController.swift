// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写，未经过任何 Swift 编译器、链接器或语法检查 ——
// InputMethodKit / AppKit 是 Apple 独有框架，在本机 import 就断。
// 本文件**不是**已验证的实现，是给 Mac 开发者的起点。见 macos/README.md
// 的「最不确定的 API」一节：本文件是那节列出的问题最集中的地方（尤其是
// 「文本怎么提交进应用」）。
//
// InputController.swift —— IMKInputController 子类 + 候选窗。
// 结构对照 crates/fcitx5-opi/cpp/opi_fcitx5.cpp（已被真实 fcitx5 加载并测过）：
//   fcitx5 OpiEngine::keyEvent  ↔  IMK handle(_:client:)
//   fcitx5 ic->commitString()   ↔  IMK client.insertText(_:replacementRange:)  ⚠️ 未验证
//   fcitx5 reset() → opi_clear  ↔  IMK deactivateServer(_:)
// 键路由本身不在这里：全部经 opi_key_event 走 Rust（本层只翻译事件与动作）。

import AppKit
import InputMethodKit

/// 类名由 Info.plist 的 `InputMethodServerControllerClass` 指定。
/// 现代 Swift 构建里该键要写 `$(PRODUCT_MODULE_NAME).OpiInputController`；
/// 这里再加 @objc 名字是双保险（模块名展开失败时仍能按裸名找到类）。
@objc(OpiInputController)
final class OpiInputController: IMKInputController {
    private let engine = OpiEngine.shared

    /// 上次设进客户端的 marked text（= preedit），用来挡掉无谓的重复刷新。
    private var lastMarked = ""
    /// 上次交给候选窗的候选（**当前页**，与 `selectPage(_:)` 的页内索引同序）。
    private var lastCandidates: [String] = []

    // ---------- 平台键 → 引擎键（本层唯一的映射表） ----------

    /// macOS 硬件键码（NSEvent.keyCode = `kVK_*`，与键盘布局无关）→ 引擎键码。
    /// 表里没有的键走「可打印字符」那条路。
    ///
    /// **必须按 keyCode 查表，不能按 characters 猜**（keys.rs 模块头明确要求）：
    /// 退格在不同来源下可能给 BS(0x08) 也可能给 DEL(0x7F)，猜错的后果是该键在
    /// 输入态下静默失效（退化成直通）。
    /// ⚠️ 空格映到裸 0x20（可打印段）而不是 0x1_0020，见 OpiKeySpace.space 的注释。
    private static let specialKeys: [UInt16: UInt32] = [
        51: OpiKeySpace.backspace, // Delete（退格）
        117: OpiKeySpace.del, // Forward Delete
        36: OpiKeySpace.ret, // Return
        76: OpiKeySpace.ret, // 小键盘 Enter
        48: OpiKeySpace.tab,
        53: OpiKeySpace.escape,
        116: OpiKeySpace.pageUp,
        121: OpiKeySpace.pageDown,
        49: OpiKeySpace.space,
        56: OpiKeySpace.shift, // 左 ⇧
        60: OpiKeySpace.shift, // 右 ⇧
        // 方向键：router.rs 有显式直通分支（不消费）。列出来是为了不依赖
        // 「取不到字符 → NO_KEYVAL」那条兜底 —— 方向键的
        // charactersIgnoringModifiers 是私用区字符（U+F700..），走兜底虽然也安全，
        // 但显式映射与 Rust 侧的分支一一对应，读起来不打哑谜。
        126: OpiKeySpace.up,
        125: OpiKeySpace.down,
        123: OpiKeySpace.left,
        124: OpiKeySpace.right,
    ]

    /// NSEvent → (keyval, states)。返回 nil = 不送引擎，直接放行给应用。
    private func wire(_ event: NSEvent) -> (keyval: UInt32, states: UInt32)? {
        // 只取设备无关位：方向键/功能键会带上 .function/.numericPad，那不是修饰键，
        // 混进来会让「⌘A」这类判断失真。
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)

        var states: UInt32 = 0
        if flags.contains(.shift) { states |= OpiKeyState.shift }
        if flags.contains(.capsLock) { states |= OpiKeyState.capsLock }
        if flags.contains(.control) { states |= OpiKeyState.ctrl }
        if flags.contains(.option) { states |= OpiKeyState.alt }
        // ⌘ 如实置位即可：router.rs 的直通掩码含 META，⌘ 组合由 Rust 侧放行。
        // **不要在这里再拦一道** —— 那会把该规则抄成第二份（第一份在 router.rs）。
        if flags.contains(.command) { states |= OpiKeyState.meta }
        if event.type == .keyUp { states |= OpiKeyState.released }
        if event.isARepeat { states |= OpiKeyState.repeatKey }
        // 长按位（1<<28）不合成：macOS 的 NSEvent 没有长按信号
        // （同 fcitx5 轨的已知边界，见 crates/fcitx5-opi/cpp/README.md）。

        // 特殊键表**优先**：⇧ 自身的 charactersIgnoringModifiers 是空串，
        // 方向键/F 键给的是私用区字符，先查表才不会被可打印分支误判。
        if let code = OpiInputController.specialKeys[event.keyCode] {
            return (code, states)
        }

        // 可打印：取 Shift 已生效的字符（Shift+a → "A"），与 fcitx5 轨取 xkb keysym
        // 同语义；拼音模式下引擎会 to_ascii_lowercase（engine-core/composer.rs），
        // 大写不会进缓冲。
        // 取不到字符（方向键/F 键/死键）→ NO_KEYVAL 哨兵：Rust 侧 char::from_u32
        // 为 None → PassThrough（action=0）→ 键自然流入应用。
        // **绝不可回退成 keyCode 本身**。
        guard let scalar = event.charactersIgnoringModifiers?.unicodeScalars.first else {
            return (OpiKeySpace.noKeyval, states)
        }
        return (scalar.value, states)
    }

    // ---------- 热键（引擎之前判） ----------

    /// 引擎之前判掉的热键（键位与桌面两轨、以及 iOS 侧同一张表）。
    private enum OpiHotkey {
        case toggleEnglish      // Ctrl + '
        case toggleSymbol       // Ctrl + \
        case toggleTraditional  // Ctrl + Shift + F
        case toggleFullwidth    // Shift + Space
        case symbolPanel        // Ctrl + Shift + Z
    }

    /// 与 `tsf-opi/src/vk.rs` 的 `mode_hotkey` / `fullwidth_hotkey`
    /// **同判**，也与 `ios/KeyboardViewController.swift` 的同名函数逐条对应。
    /// 返回 nil = 不是热键，照常送 `wire` / 引擎。
    ///
    /// 用 `charactersIgnoringModifiers` 而**不是**虚拟键码：Ctrl 生效时 `characters`
    /// 会变成控制字符，而 `charactersIgnoringModifiers` 不受 Ctrl 影响 —— 于是这里
    /// **不需要**再引一张 `kVK_ANSI_*` 常量表（少一张没编译过的表）。
    ///
    /// ⚠️ **本函数故意不过滤抬起与 autorepeat** —— 它们**也是热键**，只是不该执行。
    /// 由调用处决定「消费但不动作」：若在这里返回 nil，那一颗键就会漏给下面
    /// 的 `keyEvent`（`Shift+Space` 的后果见调用处注释）。
    private func hotkey(_ event: NSEvent) -> OpiHotkey? {
        // `wire()` 已按同一套位算过 states，这里直接读 flags：两者用途不同
        // （一个喂引擎、一个只判热键），不合并以免把两种语义搅在一起。
        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        // ⚠️ **故意不看 `event.type == .keyUp`**：抬起的那一下**也要认出来**（见调用处）
        // —— 只拦按下会给宿主一个**没有 keydown 的 keyup**。桌面两轨的
        // `mode_hotkey`/`fullwidth_hotkey` 判「抬起返回 None」，但它们是在**引擎入口**
        // 拦的，抬起本来也会被那一层消费掉；本层在引擎**之前**，不自己认就会漏给应用。
        let chars = event.charactersIgnoringModifiers ?? ""
        if flags.contains(.control) {
            // Linux 的 handleModeHotkey 同判（`opi_mode.h`）：Ctrl+Shift+F 简繁、
            // Ctrl+Shift+Z 符号画廊。先判 Shift 组，避免 Ctrl+Shift+F 落进下面。
            if flags.contains(.shift) {
                switch chars.lowercased() {
                case "f": return .toggleTraditional
                case "z": return .symbolPanel
                default: return nil
                }
            }
            switch chars {
            case "'": return .toggleEnglish
            case "\\": return .toggleSymbol
            default: return nil
            }
        }
        // ⚠️ 这里**比 vk.rs 多排一个 ⌘**：那张表是 Win32 侧的（没有 ⌘），
        // 而 macOS 上 ⌘Space 是 Spotlight、⌘⇧Space 也不是我们的 —— 不排掉会吃进系统键。
        // 这是**有意的平台差异**，不是抄漏。
        if flags.contains(.shift),
           !flags.contains(.control), !flags.contains(.option), !flags.contains(.command),
           chars == " " {
            return .toggleFullwidth
        }
        return nil
    }

    /// 执行热键。三条都**不经过** `opi_key_event`。
    ///
    /// ⚠️ **仅 `Ctrl+'`（`toggleEnglish`）这一支**：目标模式在本层算是**对的** ——
    /// 桌面两轨也是客户端算的（`hotkey_target` 在 `vk.rs`，不在引擎里）。
    /// **`Ctrl+\`（`toggleSymbol`）是唯一例外**：目标模式由引擎自己算，本层不重算 ——
    /// 见该支自己的注释。约定禁止的是客户端推**全角映射**。
    private func performHotkey(_ hot: OpiHotkey, sender: Any!) {
        switch hot {
        case .toggleEnglish:
            // 与 `vk.rs` 的 `hotkey_target` 同判：**来回切**，已在英文就回拼音。
            engine.switchMode(engine.mode() == .english ? .pinyin : .english)
        case .toggleSymbol:
            // 这个出口**自己**包办 Pinyin ⇄ Symbol 的来回切 **和** 缓冲收尾
            // （有候选提交首候选、乱码缓冲清掉不上屏），所以本层**不重算目标模式、
            // 也不自己 `switchMode`** —— 判了就是第二份 `toggle_symbol` 语义。
            // ⚠️ 下一行是**调用**：读成「本支不调这个出口」是反的。
            commit(engine.toggleSymbol(), to: sender)
        case .toggleTraditional:
            // 与 `opi_mode.h` 的 opiToggleTraditional 同判：在繁体回拼音，否则进繁体。
            engine.switchMode(engine.mode() == .traditional ? .pinyin : .traditional)
        case .symbolPanel:
            // 符号画廊是**纯 UI 面板**（不进引擎模式、不动缓冲），与 Linux 的
            // Ctrl+Shift+Z 同语义；面板非激活，未提交的 preedit 留在宿主应用里。
            OpiSymbolPanel.shared.show()
            return
        case .toggleFullwidth:
            engine.toggleFullwidth()
            // ⚠️ **不刷候选面板**：这个开关只改**后续**的标点映射，不动缓冲也不动候选
            // （`Engine::toggle_fullwidth` 只翻一个 bool），推一帧是白推。
            // 与模式热键的差别正在这里 —— 那个切模式会清缓冲换候选。
            // 桌面两轨同判（`tsf.rs` 里 `handleFullwidthHotkey` 调用点上方的注释 / `opi_fcitx5.cpp` 的
            // `handleFullwidthHotkey`，两处措辞一致）。
            return
        }
        refresh(sender)
    }

    // ---------- 事件入口 ----------

    /// ⚠️ 覆写的选择器是 `handleEvent:client:`，Swift 侧若**没有被调用**（按键完全
    /// 没反应、连日志都没有），就是这里的名字没对上 —— 改成
    /// `@objc(handleEvent:client:) func handle(...)` 或直接
    /// `override func handleEvent(_ event: NSEvent!, client sender: Any!) -> Bool`。
    /// 见 README「最不确定的 API」#1 末尾。
    override func handle(_ event: NSEvent, client sender: Any!) -> Bool {
        guard event.type == .keyDown || event.type == .keyUp else { return false }
        // 面板/菜单上屏时没有这个 sender，记下最近客户端（弱引用，不强留宿主）。
        if let client = sender { OpiClientHub.shared.focus(self, client: client as AnyObject) }
        // ⚠️ **热键必须在引擎之前判掉，不能落进下面的 `keyEvent`** ——
        // `router.rs` 的 `key_event` 直通分支（`router.rs:245`）对 `CTRL|ALT|META`
        // **在整个函数最前面就直通**，
        // 所以 `Ctrl+'`/`Ctrl+\` 送进去等于交回宿主应用（模式一动不动、无日志）；
        // 而 `Shift+Space` 的 SHIFT **不在**直通掩码里，会走到 `KEY_SPACE` 分支，
        // 又因「空格分支不看 Shift 位」变成**选首候选**。
        // 桌面两轨同判（`tsf.rs` 把 `mode_hotkey`/`fullwidth_hotkey` 判在引擎之前，
        // 注释写明「送进引擎就是普通空格，会被当成选首候选」）。
        // 抬起也一并消费：只拦按下会给客户端一个**没有 keydown 的 keyup**。
        if let hot = hotkey(event) {
            // 只在这一下是「**首次按下**」时才执行：抬起与 autorepeat 都**只消费、不动作**。
            //   * 抬起只消费：否则按住不放会反复切模式。
            //   * ⚠️ autorepeat **绝不能放行**给下面的 `keyEvent`：`Shift+Space` 的
            //     SHIFT 不在 `router.rs` 的直通掩码里，放行会被当成**普通空格 = 选首候选**
            //     —— 那比「反复切全角」更坏。四轨同判（`vk.rs` / `opi_fcitx5.cpp` 同轮加）。
            if event.type == .keyDown, !event.isARepeat {
                performHotkey(hot, sender: sender)
                OpiStatusBar.shared.refresh()
            }
            return true
        }
        guard let (keyval, states) = wire(event) else { return false }

        let result = engine.keyEvent(keyval: keyval, states: states)
        switch result.action {
        case OpiAction.commit:
            // 先清 preedit 再插入提交文本；提交后引擎缓冲已空，刷新会收起候选窗。
            commit(result.text, to: sender)
            refresh(sender)
            return true
        case OpiAction.handled:
            // 抬起不改引擎状态：`router.rs` 的 released 分支只回放按下时记下的
            // 结论（last_printable），不碰缓冲/候选/页码/模式。旧实现照常 refresh
            // ⇒ 每个物理按键付两遍「全量候选排序 + JSON 编解码」。
            if event.type == .keyUp { return true }
            refresh(sender)
            return true
        default:
            return false // 0 = 未处理：键交应用（不拦截）
        }
    }

    // ---------- 提交 / 刷新 ----------

    /// 把文本提交进客户端应用 —— IMK 侧对应 fcitx5 的 `ic->commitString(s)`。
    /// ⚠️ 最不确定的一处（README #1）：本函数里三件事都没有被验证过 ——
    ///   1. 客户端对象就是 `sender as? IMKTextInput`；
    ///   2. 「先清 marked text，再 insertText」的顺序；
    ///   3. replacementRange 用 (NSNotFound, NSNotFound) 表示「插在光标处，不替换」。
    private func commit(_ text: String, to sender: Any!) {
        if !lastMarked.isEmpty { engine.markUserWordsDirty() } // preedit = 可能刚选过词
        guard !text.isEmpty, let client = sender as? IMKTextInput else { return }
        let none = NSRange(location: NSNotFound, length: NSNotFound)
        if !lastMarked.isEmpty {
            client.setMarkedText("",
                                 selectionRange: NSRange(location: 0, length: 0),
                                 replacementRange: none)
            lastMarked = ""
        }
        client.insertText(text, replacementRange: none)
    }

    /// 刷新 preedit（marked text）+ 候选窗。
    ///
    /// action=1 按 router.rs 的契约就是「请刷新」；它同时覆盖「缓冲/页码变了」与
    /// 「⇧ 状态机等无变化」两种情况（C ABI 只有一个码，不区分）。
    /// 「与上次相同就不动客户端」只是省掉无谓的客户端调用，不是正确性依赖。
    private func refresh(_ sender: Any!) {
        guard let client = sender as? IMKTextInput else { return }
        let buffer = engine.buffer()

        if buffer != lastMarked {
            let none = NSRange(location: NSNotFound, length: NSNotFound)
            if buffer.isEmpty {
                client.setMarkedText("",
                                     selectionRange: NSRange(location: 0, length: 0),
                                     replacementRange: none)
            } else {
                client.setMarkedText(buffer,
                                     selectionRange: NSRange(location: buffer.utf16.count, length: 0),
                                     replacementRange: none)
            }
            lastMarked = buffer
        }

        // 显示当前页用引擎的分页出口：页大小/页码/页内选词同源在 router.rs，
        // 客户端不再抄 PAGE_SIZE（`OpiFFI.h` 明写禁止自己按页大小切）。
        let list = engine.candidatesPage()
        if list != lastCandidates {
            lastCandidates = list
            OpiCandidateWindow.shared.update(list, visible: !buffer.isEmpty)
        }
        // IMKCandidates 没有标题栏：拼音另画一条；显示串为空回退原文，判定仍读 buffer()。
        OpiPinyinBar.shared.update(display: engine.displayBuffer(), raw: buffer, client: client, candidatesFrame: OpiCandidateWindow.shared.visibleFrame)
    }

    // ---------- 候选窗回调 ----------

    /// IMKCandidates 向控制器要数据。README #3 记了一条「setCandidateData 不生效」
    /// 的报告，故两条路都留着：委托方法 + setCandidateData，任一条生效都能出候选。
    /// ⚠️ 形参**必须是 `Any!`**（IMK 的声明是 `- (NSArray *)candidates:(id)sender`）：
    /// 写成 `IMKCandidates!` 是**另一个选择器**，`override` 不会成功 ——
    /// 表现是候选窗永远空（委托没被调），且**不报错**。
    override func candidates(_ sender: Any!) -> [Any]! {
        return lastCandidates
    }

    /// 用户点选/回车选定候选（候选窗已先关闭）。
    override func candidateSelected(_ candidateString: NSAttributedString!) {
        let text = candidateString?.string ?? ""
        guard let index = lastCandidates.firstIndex(of: text) else { return }

        // 走引擎的**页内**选词：lastCandidates 是 `opi_candidates_page()` 的当前页，
        // 下标与 `opi_select_page(k)` 同序。绝不自己算 page*PAGE_SIZE+k（那是第三份
        // 页大小，引擎改一次就静默选错候选）。
        let committed = engine.selectPage(index)
        lastCandidates = []
        OpiCandidateWindow.shared.hide()
        OpiPinyinBar.shared.hide()
        // 这里没有 handle(_:client:) 的 sender：优先用 Hub 记下的最近客户端。
        if let stored = OpiClientHub.shared.client {
            commit(committed.isEmpty ? text : committed, to: stored)
        } else {
            commit(committed.isEmpty ? text : committed, to: client())
        }
    }

    // ---------- 输入法菜单 / 跨窗口上屏 ----------

    /// 输入法菜单（macOS 没有常驻状态栏，这里是 Linux 状态栏的等价入口）。
    override func menu() -> NSMenu! { OpiMenu.build(engine: engine, target: self) }

    /// IMK 把菜单选择转成这个调用：info 字典带回被点的 NSMenuItem。
    override func doCommand(by aSelector: Selector!, command infoDictionary: [AnyHashable: Any]!) {
        if let item = infoDictionary?[kIMKCommandMenuItemName] as? NSMenuItem {
            OpiMenu.perform(tag: item.tag, controller: self)
            return
        }
        super.doCommand(by: aSelector, command: infoDictionary)
    }

    /// 直连兜底。Apple 文档：菜单动作的 sender 可能是 info 字典（不是 NSMenuItem），
    /// 默认的 `doCommand` 就是拿它 `perform` 到 action 上 —— 两种形状都接。
    @objc func menuCommand(_ sender: Any?) {
        if let item = sender as? NSMenuItem {
            OpiMenu.perform(tag: item.tag, controller: self)
            return
        }
        if let info = sender as? [AnyHashable: Any],
           let item = info[kIMKCommandMenuItemName] as? NSMenuItem {
            OpiMenu.perform(tag: item.tag, controller: self)
        }
    }

    /// 菜单改模式后刷新 preedit/候选（没有 NSEvent sender，用 Hub 的客户端）。
    func refreshFromOutside() { refresh(OpiClientHub.shared.client) }

    /// 自绘虚拟键盘的唯一入口：把一次「按下」直接喂给引擎，**不合成系统按键**，
    /// 所以不需要辅助功能授权、也不会把焦点从宿主应用抢走。返回 false = 这一键
    /// 在引擎未处理时没有可用的 IMK 兜底（目前只有空缓冲的退格 —— IMK 没有
    /// 「删除宿主文本」的出口，这是平台限制，面板据此提示）。
    @discardableResult
    func virtualKey(keyval: UInt32, states: UInt32) -> Bool {
        let sender: Any?
        if let stored = OpiClientHub.shared.client { sender = stored } else { sender = client() }
        guard sender != nil else { return false }
        let result = engine.keyEvent(keyval: keyval, states: states)
        switch result.action {
        case OpiAction.commit:
            commit(result.text, to: sender)
            refresh(sender)
            return true
        case OpiAction.handled:
            refresh(sender)
            return true
        default:
            // 引擎放行（空缓冲的空格/回车/字母在英文模式下常走这里）：
            // 软键盘必须仍能出字，否则「点了没反应」。
            if keyval == OpiKeySpace.space {
                commit(" ", to: sender)
                return true
            }
            if keyval == OpiKeySpace.ret {
                commit("\n", to: sender)
                return true
            }
            if keyval < OpiKeySpace.specialBase, let scalar = Unicode.Scalar(keyval) {
                commit(String(Character(scalar)), to: sender)
                return true
            }
            return false // 空缓冲退格：宿主文本删不了（平台限制，见 README）
        }
    }

    /// 符号面板上屏。面板保持打开，可连续点选。
    func commitFromOutside(_ text: String) {
        guard !text.isEmpty else { return }
        if let stored = OpiClientHub.shared.client {
            commit(text, to: stored)
        } else {
            commit(text, to: client())
        }
    }

    // ---------- 生命周期 ----------

    override func activateServer(_ sender: Any!) {
        super.activateServer(sender)
        if let client = sender { OpiClientHub.shared.focus(self, client: client as AnyObject) }
        engine.clear()
        lastMarked = ""
        lastCandidates = []
        OpiPinyinBar.shared.hide()
        // 状态栏只在 opi 输入源激活时出现（用户 2026-10-21 要求）。
        OpiStatusBar.shared.setActive(true)
    }

    /// 失焦 = fcitx5 轨的 `InputMethodEngine::reset()`：清引擎缓冲，
    /// 否则换应用后 preedit 会带着上一个应用的残留。
    override func deactivateServer(_ sender: Any!) {
        engine.clear()
        lastMarked = ""
        lastCandidates = []
        OpiCandidateWindow.shared.hide()
        OpiPinyinBar.shared.hide()
        OpiKeyboardPanel.shared.hide()
        OpiStatusBar.shared.setActive(false)
        super.deactivateServer(sender)
    }
}

// ---------- 候选窗 ----------

/// 候选窗单例。main.swift 用启动时的 IMKServer 建一次。
/// ⚠️ 未验证：IMKCandidates 的 init 签名、panelType 常量名、以及**定位行为**
/// （用单例而非控制器自带窗口，窗随光标定位是否仍生效未经核对）——见 README #3。
final class OpiCandidateWindow {
    static let shared = OpiCandidateWindow()

    private var window: IMKCandidates?
    /// 当前是否由本层显示着。用于「已显示就只换数据，不再 show()」——
    /// 每键 show() 会白付一次窗口服务器调用与重新定位（面板位置由 IMK 跟着
    /// 插入点走，不需要我们每键提醒）。hide() 与 update(空/不可见) 都回写 false。
    private var shown = false

    var visibleFrame: NSRect? { nil } // IMKCandidates 无公开 frame；拼音条按光标定位

    private init() {}

    func configure(server: IMKServer) {
        // ✅ 首编确认（CI run 37934619550，2026-10-09）：init 也是**可失败**的（`IMKCandidates?`）。
        // 失败条件文档没写明；但候选窗是**显示层** —— 建不起来时输入法其余部分
        // （预编辑、上屏）照常工作，只是没有候选面板。所以这里**不**退进程（对照
        // main.swift 里 IMKServer 为 nil 时的 exit(1)：那是输入法根本没法工作），
        // 只打日志记账。window 保持 nil，`update`/`hide` 里本来就有 guard。
        guard let w = IMKCandidates(server: server,
                                    panelType: kIMKSingleRowSteppingCandidatePanel,
                                    styleType: kIMKMain) else {
            NSLog("opi: 候选窗建不起来（IMKCandidates init 返回 nil）—— 输入法仍可用，仅无候选面板")
            return
        }
        // 可见性由本层驱动（缓冲空就 hide、有候选就 show），不让面板自己关。
        // ⚠️ 默认是 true：那样「回车选中候选」会由**面板**处理并回调
        // candidateSelected → 若此时按键也进了 handle（Rust 侧回车 = 提交首候选），
        // 同一个候选会被提交两次。见 README「最不确定的 API」#3。
        w.setDismissesAutomatically(false)
        window = w
    }

    func update(_ list: [String], visible: Bool) {
        guard let w = window else { return }
        if list.isEmpty || !visible {
            w.hide()
            shown = false
            return
        }
        w.setCandidateData(list)
        // 首编确认（同轮）：ObjC 的 `updateCandidates` 在 Swift 里被改名为 `update()`
        // —— 编译器原文：「'updateCandidates()' has been renamed to 'update()'」。
        w.update()
        // 已显示时 setCandidateData+update 就够；重复 show() 只是每键多一次
        // 窗口服务器往返。首次显示（或我们自己 hide 过）才定位/显示。
        if !shown {
            w.show(kIMKLocateCandidatesBelowHint)
            shown = true
        }
    }

    func hide() {
        window?.hide()
        shown = false
    }
}
