// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · **CI 已真编译**（apple.yml 的 iOS job，2026-10-15）· **从未上机运行**
//
// 本文件在 Linux 上编写。**已过 `swiftc ios/*.swift`（-typecheck + -emit-library）** ——
// 2026-10-15 那次全目录真编译（同一轮还当场抓出一处闭包里的 `continue`，见 RarePanel.swift）；
// 但从未在设备或模拟器上跑过：**行为仍未验证**，别把「编过」读成「能用」。
// 见 ios/README.md「在 Mac 上第一件要做的事」：先让它编译过，再谈功能。
//
// 表情页：引擎表情表的**展示层**（顶部分类条 + 竖向滚动网格 + 搜索框）。
//
// 姿态与 `SymbolPanel.swift` **相同**：只画与发射点击，**不上屏、不碰引擎** ——
// 数据由控制器塞进来（`showBrowse` / `showResults`），点中的表情经 `onPick` 回控制器，
// 由它走既有的 `insert(...)`（同符号键的 `.commitText` 通路，**不过引擎**：引擎的标点表
// 会把字符改写成中文标点/全角，而用户在表情页点的就是要那个字符）。
//
// 对齐 Android `SymbolPanel.kt` 的表情页（2026-10-12 用户裁决的两套工具条）：
//   浏览态 `[‹ 返回][分类条][🔍]`；搜索态 `[‹ 返回][搜索框]`。
// 分类条点一项 = 网格滚到该组 `start`（`OpiEmojiGroup.start`，下标对齐 `OpiEngine.emojiSymbols()`）。
//
// 本层**故意没做**（Android 有、这里没接，别当成漏了）：
//   1. 「最近使用」一行（Android `recents`）—— 要跨面板状态，本骨架的符号面板也没有；
//   2. 分类条的**滚动跟随高亮**（Android 用 `firstVisibleItemIndex`）—— 要接
//      `UIScrollViewDelegate`，先只做「点分类跳过去」这一半；
//   3. 空结果的小欧宠物（Android `OpiPet`）—— iOS 侧根本没有宠物这个组件。

import UIKit

/// 表情页。只负责画与发射点击，**不上屏、不碰引擎**。
final class EmojiPanel: UIView {

    /// 点中一个表情。上屏由控制器做（它才知道 textDocumentProxy）。
    var onPick: ((String) -> Void)?
    /// 浏览态点「返回」= 离开表情页回字母层（同 Android `onClose` → `backToLetters`）。
    var onBack: (() -> Void)?
    /// 搜索框内容变了（**原始文本、未去空白**）。防抖在控制器里 —— 与 Android 的
    /// 「searchText 实时、searchQuery 防抖后生效」同一分工（那边防抖在 `ImeState`）。
    var onSearch: ((String) -> Void)?

    // MARK: - 布局常量（与 SymbolPanel 对表）
    //
    // ⚠️ 未编译：全部数值都是拍的（同 SymbolPanel），真机上按手感调。

    /// 每行几个。与 `SymbolPanel.perRow` 同一个数（Android 是 `GridCells.Fixed(8)`）。
    private static let perRow = 8
    /// 行高。滚动视图里的行**必须**有明确高度（容器 `.fillEqually` 在滚动视图里
    /// 会退化成 0 高，见 `SymbolPanel.init` 那条注释）。
    private static let rowHeight: CGFloat = 40
    /// 网格行间距。与 `SymbolPanel` 的 `stack.spacing = 6` 是**同一个数**（那边是字面量）——
    /// 本文件要用它两次（行间距 + `scrollToGroup` 的偏移换算），所以提成常量。
    private static let gridSpacing: CGFloat = 6
    /// 顶部工具条高度。与 `KeyboardLayout` 候选栏的 40 同值（iOS 侧既有数值）。
    private static let barHeight: CGFloat = 40
    /// 空网格与空搜索两句话。文案对齐 Android（`SymbolPanel.kt` 的「无匹配」）。
    private static let notReady = "（表情库未就绪）"
    private static let noMatch = "无匹配"

    // MARK: - 视图

    private let topBar = UIStackView()
    private let groupScroll = UIScrollView()
    private let groupStack = UIStackView()
    private let gridScroll = UIScrollView()
    private let gridStack = UIStackView()
    private let searchField = UITextField()

    /// 为什么是 `lazy`：`BarButton` 的 `onTap` 是**逃逸**闭包且要捕获 `self`，
    /// 存属性初始化时 `self` 还没就绪 —— `lazy` 是让「引用 self 的属性初始化」合法的
    /// 唯一写法（别改成 `let` 加 `[weak self]`：一样编不过）。
    private lazy var backButton = BarButton(title: "‹") { [weak self] in
        self?.backTapped()
    }
    private lazy var searchButton = BarButton(title: "🔍") { [weak self] in
        self?.searchTapped()
    }

    // MARK: - 状态（全部由 showBrowse/showResults 塞进来）

    /// 浏览全量（**已过滤字形**）。搜索框为空时网格回落到它 —— 对齐 Android
    /// `searchQuery.isEmpty()` 时显示浏览内容（不是「搜索结果为空」）。
    private var browseList: [String] = []
    /// 引擎口径的表情集（同一份已过滤列表的集合形态）：搜索结果拿它过一遍
    /// `isEmoji`（Android `SymbolCatalog.isEmoji` 同款）。
    private var emojiSet: Set<String> = []
    /// 重算过 `start`/`count` 的分类条。
    private var groups: [OpiEmojiGroup] = []
    /// 工具条现在画哪一套（浏览 / 搜索）。**只**影响工具条与「返回」的去向。
    private var isSearching = false

    init() {
        super.init(frame: .zero)

        topBar.axis = .horizontal
        topBar.spacing = 6
        topBar.translatesAutoresizingMaskIntoConstraints = false

        groupScroll.showsHorizontalScrollIndicator = false
        groupScroll.translatesAutoresizingMaskIntoConstraints = false
        groupStack.axis = .horizontal
        groupStack.spacing = 5
        groupStack.translatesAutoresizingMaskIntoConstraints = false
        groupScroll.addSubview(groupStack)

        gridScroll.showsVerticalScrollIndicator = true
        gridScroll.translatesAutoresizingMaskIntoConstraints = false
        gridStack.axis = .vertical
        gridStack.spacing = Self.gridSpacing
        gridStack.translatesAutoresizingMaskIntoConstraints = false
        gridScroll.addSubview(gridStack)

        searchField.placeholder = "搜索（拼音/英文）" // 同 Android 的占位文案
        searchField.font = .systemFont(ofSize: 14)
        searchField.borderStyle = .roundedRect
        searchField.autocorrectionType = .no
        searchField.autocapitalizationType = .none
        searchField.returnKeyType = .search
        searchField.addTarget(self, action: #selector(searchChanged), for: .editingChanged)

        // ⚠️ 未编译：不钉 hugging 的话，工具条里「谁拉伸」是歧义的 ——
        // 分类条/搜索框要吃掉中间的空档，两侧按钮各自按内容宽。
        backButton.setContentHuggingPriority(.required, for: .horizontal)
        searchButton.setContentHuggingPriority(.required, for: .horizontal)
        groupScroll.setContentHuggingPriority(.defaultLow, for: .horizontal)
        searchField.setContentHuggingPriority(.defaultLow, for: .horizontal)

        addSubview(topBar)
        addSubview(gridScroll)

        // ⚠️ 未编译：滚动视图的约束写法与 `SymbolPanel` 逐条同构（竖向：contentLayoutGuide
        // 定内容、宽度钉 frameLayoutGuide 防横向滚动）；分类条是候选栏那套的**横向**版本
        // （那里钉的是高度）。Mac 上确认一遍。
        NSLayoutConstraint.activate([
            topBar.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 3),
            topBar.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -3),
            topBar.topAnchor.constraint(equalTo: topAnchor),
            topBar.heightAnchor.constraint(equalToConstant: Self.barHeight),

            gridScroll.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 3),
            gridScroll.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -3),
            gridScroll.topAnchor.constraint(equalTo: topBar.bottomAnchor, constant: 4),
            gridScroll.bottomAnchor.constraint(equalTo: bottomAnchor),

            gridStack.leadingAnchor.constraint(equalTo: gridScroll.contentLayoutGuide.leadingAnchor),
            gridStack.trailingAnchor.constraint(equalTo: gridScroll.contentLayoutGuide.trailingAnchor),
            gridStack.topAnchor.constraint(equalTo: gridScroll.contentLayoutGuide.topAnchor),
            gridStack.bottomAnchor.constraint(equalTo: gridScroll.contentLayoutGuide.bottomAnchor),
            gridStack.widthAnchor.constraint(equalTo: gridScroll.frameLayoutGuide.widthAnchor),

            groupStack.leadingAnchor.constraint(equalTo: groupScroll.contentLayoutGuide.leadingAnchor),
            groupStack.trailingAnchor.constraint(equalTo: groupScroll.contentLayoutGuide.trailingAnchor),
            groupStack.topAnchor.constraint(equalTo: groupScroll.contentLayoutGuide.topAnchor),
            groupStack.bottomAnchor.constraint(equalTo: groupScroll.contentLayoutGuide.bottomAnchor),
            groupStack.heightAnchor.constraint(equalTo: groupScroll.frameLayoutGuide.heightAnchor),
        ])

        setSearching(false) // 建完就把浏览态那一套装上（此时 groups 还空，分类条是空的）
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    // MARK: - 数据入口（控制器调）

    /// 载入**浏览全量**（`OpiEngine.emojiSymbols()`）+ 分类条（`OpiEngine.emojiGroups()`）。
    ///
    /// 两个参数都是引擎的**原始**输出：字形过滤与分类下标重算都在本层做（`filterAndRegroup`），
    /// 与 Android 把过滤放在 `SymbolCatalog`、面板只管画同一分工 —— 控制器一行判据都不留。
    ///
    /// 进浏览态**一律重置搜索态**：与 Android `openEmoji()` 里那次 `resetSearch()` 对齐，
    /// 否则「上次搜到一半退出」会带着查询词再进来。
    func showBrowse(_ rawList: [String], groups raw: [OpiEmojiGroup]) {
        let (list, regrouped) = Self.filterAndRegroup(rawList, groups: raw)
        browseList = list
        emojiSet = Set(list)
        groups = regrouped
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
        // 保序去重（用户裁决 2026-10-12「表情存在重复的，去重」；Android 是最后一道
        // `distinct()` —— 上游三层各自去重过，屏幕上一格一样的东西仍不该再出现）。
        var seen = Set<String>()
        let deduped = results.filter { seen.insert($0).inserted }
        // 只留引擎口径的表情（Android `results.filter(catalog::isEmoji)`）。
        // ⚠️ `emojiSet` 是**已过滤字形**的那一份，所以「本机画不出的」在这里也被摘掉。
        let shown = deduped.filter { emojiSet.contains($0) }
        rebuildGrid(shown, empty: Self.noMatch)
    }

    /// 画浏览网格（**不碰工具条**）：搜索态里查询被清空、以及正常浏览都用它。
    private func showBrowseGrid() {
        rebuildGrid(browseList, empty: Self.notReady)
    }

    // MARK: - 网格

    private func rebuildGrid(_ texts: [String], empty: String) {
        gridStack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        guard !texts.isEmpty else {
            // 空列表**不休假成空白**：浏览态多半是「引擎还没就绪」，留白会让用户以为
            // 键盘坏了（同 `SymbolPanel.setSymbols` 的理由）；搜索态则是「真的没搜到」。
            gridStack.addArrangedSubview(BarButton(title: empty) {})
            return
        }
        for start in stride(from: 0, to: texts.count, by: Self.perRow) {
            let end = min(start + Self.perRow, texts.count)
            let row = makeRow(Array(texts[start..<end]))
            row.heightAnchor.constraint(equalToConstant: Self.rowHeight).isActive = true
            gridStack.addArrangedSubview(row)
        }
        // 换了一批就把滚动位置收回顶部 —— 否则上一批的偏移会留着，面板看起来像
        // 「打开就是空的」（内容其实在视口上方）。同 `SymbolPanel.setSymbols`。
        gridScroll.setContentOffset(.zero, animated: false)
    }

    private func makeRow(_ texts: [String]) -> UIStackView {
        let row = UIStackView()
        row.axis = .horizontal
        row.spacing = 5
        row.distribution = .fillEqually
        for t in texts {
            row.addArrangedSubview(BarButton(title: t) { [weak self] in
                self?.onPick?(t)
            })
        }
        return row
    }

    /// 网格滚到第 `start` 条所在的行（分类条点一下的去向）。
    ///
    /// 偏移 = 行号 × (行高 + 行距)：行高是显式约束、行距是 `gridStack.spacing`，
    /// 两者都不由内容决定，所以这个换算与真实布局一致（不像内容自适应那样会漂）。
    private func scrollToGroup(start: Int) {
        let row = start / Self.perRow
        let y = CGFloat(row) * (Self.rowHeight + Self.gridSpacing)
        gridScroll.setContentOffset(CGPoint(x: 0, y: y), animated: true)
    }

    // MARK: - 分类条

    private func rebuildGroupBar() {
        groupStack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        for g in groups {
            groupStack.addArrangedSubview(BarButton(title: g.name) { [weak self] in
                self?.scrollToGroup(start: g.start)
            })
        }
        groupScroll.setContentOffset(.zero, animated: false)
    }

    // MARK: - 两套工具条（Android `EmojiTopBar` 的两个状态）

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
    /// 「状态没变就不重装」这点开销不值得换一个只在首帧现形的 bug。
    private func setSearching(_ on: Bool) {
        isSearching = on
        // 先清空再按顺序装 —— 换的是整套工具条（返回键两边都在，所以要重装）。
        topBar.arrangedSubviews.forEach { $0.removeFromSuperview() }
        topBar.addArrangedSubview(backButton)
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
            topBar.addArrangedSubview(groupScroll)
            topBar.addArrangedSubview(searchButton)
            // 退出搜索就清词：不清的话下次进来网格会停在旧查询的过滤结果上
            // （Android `back()` 里那句 `onSearchText("")` 同款理由）。
            searchField.text = ""
            searchField.resignFirstResponder()
        }
    }

    // MARK: - 过滤 + 分类下标重算（纯函数，一处算清）

    /// 把引擎的**原始**表情表与原始分类条，变成「本机能画的列表 + 与之对齐的分类条」。
    ///
    /// 两步必须**一起**做，所以合成一个函数：分类条的下标是**过滤后**列表里的下标
    /// （过滤会摘掉若干条，原下标会整体偏前）—— 点分类是 `scrollToGroup(组.start)`，
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
        // ⚠️ 未编译：每条都调一次 `CTFontCreateWithName`（在 canRender 里）。3963 条
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
