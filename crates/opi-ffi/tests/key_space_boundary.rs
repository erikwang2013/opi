// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **键码空间**的边界：三套「特殊键」编码与「可打印 = 码点」那一段必须互不撞号。
//!
//! 三套空间：
//! - `engine_core::keys`：`SPECIAL_BASE(0x1_0000) | 低 16 位`，C ABI（`opi_key_event`）用；
//! - `tsf_opi::logic`：`SPECIAL_BASE(0x1_0000) | Windows VK`，TSF 胶水用；
//! - `fcitx5_opi::input_method`：fcitx5 keysym（`0xff00+`），fcitx5 胶水用。
//!
//! 这条纪律有过血的教训（见 tsf 轨 `SPECIAL_BASE` 的注释）：TSF 轨曾裸用 VK，
//! 于是 `VK_NEXT=0x22` 撞上 `"`、`VK_DELETE=0x2E` 撞上 `.` —— 表现为
//! **普通字符被吃掉**（拼音打一半敲 `.` 变退格）。所以这里不问「有没有撞」，
//! 而是把两件事都钉住：
//! 1. 特殊键码本身必须落在可打印段之外（`> 0x7f` 且 `char::from_u32` 得非 ASCII）；
//! 2. **同一个数字在两个入口下含义不同，且可打印入口永远不触发特殊行为** ——
//!    这是真正的「不相交」证明：`SPECIAL_BASE` 取 0x1_0000 之后，特殊键码本身就
//!    是**合法的 Unicode 字符**（补充平面），数学上并不与「所有可打印码点」不相交，
//!    安全完全依赖「字符走 `input_key`、键值走 `key_event`」这条入口分工。
//!    所以第 2 条必须测：把特殊键码当字符喂进去，缓冲一个字符都不能变。

use engine_core::keys as ek;
use fcitx5_opi::input_method as fim;
use fcitx5_opi::{opi_fcitx5_buffer, opi_fcitx5_input_key, opi_ffi_free_string_utf8};
use opi_ffi::cabi::{
    OpiString, opi_buffer, opi_candidates, opi_clear, opi_ffi_free_string, opi_input_key, opi_load,
    opi_set_learner, opi_switch_mode,
};
use tsf_opi::logic as tlg;

/// 取 android 部署副本：`data/generated/luna.opid` 被 gitignore（构建中间物），
/// 全新 clone 里不存在。两份内容逐字节相同。
const LUNA: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/assets/luna.opid"
);

/// 三个模块各自的特殊键常量。标签是 `轨道::常量名` —— 名字是给
/// [`every_key_code_const_in_the_three_tracks_is_registered_here`] 对着**源文件**
/// 核完整性用的（值仍由编译器从常量引用取出，文本只管名字，两边互补）。
///
/// **不含空格**：空格在 `engine_core` / `fcitx5` 里就是字符键 `0x20`，单独登记在
/// 白名单 `PRINTABLE_BY_DESIGN` 里 —— 别把「值恰好在可打印段」当通行证。
fn all_special_codes() -> Vec<(&'static str, u32)> {
    let mut v: Vec<(&'static str, u32)> = vec![
        ("engine_core::KEY_BACK_SPACE", ek::KEY_BACK_SPACE),
        ("engine_core::KEY_TAB", ek::KEY_TAB),
        ("engine_core::KEY_RETURN", ek::KEY_RETURN),
        ("engine_core::KEY_ESCAPE", ek::KEY_ESCAPE),
        ("engine_core::KEY_PAGE_UP", ek::KEY_PAGE_UP),
        ("engine_core::KEY_PAGE_DOWN", ek::KEY_PAGE_DOWN),
        ("engine_core::KEY_DELETE", ek::KEY_DELETE),
        ("engine_core::KEY_SHIFT", ek::KEY_SHIFT),
        ("engine_core::KEY_UP", ek::KEY_UP),
        ("engine_core::KEY_DOWN", ek::KEY_DOWN),
        ("engine_core::KEY_LEFT", ek::KEY_LEFT),
        ("engine_core::KEY_RIGHT", ek::KEY_RIGHT),
        ("tsf::KEY_BACK_SPACE", tlg::KEY_BACK_SPACE),
        ("tsf::KEY_TAB", tlg::KEY_TAB),
        ("tsf::KEY_RETURN", tlg::KEY_RETURN),
        ("tsf::KEY_ESCAPE", tlg::KEY_ESCAPE),
        ("tsf::KEY_PAGE_UP", tlg::KEY_PAGE_UP),
        ("tsf::KEY_PAGE_DOWN", tlg::KEY_PAGE_DOWN),
        ("tsf::KEY_DELETE", tlg::KEY_DELETE),
        ("tsf::KEY_SHIFT", tlg::KEY_SHIFT),
        ("tsf::KEY_SPACE", tlg::KEY_SPACE),
        ("fcitx5::KEY_BACK_SPACE", fim::KEY_BACK_SPACE),
        ("fcitx5::KEY_TAB", fim::KEY_TAB),
        ("fcitx5::KEY_RETURN", fim::KEY_RETURN),
        ("fcitx5::KEY_ESCAPE", fim::KEY_ESCAPE),
        ("fcitx5::KEY_PAGE_UP", fim::KEY_PAGE_UP),
        ("fcitx5::KEY_PAGE_DOWN", fim::KEY_PAGE_DOWN),
        ("fcitx5::KEY_SHIFT_L", fim::KEY_SHIFT_L),
        ("fcitx5::KEY_SHIFT_R", fim::KEY_SHIFT_R),
        ("fcitx5::KEY_DELETE", fim::KEY_DELETE),
    ];
    v.sort_by_key(|(_, c)| *c);
    v
}

/// 标签 `轨道::常量名` → 轨道（没有 `::` 时原样返回）。
fn track_of(label: &str) -> &str {
    label.split_once("::").map_or(label, |(t, _)| t)
}

/// 三套键码空间的**源文件**（编译期读入：路径写错 = 编译不过，不会静默失明）。
///
/// 两轨的键码是**手抄**的另一套空间（理由见 `keycode_contract.rs` 的
/// `coverage_boundaries_are_honest` 第 3 条），所以各自那份文件就是它们的真源。
/// 本门禁问的**不是**「抄得对不对」（那归 `two_track_keycodes.rs` 与
/// `keycode_contract.rs`），而是**上面那份清单全不全**。
const SOURCES: &[(&str, &str)] = &[
    ("engine_core", include_str!("../../engine-core/src/keys.rs")),
    (
        "tsf",
        include_str!("../../tsf-opi/src/logic_input_method.rs"),
    ),
    (
        "fcitx5",
        include_str!("../../fcitx5-opi/src/input_method.rs"),
    ),
];

/// 允许落在可打印 ASCII 段的键码：**只有空格**，而且是刻意的 —— 它是字符键，值就是 `0x20`。
///
/// 这是一张**点名**的白名单，不是「值恰好可打印就放行」：后者会让历史上那类撞号
/// （TSF 的 `VK_DELETE = 0x2E` 撞 `.`）自动豁免，门禁当场失效。
const PRINTABLE_BY_DESIGN: &[&str] = &["engine_core::KEY_SPACE", "fcitx5::KEY_SPACE"];

/// 轨道源码里的**键码名** —— 由 `syn` 的解析器给，不再手写扫描状态机。
///
/// 为什么换掉手写状态机（第四轮的账）：状态机要自己对「哪些字符算代码」建模，而输入
/// 字母表里有它没建模的成员，四轮补丁都在这上面打转 ——
/// `'"'`（字符字面量里的引号把「在字符串里」的奇偶**反转**，吃掉后续整行乃至全文 ⇒
/// 后面的真声明**静默漏掉**）、`r#"…"#`（裸字符串里的 `"` 提前收尾）、`/* /* */ */`
/// （块注释可嵌套）。而每轮补丁自己又会引入新的**假红**（注释掉的旧代码、断言消息里的
/// 散文）—— 假红比漏放更逼人犯错：「改到不红为止」的下一步是把**不存在的常量**登记进
/// `all_special_codes()`，那张表没有任何东西回头看它。
/// 解析器把这类问题**整类**消灭：它读 token 与 AST，字符串/注释/字符字面量在那里
/// 根本不是标识符，「我漏建模了哪种写法」这一问不存在了。
///
/// 收两条，都是**结构**（与写法无关 —— 带不带属性、`pub` 不 `pub`、一行几条、在不在
/// `mod` 或函数体里，全都不影响）：
/// 1. **声明**：任何 `ItemConst` / `ImplItemConst`，名字 `KEY_*` 且不是 `KEY_STATE_*`。
/// 2. **宏 token 流里的裸名字**：参数化宏
///    （`macro_rules! { ($n:ident, $v:expr) => { pub const $n: u32 = $v; }; }` 加一次调用）
///    就是这样引入常量名的，AST 里没有它的声明可看，只能在调用处认这个名字。
///    **限定路径不算**：`engine_core::keys::KEY_LEFT` 是**别处**的常量，记到本轨头上
///    会逼着人往手写表里塞假条目。
///
/// 边界**按名划定**（与写法无关）：`VK_*`（Win32 虚拟键码，`tsf-opi/src/vk.rs` 注明与
/// 引擎键码「同值不同空间」）与 `SPECIAL_KEYS` 这类聚合数组名都不收；`KEY_STATE_*` 不收 ——
/// 那类有另一道执行点：`two_track_keycodes.rs` 的非空转护栏（实测把 `KEY_STATE_ZZZ`
/// 加进 tsf 真源，那道门禁 EXIT=101，本门禁此时 GREEN 是设计内）。
fn key_names_in_code(src: &str) -> Vec<String> {
    let file = syn::parse_file(src).unwrap_or_else(|e| {
        panic!("轨道源码 syn 解析失败：{e} —— 是判据坏了，别改成「解析不过就跳过」")
    });
    let mut v = KeyNames::default();
    syn::visit::Visit::visit_file(&mut v, &file);
    v.names.sort();
    v.names.dedup();
    v.names
}

/// 走 `syn` 的完整遍历：顶层、`mod` 内容、函数体、`impl` 里的常量都会走到。
#[derive(Default)]
struct KeyNames {
    names: Vec<String>,
}

fn keep_if_key_code(ident: &syn::Ident, out: &mut Vec<String>) {
    let n = ident.to_string();
    if n.starts_with("KEY_") && !n.starts_with("KEY_STATE_") {
        out.push(n);
    }
}

impl<'ast> syn::visit::Visit<'ast> for KeyNames {
    fn visit_item_const(&mut self, c: &'ast syn::ItemConst) {
        keep_if_key_code(&c.ident, &mut self.names);
    }

    fn visit_impl_item_const(&mut self, c: &'ast syn::ImplItemConst) {
        keep_if_key_code(&c.ident, &mut self.names);
    }

    fn visit_macro(&mut self, m: &'ast syn::Macro) {
        scan_macro_tokens(&m.tokens, &mut self.names);
        syn::visit::visit_macro(self, m);
    }
}

/// 宏 token 流里的裸 `KEY_*` 名字。**`Group` 要进去走**：宏体整个活在一个花括号 Group 里
/// （`macro_rules! zz { () => { pub const KEY_X: u32 = 1; }; }` —— 只看顶层会整个漏掉，
/// 实测过：那形状在只走顶层的版本上是 GREEN）。
/// 只看裸名字：紧跟在 `::` 后面的标识符是**路径尾段**，指向别处的常量；
/// 进 Group 时 `::` 判定重置（`vec![a::b::KEY_X]` 里的 `KEY_X` 仍是路径尾段）。
fn scan_macro_tokens(ts: &proc_macro2::TokenStream, out: &mut Vec<String>) {
    let mut after_colon = false;
    for tt in ts.clone() {
        match &tt {
            proc_macro2::TokenTree::Ident(id) if !after_colon => keep_if_key_code(id, out),
            proc_macro2::TokenTree::Group(g) => scan_macro_tokens(&g.stream(), out),
            _ => {}
        }
        after_colon = matches!(&tt, proc_macro2::TokenTree::Punct(p) if p.as_char() == ':');
    }
}

/// 不变量 1：每个特殊键码都 > 0x7f，且能解码成非 ASCII 字符。
///
/// 「能解码」不是形式主义：漏了基址/越界的键码会落进 `char::from_u32 → None`
/// 那一支，行为是「静默忽略」而不是报错 —— 键按下去了没反应，最难查的一类。
#[test]
fn special_codes_stay_out_of_the_printable_ascii_range() {
    let codes = all_special_codes();
    // 条数**不在这里钉**（原来钉的是 `assert_eq!(codes.len(), 30)` —— 那正是让这份
    // 清单看起来「已经守住了」的东西）。清单全不全由
    // [`every_key_code_const_in_the_three_tracks_is_registered_here`] 对着三份源文件
    // 双向核 —— 引门禁，不引数字。
    for (who, code) in &codes {
        assert!(
            *code > 0x7f,
            "{who} 的特殊键码 {code:#x} 落进 ASCII 段 —— 普通字符会被吃掉"
        );
        let c = char::from_u32(*code).unwrap_or_else(|| {
            panic!("{who} 的特殊键码 {code:#x} 不可解码，漏 match 臂时会静默忽略")
        });
        assert!(
            !c.is_ascii(),
            "{who} 的特殊键码 {code:#x} 解码成 ASCII 字符 {c:?}：落到可打印分支会变成垃圾字符"
        );
    }
    // 反过来问一遍（这才是「不相交」的正面表述）：**任何**可打印 ASCII 都不等于特殊键码。
    for c in 0x20u32..=0x7e {
        for (who, code) in &codes {
            assert_ne!(
                c, *code,
                "可打印 ASCII {c:#x} 撞上 {who} 的特殊键码 —— 敲这个字符会触发特殊行为"
            );
        }
    }
    // 同一套空间内部也不许重复（两个键码相同 → 一个键被另一个顶掉）。
    // ⚠️ 判「同一套」必须用 `track_of` 剥掉 `::常量名`：标签带上常量名之后，
    // **同轨两条的标签也不再相等** —— 直接比标签等于把这段静默弄失效。
    for i in 0..codes.len() {
        for j in i + 1..codes.len() {
            if track_of(codes[i].0) == track_of(codes[j].0) {
                assert_ne!(
                    codes[i].1, codes[j].1,
                    "{} 与 {} 的键码撞成同一个数字：{:#x}",
                    codes[i].0, codes[j].0, codes[i].1
                );
            }
        }
    }
}

fn to_units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn read(s: OpiString) -> String {
    let out = if s.ptr.is_null() {
        String::new()
    } else {
        let units = unsafe { std::slice::from_raw_parts(s.ptr, s.len) };
        String::from_utf16(units).expect("导出必须返回合法 UTF-16")
    };
    unsafe { opi_ffi_free_string(s) };
    out
}

/// 不变量 2（C ABI 面）：**字符入口永不触发特殊键**。
///
/// 把每一个特殊键码当成**字符**送进 `opi_input_key`，缓冲必须一字不变 ——
/// 这些码点（`0x1_0008`…`0x1_0087`）都是合法的补充平面字符，与 `SPECIAL_BASE|VK`
/// 数值完全相同，所以这条是「同一个数字、两个入口、含义必须分开」的实测。
///
/// 正控在末尾：同一个数字走**键事件**入口必须真的删掉一个字符 ——
/// 没有它，「字符入口什么都不做」可能只是因为这个数字压根没被处理。
#[test]
fn the_character_entry_point_never_triggers_a_special_key() {
    let path = to_units(LUNA);
    assert!(unsafe { opi_load(path.as_ptr(), path.len()) }, "装载 luna");
    unsafe {
        opi_set_learner(false);
        opi_switch_mode(0);
        opi_clear();
    }
    for c in "ni".chars() {
        let one = to_units(&c.to_string());
        read(unsafe { opi_input_key(one.as_ptr(), one.len()) });
    }
    assert_eq!(read(unsafe { opi_buffer() }), "ni", "前置：拼音缓冲已建立");
    let cands = read(unsafe { opi_candidates(8) });
    assert!(!cands.is_empty(), "前置：有候选：{cands}");

    for (who, code) in all_special_codes() {
        let c = char::from_u32(code).expect("特殊键码是合法字符");
        let one = to_units(&c.to_string());
        let out = read(unsafe { opi_input_key(one.as_ptr(), one.len()) });
        assert_eq!(
            out, "",
            "{who} 的键码 {code:#x}（= 字符 {c:?}）从**字符**入口进来却提交了文本"
        );
        assert_eq!(
            read(unsafe { opi_buffer() }),
            "ni",
            "{who} 的键码 {code:#x}（= 字符 {c:?}）从字符入口改动了缓冲 —— 特殊键与可打印段撞号"
        );
        assert_eq!(
            read(unsafe { opi_candidates(8) }),
            cands,
            "{who} 的键码 {code:#x}（= 字符 {c:?}）从字符入口动了候选"
        );
    }

    // 正控：同一个数字（engine-core 的退格）走**键事件**入口必须真的删。
    let r = unsafe { opi_ffi::cabi::opi_key_event(ek::KEY_BACK_SPACE, 0) };
    assert_eq!(r.action, 1, "键事件入口的退格必须被引擎接管");
    assert_eq!(read(unsafe { opi_buffer() }), "n", "退格必须删掉一个码点");
}

/// 不变量 3（fcitx5 轨）：`opi_fcitx5_input_key` 的**文档契约**是
/// 「空串/多字符/**非 ASCII** → 空串」（`crates/fcitx5-opi/src/lib.rs` 该函数头注释）。
/// 实现却是 `handle_key(s, c as u32, 0)`：键值空间按**码点**解释，于是
/// U+FF00–U+FFFF 里那 9 个「数值恰好等于某个 keysym」的字符会被当成控制键。
///
/// 最小复现（本机实证）：
/// ```text
/// （先装好引擎、敲 n i 建出缓冲）
/// opi_fcitx5_input_key("－")   // U+FF0D，数值 = KEY_RETURN(0xff0d)
///   → 返回候选文本（≠ 空串，违反「非 ASCII → 空串」），且缓冲被提交清空
/// opi_fcitx5_input_key("（")   // U+FF08，数值 = KEY_BACK_SPACE(0xff08) → 删掉一个码点
/// ```
/// 受影响的字符（数值 = keysym 者）：`（` `）` `－` `；` `ｕ` `ｖ` `￡` `￠` `U+FFFF`。
/// 其中 `￡`（U+FFE1，全角英镑符）与 `ｕ`/`ｖ`（半角拉丁字母）是真实文本里会出现的字符。
///
/// **可达性**：本导出声明在 `crates/fcitx5-opi/cpp/opi_fcitx5.cpp:50`，但出货的
/// C++ 胶水**没有调用它**（按键走的是 `keyEvent` 那条路）。所以今天是潜在缺陷、
/// 不是线上故障 —— 但它是 `.so` 的公开导出，第三方或将来改胶水时按文档调用即中招。
/// 根因是「键值 = 码点」这个假设只在 ASCII 段成立（真实 fcitx5 的非 ASCII
/// keysym 是 `0x0100_0000 | 码点`）。
///
/// **已修**：入口加 `if !c.is_ascii() { return 空串 }`（贴文档那条；另一条「改按
/// keysym 解释」是**反向改契约**且牵动真实前端，未采纳）。改的是导出壳，不动
/// `input_method::handle_key`，故两轨行为对照不受影响。
#[test]
fn fcitx5_char_entry_must_not_treat_fullwidth_text_as_control_keys() {
    fcitx5_opi::install(Some(LUNA)).expect("装载 luna");
    let feed = |ch: char| -> String {
        let s = ch.to_string();
        let out = unsafe { opi_fcitx5_input_key(s.as_ptr(), s.len()) };
        let text = if out.ptr.is_null() {
            String::new()
        } else {
            String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(out.ptr, out.len) })
                .into_owned()
        };
        unsafe { opi_ffi_free_string_utf8(out) };
        text
    };
    let buffer = || -> String {
        let b = unsafe { opi_fcitx5_buffer() };
        let s = String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(b.ptr, b.len) })
            .into_owned();
        unsafe { opi_ffi_free_string_utf8(b) };
        s
    };

    for c in "ni".chars() {
        assert_eq!(feed(c), "", "ASCII 字母入缓冲，不提交");
    }
    assert_eq!(buffer(), "ni", "前置：拼音缓冲已建立");

    // U+FF0D 的数值等于 KEY_RETURN(0xff0d)。
    let before = buffer();
    assert_eq!(
        feed('\u{ff0d}'),
        "",
        "文档：非 ASCII → 空串。现值回来了「提交文本」——它被当成了回车"
    );
    assert_eq!(
        buffer(),
        before,
        "文档：非 ASCII 不该改状态。现值把缓冲提交清空了（被当成回车）"
    );

    // 其余 8 个数值等于 keysym 的字符：一个都不许动状态。
    for (c, why) in [
        ('\u{ff08}', "= KEY_BACK_SPACE(0xff08)"),
        ('\u{ff09}', "= KEY_TAB(0xff09)"),
        ('\u{ff1b}', "= KEY_ESCAPE(0xff1b)"),
        ('\u{ff55}', "= KEY_PAGE_UP(0xff55)"),
        ('\u{ff56}', "= KEY_PAGE_DOWN(0xff56)"),
        ('\u{ffe1}', "= KEY_SHIFT_L(0xffe1)"),
        ('\u{ffe2}', "= KEY_SHIFT_R(0xffe2)"),
        ('\u{ffff}', "= KEY_DELETE(0xffff)"),
    ] {
        let before = buffer();
        assert_eq!(feed(c), "", "{c:?}（{why}）是非 ASCII，文档说返回空串");
        assert_eq!(buffer(), before, "{c:?}（{why}）不得改动缓冲");
    }
}

/// **这份清单全不全** —— 逐条对着三套空间的源文件核，而不是靠上面那 30 行手抄。
///
/// 为什么需要它（2026-09-28 审计实测）：`special_codes_stay_out_of_the_printable_ascii_range`
/// **看起来**在守「三套空间的特殊键码都不落进 ASCII 段」（名字与断言都是全称口气），
/// 实际守的是 `all_special_codes()` 里手抄的那几条。往
/// `crates/tsf-opi/src/logic_input_method.rs` 加一行
/// `pub const KEY_ZZZ_AUDIT: u32 = 0x1f;`（正落在 ASCII 段、正违反那条断言），
/// `cargo test --workspace --no-fail-fast` 是 **EXIT=0 全绿** —— 新常量不进表、也就不查。
/// 而那正是这条门禁声称在防的事（TSF 那次 `0x2E = '.'` 的病根）。
///
/// 本用例把清单与源文件**双向**钉住：源里新增键码 ⇒ 红并点名；清单里有源里没有的名字
/// （改名 / 删除 / 拼错）⇒ 也红。修法是**登记**，不是把断言口径放宽。
#[test]
fn every_key_code_const_in_the_three_tracks_is_registered_here() {
    // 白名单里的「值」也钉住：它们是可打印段的**唯一**合法住户。
    assert_eq!(ek::KEY_SPACE, 0x20, "engine_core 的空格是字符键");
    assert_eq!(fim::KEY_SPACE, 0x20, "fcitx5 的空格是字符键");

    let registered: Vec<&str> = all_special_codes().iter().map(|(n, _)| *n).collect();

    // 方向 1：源里每个键码常量都必须登记（或进白名单）。
    let mut unregistered = Vec::new();
    for (track, src) in SOURCES {
        let names = key_names_in_code(src);
        assert!(
            !names.is_empty(),
            "{track} 一个键码名都没扫到 —— 扫描失明，下面的「全覆盖」不可信"
        );
        for n in names {
            let full = format!("{track}::{n}");
            if !registered.contains(&full.as_str()) && !PRINTABLE_BY_DESIGN.contains(&full.as_str())
            {
                unregistered.push(full);
            }
        }
    }
    assert!(
        unregistered.is_empty(),
        "这些键码名在轨道源码（已剔除字符串与注释）里出现、但没登记进 `all_special_codes()`：{unregistered:?}\n\
         （登记后它才会被上面那条的 `>0x7f` 断言查到；确属字符键的进 `PRINTABLE_BY_DESIGN`）"
    );

    // 方向 2：登记了但源里找不到（改名 / 删除 / 拼错）。
    let mut stale: Vec<&str> = Vec::new();
    for name in &registered {
        let (track, cnst) = name.split_once("::").expect("登记名形如 轨道::常量");
        let (_, src) = SOURCES
            .iter()
            .find(|(t, _)| *t == track)
            .unwrap_or_else(|| panic!("`{name}` 的轨道 `{track}` 不在 SOURCES 里"));
        if !key_names_in_code(src).iter().any(|n| n == cnst) {
            stale.push(*name);
        }
    }
    assert!(
        stale.is_empty(),
        "这些登记项在源文件里找不到同名常量：{stale:?}（改名了？还是拼错了？）"
    );

    println!(
        "[键码空间] 三套源文件共 {} 个键码常量，全部对得上这份清单（另有 {} 条白名单）",
        registered.len() + PRINTABLE_BY_DESIGN.len(),
        PRINTABLE_BY_DESIGN.len()
    );
}
