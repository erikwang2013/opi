// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · **CI 已真编译**（apple.yml 的 iOS job，2026-10-15）· **从未上机运行**
//
// 本文件在 Linux 上编写。**已过 `swiftc ios/*.swift`（-typecheck + -emit-library）** ——
// 2026-10-15 那次全目录真编译；但从未在设备或模拟器上跑过：**行为仍未验证**。
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
    /// 表情页搜索框的内容变了（**原始文本、未去空白**）。防抖与去空白在控制器里
    /// （对齐 Android：`searchText` 实时上报、`searchQuery` 防抖 250ms 后生效）。
    func keyboardLayout(_ layout: KeyboardLayout, didSearchEmoji query: String)
}

/// 键盘布局（候选栏 + 键位行）。全代码构建，不用 xib/storyboard ——
/// 键盘扩展加 xib 只会多一个「只能在 Mac 上调」的故障面，而本机连编译都做不到。
final class KeyboardLayout: UIView {

    /// 键盘层。六层而不是「字母/数字」两层：符号层与表情层的数据都来自引擎的查询出口，
    /// 与另外两层的键位表不是同一类东西（点它们**不过引擎**，见 `commitText`）。
    /// 表情层（2026-10-13）与符号层是**两条独立数据通路**（符号表 / UTS#51 表情表），
    /// 面板也是两个类 —— 别把表情塞进 `symbolPanel` 里当一栏画。
    /// 模式菜单与生僻字面板（2026-10-14）沿用同一条：四个面板类、四个兄弟视图、
    /// 同位置互斥显示，靠 `isPanelLayer` 一处判定「该藏键位行了」。
    enum Layer {
        case letters
        case numbers
        case symbols
        case emoji
        /// 输入模式菜单（2026-10-14）：面板占满键面，选项来自 `InputMode.allCases`。
        /// **它不改任何引擎状态** —— 选中后才由控制器走 `applyInputMode`。
        case modes
        /// 生僻字面板（2026-10-14）：部首 → 笔画 → 字的浏览式查法，**不进引擎**。
        case rare
    }

    weak var delegate: KeyboardLayoutDelegate?

    /// 当前层。**不要**在控制器里另存一份 —— 键面与层的真源在这里。
    ///
    /// 名字为什么不叫 `layer`：`UIView` 已有一个 `layer: CALayer`，同名的属性
    /// 无法用别的类型覆盖 —— 首次真编译的报错就是
    /// `property 'layer' with type 'KeyboardLayout.Layer' cannot override a property with type 'CALayer'`
    /// （2026-10-09 CI run 37933196475）。改名是唯一符合 API 的修法。
    private(set) var keyLayer: Layer = .letters

    /// 当前**输入模式**（键面画哪一套 + 菜单高亮哪一项）。
    ///
    /// 为什么不让它从引擎反推：`scheme()` + `mode()` 反推不出「大 9 键」与
    /// 「生僻字」这两种态（它们不进引擎），这正是 Android `layoutFor()` 头注
    /// 写死的那件事。所以本层自己持有用户选过的模式，控制器在 `applyInputMode`
    /// 里同步 —— 与 `modeLabel` 同一条纪律：**键面的真源在 UI 侧，引擎的真源在引擎侧**。
    private(set) var inputMode: InputMode = .pinyin26

    /// 模式键的键面字（中/繁/英）= **点它会切到哪**（2026-10-12 晚裁决；与 Android
    /// `modeKeyLabelOf` 同语义，各端按自己的循环给值）。由控制器在切模式后同步。
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
    /// 与 `rowsStack` 同位置、互斥显示 —— 它的行高规则（固定行高 + 滚动）与键位行
    /// （按权重分行）本来就不是一套，混在一起只会互相牵制。网格画法在 `SymbolPanel.swift`。
    private let symbolPanel = SymbolPanel()
    /// 表情层（2026-10-13）。与 `symbolPanel` **同位置、三者互斥显示**（字母行/符号/表情），
    /// 数据与画法都在 `EmojiPanel.swift`（那边有理由：分类条/搜索/字形过滤是表情页自己的事），
    /// 本层只管摆位置 + 转发点击与搜索词。
    private let emojiPanel = EmojiPanel()
    /// 输入模式菜单（2026-10-14）。与上面两个面板**同位置、互斥显示**（四个里只亮一个），
    /// 摆法与转发理由同上；画法与选项在 `InputModePanel.swift`。
    private let modePanel = InputModePanel()
    /// 生僻字面板（2026-10-14）。同上；数据与三级状态机在 `RarePanel.swift`。
    /// ⚠️ 索引文件（`rare_index.tsv`）本仓 iOS 侧**没有** —— 面板会显示可见提示，
    /// 见那里的头注。
    private let rarePanel = RarePanel()

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
        // .fill（2026-10-14 由 .fillEqually 改）：行高现在由 `ModeKeyRenderer` 按
        // **行高权重**给显式约束（9 键主区是底行的 3 倍高）。原来 .fillEqually
        // 「按容器高平分」表达不了权重。26 键/14 键/笔画的行权恒 1 ⇒ 结果与改前一致。
        rowsStack.distribution = .fill
        rowsStack.translatesAutoresizingMaskIntoConstraints = false
        addSubview(rowsStack)
        NSLayoutConstraint.activate([
            rowsStack.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 3),
            rowsStack.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -3),
            rowsStack.topAnchor.constraint(equalTo: candidateScroll.bottomAnchor, constant: 4),
            rowsStack.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -4),
        ])
        buildSymbolPanel()
        buildEmojiPanel()
        buildModePanel()
        buildRarePanel()
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

    /// 表情层容器（2026-10-13）。摆法与 `buildSymbolPanel` 逐条相同（同位置互斥），
    /// 只是回调多两个：返回（回字母层，`onBack`）与搜索词（`onSearch`）。
    ///
    /// 三个回调的语义：
    ///   `onPick`   → `.commitText`（直提，不过引擎 —— 同符号键，理由见 `commitText`）；
    ///   `onBack`   → `.showLetters`（**复用既有功能键**，不为「退出表情页」新造机制）；
    ///   `onSearch` → 新委托方法（去抖与查询都归控制器：那里才有 engine 与 Timer）。
    private func buildEmojiPanel() {
        emojiPanel.translatesAutoresizingMaskIntoConstraints = false
        emojiPanel.onPick = { [weak self] text in
            guard let self else { return }
            self.delegate?.keyboardLayout(self, didTapFunction: .commitText(text))
        }
        emojiPanel.onBack = { [weak self] in
            guard let self else { return }
            self.delegate?.keyboardLayout(self, didTapFunction: .showLetters)
        }
        emojiPanel.onSearch = { [weak self] query in
            guard let self else { return }
            self.delegate?.keyboardLayout(self, didSearchEmoji: query)
        }
        addSubview(emojiPanel)
        NSLayoutConstraint.activate([
            emojiPanel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 3),
            emojiPanel.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -3),
            emojiPanel.topAnchor.constraint(equalTo: candidateScroll.bottomAnchor, constant: 4),
            emojiPanel.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -4),
        ])
        emojiPanel.isHidden = true
    }

    /// 输入模式菜单容器（2026-10-14）。摆法同符号面板（同位置互斥）。
    /// ⚠️ 选中**不在这里**换引擎/换键面：那是控制器的事（`applyInputMode`）——
    /// 面板只管报「用户点了哪一项」（`.selectInputMode` 带载荷的功能键，不新造委托方法）。
    private func buildModePanel() {
        modePanel.translatesAutoresizingMaskIntoConstraints = false
        modePanel.onSelect = { [weak self] mode in
            guard let self else { return }
            self.delegate?.keyboardLayout(self, didTapFunction: .selectInputMode(mode))
        }
        modePanel.onClose = { [weak self] in
            guard let self else { return }
            self.delegate?.keyboardLayout(self, didTapFunction: .showLetters)
        }
        addSubview(modePanel)
        NSLayoutConstraint.activate([
            modePanel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 3),
            modePanel.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -3),
            modePanel.topAnchor.constraint(equalTo: candidateScroll.bottomAnchor, constant: 4),
            modePanel.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -4),
        ])
        modePanel.isHidden = true
    }

    /// 生僻字面板容器（2026-10-14）。摆法同上（同位置互斥）。
    /// 提交是**直提**（`.commitText`，同符号/表情键的理由）；关闭回字母层。
    private func buildRarePanel() {
        rarePanel.translatesAutoresizingMaskIntoConstraints = false
        rarePanel.onCommit = { [weak self] ch in
            guard let self else { return }
            self.delegate?.keyboardLayout(self, didTapFunction: .commitText(ch))
        }
        rarePanel.onClose = { [weak self] in
            guard let self else { return }
            self.delegate?.keyboardLayout(self, didTapFunction: .showLetters)
        }
        addSubview(rarePanel)
        NSLayoutConstraint.activate([
            rarePanel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 3),
            rarePanel.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -3),
            rarePanel.topAnchor.constraint(equalTo: candidateScroll.bottomAnchor, constant: 4),
            rarePanel.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -4),
        ])
        rarePanel.isHidden = true
    }

    /// 重建键位行（切层/换模式时调用）。
    ///
    /// 四个面板（符号/表情/模式菜单/生僻字）**互斥显示**、谁亮由 `keyLayer` 决定；
    /// 行/格/列的**尺寸算式**搬去了 `ModeKeyRenderer`，本函数只挑「画哪套表」并接线。
    private func rebuildKeys() {
        // 符号层/表情层的内容由控制器现取（`setSymbols` / `setEmojiBrowse`），
        // 模式菜单与生僻字面板是**静态结构**，自己管自己的内容。
        let isSymbols = (keyLayer == .symbols)
        let isEmoji = (keyLayer == .emoji)
        let isModes = (keyLayer == .modes)
        let isRare = (keyLayer == .rare)
        rowsStack.isHidden = isPanelLayer
        symbolPanel.isHidden = !isSymbols
        emojiPanel.isHidden = !isEmoji
        modePanel.isHidden = !isModes
        rarePanel.isHidden = !isRare
        if isModes { modePanel.show(current: inputMode) }
        if isRare { rarePanel.show() }
        guard !isPanelLayer else { return }
        rowsStack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        // 字母层按**当前输入模式**画（9 键/14 键/笔画各有自己的表）；数字层所有模式共用。
        let rows = keyLayer == .letters
            ? ModeKeyTables.rows(for: inputMode, modeLabel: modeLabel)
            : ModeKeyTables.numberRows(modeLabel: modeLabel)
        ModeKeyRenderer.render(rows, into: rowsStack, gap: 5) { [weak self] b in
            self?.attach(b)
        }
    }

    /// 换输入模式（键面按它重画；控制器在 `applyInputMode` 里同步）。
    /// 与 `setLayer` 一样只在**变化时**重建 —— 9 键的键面重建要造几十个按钮。
    func setInputMode(_ mode: InputMode) {
        guard mode != inputMode else { return }
        inputMode = mode
        // 菜单层与生僻字层没有键位行可重建（rebuildKeys 会早退），代价可忽略。
        rebuildKeys()
    }

    // MARK: - 触摸接线

    private func attach(_ b: KeyButton) {
        b.addTarget(self, action: #selector(onTouchDown(_:)), for: .touchDown)
        b.addTarget(
            self, action: #selector(onTouchUp(_:)),
            for: [.touchUpInside, .touchUpOutside, .touchCancel]
        )
        // 两种键要长按识别：⇧（长按 = Lock）与带 `longPressFunction` 的键（模式键）。
        if b.spec.keyval == OpiKey.shift || b.spec.longPressFunction != nil {
            // 它们的 touchDown **不能立刻上报** —— 否则先干一次短按的事、长按再补一次，
            // 等于连按两下（⇧ 是「先 tap 再 lock」，模式键是「先切模式再开菜单」）。
            // 处理：按下只起表，抬手时若长按没触发才补一次短按。
            let lp = UILongPressGestureRecognizer(target: self, action: #selector(onLongPress(_:)))
            lp.minimumPressDuration = 0.5   // ⚠️ 0.5s 是拍的，真机上按手感调
            b.addGestureRecognizer(lp)
        }
    }

    @objc private func onTouchDown(_ sender: KeyButton) {
        if let fn = sender.spec.function {
            // 带长按替代语义的功能键（模式键）：只起表，等抬手/长按定论。
            if sender.spec.longPressFunction != nil {
                sender.longPressFired = false
                return
            }
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
        // 长按的**去向**由键位表给（`KeySpec.longPressFunction`），不在这里按
        // 「这是模式键所以开菜单」硬判 —— 换表即换语义（见 KeySpec 的字段注释）。
        if let fn = b.spec.longPressFunction {
            delegate?.keyboardLayout(self, didTapFunction: fn)
            return
        }
        delegate?.keyboardLayout(self, didPress: b.spec, longPress: true)
    }

    @objc private func onTouchUp(_ sender: KeyButton) {
        if let fn = sender.spec.function {
            // 带长按替代语义的功能键：没长按 → 补发一次普通功能键（长按已上报过）。
            if sender.spec.longPressFunction != nil {
                if !sender.longPressFired {
                    delegate?.keyboardLayout(self, didTapFunction: fn)
                }
                sender.longPressFired = false
            }
            return
        }
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
        guard new != keyLayer else { return }
        keyLayer = new
        rebuildKeys()
    }

    /// 交给符号层的符号（来自 `OpiEngine.commonSymbols()`，由控制器在切到符号层时取）。
    /// 只做转发 —— 网格怎么画、空列表怎么提示，都在 `SymbolPanel`（那边有理由）。
    func setSymbols(_ list: [String]) {
        symbolPanel.setSymbols(list)
    }

    /// 交给表情层的**原始**数据（来自 `OpiEngine.emojiSymbols()` / `emojiGroups()`，
    /// 由控制器在切到表情层时取）。字形过滤与分类下标重算都在 `EmojiPanel` 里做
    /// （那边有理由），本层一个字都不判。
    func setEmojiBrowse(_ list: [String], groups: [OpiEmojiGroup]) {
        emojiPanel.showBrowse(list, groups: groups)
    }

    /// 交给表情层的搜索结果（来自 `OpiEngine.searchSymbols(keyword:)`）。
    /// `query` 传**去空白后**的查询词（控制器 trim）—— 空查询时面板画浏览内容。
    func setEmojiResults(_ results: [String], query: String) {
        emojiPanel.showResults(results, query: query)
    }

    /// 同步模式键的键面字（中/繁/英）。控制器在切模式后调 —— 键面是模式的显示器。
    func setModeLabel(_ label: String) {
        guard label != modeLabel else { return }
        modeLabel = label
        // 键面字只在重建行时才读，所以这里必须重建（代价是几行按钮重建，
        // 切模式是低频操作）。四个面板层都没有这个键（菜单/生僻字里也不画），不用重建。
        if !isPanelLayer { rebuildKeys() }
    }

    /// 当前层是不是「占满键面的面板」（四个面板里的一层）——
    /// 它们都不画键位行，`rowsStack` 该藏起来。
    private var isPanelLayer: Bool {
        keyLayer == .symbols || keyLayer == .emoji || keyLayer == .modes || keyLayer == .rare
    }
}

/// 候选栏/翻页栏/符号面板/表情面板上的小按钮。
/// 不是 `private`：`SymbolPanel.swift` 与 `EmojiPanel.swift` 都用它
/// （同一文件里才 private 的那种小类型，拆成两个文件后必须放开可见性）。
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
