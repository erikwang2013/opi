// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · **CI 已真编译**（apple.yml 的 iOS job，2026-10-15）· **从未上机运行**
//
// 本文件在 Linux 上编写。**已过 `swiftc ios/*.swift`（-typecheck + -emit-library）** ——
// 2026-10-15 那次全目录真编译（同一轮还当场抓出一处闭包里的 `continue`，见 RarePanel.swift）；
// 但从未在设备或模拟器上跑过：**行为仍未验证**，别把「编过」读成「能用」。
// UIKit 是 Apple 独有框架，本机 `import UIKit` 就会断。
//
// 生僻字面板（2026-10-14）：选部首 → 选笔画数 → 挑字，**不经过引擎**（直提）。
// 参照实现是 Android `keyboard/RarePanel.kt` + `keyboard/RareIndex.kt`：
// 抄语义（三级状态机、8 列网格、直提、面包屑回退），不抄 Kotlin 语法
// （Compose 的 `LazyVerticalGrid` / `LazyRow` 在 UIKit 里没有对应物 —— 用
// `SymbolPanel` 那套「竖向滚动 + 固定行高的横向 stack」画，见 `makeGrid`）。
//
// 数据文件：Android 的索引是 `android/app/src/main/assets/rare_index.tsv`
// （102,999 行 / 1.1MB，由 `scripts/gen_rare_index.py` 从 Unihan 生成、**入库**）。
// 2026-10-20 起 `scripts/apple/package-ios.sh` 会把它拷进扩展 bundle（与 .opid 词库同处），
// 名字必须是 `rare_index.tsv`（解析按 `<字><TAB>部首号<TAB>总笔画数>`）。
// 若源文件缺失，打包脚本会硬失败；面板侧仍保留「索引未打包」的可见提示做运行期兜底。
// ⚠️ 本机不能验证「打包后真能读到」—— 那是 Mac / 真机上的事；走 Xcode 集成时
// 仍须把 TSV 加进扩展 target 的 Copy Bundle Resources。

import UIKit

/// 生僻字索引：`rare_index.tsv` 的解析与查询。
///
/// 解析是**纯函数**（Android 侧 `RareIndexTest` 用内联夹具钉住；本端没有 Swift
/// 测试基建，**没有**对应的夹具 —— 这是缺口，见 README）。数据形态：
/// `[部首号(1..214): [总笔画数: [字]]]`，字序 = 表序（码点升序，常用字天然靠前），
/// 列表**直接复用**解析时的顺序，不在 UI 层再排。
enum RareIndex {

    /// 康熙部首数上限（Unihan kRSUnicode 的取值范围）。
    static let maxRadical = 214

    /// 解析 TSV。坏行（列数不对/非数字/空行）**跳过而不抛** —— 资产是生成物，但
    /// 设备上的文件可能被替换/截断，面板该降级成空网格而不是崩掉键盘扩展。
    static func parse(_ text: String) -> [Int: [Int: [String]]] {
        var out: [Int: [Int: [String]]] = [:]
        // 1.1MB / 10 万行：按行扫。⚠️ `components(separatedBy: .newlines)` 会先造出
        // 10 万个 String，`enumerateLines` 是逐行回调（不物化整表）。
        text.enumerateLines { line, _ in
            // ⚠️ 闭包里**不能** `continue`（2026-10-14 CI 实测：`'continue' is only allowed
            // inside a loop`）—— 用 `return` 提前退出本次回调，语义等价（跳过这一行）。
            if line.isEmpty { return }
            let parts = line.components(separatedBy: "\t")
            // 首列必须**恰好一个码点**（按标量数判，不是 utf16 —— 代理对在
            // Swift 里也是 1 个标量，与 Kotlin 的 codePointCount 同义）。
            guard parts.count == 3, parts[0].unicodeScalars.count == 1 else { return }
            guard let radical = Int(parts[1]), let strokes = Int(parts[2]) else { return }
            guard radical >= 1, radical <= maxRadical, strokes > 0 else { return }
            out[radical, default: [:]][strokes, default: []].append(parts[0])
        }
        return out
    }

    /// 部首号 → 康熙部首字形。Unicode 的 Kangxi Radicals 区（U+2F00..U+2FD5）与
    /// 部首号 1..214 **一一对应**（U+2F00 = 一 = 部首 1，U+2FD5 = 龠 = 部首 214），
    /// 故直接算码点。⚠️ 字体缺字时显示豆腐块 —— Android 侧同样只注明风险、不换字体；
    /// 视觉上换字体是设备侧的事（本端也没有「部首 N」的读屏名兜底，
    /// ⚠️ 无障碍是**已知缺口**，见 `KeyButton` 头注的同一条）。
    static func radicalGlyph(_ radical: Int) -> String {
        let r = min(max(radical, 1), maxRadical)
        guard let scalar = Unicode.Scalar(0x2F00 + (r - 1)) else { return "?" }
        return String(Character(scalar))
    }

    /// 某部首下可选的笔画数（升序）—— 面板第二级筛选的来源。
    static func strokeCounts(_ index: [Int: [Int: [String]]], radical: Int) -> [Int] {
        (index[radical]?.keys).map { Array($0).sorted() } ?? []
    }

    /// 某部首 + 笔画数下的字（无 → 空表）。
    static func chars(_ index: [Int: [Int: [String]]], radical: Int, strokes: Int) -> [String] {
        index[radical]?[strokes] ?? []
    }

    /// 从**扩展自己的 bundle** 读索引（`Bundle.main` 在键盘扩展里就是扩展 bundle，
    /// 与 `loadDictionaries()` 取 luna.opid 同一条路）。
    ///
    /// 返回 nil = **没打包**（构建漏拷）/读不出来。调用方要把它画成
    /// 可见的提示，不能留白（空白面板与「这个部首没字」分不出来）。
    ///
    /// ⚠️ 同步读 1.1MB（Android 也是首次打开时读、也缓存）：键盘扩展的主线程上
    /// 这一下会卡一帧。**没做**后台读 —— 要做得自己解决「读完后回主线程刷 UI」，
    /// 那是另一件事，别顺手在这儿起线程。
    static func loadFromBundle() -> [Int: [Int: [String]]]? {
        guard let url = Bundle.main.url(forResource: "rare_index", withExtension: "tsv"),
              let text = try? String(contentsOf: url, encoding: .utf8)
        else { return nil }
        return parse(text)
    }
}

/// 生僻字面板。三级状态机（部首 → 笔画数 → 字），提交走 `onCommit`（**直提**）。
final class RarePanel: UIView {

    /// 点中一个字。上屏由控制器做（它才知道 textDocumentProxy）。
    /// ⚠️ 与符号/表情面板同一条：**直提、不过引擎**（`commitText`）。
    var onCommit: ((String) -> Void)?
    /// 「收起」→ 回字母层（由 `KeyboardLayout` 转成 `.showLetters`）。
    var onClose: (() -> Void)?

    /// 每行几个（与 Android `RarePanel` 的 `GridCells.Fixed(8)` 同粒度）。
    private static let perRow = 8
    /// 行高。⚠️ 未编译：44 对齐 Android 的 `CellText` 高；真机上按键高调。
    /// 滚动视图里的行**必须**有明确高度（同 `SymbolPanel` 的理由）。
    private static let rowHeight: CGFloat = 44

    private let titleLabel = UILabel()
    private let backButton = BarButton(title: "‹ 返回") {}
    private let closeButton = BarButton(title: "收起") {}
    private let scroll = UIScrollView()
    private let stack = UIStackView()

    /// 索引。**失败也记成「已读过」**（`indexLoaded`）—— 与 `SymbolPanel` 的
    /// 「空结果不缓存」**方向相反**，别照抄那边：那边的空是**暂时**的（引擎还没装载），
    /// 这里的空是**永久**的（文件没进 bundle 时，进程活着的时候不会长出来）。
    private var index: [Int: [Int: [String]]] = [:]
    private var indexLoaded = false
    private var radical: Int?
    private var strokes: Int?

    init() {
        super.init(frame: .zero)
        titleLabel.font = .systemFont(ofSize: 14)
        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        backButton.addTarget(self, action: #selector(tappedBack), for: .touchUpInside)
        closeButton.addTarget(self, action: #selector(tappedClose), for: .touchUpInside)
        scroll.showsVerticalScrollIndicator = true
        scroll.translatesAutoresizingMaskIntoConstraints = false
        stack.axis = .vertical
        stack.spacing = 4
        stack.translatesAutoresizingMaskIntoConstraints = false
        scroll.addSubview(stack)

        addSubview(titleLabel)
        addSubview(backButton)
        addSubview(closeButton)
        addSubview(scroll)
        NSLayoutConstraint.activate([
            // 面包屑行（对齐 Android：标题在左，回退与「收起」在右；高 40）。
            titleLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 8),
            titleLabel.centerYAnchor.constraint(equalTo: backButton.centerYAnchor),
            backButton.leadingAnchor.constraint(greaterThanOrEqualTo: titleLabel.trailingAnchor, constant: 8),

            closeButton.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -4),
            closeButton.topAnchor.constraint(equalTo: topAnchor, constant: 4),
            backButton.trailingAnchor.constraint(equalTo: closeButton.leadingAnchor, constant: -4),
            backButton.topAnchor.constraint(equalTo: closeButton.topAnchor),

            scroll.leadingAnchor.constraint(equalTo: leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: trailingAnchor),
            scroll.topAnchor.constraint(equalTo: closeButton.bottomAnchor, constant: 4),
            scroll.bottomAnchor.constraint(equalTo: bottomAnchor),

            // 滚动内容的约束写法与 `SymbolPanel` 逐条相同（contentLayoutGuide 定内容、
            // 宽度钉 frameLayoutGuide 防横向滚动）。
            stack.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor),
            stack.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor),
            stack.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor),
            stack.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("not used: 布局全代码构建") }

    /// 打开面板：首次调用读索引（1.1MB，见 `loadFromBundle`），状态回到第一级。
    /// 「每次打开都回部首页」是 Android 的行为（`remember` 随组合重建）——
    /// 面板不是输入区，没有「上次看到哪」的期待。
    func show() {
        if !indexLoaded {
            index = RareIndex.loadFromBundle() ?? [:]
            indexLoaded = true
        }
        radical = nil
        strokes = nil
        render()
    }

    // MARK: - 三级状态机

    private func render() {
        stack.arrangedSubviews.forEach { $0.removeFromSuperview() }
        backButton.isHidden = (radical == nil)
        switch (radical, strokes) {
        case (nil, _):
            titleLabel.text = "生僻字 · 选部首"
            renderGrid((1...RareIndex.maxRadical).map { RareIndex.radicalGlyph($0) })
        case (let r?, nil):
            titleLabel.text = "生僻字 · \(RareIndex.radicalGlyph(r))（部首 \(r)）· 选笔画数"
            let counts = RareIndex.strokeCounts(index, radical: r)
            if counts.isEmpty {
                // 该部首在索引里没有字 —— 与「索引没打包」是两回事，分开说。
                // （Android：`该部首没有索引数据`；索引整个没打包时这里在**每个**
                // 部首下都会出现，所以另有第一级的提示行，见下。）
                stack.addArrangedSubview(BarButton(title: emptyHint()) {})
            } else {
                renderGrid(counts.map { "\($0) 画" }, tap: { [weak self] i in
                    self?.strokes = counts[i]
                    self?.render()
                })
            }
        case (let r?, let s?):
            titleLabel.text = "生僻字 · 部首 \(r) · \(s) 画"
            let chars = RareIndex.chars(index, radical: r, strokes: s)
            if chars.isEmpty {
                stack.addArrangedSubview(BarButton(title: "该笔画数下没有字") {})
            } else {
                renderGrid(chars, tap: { [weak self] i in self?.onCommit?(chars[i]) })
            }
        }
    }

    /// 索引没打包时的**可见**提示（不留白 —— 空白与「这个部首没字」分不出来）。
    private func emptyHint() -> String {
        index.isEmpty ? "（生僻字索引未打包 · 见 ios/README.md）" : "该部首没有索引数据"
    }

    /// 等宽网格（8 列），`tap` 给的是**下标**。
    private func renderGrid(_ texts: [String], tap: ((Int) -> Void)? = nil) {
        guard !texts.isEmpty else {
            stack.addArrangedSubview(BarButton(title: emptyHint()) {})
            return
        }
        for start in stride(from: 0, to: texts.count, by: Self.perRow) {
            let end = min(start + Self.perRow, texts.count)
            let row = UIStackView()
            row.axis = .horizontal
            row.spacing = 4
            row.distribution = .fillEqually
            for i in start..<end {
                let idx = i
                row.addArrangedSubview(BarButton(title: texts[idx]) { [weak self] in
                    if let tap {
                        tap(idx)
                    } else {
                        // 无回调 = 部首页：点第 i 个 = 部首号 i+1。
                        self?.radical = idx + 1
                        self?.strokes = nil
                        self?.render()
                    }
                })
            }
            row.heightAnchor.constraint(equalToConstant: Self.rowHeight).isActive = true
            stack.addArrangedSubview(row)
        }
        // 换了一级/换了一批就把滚动位置收回顶部（同 `SymbolPanel`：残留的偏移会让
        // 面板看起来像空的）。
        scroll.setContentOffset(.zero, animated: false)
    }

    @objc private func tappedBack() {
        // 笔画 → 部首；部首 → 无上级（`isHidden` 已挡住这一档）。
        if strokes != nil { strokes = nil } else { radical = nil }
        render()
    }

    @objc private func tappedClose() { onClose?() }
}
