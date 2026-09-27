// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **`# Safety` 段的执行点**：`cabi.rs` 里带**裸指针 / 所有权句柄**入参的导出，
//! 前置条件必须写在**函数自己**的文档块里。
//!
//! 为什么要有这道门禁：`cabi.rs` 顶上有一条 `#![allow(clippy::missing_safety_doc)]`
//! （为压掉二十多条逐字重复的句子，见那里的说明）。它按**模块**生效，所以 clippy 的
//! 原生 lint 对本模块**将来新增的每一个导出**都不再说话 —— 新加一个 `ptr`/`len` 出口
//! 而忘了写 `# Safety`，构建与 clippy 全绿（本轮审计实测：去掉那条 `allow` 后同一形状
//! 的模块 EXIT=101，加回 EXIT=0）。本文件是那道 lint 的**替代执行点**：判据从
//! `cabi.rs` 的**源码文本**算出来，对**新导出自动生效**，不需要作者记得加任何属性。
//!
//! 判据**恰好**是「有外部内存参数」那一档（见 [`takes_unsafe_arg`]）：纯标量入参的
//! 导出**不**要求 —— 它们的前置条件统一写在模块头，逐条要求只会变成没人看的噪音门禁。
//!
//! 同一条理由也适用于模块头那句「**每个**函数都以 `catch_unwind` 包裹」：本轮审计抓到它
//! 与同文件代码相反（34 个导出里只有 33 处包装，`opi_ffi_free_string` 是例外）。散文里的
//! 全称句无从变异，所以那句话也被钉成下面的断言 —— 谁再动这批出口的包装，先红再说。
//!
//! 两个方向都要验（本项目教训：全绿只证明被断言的部分）：缺段的指针导出**必须**被
//! 报出来（否则是失明），有段的指针导出与无指针导出**不许**被报（否则是噪音）。

/// 本目标只用其中几个函数，其余条目天然是死代码（照 keycode_contract.rs 的写法）。
#[allow(dead_code)]
#[path = "c_abi/checker.rs"]
mod checker;

use checker::*;

// ---------- 机械部分（只有本目标用，故不放进共享的 `c_abi/checker.rs`：
// 放进那里会让其余「包含 checker 但用不到这几个函数」的测试目标吃到 dead_code） ----------

/// 该签名是否**必须自带** `# Safety` 段：入参里有裸指针，或有所有权句柄
/// （`OpiString` —— 调用方把所有权交给 Rust 侧的那种，`opi_ffi_free_string` 收的就是它）。
/// 纯标量/布尔入参的导出**不**在此列 —— 它们的前置条件统一写在模块头，逐条要求只会
/// 变成噪音门禁（没人看，等于没有）。
fn takes_unsafe_arg(sig: &Sig) -> bool {
    sig.args.iter().any(|a| a.contains('*') || a == "OpiString")
}

/// 每个导出 → 紧邻其上的 `///` 文档块（**只认紧邻**：中间只许隔属性行
/// `#[unsafe(no_mangle)]`，遇空行/别的代码即止 —— 否则会把上一个函数的文档算过来，
/// 变成「只要文件里某处写过 `# Safety` 就算覆盖」的假绿）。
fn doc_blocks(src: &str) -> Vec<(String, String)> {
    const NEEDLE: &str = "pub unsafe extern \"C\" fn ";
    let mut out = Vec::new();
    for (off, _) in src.match_indices(NEEDLE) {
        let rest = &src[off + NEEDLE.len()..];
        let Some(paren_rel) = rest.find('(') else {
            continue;
        };
        let name = rest[..paren_rel].trim().to_string();
        let mut doc: Vec<&str> = Vec::new();
        for line in src[..off].lines().rev() {
            let t = line.trim();
            if let Some(d) = t.strip_prefix("///") {
                doc.push(d);
            } else if t.starts_with("#[") || t.starts_with("#!") {
                // 属性行夹在文档块与函数之间，继续往上找。
            } else {
                break;
            }
        }
        doc.reverse();
        out.push((name, doc.join("\n")));
    }
    out
}

/// 带指针/句柄入参、却**没在自己的文档块里**写 `# Safety` 的导出名（空 = 契约成立）。
///
/// 判据是「有一行**以** `# Safety` 开头」（rustdoc 的段落标题写法），**不是**「出现过
/// 这个子串」—— 后者会让「本函数没有 `# Safety` 段」这种散文也算作有，门禁当场失效。
/// **不吃文件**：喂字符串，故自检可以喂故意改坏的语料。
fn exports_missing_safety_doc(src: &str) -> Vec<String> {
    let docs = doc_blocks(src);
    parse_rust_exports(src)
        .into_iter()
        .filter(|(_, sig)| takes_unsafe_arg(sig))
        .filter(|(name, _)| {
            !docs.iter().any(|(n, d)| {
                n == name && d.lines().any(|l| l.trim_start().starts_with("# Safety"))
            })
        })
        .map(|(name, _)| name)
        .collect()
}

/// 每个导出名 → 它的**函数实现切片**（从 `fn` 名到下一个导出之前）—— 「实现里包没包
/// `catch_unwind`」这类**实现**性质的判据（契约面只看声明，用 `parse_rust_exports`）。
fn export_bodies(src: &str) -> Vec<(String, String)> {
    const NEEDLE: &str = "pub unsafe extern \"C\" fn ";
    let hits: Vec<usize> = src.match_indices(NEEDLE).map(|(o, _)| o).collect();
    let mut out = Vec::new();
    for (i, &off) in hits.iter().enumerate() {
        let end = hits.get(i + 1).copied().unwrap_or(src.len());
        let rest = &src[off + NEEDLE.len()..];
        let Some(paren_rel) = rest.find('(') else {
            continue;
        };
        out.push((
            rest[..paren_rel].trim().to_string(),
            src[off..end].to_string(),
        ));
    }
    out
}

/// 合成语料（**不读仓里的活文件**）：一条该报的 + 三条不该报的。
/// `opi_bad_ptr` 的文档**故意在散文里写了 `# Safety` 三个字**（「**没有** `# Safety` 段」）：
/// 判据必须是「有 `# Safety`**开头的行**」，不是「出现过这个子串」，否则提一句就算覆盖。
const FIXTURE: &str = r#"
/// 带指针、**有** `# Safety` 段。
/// # Safety
/// `p` 必须指向至少 `n` 个有效 `u16`。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_ok_ptr(p: *const u16, n: usize) -> bool { true }

/// 收所有权句柄、**有**段。
/// # Safety
/// `s` 必须是本库分配且未释放过的句柄。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_ok_handle(s: OpiString) {}

/// 带指针、**没有** `# Safety` 段 —— 必须被报出来。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_bad_ptr(p: *const u16, n: usize) -> bool { true }

/// 纯标量入参（前置条件在模块头）—— **不许**被报。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_plain(mode: i32) -> u32 { 0 }
"#;

/// 检查器自检：**只**报 `opi_bad_ptr`。少报 = 失明，多报 = 噪音门禁 —— 两个方向都在这
/// 一条断言里。它同时钉住了「只认紧邻文档块」：若向上抓过头，`opi_bad_ptr` 会继承上一条
/// 的 `# Safety` 而漏报，这里也会红。
#[test]
fn safety_doc_checker_reports_exactly_the_bare_pointer_export() {
    assert_eq!(
        exports_missing_safety_doc(FIXTURE),
        vec!["opi_bad_ptr".to_string()],
        "只该报出没有 `# Safety` 段的 `opi_bad_ptr`（少报/多报都是坏门禁）"
    );
    // 非空转护栏：上面那条若因解析器失明而比的是两个空列表就会假绿，
    // 故把「解析出几条 / 判据认出几条」单独钉住。
    assert_eq!(
        parse_rust_exports(FIXTURE).len(),
        4,
        "语料只该解析出 4 个导出（解析器失明？）"
    );
    let armed = parse_rust_exports(FIXTURE)
        .iter()
        .filter(|(_, s)| takes_unsafe_arg(s))
        .count();
    assert_eq!(
        armed, 3,
        "语料里带指针/句柄的应是 3 条 —— 判据失明则上面的空列表无意义"
    );
}

/// 真门禁：`cabi.rs` 那批指针/句柄导出，每一条都必须自带 `# Safety`。
#[test]
fn every_cabi_export_with_raw_args_documents_its_preconditions() {
    let src = read(CABI);
    let parsed = parse_rust_exports(&src);
    // 解析器失明 => 下面的空列表是假绿。锚点取本门禁判据面上的那几条。
    for anchor in [
        "opi_load",
        "opi_input_key",
        "opi_import_user_words",
        "opi_ffi_free_string",
    ] {
        assert!(
            parsed.iter().any(|(n, _)| n == anchor),
            "解析器没解析出锚点 `{anchor}` —— 失明，其余断言不可信"
        );
    }
    // 兜底下限（照 line_limit.rs 的写法）：只抓「判据整个坏掉」，不表达当前条数。
    let armed = parsed.iter().filter(|(_, s)| takes_unsafe_arg(s)).count();
    assert!(
        armed >= 6,
        "只认出 {armed} 个带裸指针/句柄入参的导出 —— 判据失明，不是导出真的变少了"
    );

    let missing = exports_missing_safety_doc(&src);
    assert!(
        missing.is_empty(),
        "这些导出有裸指针/句柄入参，却没在自己的文档块里写 `# Safety`：{missing:?}\n\
         （模块头的 `allow` 关掉了 clippy 这条 lint，本用例是唯一会红的地方）"
    );
    println!("[契约] cabi.rs：{armed} 个带裸指针/句柄入参的导出全部自带 `# Safety` 段");
}

/// 模块头那句「除 `opi_ffi_free_string` 外每个函数都以 `catch_unwind` 包裹」的**执行点**：
/// 不包装的导出必须**恰好**是它。散文改不动，代码改得动 —— 所以把事实钉在这里，
/// 模块头那句只负责指向本用例。
///
/// ⚠️ 本用例**证明不了**「有人偷偷把散文改成假话」（那要靠读文档）；它证明的是**代码**这一侧：
/// 新增一个不包装的出口、或哪个出口漏了包装 ⇒ 红。
#[test]
fn the_catch_unwind_exception_is_exactly_the_documented_one() {
    const DOCUMENTED_EXCEPTION: &str = "opi_ffi_free_string";
    let unwrapped: Vec<String> = export_bodies(&read(CABI))
        .into_iter()
        .filter(|(_, body)| !body.contains("catch_unwind("))
        .map(|(n, _)| n)
        .collect();
    assert_eq!(
        unwrapped,
        vec![DOCUMENTED_EXCEPTION.to_string()],
        "模块头写的是「除 `{DOCUMENTED_EXCEPTION}` 外每个导出都包 `catch_unwind`」——\
         实际没包装的不止/不是它"
    );
}
