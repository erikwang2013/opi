// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · **CI 已真编译**（apple.yml 的 iOS job，2026-10-15）· **从未上机运行**
//
// 本文件在 Linux 上编写。2026-10-15 那次 `swiftc ios/*.swift` 全目录真编译过；
// 但从未在设备或模拟器上跑过：**行为仍未验证**，别把「编过」读成「能用」。
// ⚠️ **2026-10-18 的布局改写尚未经过任何编译**（本机没有 swiftc）——
// 待 CI apple.yml 的 iOS job；见 ios/README.md。
//
// 表情页：引擎表情表 + 颜文字表的**展示层**（顶部 Emoji/颜文字 Tab + 网格 + 底部横滑分类条）。
//
// 布局与 Android `keyboard/EmojiPanel.kt`、桌面 `candidate/EmojiPage.kt`（2026-10-18）对齐：
//
// ```
// [‹ 返回]   [Emoji | 颜文字]   [🔍]
// ┌────────────── 网格 ──────────────┐
// └──── 可横滑的分类条（底部一条）────┘
// ```
//
// - **Emoji** 分类 = `自有` + 引擎 `emojiGroups()`（与 Android `PetEmoji.categories` 同构：
//   引擎的 `start` 整体后移 `PetEmojiTable.forms.count`）。`自有` 的 12 个形态在 iOS 上
//   **没有小欧矢量绘制**，网格里用 emoji 字形 + 小标签表达，点击提交该 emoji
//   （映射见 `PetEmojiTable.swift`，与 `shared/pet/PetEmojiMap.kt` 同源）。
// - **颜文字** 分类 = `KaomojiData.categories`（生成自 `data/raw/kaomoji.tsv`，
//   口径同 Android `KaomojiCategories`）。
// - 分类条点一下 = 网格滚到该组首项所在行；高亮由网格的 `onScroll` 按
//   「首个可见项」反推（同 Android `groupIndexOf`）。
//
// 姿态与 `SymbolPanel.swift` 相同：只画与发射点击，**不上屏、不碰引擎** ——
// 数据由控制器塞进来（`showBrowse` / `showResults`），点中的条目经 `onPick` 回控制器，
// 由它走既有的 `insert(...)`（同符号键的 `.commitText` 通路，**不过引擎**：引擎的标点表
// 会把字符改写成中文标点/全角，而用户在表情页点的就是要那个字符）。
//
// 本层**故意没做**（Android 有、这里没接，别当成漏了）：
//   1. 「最近使用」一行（Android `recents`）—— 要跨面板状态，本骨架的符号面板也没有。

import UIKit

/// 表情页。只负责画与发射点击，**不上屏、不碰引擎**。
final class EmojiPanel: UIView {

    /// 点中一个表情/颜文字。上屏由控制器做（它才知道 textDocumentProxy）。
    var onPick: ((String) -> Void)?
    /// 浏览态点「返回」= 离开表情页回字母层（同 Android `onClose` → `backToLetters`）。
    var onBack: (() -> Void)?
    /// 搜索框内容变了（**原始文本、未去空白**）。防抖在控制器里 —— 与 Android 的
    /// 「searchText 实时、searchQuery 防抖后生效」同一分工（那边防抖在 `ImeState`）。
    var onSearch: ((String) -> Void)?

    // MARK: - 布局常量（与 SymbolPanel 对表）
    //
    // ⚠️ 未编译：数值都是拍的（同 SymbolPanel），真机上按手感调。

    /// Emoji 每行几个（Android `GridCells.Fixed(8)`）。
    private static let emojiColumns = 8
    /// 颜文字比单个 emoji 宽得多：3 列 + 允许缩字，少截成省略号（Android 同）。
    private static let kaomojiColumns = 3
    /// 顶部工具条 / 底部分类条高度。与 `KeyboardLayout` 候选栏的 40 同值。
    private static let barHeight: CGFloat = 40
    /// 空颜文字 / 空搜索两句话（文案对齐 Android）。
    private static let noKaomoji = "没有颜文字数据"
    private static let noMatch = "无匹配"

    /// 表情页的两个子页（2026-10-18 用户参考布局：顶部 Emoji / 颜文字）。
    private enum SubTab { case emoji, kaomoji }

    // MARK: - 视图

    private let topBar = UIStackView()
    private let tabs = UIStackView()
    private let groupScroll = UIScrollView()
    private let groupStack = UIStackView()
    private let grid = EmojiGridView()
    private let searchField = UITextField()

    /// 网格底边的两种约束：分类条可见时止于它，搜索态（或没有分类）时贴底。
    private var gridBottomToBar: NSLayoutConstraint!
    private var gridBottomToBottom: NSLayoutConstraint!

    /// 为什么是 `lazy`：`BarButton` 的 `onTap` 是**逃逸**闭包且要捕获 `self`，
    /// 存属性初始化时 `self` 还没就绪 —— `lazy` 是让「引用 self 的属性初始化」合法的
    /// 唯一写法（别改成 `let` 加 `[weak self]`：一样编不过）。
    private lazy var backButton = BarButton(title: "‹") { [weak self] in
        self?.backTapped()
    }
    private lazy var searchButton = BarButton(title: "🔍") { [weak self] in
        self?.searchTapped()
    }
    private lazy var emojiTab = BarButton(title: "Emoji") { [weak self] in
        self?.switchSub(.emoji)
    }
    private lazy var kaomojiTab = BarButton(title: "颜文字") { [weak self] in
        self?.switchSub(.kaomoji)
    }

    // MARK: - 状态（全部由 showBrowse/showResults 塞进来）

    /// 当前子页。进浏览态（`showBrowse`）一律回 Emoji —— 与 Android 的 `remember` 同义：
    /// 面板重新打开时不该停在上次的颜文字页。
    private var sub: SubTab = .emoji
    /// 浏览全量（**已过滤字形**，不含 `自有` 那 12 格）。搜索框为空时网格回落到它。
    private var browseList: [String] = []
    /// 引擎口径的表情集（同一份已过滤列表的集合形态）：搜索结果拿它过一遍。
    private var emojiSet: Set<String> = []
    /// 引擎分类（**已按过滤后下标重算**，尚未加 `自有` 的偏移）。
    private var emojiGroups: [OpiEmojiGroup] = []
    /// 当前分类条（Emoji = `自有` + 引擎组；颜文字 = 数据分类）。
    private var groups: [OpiEmojiGroup] = []
    /// 分类条按钮（与 `groups` 同序；滚动时就地换样式，不重建）。
    private var groupButtons: [BarButton] = []
    private var selectedGroup = -1
    /// 工具条现在画哪一套（浏览 / 搜索）。**只**影响工具条与「返回」的去向。
    private var isSearching = false

    init() {
        super.init(frame: .zero)

        topBar.axis = .horizontal
        topBar.spacing = 6
        topBar.alignment = .center
        topBar.translatesAutoresizingMaskIntoConstraints = false

        tabs.axis = .horizontal
        tabs.spacing = 2
        tabs.distribution = .fillEqually
        tabs.translatesAutoresizingMaskIntoConstraints = false
        tabs.addArrangedSubview(emojiTab)
        tabs.addArrangedSubview(kaomojiTab)

        groupScroll.showsHorizontalScrollIndicator = false
        groupScroll.translatesAutoresizingMaskIntoConstraints = false
        groupStack.axis = .horizontal
        groupStack.spacing = 8
        groupStack.translatesAutoresizingMaskIntoConstraints = false
        groupScroll.addSubview(groupStack)

        grid.translatesAutoresizingMaskIntoConstraints = false
        grid.onPick = { [weak self] text in
            self?.onPick?(text)
        }
        // 滚动时反推高亮（搜索态没有分类条，跳过）。
        grid.onScroll = { [weak self] in
            guard let self, !self.isSearching else { return }
            self.updateCategoryHighlight()
        }

        searchField.placeholder = "搜索（拼音/英文）"
        searchField.font = .systemFont(ofSize: 14)
        searchField.borderStyle = .roundedRect
        searchField.autocorrectionType = .no
        searchField.autocapitalizationType = .none
        searchField.returnKeyType = .search
        searchField.addTarget(self, action: #selector(searchChanged), for: .editingChanged)

        // 不钉 hugging 的话，工具条里「谁拉伸」是歧义的 —— 中间的子页切换/搜索框
        // 吃掉空档，两侧按钮各自按内容宽。
        backButton.setContentHuggingPriority(.required, for: .horizontal)
        searchButton.setContentHuggingPriority(.required, for: .horizontal)
        tabs.setContentHuggingPriority(.defaultLow, for: .horizontal)
        searchField.setContentHuggingPriority(.defaultLow, for: .horizontal)

        addSubview(topBar)
        addSubview(grid)
        addSubview(groupScroll)

        gridBottomToBar = grid.bottomAnchor.constraint(
            equalTo: groupScroll.topAnchor, constant: -4
        )
        gridBottomToBottom = grid.bottomAnchor.constraint(equalTo: bottomAnchor)

        // 滚动视图的约束写法与 `SymbolPanel` 逐条同构；
        // 分类条是候选栏那套的**横向**版本（那里钉的是高度）。
        NSLayoutConstraint.activate([
            topBar.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 3),
            topBar.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -3),
            topBar.topAnchor.constraint(equalTo: topAnchor),
            topBar.heightAnchor.constraint(equalToConstant: Self.barHeight),

            grid.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 3),
            grid.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -3),
            grid.topAnchor.constraint(equalTo: topBar.bottomAnchor, constant: 4),
            gridBottomToBottom,

            groupScroll.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 3),
            groupScroll.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -3),
            groupScroll.bottomAnchor.constraint(equalTo: bottomAnchor),
            groupScroll.heightAnchor.constraint(equalToConstant: Self.barHeight),

            groupStack.leadingAnchor.constraint(equalTo: groupScroll.contentLayoutGuide.leadingAnchor),
            groupStack.trailingAnchor.constraint(equalTo: groupScroll.contentLayoutGuide.trailingAnchor),
            groupStack.topAnchor.constraint(equalTo: groupScroll.contentLayoutGuide.topAnchor),
            groupStack.bottomAnchor.constraint(equalTo: groupScroll.contentLayoutGuide.bottomAnchor),
            groupStack.heightAnchor.constraint(equalTo: groupScroll.frameLayoutGuide.heightAnchor),
        ])

        styleTabs()
        setSearching(false) // 建完就把浏览态那一套装上（此时 groups 还空，分类条是空的）
        rebuildGroupBar()
        showBrowseGrid()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    // MARK: - 数据入口（控制器调）

    /// 载入**浏览全量**（`OpiEngine.emojiSymbols()`）+ 分类条（`OpiEngine.emojiGroups()`）。
    ///
    /// 两个参数都是引擎的**原始**输出：字形过滤与分类下标重算都在本层做（`filterAndRegroup`），
    /// 与 Android 把过滤放在 `SymbolCatalog`、面板只管画同一分工 —— 控制器一行判据都不留。
    ///
    /// 进浏览态**一律重置搜索态与子页**：与 Android `openEmoji()` 里那次 `resetSearch()` 对齐，
    /// 否则「上次搜到一半退出」会带着查询词/颜文字页再进来。
    func showBrowse(_ rawList: [String], groups raw: [OpiEmojiGroup]) {
        let (list, regrouped) = Self.filterAndRegroup(rawList, groups: raw)
        browseList = list
        emojiSet = Set(list)
        emojiGroups = regrouped
        sub = .emoji
        styleTabs()
        setSearching(false)
        rebuildGroupBar()
        showBrowseGrid()
    }

    /// 载入搜索结果（`OpiEngine.searchSymbols(keyword:)`）。`query` 是**去空白后**的查询词
    /// （控制器 trim，对齐 Android `searchText.trim()`）。
    ///
    /// 空查询 = 回浏览网格，**不是**「搜了没结果」—— Android 的内容分支就是
    /// `searchActive && searchQuery.isNotEmpty()`，空查询时画的是浏览内容。
    func showResults(_ results: [String], query: String) {
        guard !query.isEmpty else {
            showBrowseGrid()
            return
        }
        // 保序去重（用户裁决 2026-10-12「表情存在重复的，去重」）。
        var seen = Set<String>()
        switch sub {
        case .emoji:
            // 只留引擎口径的表情（Android `results.filter(catalog::isEmoji)`）。
            // ⚠️ `emojiSet` 是**已过滤字形**的那一份，所以「本机画不出的」在这里也被摘掉。
            let shown = results
                .filter { seen.insert($0).inserted }
                .filter { emojiSet.contains($0) }
            grid.showTexts(shown, columns: Self.emojiColumns, empty: Self.noMatch, shrink: false)
        case .kaomoji:
            // 颜文字是**本地子串匹配**（同 Android `KaomojiCategories.flatten(...).filter`）。
            // 控制器仍会先打一次引擎搜索（它不知道当前子页），结果在这里被丢弃 ——
            // 为省一次低频 FFI 调用不值得给控制器新增一条「当前子页」查询。
            let shown = KaomojiData.all
                .filter { $0.range(of: query, options: .caseInsensitive) != nil }
                .filter { seen.insert($0).inserted }
            grid.showTexts(
                shown, columns: Self.kaomojiColumns, empty: Self.noMatch, shrink: true
            )
        }
        updateCategoryHighlight()
    }

    /// 画浏览网格（**不碰工具条**）：搜索态里查询被清空、以及正常浏览都用它。
    private func showBrowseGrid() {
        switch sub {
        case .emoji:
            grid.showEmoji(
                pets: PetEmojiTable.forms, emojis: browseList, columns: Self.emojiColumns
            )
        case .kaomoji:
            grid.showTexts(
                KaomojiData.all, columns: Self.kaomojiColumns,
                empty: Self.noKaomoji, shrink: true
            )
        }
        updateCategoryHighlight()
    }

    // MARK: - 分类条

    /// 当前子页的分类条（Emoji 的引擎组整体后移 `自有` 的格数）。
    private func groupsFor(_ sub: SubTab) -> [OpiEmojiGroup] {
        switch sub {
        case .emoji:
            let own = PetEmojiTable.forms.count
            let ownGroup = OpiEmojiGroup(name: "自有", start: 0, count: own)
            return [ownGroup] + emojiGroups.map {
                OpiEmojiGroup(name: $0.name, start: $0.start + own, count: $0.count)
            }
        case .kaomoji:
            return KaomojiData.categories.map {
                OpiEmojiGroup(name: $0.label, start: $0.start, count: $0.items.count)
            }
        }
    }

    private func rebuildGroupBar() {
        groups = groupsFor(sub)
        groupButtons = groups.map { g in
            BarButton(title: g.name) { [weak self] in
                self?.grid.scrollTo(start: g.start)
            }
        }
        groupStack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        for b in groupButtons {
            groupStack.addArrangedSubview(b)
        }
        groupScroll.setContentOffset(.zero, animated: false)
        selectedGroup = -1
        updateCategoryHighlight()
        updateBarVisibility()
    }

    /// 分类条只在浏览态且**有分类**时出现；它藏起来时网格要吃掉这一条的高度。
    private func updateBarVisibility() {
        let visible = !isSearching && !groups.isEmpty
        groupScroll.isHidden = !visible
        // 先撤旧约束再上新约束：两条同时 active 的那一瞬间是互相矛盾的底边。
        if visible {
            gridBottomToBottom.isActive = false
            gridBottomToBar.isActive = true
        } else {
            gridBottomToBar.isActive = false
            gridBottomToBottom.isActive = true
        }
    }

    /// 网格下标 → 分类条要亮第几项（同 Android `SymbolCatalog.groupIndexOf`）。
    /// 分组连续覆盖全表（引擎门禁保证），取「最后一个 start ≤ index 的组」即可。
    private static func groupIndexOf(_ groups: [OpiEmojiGroup], _ index: Int) -> Int {
        var found = -1
        for (i, g) in groups.enumerated() where g.start <= index {
            found = i
        }
        return found
    }

    private func updateCategoryHighlight() {
        guard !groups.isEmpty else { return }
        let index = Self.groupIndexOf(groups, grid.firstVisibleItemIndex())
        guard index != selectedGroup else { return }
        selectedGroup = index
        for (i, b) in groupButtons.enumerated() {
            styleCategory(b, selected: i == index)
        }
    }

    private func styleCategory(_ b: BarButton, selected: Bool) {
        b.titleLabel?.font = .systemFont(ofSize: 13, weight: selected ? .bold : .regular)
        b.setTitleColor(selected ? .systemBlue : .label, for: .normal)
        // 选中 = 药丸底（同 Android 的 keyBg 底 + accent 字）。
        b.backgroundColor = selected ? .secondarySystemBackground : .clear
        b.layer.cornerRadius = 8
        let name = b.title(for: .normal) ?? ""
        b.accessibilityLabel = "分类 \(name)" + (selected ? "，已选中" : "")
        if selected {
            b.accessibilityTraits.insert(.selected)
        } else {
            b.accessibilityTraits.remove(.selected)
        }
    }

    // MARK: - 两套工具条（Android `EmojiTopBar` 的两个状态）

    private func switchSub(_ new: SubTab) {
        guard new != sub else { return }
        sub = new
        styleTabs()
        rebuildGroupBar()
        showBrowseGrid()
    }

    private func styleTabs() {
        styleTab(emojiTab, title: "Emoji", selected: sub == .emoji)
        styleTab(kaomojiTab, title: "颜文字", selected: sub == .kaomoji)
    }

    private func styleTab(_ b: BarButton, title: String, selected: Bool) {
        b.titleLabel?.font = .systemFont(ofSize: 15, weight: selected ? .bold : .regular)
        b.setTitleColor(selected ? .systemBlue : .secondaryLabel, for: .normal)
        b.accessibilityLabel = title + (selected ? "，已选中" : "")
        if selected {
            b.accessibilityTraits.insert(.selected)
        } else {
            b.accessibilityTraits.remove(.selected)
        }
    }

    @objc private func searchChanged() {
        onSearch?(searchField.text ?? "")
    }

    /// 🔍：进搜索态。**不发搜索** —— 空查询时网格本就该是浏览内容（见 `showResults`），
    /// 发一次只会白白捞一遍全表（Android 进搜索态也不搜）。
    private func searchTapped() {
        setSearching(true)
    }

    /// 返回键：搜索态 → 退回浏览态并清查询（网格回全量）；浏览态 → 离开表情页。
    /// 同 Android `EmojiTopBar.back()` 的两分支。
    private func backTapped() {
        if isSearching {
            setSearching(false)
            showBrowseGrid()
            // ⚠️ 必须补报一次空查询：控制器那边可能还挂着一张**防抖表**（用户打完字
            // 250ms 内就按了返回）。不报的话那张表到点会带着**旧查询**回来，
            // 把过滤结果画进浏览态（网格停在过滤结果上，且没有任何提示）。
            // Android 的 `back()` 正是这么做的（`onSearchText("")` → `cancelDebounce`）。
            onSearch?("")
        } else {
            onBack?()
        }
    }

    /// 装上工具条的那一套。**故意没有 `guard on != isSearching else { return }`**
    /// —— 别加回去：`init` 里第一次就是 `setSearching(false)`，而 `isSearching` 初值
    /// 也是 `false`，一个提前返回会让工具条**一条子视图都装不上**（面板顶部全空）。
    private func setSearching(_ on: Bool) {
        isSearching = on
        // 先清空再按顺序装 —— 换的是整套工具条（返回键两边都在，所以要重装）。
        topBar.arrangedSubviews.forEach { $0.removeFromSuperview() }
        topBar.addArrangedSubview(backButton)
        backButton.accessibilityLabel = on ? "返回表情" : "关闭表情页"
        if on {
            topBar.addArrangedSubview(searchField)
            // ⚠️⚠️ **未编译 + 未验证的关键一跳**：键盘扩展里 `becomeFirstResponder()`
            // 能不能让这个输入框真的收到键盘输入，是本文件最大的未知数 ——
            // 扩展**不能**唤起系统键盘（iOS 只允许宿主 App 的输入框唤起），而 Android
            // 是靠 IME 窗口内的 TextField + 在面板下方**叠一层字母盘**（焦点事件走
            // `onSearchFocus` → 宿主叠盘）解决的，iOS 没有等价机制可直接抄。
            //
            // Mac 上按顺序验，**三条路都试完再决定留不留这个键**：
            //   ① 直接跑：能弹出系统键盘（或接硬件键盘能输进去）→ 就这样；
            //   ② 不弹：退回 **Gboard 式自喂** —— 搜索态仍由本键盘画字母盘，把击键
            //      自己喂进 `searchField`（`insertText` 或直接拼 `text` 后再发 `onSearch`）。
            //      这是 Android 那套叠盘的 iOS 等价物，但要动 `KeyboardLayout` 的层机制
            //      （搜索态得多一层字母盘），**本骨架没做** —— 没真机结论前别先写；
            //   ③ 都不行：把 🔍 键连同 `searchField` 一起删掉（搜索的数据面
            //      `OpiEngine.searchSymbols` 留着不动），**别留一个点了没反应的按钮**
            //      —— 这比没有搜索更坏。
            searchField.becomeFirstResponder()
        } else {
            topBar.addArrangedSubview(tabs)
            topBar.addArrangedSubview(searchButton)
            searchButton.accessibilityLabel = "搜索"
            // 退出搜索就清词：不清的话下次进来网格会停在旧查询的过滤结果上
            // （Android `back()` 里那句 `onSearchText("")` 同款理由）。
            searchField.text = ""
            searchField.resignFirstResponder()
        }
        updateBarVisibility()
    }

    // MARK: - 过滤 + 分类下标重算（纯函数，一处算清）

    /// 把引擎的**原始**表情表与原始分类条，变成「本机能画的列表 + 与之对齐的分类条」。
    ///
    /// 两步必须**一起**做，所以合成一个函数：分类条的下标是**过滤后**列表里的下标
    /// （过滤会摘掉若干条，原下标会整体偏前）—— 点分类是 `scrollTo(start:)`，
    /// 不重算就会滚错组。分成两个函数就给了「只调其中一个」的机会，那是静默错位。
    ///
    /// 逐条对齐 Android `SymbolCatalog.emojiGroups`（前缀存活表）：
    /// `aliveUpTo[i]` = 原始表 `[0, i)` 里有多少条通过了 `canRender`；组的 `start`/`count`
    /// **一起**重算（`count` 是分类条读屏文案，不跟着减就会把摘掉的条数也报进去）。
    ///
    /// ⚠️ 表拿不到（未装载 / 没取到）⇒ **原样返回，不重算**：没有列表就没有「过滤后下标」
    /// 可言，硬算会把所有组的 `start` 压成 0（那比不显示分类更坏）。同 Android。
    static func filterAndRegroup(
        _ rawList: [String], groups raw: [OpiEmojiGroup]
    ) -> (list: [String], groups: [OpiEmojiGroup]) {
        // 字形过滤用 SymbolPanel 那一份判据（**只判单码位**，多码位一律放行）——
        // 探针字体也是它的那一颗，别在本文件另造（见 SymbolPanel.probeFont 的注释）。
        //
        // ⚠️ 未编译：每条都调一次 `CTFontCreateWithName`（在 canRender 里）。687 条
        // 一次性跑，Mac 上量一下开面板的耗时；若肉眼可见卡，把 CTFont 提到循环外
        // （要改 canRender 的签名，**别**在这里抄一份判定逻辑）。
        var aliveUpTo = [Int](repeating: 0, count: rawList.count + 1)
        var alive = 0
        var list: [String] = []
        list.reserveCapacity(rawList.count)
        for i in rawList.indices {
            aliveUpTo[i] = alive
            if SymbolPanel.canRender(rawList[i], font: SymbolPanel.probeFont) {
                alive += 1
                list.append(rawList[i])
            }
        }
        aliveUpTo[rawList.count] = alive
        guard !rawList.isEmpty else { return (list, raw) }
        let groups = raw.map { g -> OpiEmojiGroup in
            let from = min(max(g.start, 0), rawList.count)
            let to = min(max(g.start + g.count, 0), rawList.count)
            return OpiEmojiGroup(
                name: g.name, start: aliveUpTo[from], count: aliveUpTo[to] - aliveUpTo[from]
            )
        }
        return (list, groups)
    }
}
