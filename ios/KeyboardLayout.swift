// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写，未经过任何 Swift 编译器。
// 目标**不是**生产级键盘外观 —— 那是在 Mac 上对着真机迭代出来的东西。
// 目标是**结构正确的最小骨架**：候选栏 + 键位行 + 按下/抬起事件，
// 让接手人拿到手就能跑起来看行为，而不是从零搭 UI。
//
// 分层（别搞反）：
//   KeyboardLayout  —— 只认得「哪个键被按下」。它携带键码（OpiKey.*），
//                      但**不做任何分流判断**；判断在 Rust 的 KeyRouter。
//                      唯一属于本层的信息是 fallback：当引擎说「这键我不管」
//                      时，客户端该发生什么（这是平台语义，Rust 侧无从知道）。
//   KeyboardViewController —— 调 opi_key_event，按 action 决定 insertText /
//                      刷新 / 走 fallback。

import UIKit

/// 引擎「未处理」该键时，客户端该做什么。
///
/// 为什么必须在 Swift 侧：`opi_key_event` 返回 .passThrough 的意思是
/// 「这个键不该由输入法消费」，但**该发生什么只有平台知道** ——
/// 在 macOS 上回车交给 TextView，在 iOS 软键盘上回车得自己 insertText("\n")。
/// 这一处分流不是把 Rust 的路由表抄第四份：Rust 判「管不管」，这里判「不管时怎么办」。
enum KeyFallback {
    /// 什么都不做（例：方向键、Tab、Esc —— 软键盘上本就不可达）。
    case none
    /// 直接插入字面文本（例：数字/符号键、Number 模式下的字母）。
    case text(String)
    /// 交给 textDocumentProxy 删一个字符（空缓冲的退格）。
    case deleteBackward
}

/// 非输入键（不产生 opi_key_event，改的是本层/控制器状态）。
enum KeyboardFunctionKey {
    case toggleLayer      // 字母 ⇄ 数字/符号
    case toggleLanguage   // 中/英（引擎的 Pinyin ⇄ English）
    case nextInputMode    // 地球键（切到下一个输入法）
}

/// 键盘布局上的一个键。
struct KeySpec {
    /// 显示文字。
    let label: String
    /// 传给 opi_key_event 的 keyval（见 OpiKey）。功能键不用，留 0。
    let keyval: UInt32
    /// 按下时附带的修饰位（⇧/⌘ 等）。
    let states: UInt32
    /// 引擎 passThrough 时的兜底动作。
    let fallback: KeyFallback
    /// 功能键标识；nil = 普通输入键。
    ///
    /// 显式字段而不是「按 label 认键」：label 一旦本地化（英文界面 "space"）
    /// 按字符串匹配就全断，而且断得静默。加这个字段只多一行。
    let function: KeyboardFunctionKey?
    /// 相对宽度（1 = 普通字母键）。
    let widthUnits: CGFloat

    init(
        _ label: String,
        keyval: UInt32 = 0,
        states: UInt32 = 0,
        fallback: KeyFallback = .none,
        function: KeyboardFunctionKey? = nil,
        widthUnits: CGFloat = 1
    ) {
        self.label = label
        self.keyval = keyval
        self.states = states
        self.fallback = fallback
        self.function = function
        self.widthUnits = widthUnits
    }

    /// 可打印字符键的便捷构造：keyval = 码点，兜底 = 插入该字符。
    static func char(_ c: Character) -> KeySpec {
        let s = String(c)
        // 键码取可打印 Unicode 码点（keys.rs 的编码约定）。
        // ⚠️ 未编译：这里只取**第一个**标量。对分解形（"é" = e + U+0301）
        // 会取到基字符 'e' —— 本骨架的键位全是 ASCII，不受影响；将来加
        // 带音标/emoji 的键位时必须显式处理多标量字符。
        let kv = c.unicodeScalars.first.map { OpiKey.printable($0) } ?? 0
        return KeySpec(s, keyval: kv, fallback: .text(s))
    }
}

/// 按键按钮。只带数据与触摸事件，不含业务。
final class KeyButton: UIButton {
    let spec: KeySpec
    /// ⇧ 专用：长按（Lock）是否已触发 —— 避免「长按后又补一次 tap」。
    var longPressFired = false

    init(spec: KeySpec) {
        self.spec = spec
        super.init(frame: .zero)
        setTitle(spec.label, for: .normal)
        // ⚠️ 未编译：系统配色 API 按记忆写的，Mac 上确认语义色名与深色模式表现。
        setTitleColor(.label, for: .normal)
        backgroundColor = .secondarySystemBackground
        titleLabel?.font = .systemFont(ofSize: 20)
        layer.cornerRadius = 5
        // ⚠️ 无障碍：正式版要给每个键设 accessibilityLabel（尤其 ⇧/⌫/回车这类
        // 符号键），否则 VoiceOver 读不出来。本骨架未做 —— 这是**已知缺口**。
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }
}

/// 布局回调。
protocol KeyboardLayoutDelegate: AnyObject {
    /// 按下。longPress = true 表示这是一次长按（目前只有 ⇧ 会产生）。
    func keyboardLayout(_ layout: KeyboardLayout, didPress spec: KeySpec, longPress: Bool)
    /// 抬起（touchUpInside/Outside/Cancel）。
    func keyboardLayout(_ layout: KeyboardLayout, didRelease spec: KeySpec)
    /// 候选词被点选。index 是**页内**序号（0 起，当前页第几个）——
    /// 调用方直接用 `OpiEngine.selectPage(_:)`，**不要**再换算成全局索引。
    func keyboardLayout(_ layout: KeyboardLayout, didSelectCandidateAt index: Int)
    /// 翻页（-1 上一页 / +1 下一页）。走 opi_key_event 的 PAGE_UP/PAGE_DOWN 编码。
    func keyboardLayout(_ layout: KeyboardLayout, didRequestPage delta: Int)
    /// 功能键按下。
    func keyboardLayout(_ layout: KeyboardLayout, didTapFunction name: KeyboardFunctionKey)
}

/// 键盘布局（候选栏 + 键位行）。全代码构建，不用 xib/storyboard ——
/// 键盘扩展加 xib 只会多一个「只能在 Mac 上调」的故障面，而本机连编译都做不到。
final class KeyboardLayout: UIView {

    weak var delegate: KeyboardLayoutDelegate?

    /// 当前是否字母层。
    private(set) var isLetterLayer = true

    // 这里**故意没有** ⇧ 高亮状态。原因：⇧ 的真源是 Rust 的
    // `KeyRouter::ShiftState`（Off/Single/Lock 三态），本地镜像一个「亮/不亮」
    // 必然与引擎漂移（Single 与 Lock 外观不同、且引擎会在提交后自动复位）。
    // 宁可不做，也不做一份会撒谎的高亮。
    //
    // 出口**已落地**：`opi_shift_state() -> int32_t`（0=OFF 1=SINGLE 2=LOCK，
    // 未装载 → 0）。现在**故意没接** —— `macos/OpiFFI.h` 补上声明之前，
    // `ios/OpiFFI.h`（转发头）里没有它，调用即编译不过。
    // 接法：先过编译，再让 UI 读这个出口，别自己镜像状态。
    // 详见 ios/README.md 的「缺的出口：已落地」一节。

    private let candidateScroll = UIScrollView()
    private let candidateStack = UIStackView()
    private let rowsStack = UIStackView()

    init() {
        super.init(frame: .zero)
        // ⚠️ 未编译：键盘扩展的**背景**推荐用 UIInputView(style: .keyboard) 承载，
        // 它能拿到系统的毛玻璃/键盘外观，且随系统主题变化。本骨架用普通 UIView
        // + systemBackground，Mac 上做外观时应换基类。
        backgroundColor = .systemBackground
        buildCandidateBar()
        buildRows()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    private func buildCandidateBar() {
        candidateScroll.showsHorizontalScrollIndicator = false
        candidateScroll.translatesAutoresizingMaskIntoConstraints = false
        candidateStack.axis = .horizontal
        candidateStack.spacing = 8
        candidateStack.translatesAutoresizingMaskIntoConstraints = false

        // 翻页键走 opi_key_event 的 PAGE_UP/PAGE_DOWN（页码钳制在 router.rs 里）。
        let prev = makeBarButton("‹") { [weak self] in
            guard let self else { return }
            self.delegate?.keyboardLayout(self, didRequestPage: -1)
        }
        let next = makeBarButton("›") { [weak self] in
            guard let self else { return }
            self.delegate?.keyboardLayout(self, didRequestPage: 1)
        }

        addSubview(prev)
        addSubview(candidateScroll)
        addSubview(next)
        candidateScroll.addSubview(candidateStack)

        // ⚠️ 未编译：以下约束按常见写法写，Mac 上大概率要调数值/优先级。
        NSLayoutConstraint.activate([
            prev.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 4),
            prev.centerYAnchor.constraint(equalTo: candidateScroll.centerYAnchor),
            prev.widthAnchor.constraint(equalToConstant: 28),

            next.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -4),
            next.centerYAnchor.constraint(equalTo: candidateScroll.centerYAnchor),
            next.widthAnchor.constraint(equalToConstant: 28),

            candidateScroll.leadingAnchor.constraint(equalTo: prev.trailingAnchor, constant: 4),
            candidateScroll.trailingAnchor.constraint(equalTo: next.leadingAnchor, constant: -4),
            candidateScroll.topAnchor.constraint(equalTo: topAnchor),
            candidateScroll.heightAnchor.constraint(equalToConstant: 40),

            candidateStack.leadingAnchor.constraint(equalTo: candidateScroll.contentLayoutGuide.leadingAnchor),
            candidateStack.trailingAnchor.constraint(equalTo: candidateScroll.contentLayoutGuide.trailingAnchor),
            candidateStack.topAnchor.constraint(equalTo: candidateScroll.contentLayoutGuide.topAnchor),
            candidateStack.bottomAnchor.constraint(equalTo: candidateScroll.contentLayoutGuide.bottomAnchor),
            candidateStack.heightAnchor.constraint(equalTo: candidateScroll.frameLayoutGuide.heightAnchor),
        ])
    }

    private func buildRows() {
        rowsStack.axis = .vertical
        rowsStack.spacing = 6
        rowsStack.distribution = .fillEqually
        rowsStack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(rowsStack)
        NSLayoutConstraint.activate([
            rowsStack.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 3),
            rowsStack.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -3),
            rowsStack.topAnchor.constraint(equalTo: candidateScroll.bottomAnchor, constant: 4),
            rowsStack.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -4),
        ])
        rebuildKeys()
    }

    /// 重建键位行（切层时调用）。
    private func rebuildKeys() {
        rowsStack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        let rows = isLetterLayer ? Self.letterRows() : Self.numberRows()
        let gap: CGFloat = 5
        for specs in rows {
            let row = UIStackView()
            row.axis = .horizontal
            row.spacing = gap
            // .fill（不是 .fillProportionally）：宽度由下面的显式比例约束决定。
            // fillProportionally 只看 intrinsicContentSize，会无视 widthUnits。
            row.distribution = .fill
            let total = specs.reduce(0) { $0 + $1.widthUnits }
            // 键间空隙要从总宽里扣掉，否则「各键比例之和 = 1」再加上空隙会超出行宽，
            // 结果是约束冲突（布局崩 + 控制台刷警告）。
            let gaps = gap * CGFloat(max(0, specs.count - 1))
            for spec in specs {
                let b = KeyButton(spec: spec)
                attach(b)
                row.addArrangedSubview(b)
                // b.width = row.width × 比例 − 本键分摊的空隙
                // 各键求和 = row.width − gaps，再加 gaps 恰好等于行宽。
                let frac = spec.widthUnits / total
                b.widthAnchor.constraint(
                    equalTo: row.widthAnchor, multiplier: frac, constant: -gaps * frac
                ).isActive = true
            }
            rowsStack.addArrangedSubview(row)
        }
    }

    /// 字母层三行 + 功能行。
    ///
    /// 字母键一律送**小写码点 + states 0**，不自己做大写化：keys.rs 的约定是
    /// 「大小写由码点携带」是给**物理键盘**的（平台已应用 ⇧）；而软键盘的 ⇧ 走
    /// `handle_shift` 维护的引擎内状态机，由它决定英文模式的 Input 与拼音
    /// composer 的大小写。这里若再自己转一次大写，就是第五份大小写逻辑。
    private static func letterRows() -> [[KeySpec]] {
        let top = "qwertyuiop".map { KeySpec.char($0) }
        let mid = "asdfghjkl".map { KeySpec.char($0) }
        var bottom: [KeySpec] = [
            // ⇧：单击/长按的**语义**（Off→Single→Lock）全在 Rust 的 handle_shift，
            // 这里只把「长按」翻成 stateLongPressed 位。states 不送 KEY_STATE_SHIFT：
            // handle_shift 不读它。
            KeySpec("⇧", keyval: OpiKey.shift, widthUnits: 1.4)
        ]
        bottom += "zxcvbnm".map { KeySpec.char($0) }
        bottom.append(
            KeySpec("⌫", keyval: OpiKey.backspace, fallback: .deleteBackward, widthUnits: 1.4)
        )

        let funcRow: [KeySpec] = [
            KeySpec("123", function: .toggleLayer),
            KeySpec("中/英", function: .toggleLanguage),
            KeySpec("空格", keyval: OpiKey.space, fallback: .text(" "), widthUnits: 5),
            KeySpec("回车", keyval: OpiKey.ret, fallback: .text("\n"), widthUnits: 1.6),
            // 地球键：Apple 要求自定义键盘自己提供切输入法入口（系统不会替你画）。
            // ⚠️ 未验证：`needsInputModeSwitchKey` 为 false 时是否该隐藏它，
            // 我按「始终显示」写（点了就是 advanceToNextInputMode，无副作用）；
            // Mac 上照 HIG 调整。
            KeySpec("🌐", function: .nextInputMode, widthUnits: 1),
        ]
        return [top, mid, bottom, funcRow]
    }

    /// 数字/符号层（**最小集合**：正式版应加符号翻页，参照 Android 的 SymbolPanel）。
    private static func numberRows() -> [[KeySpec]] {
        let r1 = "1234567890".map { KeySpec.char($0) }
        let r2 = ["-", "/", ":", ";", "(", ")", "$", "&", "@", "\""].map { KeySpec.char(Character($0)) }
        let r3 = [
            KeySpec("ABC", function: .toggleLayer, widthUnits: 1.4),
            KeySpec.char("."),
            KeySpec.char(","),
            KeySpec.char("?"),
            KeySpec.char("!"),
            KeySpec.char("'"),
            KeySpec("⌫", keyval: OpiKey.backspace, fallback: .deleteBackward, widthUnits: 1.4),
        ]
        let funcRow: [KeySpec] = [
            KeySpec("#+=", function: .toggleLayer),
            KeySpec("中/英", function: .toggleLanguage),
            KeySpec("空格", keyval: OpiKey.space, fallback: .text(" "), widthUnits: 5),
            KeySpec("回车", keyval: OpiKey.ret, fallback: .text("\n"), widthUnits: 1.6),
        ]
        return [r1, r2, r3, funcRow]
    }

    // MARK: - 触摸接线

    private func attach(_ b: KeyButton) {
        b.addTarget(self, action: #selector(onTouchDown(_:)), for: .touchDown)
        b.addTarget(
            self, action: #selector(onTouchUp(_:)),
            for: [.touchUpInside, .touchUpOutside, .touchCancel]
        )
        if b.spec.keyval == OpiKey.shift {
            // ⇧ 有第三种语义（长按 = Lock），所以它的 touchDown **不能立刻上报** ——
            // 上报了就先发生一次 tap，长按再补一次 lock，等于连按两下。
            // 处理：按下时只起表，抬手时若长按没触发才补一次 tap。
            let lp = UILongPressGestureRecognizer(target: self, action: #selector(onLongPress(_:)))
            lp.minimumPressDuration = 0.5   // ⚠️ 0.5s 是拍的，真机上按手感调
            b.addGestureRecognizer(lp)
        }
    }

    @objc private func onTouchDown(_ sender: KeyButton) {
        if let fn = sender.spec.function {
            delegate?.keyboardLayout(self, didTapFunction: fn)
            return
        }
        if sender.spec.keyval == OpiKey.shift {
            sender.longPressFired = false   // 只起表，不上报
            return
        }
        delegate?.keyboardLayout(self, didPress: sender.spec, longPress: false)
    }

    @objc private func onLongPress(_ g: UILongPressGestureRecognizer) {
        guard g.state == .began, let b = g.view as? KeyButton else { return }
        b.longPressFired = true
        delegate?.keyboardLayout(self, didPress: b.spec, longPress: true)
    }

    @objc private func onTouchUp(_ sender: KeyButton) {
        // 功能键按下时已处理完，没有引擎事件可发。
        if sender.spec.function != nil { return }
        if sender.spec.keyval == OpiKey.shift {
            if !sender.longPressFired {
                // 没长按 → 这是一次普通点击：补发按下，抬起照常发。
                delegate?.keyboardLayout(self, didPress: sender.spec, longPress: false)
            }
            sender.longPressFired = false
        }
        delegate?.keyboardLayout(self, didRelease: sender.spec)
    }

    private func makeBarButton(_ title: String, onTap: @escaping () -> Void) -> UIButton {
        let b = BarButton(title: title, onTap: onTap)
        b.translatesAutoresizingMaskIntoConstraints = false
        return b
    }

    // MARK: - 外部刷新

    /// 刷新候选栏。texts 是**当前页**的候选（来自 `opi_candidates_page()`）。
    ///
    /// 本方法**不需要页码**：回调传出去的是**页内**序号，页内换算由引擎做
    /// （`opi_select_page`）。之前那句 `page * pageSize + i` 已删除 ——
    /// 那是 `PAGE_SIZE` 在 UI 侧的第三份拷贝。
    func updateCandidates(_ texts: [String]) {
        candidateStack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        guard !texts.isEmpty else { return }
        for (i, t) in texts.enumerated() {
            candidateStack.addArrangedSubview(
                BarButton(title: t) { [weak self] in
                    guard let self else { return }
                    self.delegate?.keyboardLayout(self, didSelectCandidateAt: i)
                }
            )
        }
    }

    /// 切换字母 / 数字层。
    func setLetterLayer(_ letter: Bool) {
        guard letter != isLetterLayer else { return }
        isLetterLayer = letter
        rebuildKeys()
    }
}

/// 候选栏/翻页栏上的小按钮。
private final class BarButton: UIButton {
    private let onTap: () -> Void
    init(title: String, onTap: @escaping () -> Void) {
        self.onTap = onTap
        super.init(frame: .zero)
        setTitle(title, for: .normal)
        setTitleColor(.label, for: .normal)
        titleLabel?.font = .systemFont(ofSize: 18)
        // ⚠️ 未编译：contentEdgeInsets 在 iOS 15 起被 UIButton.Configuration 取代
        // （废弃但未移除）。Mac 上做外观时换成 Configuration。
        contentEdgeInsets = UIEdgeInsets(top: 4, left: 8, bottom: 4, right: 8)
        addTarget(self, action: #selector(tapped), for: .touchUpInside)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used") }

    @objc private func tapped() { onTap() }
}
