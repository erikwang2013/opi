// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写，未经过任何 Swift 编译器。
// UIKit 是 Apple 独有框架，本机 `import UIKit` 就会断。
//
// 「一个键是什么」：键位的**数据类型与按钮**，不含布局与触摸分流。
// 布局在 `KeyboardLayout.swift`（它才认得层、行、候选栏）。
// 拆出来是因为 `KeyboardLayout.swift` 顶到了本仓的 500 行源码上限；
// 这两件事本来就是两个概念 —— 键的定义是**编译期常量**，布局是视图树装配。

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
///
/// 层与模式都写成**显式的目标**而不是「切换」：`toggle` 语义在三个层/三个模式上
/// 会让键面字与实际去向脱钩（键面写「ABC」却切到了符号层），而键面字是用户唯一的路标。
/// 三层的导航关系对齐 Android：字母 →「123」→数字 →「#+=」→符号 →「ABC」→字母。
enum KeyboardFunctionKey {
    /// 切到字母层（⇧ 键所在的那层）。
    case showLetters
    /// 切到数字/常用标点层。
    case showNumbers
    /// 切到符号层。符号数据来自引擎的符号库出口（`OpiEngine.commonSymbols()`）。
    case showSymbols
    /// 中 → 繁 → 英 → 中（**逐字对齐 Android `ImeScreen.kt` 的 `toggleMode()`**：
    /// 离开拼音类模式前先 `clear()`，否则残留的拼音缓冲会被下一个空格/回车
    /// 意外提交）。
    case cycleMode
    /// 地球键（切到下一个输入法）。
    case nextInputMode
    /// 直接上屏字面文本，**不过引擎**。
    ///
    /// 符号面板的键走这条：引擎的标点表会把 `,` 改写成中文标点/全角（那是**文本**
    /// 模式该做的事），而用户在符号面板上点的 `,` 就是要 `,`。
    /// Android 的 `SymbolPanel` 同样绕过引擎直提（`router::commitText`）。
    case commitText(String)
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
