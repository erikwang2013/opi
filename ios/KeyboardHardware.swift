// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写，未经过任何 Swift 编译器 —— 一个字符都没有编译过。
// UIKit 是 Apple 独有框架，本机 `import UIKit` 就会断，所以连语法检查都没做过。
//
// 外接键盘（iPad 外接键盘 / 硬件键盘）那一路：`UIKey`/`UIPress` → 引擎键码，
// 以及**引擎之前**判掉的热键。拆出来是因为 `KeyboardViewController.swift` 顶到了
// 本仓 500 行源码上限，而这两件事本来就是两个概念 —— 一个是**软键盘**
// （触摸、层、候选栏），一个是**硬件键盘**（HID 键码、修饰位、autorepeat）。
// （同 `KeyboardKeys.swift` 的拆法：那次拆的是「键的定义 vs 布局」。）
//
// ⚠️ `pressesBegan` / `pressesEnded` 两个 **override 不在本文件**，留在
//    `KeyboardViewController.swift`：覆写 ObjC 方法能否写在**跨文件**扩展里，
//    本机没有编译器可问（这是**未验证的取舍**，不是已验证的结论），故按最保守的
//    写法留在类自己的文件里。它们只做「把整批按键交给 `routeHardware`，没接管就 super」。
//
// ⚠️ 本扩展用到 `engine` / `layout` / `insert` / `refresh` / `modeLabel`，它们因此
//    在主文件里是 **internal**（Swift 的 `private` 只跨「同文件的扩展」可见，跨文件必须放开）。
//    **不要为了整洁把它们改回 `private`** —— 那会让本文件编译不过。

import UIKit

extension KeyboardViewController {

    /// 外接键盘一批按键的完整处理。返回 true = 本次事件已被引擎处理（不要再交回系统）。
    ///
    /// ⚠️ 本节的 API 用法是本目录**最不确定**的部分：`UIKey.keyCode` 的
    /// `UIKeyboardHIDUsage` 枚举成员名、以及 `pressesBegan` 的参数签名，
    /// 都是凭记忆写的，Mac 上很可能要改（见 README 最不确定清单第 1 条）。
    ///
    /// 为什么值得写：router.rs 的路由表**依赖按下/抬起成对出现**，而软键盘
    /// 只覆盖了一部分场景；外接键盘是唯一能产生 PageUp/PageDown/方向键的来源。
    ///
    /// ⚠️ **热键不能送进 `keyEvent`** —— 两个都会被引擎吃掉，而且是**静默**的，
    /// 且**症状不同**（后者更难查：「按了有反应但是错的」比「按了没反应」坏）：
    ///   * `Ctrl+'` / `Ctrl+\`：`router.rs` 的 `key_event` 直通分支
    ///     （`router.rs:245`）对 `CTRL|ALT|META` **一律直通**，而那句在 `key_event`
    ///     的最前面 —— 送进去等于交回宿主 App，模式一动不动、无日志。
    ///   * `Shift+Space`：SHIFT **不在**直通掩码里，所以它会一路走到空格分支；而
    ///     `KEY_SPACE` 分支**不看 Shift 位**，表现为「Shift+空格＝选首候选」而不是切全角。
    /// 所以热键在**本层、调 `keyEvent` 之前**判掉（`hotkey(for:states:)`）——
    /// 桌面两轨也是这么做的（`tsf-opi/src/tsf.rs` 里 `mode_hotkey` 的**调用点**判在引擎之前，
    /// 注释写了同一条理由：送进引擎就是普通空格，会被当成选首候选）。
    /// 键位与 `tsf-opi/src/vk.rs` / `opi_fcitx5.cpp` 同一张表：
    /// `Ctrl+'` 切英文、`Ctrl+\` 切符号、`Shift+Space` 切全角。
    func routeHardware(
        _ presses: Set<UIPress>, released: Bool, event: UIPressesEvent?
    ) -> Bool {
        var handledAny = false
        for press in presses {
            guard let key = press.key, let kv = keyval(for: key) else { continue }
            var states = modifierStates(key.modifierFlags)
            if released { states |= OpiKey.stateReleased }
            // ⚠️ **热键必须在这里判掉，不能落到下面的 `keyEvent`** —— 两种静默失效
            // 见本节头注释（`Ctrl+'\` 会被整颗直通交回宿主 App；`Shift+Space` 会变成
            // 「选首候选」）。桌面两轨也是在引擎之前判的（`tsf-opi/src/tsf.rs`）。
            if let hot = hotkey(for: key, states: states) {
                // 只在这一下是「**首次按下**」时才执行：抬起与 autorepeat 都**只消费、不动作**。
                //   * 抬起只消费：否则按住不放会反复切模式。（桌面两轨同判：
                //     `opi_fcitx5.cpp` 的 handleFullwidthHotkey 对抬起也一并消费、但不再切，
                //     注释写明「只拦按下会给客户端一个无 keydown 的 keyup」。）
                //   * ⚠️ autorepeat **绝不能放行**给下面的 `keyEvent`：`Shift+Space` 的
                //     SHIFT 不在 router.rs 的直通掩码里，放行会被当成**普通空格＝选首候选**
                //     —— 那比「反复切全角」更坏。四轨同判（`vk.rs` / `opi_fcitx5.cpp` 同轮加）。
                //   * ⚠️ 本端当前**收不到** `stateRepeat`（见 `modifierStates` ——
                //     `UIKeyModifierFlags` 里没有 repeat 位，`UIKey`/`UIPress` 也没有别的来源），
                //     所以这半句在 iOS 上是**预防性**的、不是活代码；macOS 侧是活的
                //     （`NSEvent.isARepeat`）。**不要**为了「对称」去编一个位出来。
                if !released, states & OpiKey.stateRepeat == 0 { performHotkey(hot) }
                handledAny = true
                continue
            }
            switch engine.keyEvent(keyval: kv, states: states) {
            case .passThrough:
                continue    // 这一个交回系统（外层会调 super）
            case .commit(let text):
                // ⚠️ 容易漏：硬件键盘的提交文本也得自己 insertText，
                // 不能因为「外接键盘」就以为系统会替我们上屏。漏了的表现是
                // 「按字母有候选、按空格候选消失但字没上屏」。
                insert(text)
                handledAny = true
            case .handled:
                handledAny = true
            }
        }
        // 有键被受理 → 刷新候选栏（缓冲/页码可能变了）。
        if handledAny { refresh() }
        return handledAny
    }

    /// `UIKey` → keyval。**不要按 `characters` 猜特殊键**（keys.rs 模块头明确警告：
    /// 退格可能是 BS 也可能是 DEL，猜错的后果是该键在输入态下静默失效）。
    /// 特殊键一律认 HID usage；只有可打印字符才取 `characters` 的码点。
    func keyval(for key: UIKey) -> UInt32? {
        switch key.keyCode {
        // ⚠️ 是 `.keyboardDeleteOrBackspace`（HID 0x2A），**没有** `.keyboardBackspace`
        // 这个成员。写错是**编译错误**而不是静默失效 —— 也就是说这类错在 Mac 上
        // 第一轮编译就会全部暴露，不会带到运行时。
        case .keyboardDeleteOrBackspace: return OpiKey.backspace
        // 注意：router.rs 把 KEY_DELETE 与 KEY_BACK_SPACE 归到同一分支（都按退格处理）,
        // 这是既有的两轨语义，不是这里写错。
        case .keyboardDeleteForward: return OpiKey.delete
        case .keyboardReturnOrEnter: return OpiKey.ret
        case .keyboardTab: return OpiKey.tab
        case .keyboardEscape: return OpiKey.escape
        case .keyboardPageUp: return OpiKey.pageUp
        case .keyboardPageDown: return OpiKey.pageDown
        case .keyboardLeftShift, .keyboardRightShift: return OpiKey.shift
        case .keyboardUpArrow: return OpiKey.up
        case .keyboardDownArrow: return OpiKey.down
        case .keyboardLeftArrow: return OpiKey.left
        case .keyboardRightArrow: return OpiKey.right
        case .keyboardSpacebar: return OpiKey.space
        default:
            // 可打印字符：取 `characters`（**大小写已由平台应用**，keys.rs 的约定）。
            // 空串（F1-F12、媒体键等无可打印表示）→ 返回 nil，整键交回系统。
            guard let scalar = key.characters.unicodeScalars.first,
                  key.characters.unicodeScalars.count == 1
            else { return nil }
            return OpiKey.printable(scalar)
        }
    }

    /// 引擎之前判掉的热键（键位与桌面两轨一致）。
    enum OpiHotkey {
        case toggleEnglish      // Ctrl + '
        case toggleSymbol       // Ctrl + \
        case toggleFullwidth    // Shift + Space
    }

    /// 与 `tsf-opi/src/vk.rs` 的 `mode_hotkey` 与 `fullwidth_hotkey`
    /// **同判**。返回 nil = 不是热键，照常送引擎。
    ///
    /// ⚠️ **本函数故意不过滤抬起与 autorepeat** —— 它们**也是热键**，只是不该执行。
    /// 由调用处决定「消费但不动作」：若在这里返回 nil，那一颗键就会漏给下面
    /// 的 `keyEvent`（`Shift+Space` 的后果见调用处注释）。
    ///
    /// ⚠️ `UIKeyboardHIDUsage` 的 `.keyboardQuote` / `.keyboardBackslash` 又是我
    /// **没编译过**的成员名（同 `keyval(for:)` 那批）。写错的后果是**编译错误**，
    /// 不是静默失效 —— 也就是说这处在 Mac 上第一轮编译就会暴露。
    func hotkey(for key: UIKey, states: UInt32) -> OpiHotkey? {
        // Ctrl + ' / Ctrl + \ —— `mode_hotkey` 要求 Ctrl 位，且只认这两个键。
        if states & OpiKey.stateCtrl != 0 {
            switch key.keyCode {
            case .keyboardQuote: return .toggleEnglish
            case .keyboardBackslash: return .toggleSymbol
            default: return nil
            }
        }
        // Shift + Space —— `fullwidth_hotkey` 要求 Shift、排掉 Ctrl/Alt。
        // ⚠️ 这里**比 vk.rs 多排一个 META**：那张表是 Win32 侧的（没有 ⌘）。
        // Apple 两平台上 ⌘ 组合都不该被输入法吃掉（macOS 的 ⌘Space 更是 Spotlight），
        // 所以 `⌘⇧Space` 也不是我们的。**这是有意的平台差异，不是抄漏**
        // —— 两侧（`KeyboardHardware.swift` / `InputController.swift`）写的是同一句。
        if states & OpiKey.stateShift != 0,
           states & (OpiKey.stateCtrl | OpiKey.stateAlt | OpiKey.stateMeta) == 0,
           key.keyCode == .keyboardSpacebar {
            return .toggleFullwidth
        }
        return nil
    }

    /// 执行热键。三条都**不经过** `opi_key_event`。
    func performHotkey(_ hot: OpiHotkey) {
        switch hot {
        case .toggleEnglish:
            // 与 `vk.rs` 的 `hotkey_target` 同判：**来回切**，已在英文就回拼音。
            // 目标模式在本层算是对的 —— 桌面两轨也是客户端算的（`hotkey_target` 在
            // `vk.rs`，不在引擎里）。契约禁止的是客户端推**全角映射**，不是这个。
            engine.switchMode(engine.mode() == .english ? .pinyin : .english)
        case .toggleSymbol:
            // 这个出口**自己**包办 Pinyin ⇄ Symbol 的来回切 **和** 缓冲收尾
            // （有候选提交首候选、乱码缓冲清掉不上屏），所以本层**不判目标模式**
            // —— 判了就是第二份 `toggle_symbol` 语义。
            insert(engine.toggleSymbol())
        case .toggleFullwidth:
            engine.toggleFullwidth()
            // ⚠️ **不刷候选面板**，直接返回：这个开关只改**后续**的标点映射，
            // 不动缓冲也不动候选（`Engine::toggle_fullwidth` 只翻一个 bool），
            // 推一帧是白推。**与上面两跳的差别正在这里** —— 那两跳切模式会清缓冲换候选，
            // 不刷会留着上一次的候选。桌面两轨同判（`tsf.rs` 里 `handleFullwidthHotkey` 调用点上方那条注释、
            // 与 `opi_fcitx5.cpp` 的 `handleFullwidthHotkey`
            // 的 handleFullwidthHotkey 都注了这条，两处措辞一致）。
            return
        }
        // 模式可能变了 → 键面必须跟着说实话（它同时是「当前是什么模式」的显示器）。
        layout.setModeLabel(Self.modeLabel(of: engine.mode()))
        // ⚠️ 约定 1 说 `switchMode` / `toggleSymbol` 之后必须重读全角。本目录
        // **没有全角指示灯**（同 `shiftState()` 那条：出口有了、包装有了、UI 没读），
        // 所以当前没有陈旧值可清。**哪天真加了指示灯，重读就写在这里** ——
        // 别用本地翻位代替，那正是漂移的来源。
        refresh()
    }

    /// 修饰位映射。CTRL/ALT/META 必须传：router.rs 对它们**一律直通**，
    /// 否则 ⌘A / ⌘C 会被吃进拼音缓冲。
    ///
    /// ⚠️ 本函数**不置 `stateRepeat`**（`1<<27`）：`UIKeyModifierFlags` 里没有
    /// repeat 位，`UIKey`/`UIPress` 上也没有别的来源可取（凭记忆，本机无从核对）。
    /// 这是**有意留空**，不是漏写 —— 全引擎只有一个消费者
    /// （`router.rs` 的 `handle_shift`：按住 ⇧ 时不要反复切 shift 状态机），
    /// 而 iOS 的 ⇧ 用的是**另一种**机制（软键盘长按 → `stateLongPressed` = Lock）。
    /// **哪天真在 Mac 上找到 autorepeat 信号，就加在这里**，别在调用处手搓一个位。
    func modifierStates(_ f: UIKeyModifierFlags) -> UInt32 {
        var s: UInt32 = 0
        if f.contains(.shift) { s |= OpiKey.stateShift }
        if f.contains(.alphaShift) { s |= OpiKey.stateCapsLock }
        if f.contains(.control) { s |= OpiKey.stateCtrl }
        if f.contains(.alternate) { s |= OpiKey.stateAlt }
        if f.contains(.command) { s |= OpiKey.stateMeta }
        return s
    }
}
