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

    /// 键盘层。三层而不是「字母/数字」两层：符号层的数据来自引擎的符号库出口，
    /// 与另外两层的键位表不是同一类东西（符号键**不过引擎**，见 `commitText`）。
    enum Layer {
        case letters
        case numbers
        case symbols
    }

    weak var delegate: KeyboardLayoutDelegate?

    /// 当前层。**不要**在控制器里另存一份 —— 键面与层的真源在这里。
    private(set) var layer: Layer = .letters

    /// 模式键的键面字（中/繁/英）。由控制器在切模式后同步（对齐 Android `modeLabelOf`）。
    ///
    /// 为什么键面要跟着模式变：这个键同时是「当前是什么模式」的显示器 ——
    /// 写死「中/英」时切到繁体后键面仍在说「中/英」，用户按下去看到的是变繁，
    /// 等于键面在撒谎。（Android 那边踩过同一条：叠盘里曾硬编码「中」。）
    private var modeLabel = "中"

    // 这里**故意没有** ⇧ 高亮状态。原因：⇧ 的真源是 Rust 的
    // `KeyRouter::ShiftState`（Off/Single/Lock 三态），本地镜像一个「亮/不亮」
    // 必然与引擎漂移（Single 与 Lock 外观不同、且引擎会在提交后自动复位）。
    // 宁可不做，也不做一份会撒谎的高亮。
    //
    // 两步里的第 ① 步**已写**：`ios/OpiEngine.swift` 有 `shiftState() -> OpiShiftState`
    // 包着 `opi_shift_state()`（0=OFF 1=SINGLE 2=LOCK，未装载 → 0，取值含非静默兜底）。
    // 第 ② 步（让本文件读它画高亮）**没做** —— 所以现在仍然没有高亮，也就仍然
    // 没有会漂移的镜像。真要做高亮时从 `shiftState()` 读，**别**拿
    // `KeyButton.longPressFired` 推状态：那只是防「长按后又补一次 tap」的去抖标志。
    // 详见 ios/README.md 的「缺的出口：已落地」一节。

    private let candidateScroll = UIScrollView()
    private let candidateStack = UIStackView()
    private let rowsStack = UIStackView()
    /// 符号层**自带**一个可竖向滚动的容器（符号数不是常量：当前数据表里
    /// 「常用」= CJK 符号块 60 个，将来加 common 块只会更多）。
    /// 与 `rowsStack` 同位置、互斥显示 —— 不去改 `rowsStack` 的 `.fillEqually`
    /// 布局，那会连字母层的行高一起改掉。
    /// 网格的画法搬到 `SymbolPanel.swift`（键位行是编译期常量、符号是运行时数据，
    /// 两件东西；顺带把本文件压回 500 行源码上限内）。本层只管摆位置与转发点击。
    private let symbolPanel = SymbolPanel()

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
        buildSymbolPanel()
        rebuildKeys()
    }

    /// 符号层容器：竖向滚动 + 每行等宽按钮。与 `rowsStack` 同位置（互斥显示）。
    ///
    /// 画法在 `SymbolPanel`（那边用固定行高 —— 滚动视图的内容高度必须由子视图
    /// 撑出来，而 `.fillEqually` 是「按容器高度平分」，在滚动视图里会退化成 0 高）。
    /// 本层只摆位置 + 把面板的点击转发成 `.commitText`（直提，不过引擎）。
    private func buildSymbolPanel() {
        symbolPanel.translatesAutoresizingMaskIntoConstraints = false
        symbolPanel.onPick = { [weak self] text in
            guard let self else { return }
            self.delegate?.keyboardLayout(self, didTapFunction: .commitText(text))
        }
        addSubview(symbolPanel)
        NSLayoutConstraint.activate([
            symbolPanel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 3),
            symbolPanel.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -3),
            symbolPanel.topAnchor.constraint(equalTo: candidateScroll.bottomAnchor, constant: 4),
            symbolPanel.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -4),
        ])
        symbolPanel.isHidden = true
    }

    /// 重建键位行（切层时调用）。
    private func rebuildKeys() {
        // 符号层由 `setSymbols(_:)` 画（数据来自引擎，不在这里现算）。
        let isSymbols = (layer == .symbols)
        rowsStack.isHidden = isSymbols
        symbolPanel.isHidden = !isSymbols
        guard !isSymbols else { return }
        rowsStack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        let rows = layer == .letters ? letterRows() : numberRows()
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
    private func letterRows() -> [[KeySpec]] {
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
            KeySpec("123", function: .showNumbers),
            // 键面字跟着模式走（中/繁/英），不是写死的 "中/英"。
            KeySpec(modeLabel, function: .cycleMode),
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

    /// 数字/标点层（**最小集合**）。
    ///
    /// 这里没有「符号」键位表：符号来自引擎的符号库（`#+=` 切到 `.symbols` 层），
    /// 不再在 Swift 里另抄一份符号表 —— Android 同样只把符号交给引擎/面板。
    private func numberRows() -> [[KeySpec]] {
        let r1 = "1234567890".map { KeySpec.char($0) }
        let r2 = ["-", "/", ":", ";", "(", ")", "$", "&", "@", "\""].map { KeySpec.char(Character($0)) }
        let r3 = [
            KeySpec("ABC", function: .showLetters, widthUnits: 1.4),
            KeySpec.char("."),
            KeySpec.char(","),
            KeySpec.char("?"),
            KeySpec.char("!"),
            KeySpec.char("'"),
            KeySpec("⌫", keyval: OpiKey.backspace, fallback: .deleteBackward, widthUnits: 1.4),
        ]
        let funcRow: [KeySpec] = [
            KeySpec("#+=", function: .showSymbols),
            KeySpec(modeLabel, function: .cycleMode),
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

    /// 切换层。符号层的数据**不在这里取** —— 见 `setSymbols(_:)`。
    func setLayer(_ new: Layer) {
        guard new != layer else { return }
        layer = new
        rebuildKeys()
    }

    /// 交给符号层的符号（来自 `OpiEngine.commonSymbols()`，由控制器在切到符号层时取）。
    /// 只做转发 —— 网格怎么画、空列表怎么提示，都在 `SymbolPanel`（那边有理由）。
    func setSymbols(_ list: [String]) {
        symbolPanel.setSymbols(list)
    }

    /// 同步模式键的键面字（中/繁/英）。控制器在切模式后调 —— 键面是模式的显示器。
    func setModeLabel(_ label: String) {
        guard label != modeLabel else { return }
        modeLabel = label
        // 键面字只在重建行时才读，所以这里必须重建（代价是 4 行按钮重建，
        // 切模式是低频操作）。符号层没有这个键，不用重建。
        if layer != .symbols { rebuildKeys() }
    }
}

/// 候选栏/翻页栏/符号面板上的小按钮。
/// 不是 `private`：`SymbolPanel.swift` 也用它（同一文件里才 private 的那种小类型，
/// 拆成两个文件后必须放开可见性）。
final class BarButton: UIButton {
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
