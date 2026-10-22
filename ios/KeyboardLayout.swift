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
    /// 按下。longPress = true 表示这是一次长按（⇧ Lock / ⌫ 清组合，见 KeySpec）。
    func keyboardLayout(_ layout: KeyboardLayout, didPress spec: KeySpec, longPress: Bool)
    /// 抬起（touchUpInside/Outside/Cancel）。
    func keyboardLayout(_ layout: KeyboardLayout, didRelease spec: KeySpec)
    /// 候选词被点选。index 是**全局**下标（`opi_candidates(limit)` 的次序），
    /// 调用方直接用 `OpiEngine.select(index:)`；本轮候选栏不再分页。
    func keyboardLayout(_ layout: KeyboardLayout, didSelectCandidateAt index: Int)
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
        /// 皮肤面板（2026-10-20）：12 款色板，选中即换。
        case skins
        /// 键盘模式面板（2026-10-20）：繁体 / 单手 / 暗光 / 长辈（悬浮、游戏禁用）。
        case keyboardModes
        /// 关于面板（2026-10-20）。
        case about
        /// 设置面板（2026-10-21）：Android 设置页的可用子集，齿轮入口。
        case settings
        /// 平铺候选（2026-10-21）：候选栏最右 ⌄ 打开，**盖住整个键盘**
        /// （含顶部 44pt 槽位，Android `CandidateTiledView` 同义）。
        /// 键位行藏起来、单手不再收窄。
        case candidatesTiled
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
    private(set) var inputMode: InputMode = .pinyin9

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

    // 候选栏子视图是 **internal**：建栏在 KeyboardCandidateBar.swift 的扩展里
    // （2026-10-20 拆出，500 行门禁），Swift 的 private 只跨同文件扩展可见。
    /// 固定槽位的候选行（2026-10-21：不再横向滚动）。
    let candidateStack = UIStackView()
    /// 英文模式右上角的隐藏「中」键（2026-10-20 用户裁决「低存在感，用于切回中文」）。
    /// 它压在 ⌄ 左侧；非英文模式隐藏。工具条在空闲态另有自己的一枚。
    let chineseButton = UIButton(type: .system)
    /// 候选栏最右 ⌄（2026-10-21）：打开平铺候选。用 `BarButton` 是为了跟候选槽位
    /// 共用 `applyTheme`（换肤/长辈字号时不会漏掉这一枚）。
    lazy var expandButton = BarButton(title: "⌄") { [weak self] in
        guard let self else { return }
        self.delegate?.keyboardLayout(self, didTapFunction: .showTiledCandidates)
    }
    /// 顶部 44pt 槽位（2026-10-20）：候选栏与工具条二选一，**同位置互斥**。
    /// 对齐 Android `ImeScreen` 的 `[CandidateBar | ImeToolbar]` 槽 —— 空闲时是工具条，
    /// 有缓冲/候选时是候选栏。候选栏隐藏时仍占 44pt（键区锚在 `candidateBar.bottom`）。
    let candidateBar = UIView()
    let toolbar = KeyboardToolbar()
    /// 顶部拼音行（2026-10-23）：组合中显示分词拼音；常驻 24pt，空串不铺底。
    let composingBar = ComposingBar()
    /// 键区容器：单手模式只收窄/偏移这一块（工具条与候选栏保持全宽，Android 同）。
    let keyArea = UIView()

    // 四个旧面板 + 三个新面板都是 internal：建栏/建面板在 `KeyboardPanelHost.swift`
    // 的扩展里（500 行门禁），Swift 的 private 只跨同文件扩展可见。
    let rowsStack = UIStackView()
    /// 符号层**自带**一个可竖向滚动的容器（符号数不是常量：当前数据表里
    /// 「常用」= CJK 符号块 60 个，将来加 common 块只会更多）。
    /// 与 `rowsStack` 同位置、互斥显示 —— 它的行高规则（固定行高 + 滚动）与键位行
    /// （按权重分行）本来就不是一套，混在一起只会互相牵制。网格画法在 `SymbolPanel.swift`。
    let symbolPanel = SymbolPanel()
    /// 表情层（2026-10-13）。与 `symbolPanel` **同位置、三者互斥显示**（字母行/符号/表情），
    /// 数据与画法都在 `EmojiPanel.swift` + `EmojiGridView.swift`（那边有理由：工具条、
    /// 分类条、搜索、字形过滤与网格渲染是表情页自己的事），本层只管摆位置 + 转发点击与搜索词。
    let emojiPanel = EmojiPanel()
    /// 输入模式菜单（2026-10-14）。与上面两个面板**同位置、互斥显示**（四个里只亮一个），
    /// 摆法与转发理由同上；画法与选项在 `InputModePanel.swift`。
    let modePanel = InputModePanel()
    /// 生僻字面板（2026-10-14）。同上；数据与三级状态机在 `RarePanel.swift`。
    /// 索引文件（`rare_index.tsv`）由 `scripts/apple/package-ios.sh` 拷进 appex
    /// （2026-10-20 起）；缺失时面板会显示可见提示，见那里的头注。
    let rarePanel = RarePanel()
    let skinPanel = SkinPanel()
    let keyboardModePanel = KeyboardModePanel()
    let aboutPanel = AboutPanel()
    /// 设置面板（2026-10-21，齿轮入口）。引擎开关 + 外观/键盘模式跳转。
    let settingsPanel = SettingsPanel()
    /// 平铺候选（2026-10-21）。与面板同位置；内容由控制器每次 refresh 灌入。
    let candidateTiledPanel = CandidateTiledPanel()
    /// 9 键左列读数 / 英文 26 键左列候选词（2026-10-21，见 `CandidateColumns.swift`）。
    let readingColumn = CandidateSideColumn(cells: 5, fontSize: 15)
    let wordColumn = CandidateSideColumn(cells: 5, fontSize: 15)

    /// 最近一次刷新拿到的候选/读数/钉住态（候选栏、左列、平铺三处共用一份快照）。
    var candidateTexts: [String] = []
    /// 候选栏当前槽位数（按宽度算；见 `KeyboardCandidateBar.updateCandidateSlots`）。
    var candidateSlotCount = 5
    var candidateReadingTexts: [String] = []
    var pinnedReading = ""
    /// 读数/左列滚动复位键（拼音缓冲）。缓冲变了整列回顶部（Android `scrollKey`）。
    var candidateResetKey = ""

    /// 当前皮肤 / 键盘模式（控制器从 `KeyboardPreferences` 读入并同步）。
    var skin: SkinOption = .follow
    var keyboardModes = KeyboardModes()
    /// 繁体是引擎模式；面板只负责显示与上报，这里存一份供重建 chip。
    var traditional = false
    /// 设置面板的引擎开关快照（控制器从 `KeyboardPreferences` 读入，面板改动回传）。
    var settings = KeyboardSettings()
    /// 引擎给的模糊音组名（设置面板逐组 chip 的唯一名源，`opi_fuzzy_pair_names`）。
    var fuzzyPairNames: [String] = []
    /// 当前色板与字号倍数（`applyTheme` 设置，重建键位时要重新刷到新按钮上）。
    var palette: KeyboardPalette = .light
    var fontScale: CGFloat = 1
    /// 顶部槽位的当前选择：true = 候选栏，false = 工具条。面板层恒为工具条。
    /// （internal：`KeyboardPanelHost.swift` 的扩展要读写。）
    var wantsCandidateBar = false
    var english = false
    /// 单手模式的约束（`buildRows` 里建，`applyOneHand` 切换）。
    var keyAreaWidth: NSLayoutConstraint?
    var keyAreaLeading: NSLayoutConstraint?
    var keyAreaTrailing: NSLayoutConstraint?
    /// 键区顶锚：平常接候选栏下沿；平铺候选态改接拼音行下沿，让平铺盖住整个
    /// 44pt 槽位（Android `CandidateTiledView` 同义），但**不盖**上面的拼音行。
    var keyAreaTop: NSLayoutConstraint?
    var keyAreaTopFull: NSLayoutConstraint?
    /// 键位行左锚。英文 26 键左侧候选词列出现时，切换到词列右边
    /// （两条互斥约束，见 `CandidateColumns.applySideColumns`）。
    var rowsStackLeading: NSLayoutConstraint?
    var rowsStackToWord: NSLayoutConstraint?

    init() {
        super.init(frame: .zero)
        // ⚠️ 未编译：键盘扩展的**背景**推荐用 UIInputView(style: .keyboard) 承载，
        // 它能拿到系统的毛玻璃/键盘外观，且随系统主题变化。本骨架用普通 UIView。
        backgroundColor = .systemBackground
        buildCandidateBar()
        buildToolbar()
        buildRows()
        buildPanels()
        rebuildKeys()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    /// 键区容器 + 键位行。顶部槽位（候选栏/工具条，44pt）由两个子视图重叠占位；
    /// 键区从槽位下沿开始，所以**候选栏隐藏时键位不会上移**（Android 同：槽位常驻）。
    private func buildRows() {
        keyArea.translatesAutoresizingMaskIntoConstraints = false
        addSubview(keyArea)
        // 横屏刘海把左右 44pt 划进安全区：键区跟着缩，别让首尾键压在传感器上。
        keyAreaLeading = keyArea.leadingAnchor.constraint(equalTo: safeAreaLayoutGuide.leadingAnchor, constant: 3)
        keyAreaTrailing = keyArea.trailingAnchor.constraint(equalTo: safeAreaLayoutGuide.trailingAnchor, constant: -3)
        keyAreaWidth = keyArea.widthAnchor.constraint(equalTo: safeAreaLayoutGuide.widthAnchor, multiplier: 0.76)
        keyAreaLeading?.isActive = true
        keyAreaTrailing?.isActive = true
        keyAreaTop = keyArea.topAnchor.constraint(
            equalTo: candidateBar.bottomAnchor, constant: 4
        )
        keyAreaTop?.isActive = true
        keyAreaTopFull = keyArea.topAnchor.constraint(equalTo: composingBar.bottomAnchor)
        // 钉安全区：home indicator 那条空白由键盘背景填，键不被遮住。
        keyArea.bottomAnchor.constraint(equalTo: safeAreaLayoutGuide.bottomAnchor, constant: -4).isActive = true

        rowsStack.axis = .vertical
        rowsStack.spacing = 6
        // .fill（2026-10-14 由 .fillEqually 改）：行高现在由 `ModeKeyRenderer` 按
        // **行高权重**给显式约束（9 键主区是底行的 3 倍高）。原来 .fillEqually
        // 「按容器高平分」表达不了权重。26 键/14 键/笔画的行权恒 1 ⇒ 结果与改前一致。
        rowsStack.distribution = .fill
        rowsStack.translatesAutoresizingMaskIntoConstraints = false
        keyArea.addSubview(rowsStack)
        // 左锚单独存：英文 26 键的候选词列出现时改成接词列右边。
        rowsStackLeading = rowsStack.leadingAnchor.constraint(equalTo: keyArea.leadingAnchor)
        rowsStackLeading?.isActive = true
        NSLayoutConstraint.activate([
            rowsStack.trailingAnchor.constraint(equalTo: keyArea.trailingAnchor),
            rowsStack.topAnchor.constraint(equalTo: keyArea.topAnchor),
            rowsStack.bottomAnchor.constraint(equalTo: keyArea.bottomAnchor),
        ])
        buildSideColumns()
    }

    /// 重建键位行（切层/换模式时调用）。
    ///
    /// 四个面板（符号/表情/模式菜单/生僻字）**互斥显示**、谁亮由 `keyLayer` 决定；
    /// 行/格/列的**尺寸算式**搬去了 `ModeKeyRenderer`，本函数只挑「画哪套表」并接线。
    private func rebuildKeys() {
        // 符号层/表情层的内容由控制器现取（`setSymbols` / `setEmojiBrowse`），
        // 其余面板是**静态结构**，自己管自己的内容。
        let isSymbols = (keyLayer == .symbols)
        let isEmoji = (keyLayer == .emoji)
        let isModes = (keyLayer == .modes)
        let isRare = (keyLayer == .rare)
        let isSkins = (keyLayer == .skins)
        let isKeyboardModes = (keyLayer == .keyboardModes)
        let isAbout = (keyLayer == .about)
        let isSettings = (keyLayer == .settings)
        let isTiled = (keyLayer == .candidatesTiled)
        rowsStack.isHidden = isPanelLayer
        symbolPanel.isHidden = !isSymbols
        emojiPanel.isHidden = !isEmoji
        modePanel.isHidden = !isModes
        rarePanel.isHidden = !isRare
        skinPanel.isHidden = !isSkins
        keyboardModePanel.isHidden = !isKeyboardModes
        aboutPanel.isHidden = !isAbout
        settingsPanel.isHidden = !isSettings
        candidateTiledPanel.isHidden = !isTiled
        if isTiled { candidateTiledPanel.reloadIfNeeded() }
        if isModes { modePanel.show(current: inputMode) }
        if isRare { rarePanel.show() }
        if isSkins { skinPanel.show(current: skin) }
        if isKeyboardModes {
            keyboardModePanel.show(traditional: traditional, modes: keyboardModes)
        }
        if isSettings {
            settingsPanel.show(settings: settings, modes: keyboardModes, fuzzyNames: fuzzyPairNames)
        }
        updateTopBarVisibility()
        applySideColumns()
        applyOneHand(keyboardModes.oneHand)
        guard !isPanelLayer else { return }
        rowsStack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        // 字母层按**当前输入模式**画（9 键/14 键/笔画各有自己的表）；数字层所有模式共用。
        let rows = keyLayer == .letters
            ? ModeKeyTables.rows(for: inputMode, modeLabel: modeLabel)
            : ModeKeyTables.numberRows()
        ModeKeyRenderer.render(rows, into: rowsStack, gap: 5) { [weak self] b in
            self?.attach(b)
        }
        // 行重建后读数浮层要重新贴到新的标点列上（旧列随 arrangedSubviews 一起没了）。
        installReadingColumn()
        applySideColumns()
        applyKeyTheme()
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
        // 三种键要长按识别：⇧（Lock）/ longPressFunction（模式键）/ ⌫ 清组合。
        if b.spec.keyval == OpiKey.shift || b.spec.longPressFunction != nil || b.spec.longPressClearsComposing {
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
        if sender.spec.keyval == OpiKey.shift || sender.spec.longPressClearsComposing {
            sender.longPressFired = false   // 只起表，不上报（抬手/长按再定论）
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
        if sender.spec.keyval == OpiKey.shift || sender.spec.longPressClearsComposing {
            // 没长按 → 补发一次普通点击；长按已由 onLongPress 上报，不补短按。
            if !sender.longPressFired {
                delegate?.keyboardLayout(self, didPress: sender.spec, longPress: false)
            }
            sender.longPressFired = false
        }
        delegate?.keyboardLayout(self, didRelease: sender.spec)
    }

    // MARK: - 外部刷新

    /// 刷新候选三处显示（栏 / 左列 / 平铺）。texts 是**全局**候选
    /// （`opi_candidates(limit)`），下标与 `opi_select(index)` 同序。
    ///
    /// readings 只在 9 键方案下非空（`opi_candidate_readings()`）；pinned 是 9 键
    /// 左列的钉住读数；resetKey 通常是拼音缓冲，变了让左列回顶部。
    func updateCandidates(
        _ texts: [String],
        readings: [String] = [],
        pinned: String = "",
        resetKey: String = ""
    ) {
        candidateTexts = texts
        candidateReadingTexts = readings
        pinnedReading = pinned
        candidateResetKey = resetKey
        updateCandidateBar(texts)
        candidateTiledPanel.update(
            candidates: texts, readings: readings, pinned: pinned, resetKey: resetKey
        )
        installReadingColumn()
        applySideColumns()
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
    /// [english] 是**引擎模式**的权威判定（`engine.mode() == .english`）：隐藏「中」键
    /// 只在英文键盘出现。它必须先于下面的早退更新 —— Number/Symbol 的键面字也是「中」，
    /// 只比字面会把它们一起显示出来。
    func setModeLabel(_ label: String, english: Bool) {
        showChineseButton(english)
        guard label != modeLabel else { return }
        modeLabel = label
        // 键面字只在重建行时才读，所以这里必须重建（代价是几行按钮重建，
        // 切模式是低频操作）。四个面板层都没有这个键（菜单/生僻字里也不画），不用重建。
        if !isPanelLayer { rebuildKeys() }
    }

    /// 当前层是不是面板层（八个面板 + 平铺候选）——
    /// 它们都不画键位行，`rowsStack` 该藏起来；面板顶部槽位是工具条，
    /// 平铺候选则连顶部槽位一起盖住（Android 同）。
    var isPanelLayer: Bool {
        switch keyLayer {
        case .letters, .numbers: return false
        case .symbols, .emoji, .modes, .rare, .skins, .keyboardModes, .about, .settings,
             .candidatesTiled:
            return true
        }
    }

    /// 同步设置面板的数据（控制器在启动、打开设置、面板改动回传后调）。
    /// 面板可见时立即重画；不可见时只存快照，等 `rebuildKeys` 打开时再用。
    func setEngineSettings(_ s: KeyboardSettings, fuzzyPairNames names: [String]) {
        settings = s
        fuzzyPairNames = names
        if keyLayer == .settings {
            settingsPanel.show(settings: s, modes: keyboardModes, fuzzyNames: names)
        }
    }
}

/// 候选栏/翻页栏/符号面板/表情面板上的小按钮。
/// 不是 `private`：`SymbolPanel.swift`、`EmojiPanel.swift` 与 `EmojiGridView.swift` 都用它
/// （同一文件里才 private 的那种小类型，拆成多个文件后必须放开可见性）。
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

    /// 换肤（候选栏的按钮由 `KeyboardLayout.applyTheme` 递归刷到）。
    func applyTheme(_ p: KeyboardPalette, fontScale: CGFloat) {
        setTitleColor(p.barBufferFg, for: .normal)
        titleLabel?.font = .systemFont(ofSize: 18 * fontScale)
    }

    @objc private func tapped() { onTap() }
}
