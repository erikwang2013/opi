// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写，未经过任何 Swift 编译器 —— 一个字符都没有编译过。
// UIKit 是 Apple 独有框架，本机 `import UIKit` 就会断，所以连语法检查都没做过。
// 见 ios/README.md「在 Mac 上第一件要做的事」：先让它编译过，再谈功能。
//
// 键盘扩展入口。职责只有三件：
//   1. 生命周期：建引擎、装词库、给 inputView 定高。
//   2. 把 UI 事件翻成 `opi_key_event(keyval, states)` 的入参。
//   3. 按返回的 action 决定 insertText / 刷新 / 走 fallback / 交系统。
//
// **不要**在这里加输入法逻辑（拼音怎么切、候选怎么排、⇧ 怎么切）。
// 那些全在 Rust（crates/engine-core/src/router.rs）。本文件越薄越好。

import UIKit

/// 类名写死成不带模块前缀的 @objc 名，供 Info.plist 的
/// `NSExtensionPrincipalClass = $(PRODUCT_MODULE_NAME).KeyboardViewController` 找到。
/// ⚠️ 未验证：模块名 + 类名的拼法按记忆写的，Mac 上若启动即崩，
/// 第一个要查的就是这里（连同 Info.plist 的 NSExtensionPrincipalClass）。
@objc(KeyboardViewController)
final class KeyboardViewController: UIInputViewController {

    // MARK: - 状态

    private let engine = OpiEngine()
    private var layout: KeyboardLayout!
    // 这里**没有** `page` 状态了：页码只在显示「第 N 页 / 共 M 页」时读一次
    // `engine.page()`，选词走 `engine.selectPage(k)` 的页内索引 —— 都不需要本层持有。

    /// 词库装载失败时置位 —— 用来在 UI 上显示可见的失败状态。
    ///
    /// 为什么非要显示：`opi_load` 返回 false 时，Rust 侧所有出口都会退化成
    /// 空操作（引擎单例保持为空）。不显示的话表现就是「键盘毫无反应」，
    /// 与 fcitx5 轨「词库损坏时插件静默全失效」是同一个坑。
    private var dictionaryLoaded = false

    // MARK: - 生命周期

    override func viewDidLoad() {
        super.viewDidLoad()
        loadDictionaries()

        layout = KeyboardLayout()
        layout.delegate = self
        layout.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(layout)
        NSLayoutConstraint.activate([
            layout.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            layout.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            layout.topAnchor.constraint(equalTo: view.topAnchor),
            layout.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])
        refresh()
    }

    /// 键盘高度约束。只建一次 —— `viewWillAppear` 会被多次调用
    /// （旋转、切回、宿主重排），每次都 `isActive = true` 一条新约束的话，
    /// 会堆出一把互相冲突的高度约束。
    private var heightConstraint: NSLayoutConstraint?

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        guard heightConstraint == nil else { return }
        // ⚠️ 未编译：键盘高度必须由扩展自己给。264 这个值是拍的，
        // 真机上要按候选栏 + 4 行的实际高度算（或让 layout 用 intrinsicContentSize）。
        // priority 用 defaultHigh：避免与系统在旋转/分屏时的约束冲突。
        let h = view.heightAnchor.constraint(equalToConstant: 264)
        h.priority = .defaultHigh
        h.isActive = true
        heightConstraint = h
    }

    /// 装载词库。
    ///
    /// 词库文件来自**扩展自己的 bundle**（不是宿主 App、不是 App Group）——
    /// 与 Android 把 luna.opid / trad.opid 放进 assets 同一思路。
    /// 文件从哪来见 ios/README.md「构建集成」。
    private func loadDictionaries() {
        if let p = Bundle.main.path(forResource: "luna", ofType: "opid") {
            dictionaryLoaded = engine.load(path: p)
        } else {
            // 没打包词库文件 → 退化用编译进二进制的内置回退词库
            // （opi_load 对空路径的语义）。可用但候选很少，UI 上要提示。
            dictionaryLoaded = engine.load(path: "")
        }
        // 繁体库可选：失败**不是**错误 —— 繁体模式会回退简体库（Rust 侧语义）。
        if let t = Bundle.main.path(forResource: "trad", ofType: "opid") {
            engine.loadTrad(path: t)
        }
        if !dictionaryLoaded {
            // 不静默。候选栏里会显示「词库未装载」。
            NSLog("[OPI] 词库装载失败 —— 引擎将退化为空操作")
        }
    }

    // MARK: - 刷新 UI

    /// 拉一次引擎状态刷新候选栏。**在这条路径上不要做别的**（不查磁盘、不发请求）。
    private func refresh() {
        guard dictionaryLoaded else {
            layout.updateCandidates(["词库未装载"])
            return
        }
        // 候选列表**读引擎**（引擎分好的当前页）。本层既不按 8 切片、也不算页码 ——
        // 两处都曾是 PAGE_SIZE 的副本，现已分别由 opi_candidates_page / opi_select_page 消灭。
        layout.updateCandidates(engine.candidatesPage())
    }

    // MARK: - 文本提交

    /// 提交文本到宿主 App。所有 insertText 都走这里，方便将来加
    /// 「插前插后要通知宿主」这类修正时只有一处要改。
    private func insert(_ text: String) {
        guard !text.isEmpty else { return }
        textDocumentProxy.insertText(text)
    }

    /// 引擎说「这键我不管」时，客户端该做什么（平台语义，见 KeyFallback 注释）。
    private func applyFallback(_ f: KeyFallback) {
        switch f {
        case .none:
            break
        case .text(let t):
            insert(t)
        case .deleteBackward:
            textDocumentProxy.deleteBackward()
        }
    }

    // MARK: - 路由（软键盘的唯一入口）

    /// 一个软键盘按键的完整处理：路由 → 按 action 分流 → 刷新。
    ///
    /// `action` 的三种含义见 `cabi.rs`：
    ///   0 passThrough → 引擎不管，走平台 fallback（软键盘没有「交回系统」这条路）
    ///   1 handled     → 引擎消费了，只需刷新候选栏 / preedit
    ///   2 commit      → 有文本要上屏
    private func route(keyval: UInt32, states: UInt32, fallback: KeyFallback) {
        // 词库没装上时引擎的所有出口都会退化成 PassThrough（空操作）。
        // 直接走 fallback 打字，至少键盘还能当个英文键盘用，且不静默。
        guard dictionaryLoaded else {
            applyFallback(fallback)
            return
        }
        switch engine.keyEvent(keyval: keyval, states: states) {
        case .commit(let text):
            insert(text)
            refresh()
        case .handled:
            refresh()
        case .passThrough:
            applyFallback(fallback)
            refresh()
        }
    }
}

// MARK: - 软件键盘：按下 / 抬起

extension KeyboardViewController: KeyboardLayoutDelegate {

    func keyboardLayout(_ layout: KeyboardLayout, didPress spec: KeySpec, longPress: Bool) {
        // 从 spec.states 起算（键上自带的基础修饰位），再叠加本次的长按。
        var states = spec.states
        if longPress { states |= OpiKey.stateLongPressed }
        route(keyval: spec.keyval, states: states, fallback: spec.fallback)
    }

    func keyboardLayout(_ layout: KeyboardLayout, didRelease spec: KeySpec) {
        // 抬起事件必须发：router.rs 对可打印键「抬起按按下的结论回复」
        // （last_printable 单槽），对退格/回车「抬起与按下同判」，只发按下会让
        // 引擎的 ⇧ 状态机与键盘状态不完整。抬起永不返回 commit。
        // 这里没有 fallback —— 抬起时 falls through 不需要客户端做任何事。
        _ = engine.keyEvent(keyval: spec.keyval, states: spec.states | OpiKey.stateReleased)
    }

    func keyboardLayout(_ layout: KeyboardLayout, didSelectCandidateAt index: Int) {
        // index 是**页内**序号（当前页第几个）。走 opi_select_page，页内换算归引擎 ——
        // 所以这里**不出现** `page * pageSize` 这种算式，本层也不持有页大小常量。
        // 越界返回空串，不会崩。
        let text = engine.selectPage(index)
        insert(text)
        refresh()
    }

    func keyboardLayout(_ layout: KeyboardLayout, didRequestPage delta: Int) {
        // 翻页走引擎（页码钳制在 router.rs 里），用 PAGE_UP / PAGE_DOWN 键码。
        // 这比在 Swift 侧自己加减页码更对：页码真源在引擎，且空页会被钳住。
        let keyval = delta < 0 ? OpiKey.pageUp : OpiKey.pageDown
        if case .handled = engine.keyEvent(keyval: keyval, states: 0) {
            // 引擎已翻页。**不本地加减 delta** —— 引擎会把页码钳在末页，本地累加就漂了。
            // 直接重拉：候选列表由 opi_candidates_page 给，本层不持有页码状态。
            refresh()
        }
    }

    func keyboardLayout(_ layout: KeyboardLayout, didTapFunction name: KeyboardFunctionKey) {
        switch name {
        case .toggleLayer:
            layout.setLetterLayer(!layout.isLetterLayer)
        case .toggleLanguage:
            // 中/英切换：0=Pinyin 1=English。走备用出口是因为 opi_key_event 的
            // 键空间里没有「切语言」这个键（两轨是走 UI 按钮的）。
            // 注意：切到 English 后 ⇧ 状态机仍在引擎里，不受影响。
            engine.switchMode(engine.mode() == .pinyin ? .english : .pinyin)
            refresh()
        case .nextInputMode:
            // 地球键：交给系统切到下一个输入法（Apple 要求自定义键盘自己提供）。
            advanceToNextInputMode()
        }
    }
}

// MARK: - 硬件键盘（iPad 外接键盘）

extension KeyboardViewController {

    /// ⚠️ 本节的 API 用法是本文件**最不确定**的部分：`UIKey.keyCode` 的
    /// `UIKeyboardHIDUsage` 枚举成员名、以及 pressesBegan 的参数签名，
    /// 都是凭记忆写的，Mac 上很可能要改（见 README 最不确定清单第 1 条）。
    ///
    /// 为什么值得写：router.rs 的路由表**依赖按下/抬起成对出现**，而软键盘
    /// 只覆盖了一部分场景；外接键盘是唯一能产生 PageUp/PageDown/方向键的来源。
    override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        if !routeHardware(presses, released: false, event: event) {
            // 引擎没接管的键，必须交回系统 —— 否则宿主 App 收不到方向键、
            // ⌘ 组合键等（用户的键盘就像坏了一样）。
            super.pressesBegan(presses, with: event)
        }
    }

    override func pressesEnded(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        if !routeHardware(presses, released: true, event: event) {
            super.pressesEnded(presses, with: event)
        }
    }

    /// 返回 true = 本次事件已被引擎处理（不要再交回系统）。
    private func routeHardware(
        _ presses: Set<UIPress>, released: Bool, event: UIPressesEvent?
    ) -> Bool {
        var handledAny = false
        for press in presses {
            guard let key = press.key, let kv = keyval(for: key) else { continue }
            var states = modifierStates(key.modifierFlags)
            if released { states |= OpiKey.stateReleased }
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
    private func keyval(for key: UIKey) -> UInt32? {
        switch key.keyCode {
        case .keyboardBackspace: return OpiKey.backspace
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

    /// 修饰位映射。CTRL/ALT/META 必须传：router.rs 对它们**一律直通**，
    /// 否则 ⌘A / ⌘C 会被吃进拼音缓冲。
    private func modifierStates(_ f: UIKeyModifierFlags) -> UInt32 {
        var s: UInt32 = 0
        if f.contains(.shift) { s |= OpiKey.stateShift }
        if f.contains(.alphaShift) { s |= OpiKey.stateCapsLock }
        if f.contains(.control) { s |= OpiKey.stateCtrl }
        if f.contains(.alternate) { s |= OpiKey.stateAlt }
        if f.contains(.command) { s |= OpiKey.stateMeta }
        return s
    }
}

// MARK: - 宿主文本变化

extension KeyboardViewController {
    // ⚠️ 未编译 / 待核对：这两个是**系统回调**（宿主文本变化时调用），
    // 我把它们当「需要刷新本地状态」的信号来重写 —— 正是官方文档描述的用途
    // （典型场景是重读 documentContextBeforeInput 决定 ⇧ 状态）。
    // 但「插入前后要不要主动调一次通知宿主」我无法查证（Sandbox 里查不了文档），
    // 按不加写。Mac 上若发现宿主文本状态不同步，这里是第一个嫌疑点。

    override func textWillChange(_ textInput: UITextInput?) {
        super.textWillChange(textInput)
    }

    override func textDidChange(_ textInput: UITextInput?) {
        super.textDidChange(textInput)
        // 宿主文本变了 → 候选栏可能已经过期（例：用户点进另一个输入框）。
        refresh()
    }
}
