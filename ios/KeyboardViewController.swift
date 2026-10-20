// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · **CI 已真编译**（apple.yml 的 iOS job，2026-10-15）· **从未上机运行**
//
// 本文件在 Linux 上编写。**已过 `swiftc ios/*.swift`（-typecheck + -emit-library）** ——
// 2026-10-15 那次全目录真编译（同一轮还当场抓出一处闭包里的 `continue`，见 RarePanel.swift）；
// 但从未在设备或模拟器上跑过：**行为仍未验证**，别把「编过」读成「能用」。
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

    // ⚠️ 下面这几个成员与 `refresh` / `insert` / `modeKeyLabel` 都是 **internal**
    // （不是 `private`）：`KeyboardHardware.swift` 那个扩展要用它们，而 Swift 的
    // `private` 只跨「**同文件**的扩展」可见，跨文件必须放开。
    // **不要为了整洁把它们改回 `private`** —— 那会让 `KeyboardHardware.swift` 编译不过。
    let engine = OpiEngine()
    var layout: KeyboardLayout!
    // 这里**没有** `page` 状态了：页码只在显示「第 N 页 / 共 M 页」时读一次
    // `engine.page()`，选词走 `engine.selectPage(k)` 的页内索引 —— 都不需要本层持有。

    /// 词库装载失败时置位 —— 用来在 UI 上显示可见的失败状态。
    ///
    /// 为什么非要显示：`opi_load` 返回 false 时，Rust 侧所有出口都会退化成
    /// 空操作（引擎单例保持为空）。不显示的话表现就是「键盘毫无反应」，
    /// 与 fcitx5 轨「词库损坏时插件静默全失效」是同一个坑。
    /// internal：装载实现搬到 `KeyboardDictionaryLoader.swift`（本文件压 500 行门禁）。
    var dictionaryLoaded = false

    /// 皮肤与键盘模式（2026-10-20）：启动时从扩展自己的 UserDefaults 读，
    /// 切换后即写回（Android `ImePreferences` 的等价物，但没有设置 App）。
    /// internal：`KeyboardTheme.swift` 里的 `applyTheme` 扩展要读。
    var skin: SkinOption = .follow
    var modes = KeyboardModes()
    /// 引擎开关（学习/模糊音/简拼/纠错/中文标点/双拼布局/自己表情）。
    /// 只在本文件用；设置面板拿到的永远是 `layout.setEngineSettings` 的快照。
    private var settings = KeyboardSettings()
    /// 设置面板 → 皮肤/键盘模式/关于 的返回目标：从设置进入的子面板，返回回设置，
    /// 再返回才回字母层（二级导航，不把用户直接弹回键盘）。
    private var settingsReturn = false
    /// 最近一次中文键面（英→中时恢复它；Android `cycleMode` 的「回上次中文方案」）。
    var lastChineseInputMode: InputMode = .pinyin26

    /// 表情搜索的防抖表（对齐 Android `ImeState.SEARCH_DEBOUNCE_MS = 250`：
    /// 每敲一个字就查一次全表是白费的，手停下才查）。
    ///
    /// 这里 `private` 是**对的**（与上面那几个成员相反，别顺手改成 internal）：
    /// 它只在本文件的委托扩展里用，`KeyboardHardware.swift` 碰不到它。
    /// ⚠️ 未编译：`Timer(timeInterval:repeats:block:)` 与 `RunLoop.add(_:forMode:)`
    /// 都是本仓**没用过**的 API（iOS 10+ 的块回调版）—— Mac 上第一件事是核实这两个，
    /// 若不便，退回 `Selector` 版（`scheduledTimer(timeInterval:target:selector:...)`）。
    private var emojiSearchTimer: Timer?
    private static let emojiSearchDebounce: TimeInterval = 0.25

    // MARK: - 生命周期

    override func viewDidLoad() {
        super.viewDidLoad()
        loadDictionaries()
        // 可选增强（笔画 / 五笔码表）在同一刻装：引擎刚就绪，正是「文件 → 引擎」的落点
        // （对齐 Android：`EngineLoader.load(context)` 里 luna → trad → stroke → wubi
        // 一口气装完，缺哪个降级哪个）。
        loadBundledDictionaries()

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
        // 皮肤/键盘模式从本扩展的偏好恢复（2026-10-20）；输入模式不持久化，
        // 沿用引擎当前方案（与 Android 的差异记在 README）。
        skin = KeyboardPreferences.skin
        modes = KeyboardPreferences.modes
        settings = KeyboardPreferences.settings
        layout.setSkin(skin)
        layout.setKeyboardModes(modes, traditional: engine.mode() == .traditional)
        // 引擎开关必须在装载词库**之后**推（未装载时写侧是空操作），且要在
        // 第一次 refresh 之前 —— 否则首屏候选按旧开关算。
        engine.applySettings(settings)
        layout.setEngineSettings(settings, fuzzyPairNames: engine.fuzzyPairNames())
        layout.emojiPanel.setShowOwn(settings.showOwn)
        applyTheme()
        // 键面字要跟真实模式一致：装载后引擎可能是任何模式（同一进程里另一个
        // 键盘实例切过模式也会留在这里 —— 引擎单例是进程级的）。
        updateModeLabel()
        refresh()
    }

    /// 键盘高度约束。只建一次 —— `viewWillAppear` 会被多次调用
    /// （旋转、切回、宿主重排），每次都 `isActive = true` 一条新约束的话，
    /// 会堆出一把互相冲突的高度约束。
    private var heightConstraint: NSLayoutConstraint?

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        guard heightConstraint == nil else { return }
        // 键盘高度必须由扩展自己给。portrait 264 / landscape 200：横屏时屏幕
        // 纵向只有 ~320pt，264 会把宿主输入框挤没；键区仍够放 4 行 + 候选栏。
        // priority 用 defaultHigh：避免与系统在旋转/分屏时的约束冲突。
        let h = view.heightAnchor.constraint(equalToConstant: preferredKeyboardHeight)
        h.priority = .defaultHigh
        h.isActive = true
        heightConstraint = h
    }

    /// 旋转后由系统重排触发；只改常数，**不**新建约束（新建会叠加冲突）。
    override func viewWillLayoutSubviews() {
        super.viewWillLayoutSubviews()
        guard let h = heightConstraint else { return }
        let target = preferredKeyboardHeight
        if abs(h.constant - target) > 0.5 { h.constant = target }
    }

    private var preferredKeyboardHeight: CGFloat {
        // 不能拿 `view.bounds` 判横竖：键盘 view 的宽度是屏宽、高度正是本约束
        // 给的 264 —— 320pt 竖屏会被误判成「宽 > 高」的横屏。看屏幕窗口。
        let screen = view.window?.windowScene?.screen
        let size = screen?.bounds.size ?? UIScreen.main.bounds.size
        return size.width > size.height ? 200 : 264
    }

    // 词库装载已搬到 `KeyboardDictionaryLoader.swift`（2026-10-21：本文件压 500 行门禁）。
    // `loadDictionaries()` / `loadBundledDictionaries()` 仍是控制器的方法，见那边。

    // MARK: - 刷新 UI

    /// 拉一次引擎状态刷新候选栏。**在这条路径上不要做别的**（不查磁盘、不发请求）。
    /// ⚠️ internal（见上方状态节的说明）：`KeyboardHardware.swift` 的热键路径要调它。
    func refresh() {
        guard dictionaryLoaded else {
            layout.updateCandidates(["词库未装载"])
            layout.setTopBar(candidatesVisible: true, english: false)
            return
        }
        // 候选列表**读引擎**（引擎分好的当前页）。本层既不按 8 切片、也不算页码 ——
        // 两处都曾是 PAGE_SIZE 的副本，现已分别由 opi_candidates_page / opi_select_page 消灭。
        let texts = engine.candidatesPage()
        layout.updateCandidates(texts)
        // 顶部槽位：有缓冲/候选 → 候选栏；否则工具条（Android `ImeScreen` 同判）。
        layout.setTopBar(
            candidatesVisible: !texts.isEmpty || !engine.buffer().isEmpty,
            english: engine.mode() == .english
        )
        syncMarkedText()
    }

    /// 组合串进字段 —— iOS 的原语是 **marked text**（`UITextDocumentProxy.setMarkedText`）。
    ///
    /// 功能与 Android 两条裁决同源（2026-10-10「打字期间输入框里能看到拼音」+
    /// 2026-10-12 晚「待编的键必须看得见」）：此前本端**一个字都不进字段** ——
    /// 打拼音时字段毫无动静，无匹时更是完全没反馈，与 Android 修之前是同一种病。
    /// 本端缓冲就是字母（全键盘方案），原样标出去即可，不需要 Android 那套「·」记号。
    ///
    /// 收尾：缓冲空 ⇒ `unmarkText()`（组合区收干净，别把标记留成孤岛）；
    /// 提交路径见 [insert]（先 unmark 再插，与 Android `dropComposing` 的次序同理由）。
    /// ⚠️ internal：热键路径（`KeyboardHardware.swift`）也走 [refresh]。
    ///
    /// ⚠️ `unmarkText()` 是**无条件**调的：键盘扩展的 `textDocumentProxy` 上**没有**
    /// `markedTextRange`（那是 `UITextInput` 的属性）—— 原写法 `if …markedTextRange != nil`
    /// 在真机 Swift 编译期就是 error（2026-10-13 apple.yml 的 iOS job 两处实测）。
    /// 语义上本就不需要那个守卫：`unmarkText()` = 解除**当前**标记，没有标记就无事可做。
    func syncMarkedText() {
        let buffer = engine.buffer()
        guard !buffer.isEmpty else {
            textDocumentProxy.unmarkText()
            return
        }
        textDocumentProxy.setMarkedText(
            buffer,
            selectedRange: NSRange(location: (buffer as NSString).length, length: 0)
        )
    }

    // MARK: - 文本提交

    /// 提交文本到宿主 App。所有 insertText 都走这里，方便将来加
    /// 「插前插后要通知宿主」这类修正时只有一处要改。
    /// ⚠️ internal（见上方状态节的说明）：`KeyboardHardware.swift` 的提交路径要调它。
    func insert(_ text: String) {
        guard !text.isEmpty else { return }
        // 先收组合区再插：否则 insertText 落进 marked range 里、替换掉那段拼音，
        // 表现随宿主而异（有的留下拼音、有的吞字）。对齐 Android `dropComposing`。
        // 无条件 unmark（理由见 [syncMarkedText] 的注：proxy 上没有 markedTextRange）。
        textDocumentProxy.unmarkText()
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

    // MARK: - 硬件键盘（iPad 外接键盘）

    /// 外接键盘的按下 / 抬起入口。**故意只做两件事**：把整批按键交给
    /// `routeHardware`（在 `KeyboardHardware.swift`），没被接管的整批交回系统。
    ///
    /// ⚠️ 这两个 `override` 留在本文件、**不**跟其余硬件键盘逻辑一起搬走：覆写 ObjC
    /// 方法能否写在**跨文件**的扩展里，本机没有编译器可问 —— 这是**未验证的取舍**，
    /// 故按最保守的写法放在类自己的文件里。搬走的是纯方法
    /// （`routeHardware` / `keyval(for:)` / `hotkey(for:states:)` /
    /// `performHotkey(_:)` / `modifierStates(_:)`，见 `KeyboardHardware.swift`）。
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
        case .showLetters:
            // 从设置进入的子面板（皮肤/键盘模式/关于）先回设置，再回字母层。
            if settingsReturn {
                let wasSettings = (layout.keyLayer == .settings)
                settingsReturn = false
                if !wasSettings {
                    layout.setLayer(.settings)
                    return
                }
            }
            layout.setLayer(.letters)
        case .showNumbers:
            settingsReturn = false
            layout.setLayer(.numbers)
        case .showSymbols:
            // 符号数据**现取**，不缓存：空结果是「没拿到」而不是「真的没有」，
            // 缓存空会让面板从此永远空（Android `SymbolCatalog` 踩过这个坑）。
            settingsReturn = false
            layout.setSymbols(engine.commonSymbols())
            layout.setLayer(.symbols)
        case .showEmoji:
            // 表情数据**现取**，不缓存（同符号层）。
            // ⚠️ 与符号层有一处**实质差别**：`emojiSymbols()` / `emojiGroups()` 两个出口
            // **不经引擎单例** —— 编译期静态表，`opi_load` 失败时照样返回全量
            // （cabi 在 `cabi_symbols.rs` 头注与 `cabi_unloaded.rs` 的
            // `emoji_groups_index_the_full_emoji_list` 两处写明，并点名「别照抄它们的
            // 姿态去改别的出口」）。所以这里**不看 `dictionaryLoaded`**、面板也不会
            // 显示「词库未装载」—— 表情本来就不依赖词库。
            settingsReturn = false
            layout.setEmojiBrowse(engine.emojiSymbols(), groups: engine.emojiGroups())
            layout.setLayer(.emoji)
        case .showModes:
            // 输入模式菜单（2026-10-14）：**只是切层** —— 菜单里的选项本身不动引擎，
            // 换方案/换键面在用户选中后走 `applyInputMode`（`InputModes.swift`）。
            settingsReturn = false
            layout.setLayer(.modes)
        case .showRare:
            // 生僻字面板（2026-10-14）：同上，只是切层。**数据不在这里取** ——
            // 索引是 `rare_index.tsv`（不是引擎出口），面板自己在首次打开时读，
            // 见 `RarePanel.show()`。这与符号/表情两层「控制器现取引擎数据」不同，
            // 差别的原因就是数据来源不同（引擎 vs bundle 文件）。
            settingsReturn = false
            layout.setLayer(.rare)
        case .showSkin:
            // 皮肤面板（2026-10-20）：与 Android 工具条入口同义。选中不关面板。
            settingsReturn = (layout.keyLayer == .settings)
            layout.setSkin(skin)
            layout.setLayer(.skins)
        case .selectSkin(let newSkin):
            skin = newSkin
            KeyboardPreferences.skin = newSkin
            layout.setSkin(newSkin)
            applyTheme()
        case .showKeyboardModes:
            settingsReturn = (layout.keyLayer == .settings)
            layout.setKeyboardModes(modes, traditional: engine.mode() == .traditional)
            layout.setLayer(.keyboardModes)
        case .updateKeyboardModes(let newModes):
            modes = newModes
            KeyboardPreferences.modes = newModes
            layout.setKeyboardModes(
                newModes, traditional: engine.mode() == .traditional
            )
            applyTheme()
        case .toggleTraditional:
            toggleTraditional()
        case .showAbout:
            settingsReturn = (layout.keyLayer == .settings)
            layout.setLayer(.about)
        case .showSettings:
            // 每次都重取一份引擎组名（装载可能刚完成/失败），保证逐组 chip 与引擎同步。
            settingsReturn = true
            layout.setEngineSettings(settings, fuzzyPairNames: engine.fuzzyPairNames())
            layout.setLayer(.settings)
        case .updateSettings(let newSettings):
            settings = newSettings
            KeyboardPreferences.settings = newSettings
            // 立即推给引擎（Android 设置页同姿态：没有「保存」按钮）。
            engine.applySettings(newSettings)
            layout.setEngineSettings(newSettings, fuzzyPairNames: engine.fuzzyPairNames())
            layout.emojiPanel.setShowOwn(newSettings.showOwn)
            // 模糊音/简拼/纠错会改变当前缓冲的候选，重拉一次。
            refresh()
        case .closeKeyboard:
            // 工具条最右的收起键（Android 同位置）。
            dismissKeyboard()
        case .segment:
            // 分词键：缓冲空时**不发**（空缓冲的 `'` 在引擎里是引号，不是分词符）。
            guard !engine.buffer().isEmpty, let quote = "'".unicodeScalars.first else {
                return
            }
            route(keyval: OpiKey.printable(quote), states: 0, fallback: .none)
        case .openURL(let raw):
            openExternal(raw)
        case .retype:
            // 重输（9 键右列）：清掉当前组合重来。**不提交** —— 这个键的语义就是
            // 「打错了重来」，直提缓冲会把 9 键的数字打进输入框。
            // `clear()` 幂等（缓冲空时是空操作），所以不必先判空。
            engine.clear()
            refresh()
        case .selectInputMode(let mode):
            // 菜单选中一项。语义（换方案 + 换模式 + 换键面 + 清缓冲的取舍）全在
            // `applyInputMode` 里，那里也写明了与 Android 的**能力缺口**。
            settingsReturn = false
            applyInputMode(mode)
        case .cycleMode:
            // 非中文（英文/数字/符号）→ 中文：走 applyInputMode 而不是只 switchMode
            // —— 后者不会同步 layout.inputMode，英文键盘上点「中」/模式键会看起来没反应
            // （2026-10-21）。判据与 Android `cycleMode` 的 else 档同：不在中/繁就回
            // 上次中文方案；中→繁→英两跳交给 `cycleMode()`。
            if engine.mode() != .pinyin && engine.mode() != .traditional {
                // 非中文态回到键面：设置面板若开着，这条路径也会关掉它。
                settingsReturn = false
                applyInputMode(lastChineseInputMode)
            } else {
                cycleMode()
            }
        case .nextInputMode:
            // 地球键：交给系统切到下一个输入法（Apple 要求自定义键盘自己提供）。
            advanceToNextInputMode()
        case .commitText(let text):
            // 符号面板的键**直提，不过引擎**：引擎的标点表会把 `,` 改写成中文标点/
            // 全角（那是文本模式该做的事），而用户点的就是 `,`。
            // 不调 refresh：缓冲没变，候选栏也不该变。
            insert(text)
        }
    }

    func keyboardLayout(_ layout: KeyboardLayout, didSearchEmoji query: String) {
        // 防抖 250ms（对齐 Android `ImeState`）：每敲一个字打一次 FFI 是白费的
        // —— 符号搜索要扫全表。去空白与「空查询怎么办」都在 [runEmojiSearch]。
        emojiSearchTimer?.invalidate()
        let t = Timer(timeInterval: Self.emojiSearchDebounce, repeats: false) { [weak self] _ in
            self?.runEmojiSearch(query)
        }
        // 加进 `.common` 而不是默认模式：`.default` 在滚动/连续触摸时会停表，
        // 防抖的语义是「手停下来才查」，停表会把它拖到松手之后。
        RunLoop.current.add(t, forMode: .common)
        emojiSearchTimer = t
    }

    /// 真去查（防抖到点后）。搜索词在**这里**去空白（对齐 Android `searchText.trim()`
    /// —— 那边也是实时 text、去抖后 query 才生效）。
    ///
    /// ⚠️ 未编译：`trimmingCharacters(in: .whitespacesAndNewlines)` 是 Foundation 常规
    /// API，但本仓没用过；它与 Kotlin `trim()` 的空白集不完全相同（Kotlin 按
    /// `Char.isWhitespace`，更宽），差集在少数控制字符上 —— Mac 上顺手核一眼。
    ///
    /// 空查询 = **回浏览内容，不是「搜到 0 条」**：Android 的内容分支是
    /// `searchActive && searchQuery.isNotEmpty()`，空查询画的就是浏览网格。
    /// 所以这里连 FFI 都不发 —— `searchSymbols("")` 会返回**全部**符号，白捞一遍全表。
    private func runEmojiSearch(_ raw: String) {
        let q = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        let results = q.isEmpty ? [] : engine.searchSymbols(keyword: q)
        layout.setEmojiResults(results, query: q)
    }

    /// 模式三态循环与模式键键面字（2026-10-20 移到 `KeyboardTheme.swift`；
    /// 本文件压 500 行门禁）。调用点见本文件的 `.cycleMode` 分支与 `updateModeLabel()`。
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
