// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案：2026-10-15 CI 编译过；10-18 布局与 10-21 `showOwn` 改动未编译、未上机。
//
// 表情页：自有贴纸 + 引擎表情表 + 颜文字表的**展示层**（顶部 Tab + 网格 + 底部横滑分类条）。
//
// 布局与 Android `keyboard/EmojiPanel.kt`、桌面 `candidate/EmojiPage.kt`（2026-10-18）对齐：
//
// ```
// [‹ 返回]   [自有 | Emoji | 颜文字]   [🔍]
// ┌────────────── 网格 ──────────────┐
// └──── 可横滑的分类条（底部一条）────┘
// ```
//
// - **自有** = 独立顶级分类（2026-10-18 用户裁决），4 列网格画 `PetStickerImages`
//   内嵌的小欧 PNG；点击**复制图片到系统剪贴板**（`PetStickerClipboard`）并弹键盘内
//   提示，**不提交任何文本**、不混入 Emoji 的 recents/搜索。
//   iOS 键盘扩展没有往宿主文本框插图片的接口，剪贴板是唯一通路（见 README 已知缺口）。
//   自有页没有分类条，也没有 🔍（搜索只覆盖 Emoji/颜文字）。
// - **Emoji** = 只用引擎 `emojiGroups()`（9 组，**不再加 `自有` 偏移**）。
// - **颜文字** = `KaomojiData.categories`（生成自 `data/raw/kaomoji.tsv`，
//   口径同 Android `KaomojiCategories`）。
// - 分类条点一下 = 网格滚到该组首项所在行；高亮由网格的 `onScroll` 按
//   「首个可见项」反推（同 Android `groupIndexOf`）。
//
// 姿态与 `SymbolPanel.swift` 相同：只画与发射点击，**不上屏、不碰引擎** ——
// 数据由控制器塞进来（`showBrowse` / `showResults`），点中的文本条目经 `onPick` 回控制器，
// 由它走既有的 `insert(...)`（同符号键的 `.commitText` 通路，**不过引擎**：引擎的标点表
// 会把字符改写成中文标点/全角，而用户在表情页点的就是要那个字符）。
//
// 本层**故意没做**：Android 的「最近使用」一行（要跨面板状态，符号面板也没有）。

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

    // MARK: - 布局（数值未上机校准；compact 降档见 `setCompact`，2026-10-23）

    /// Emoji 每行几个（Android `GridCells.Fixed(8)`；compact 降 6）。
    private var emojiColumns = 8
    /// 「自有」贴纸每行 4 个（12 形态 = 3 行；图 44pt 是内容尺寸，不随 compact 变列）。
    private let petColumns = 4
    /// 颜文字比单个 emoji 宽得多：3 列 + 允许缩字，少截成省略号（compact 降 2）。
    private var kaomojiColumns = 3
    /// 顶部工具条 / 底部分类条高度（compact 36，常规 48）。
    private var barHeight: CGFloat = 48
    /// 网格行高（compact 32 / 常规 40；贴纸行 52 / 64）。
    private var gridRowHeight = EmojiGridView.rowHeight
    private var stickerRowHeight = EmojiGridView.stickerRowHeight
    private var compact = false
    /// 两条高度约束在 `setCompact` 时改常数（init 里建）。
    private var topBarHeight: NSLayoutConstraint!
    private var groupBarHeight: NSLayoutConstraint!
    /// 空颜文字 / 空搜索两句话（文案对齐 Android）。
    private static let noKaomoji = "没有颜文字数据"
    private static let noMatch = "无匹配"

    /// 表情页的三个子页（2026-10-18 用户裁决：自有是独立顶级分类）。
    /// internal：分类条代码搬到 `EmojiPanelGroups.swift`（500 行门禁）。
    enum SubTab { case pet, emoji, kaomoji }

    /// 「自有」是否显示（Android `showOwn`，缺省隐藏）。2026-10-21 起由设置面板
    /// 的开关驱动（`setShowOwn(_:)`），不再需要改代码重编。
    private var showOwn = false

    // MARK: - 视图

    private let topBar = UIStackView()
    private let tabs = UIStackView()
    // 分类条那一段（分类表 → 按钮 → 高亮）在 `EmojiPanelGroups.swift` 的扩展里；
    // Swift 的 private 只跨同文件扩展可见，故下面几处状态/视图放开为 internal。
    let groupScroll = UIScrollView()
    let groupStack = UIStackView()
    let grid = EmojiGridView()
    private let searchField = UITextField()
    /// 复制贴纸后的键盘内提示（系统对写剪贴板没有任何反馈）。
    private let toast = ToastLabel()

    /// 网格底边的两种约束：分类条可见时止于它，搜索态（或没有分类）时贴底。
    /// internal：分类条的显隐在 `EmojiPanelGroups.swift` 里切这两条。
    var gridBottomToBar: NSLayoutConstraint!
    var gridBottomToBottom: NSLayoutConstraint!

    /// 为什么是 `lazy`：`BarButton` 的 `onTap` 是**逃逸**闭包且要捕获 `self`，
    /// 存属性初始化时 `self` 还没就绪 —— `lazy` 是让「引用 self 的属性初始化」合法的
    /// 唯一写法（别改成 `let` 加 `[weak self]`：一样编不过）。
    private lazy var backButton = BarButton(title: "‹") { [weak self] in
        self?.backTapped()
    }
    private lazy var searchButton = BarButton(title: "🔍") { [weak self] in
        self?.searchTapped()
    }
    private lazy var petTab = BarButton(title: "自有") { [weak self] in
        self?.switchSub(.pet)
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
    var sub: SubTab = .emoji
    /// 浏览全量（**已过滤字形**，不含 `自有` 那 12 格）。搜索框为空时网格回落到它。
    private var browseList: [String] = []
    /// 引擎口径的表情集（同一份已过滤列表的集合形态）：搜索结果拿它过一遍。
    private var emojiSet: Set<String> = []
    /// 引擎分类（**已按过滤后下标重算**，尚未加 `自有` 的偏移）。
    var emojiGroups: [OpiEmojiGroup] = []
    /// 当前分类条（Emoji = 引擎组；自有页无分类条；颜文字 = 数据分类）。
    var groups: [OpiEmojiGroup] = []
    /// 分类条按钮（与 `groups` 同序；滚动时就地换样式，不重建）。
    var groupButtons: [BarButton] = []
    var selectedGroup = -1
    /// 工具条现在画哪一套（浏览 / 搜索）。**只**影响工具条与「返回」的去向。
    var isSearching = false

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
        if showOwn { tabs.addArrangedSubview(petTab) }
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
        // 自有贴纸：复制图片，**不**经 onPick（那条是文本上屏）。
        grid.onSticker = { [weak self] form in
            self?.copySticker(form)
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
        addSubview(toast)

        gridBottomToBar = grid.bottomAnchor.constraint(
            equalTo: groupScroll.topAnchor, constant: -4
        )
        gridBottomToBottom = grid.bottomAnchor.constraint(equalTo: bottomAnchor)

        topBarHeight = topBar.heightAnchor.constraint(equalToConstant: barHeight)
        groupBarHeight = groupScroll.heightAnchor.constraint(equalToConstant: barHeight)
        // 滚动视图的约束写法与 `SymbolPanel` 逐条同构；
        // 分类条是候选栏那套的**横向**版本（那里钉的是高度）。
        NSLayoutConstraint.activate([
            topBar.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 3),
            topBar.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -3),
            topBar.topAnchor.constraint(equalTo: topAnchor),
            topBarHeight,

            grid.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 3),
            grid.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -3),
            grid.topAnchor.constraint(equalTo: topBar.bottomAnchor, constant: 4),
            gridBottomToBottom,

            groupScroll.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 3),
            groupScroll.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -3),
            groupScroll.bottomAnchor.constraint(equalTo: bottomAnchor),
            groupBarHeight,

            groupStack.leadingAnchor.constraint(equalTo: groupScroll.contentLayoutGuide.leadingAnchor),
            groupStack.trailingAnchor.constraint(equalTo: groupScroll.contentLayoutGuide.trailingAnchor),
            groupStack.topAnchor.constraint(equalTo: groupScroll.contentLayoutGuide.topAnchor),
            groupStack.bottomAnchor.constraint(equalTo: groupScroll.contentLayoutGuide.bottomAnchor),
            groupStack.heightAnchor.constraint(equalTo: groupScroll.frameLayoutGuide.heightAnchor),

            // 提示浮在网格底部上方，不参与布局（进出只改 alpha/isHidden）。
            toast.centerXAnchor.constraint(equalTo: centerXAnchor),
            toast.bottomAnchor.constraint(equalTo: bottomAnchor, constant: -barHeight - 8),
            toast.leadingAnchor.constraint(greaterThanOrEqualTo: leadingAnchor, constant: 12),
            toast.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor, constant: -12),
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
    /// 两个参数都是引擎的**原始**输出：字形过滤与分类下标重算都在本层做（`EmojiFilter`），
    /// 与 Android 把过滤放在 `SymbolCatalog`、面板只管画同一分工 —— 控制器一行判据都不留。
    ///
    /// 进浏览态**一律重置搜索态与子页**：与 Android `openEmoji()` 里那次 `resetSearch()` 对齐，
    /// 否则「上次搜到一半退出」会带着查询词/颜文字页再进来。
    func showBrowse(_ rawList: [String], groups raw: [OpiEmojiGroup]) {
        let (list, regrouped) = EmojiFilter.filterAndRegroup(rawList, groups: raw)
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
        case .pet:
            // 自有没有搜索（🔍 不在这一页）；走到这里只可能是切换子页前挂着的
            // 旧防抖回来了 —— 直接画浏览网格，别把「无匹配」扣在贴纸头上。
            showBrowseGrid()
            return
        case .emoji:
            // 只留引擎口径的表情（Android `results.filter(catalog::isEmoji)`）。
            // ⚠️ `emojiSet` 是**已过滤字形**的那一份，所以「本机画不出的」在这里也被摘掉。
            let shown = results
                .filter { seen.insert($0).inserted }
                .filter { emojiSet.contains($0) }
            grid.showTexts(
                shown, columns: emojiColumns, empty: Self.noMatch, shrink: false,
                rowHeight: gridRowHeight
            )
        case .kaomoji:
            // 颜文字是本地子串匹配（同 Android）；控制器先打的那次引擎搜索在这里
            // 丢弃 —— 为省一次低频 FFI 调用不值得新增「当前子页」查询。
            let shown = KaomojiData.all
                .filter { $0.range(of: query, options: .caseInsensitive) != nil }
                .filter { seen.insert($0).inserted }
            grid.showTexts(
                shown, columns: kaomojiColumns, empty: Self.noMatch, shrink: true,
                rowHeight: gridRowHeight
            )
        }
        updateCategoryHighlight()
    }

    /// 画浏览网格（**不碰工具条**）：搜索态里查询被清空、以及正常浏览都用它。
    private func showBrowseGrid() {
        switch sub {
        case .pet:
            grid.showStickers(
                PetEmojiTable.forms, columns: petColumns, rowHeight: stickerRowHeight
            )
        case .emoji:
            grid.showEmoji(browseList, columns: emojiColumns, rowHeight: gridRowHeight)
        case .kaomoji:
            grid.showTexts(
                KaomojiData.all, columns: kaomojiColumns,
                empty: Self.noKaomoji, shrink: true, rowHeight: gridRowHeight
            )
        }
        updateCategoryHighlight()
    }

    // 分类条（分组表 → 按钮 → 滚动高亮）在 `EmojiPanelGroups.swift` 的扩展里
    // （2026-10-23 搬出：本文件压 500 行门禁）。

    // MARK: - 两套工具条（Android `EmojiTopBar` 的两个状态）

    private func switchSub(_ new: SubTab) {
        guard new != sub else { return }
        let wasSearching = isSearching
        sub = new
        styleTabs()
        // 重装工具条：自有页没有 🔍，其余页有；从搜索态里切过来时顺手退出搜索。
        setSearching(false)
        if wasSearching { onSearch?("") }
        rebuildGroupBar()
        showBrowseGrid()
    }

    private func styleTabs() {
        if showOwn { styleTab(petTab, title: "自有", selected: sub == .pet) }
        styleTab(emojiTab, title: "Emoji", selected: sub == .emoji)
        styleTab(kaomojiTab, title: "颜文字", selected: sub == .kaomoji)
    }

    /// 设置面板的「表情页显示自有贴纸」开关（Android `showOwn`）。
    /// 关掉时若正停在自有页，回 Emoji 页（分类条/网格一起重排）。
    func setShowOwn(_ on: Bool) {
        guard on != showOwn else { return }
        showOwn = on
        if on {
            if !tabs.arrangedSubviews.contains(petTab) {
                tabs.insertArrangedSubview(petTab, at: 0)
            }
        } else {
            petTab.removeFromSuperview()
            if sub == .pet {
                sub = .emoji
                // 面板多半是隐藏的（开关在设置面板里改）：隐藏态不重画，
                // 下次 `showBrowse` 会按新子页重建。
                if !isHidden {
                    setSearching(false)
                    rebuildGroupBar()
                    showBrowseGrid()
                }
            }
        }
        styleTabs()
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
            // 必须补报空查询：控制器挂着的防抖表到点会带旧查询回来，把过滤结果
            // 画进浏览态（Android `back()` 同款）。
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
            // 未验证的关键一跳：扩展里 `becomeFirstResponder()` 能否真收到键入。
            // 若真机不弹键盘，两条后路：② 搜索态自画字母盘把击键喂进 searchField
            // （要改 KeyboardLayout 的层机制）；③ 连 🔍 与 searchField 一起删掉，
            // 不留点了没反应的按钮。详见 README。
            searchField.becomeFirstResponder()
        } else {
            topBar.addArrangedSubview(tabs)
            // 自有子页没有可搜的文本：不画 🔍（别留一个点了没反应的按钮）。
            if sub != .pet {
                topBar.addArrangedSubview(searchButton)
                searchButton.accessibilityLabel = "搜索"
            }
            // 退出搜索就清词：不清的话下次进来网格会停在旧查询的过滤结果上
            // （Android `back()` 里那句 `onSearchText("")` 同款理由）。
            searchField.text = ""
            searchField.resignFirstResponder()
        }
        updateBarVisibility()
    }

    // MARK: - 响应式 / 释放（2026-10-23）

    /// 响应式降档：工具条、列数、行高一起降一档（由 `KeyboardLayout.setCompact` 调）。
    func setCompact(_ on: Bool) {
        guard on != compact else { return }
        compact = on
        barHeight = on ? 36 : 48
        emojiColumns = on ? 6 : 8
        kaomojiColumns = on ? 2 : 3
        gridRowHeight = on ? 32 : EmojiGridView.rowHeight
        stickerRowHeight = on ? 52 : EmojiGridView.stickerRowHeight
        topBarHeight.constant = barHeight
        groupBarHeight.constant = barHeight
        // 不可见/搜索中不重画：不可见的内容已在 isHidden 时释放；搜索中重画会把
        // 结果换成浏览内容。下次打开 / 下次查询会按新几何重建。
        guard !isHidden, !isSearching, !browseList.isEmpty else { return }
        rebuildGroupBar()
        showBrowseGrid()
    }

    /// 离开面板前的清理（`returnToCurrentInputMode` 调）：退出搜索并 resign。
    /// 控制器同时会取消防抖，所以这里**不**补发空查询。
    func exitSearchForClose() {
        if isSearching { setSearching(false) }
    }

    /// 释放大数组与网格子视图。689 个表情按钮是扩展内存的第一热点：关闭面板
    /// 必须真的把它们从视图树摘掉（只 isHidden 不释放内存）。
    func releaseContent() {
        setSearching(false)
        browseList = []
        emojiSet = []
        emojiGroups = []
        groups = []
        groupStack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        groupButtons = []
        selectedGroup = -1
        grid.releaseContent()
        updateBarVisibility()
    }

    /// 关闭 = 释放。九个面板的显隐都走 `KeyboardLayout.rebuildKeys`，这里一处覆盖
    /// 全部关闭路径（返回、切层、平铺、换模式）。⚠️ 同值重复赋值也会触发 didSet，
    /// 所以只在「刚变隐藏」时释放。
    override var isHidden: Bool {
        didSet { if isHidden, !oldValue { releaseContent() } }
    }

    // MARK: - 自有贴纸（剪贴板回退）

    /// 点一格自有贴纸：复制图片到系统剪贴板 + 键盘内可见提示。**不走 `onPick`** ——
    /// 键盘扩展没有往宿主文本框插图片的接口，剪贴板是唯一通路（README 已知缺口）。
    private func copySticker(_ form: PetEmojiTable.Form) {
        guard PetStickerClipboard.copy(form) else {
            // 内嵌数据缺了这张图：说出来，别让用户点了个没反应的格子。
            toast.show("贴纸缺失：\(form.sticker).png")
            return
        }
        toast.show("已复制，去粘贴")
    }
}
