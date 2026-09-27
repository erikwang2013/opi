// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 契约检查器的**机械**部分：两侧解析成规范形 + 逐条比对。
//!
//! 单独一个文件是因为 500 行硬规矩：`c_abi_contract.rs` 装用例、本文件装解析与比对。
//! `mod checker;` 不会被当独立测试目标编译，所以这里没有 `#[test]`。

use std::fs;

// ---------- 规范形 ----------

/// 一个函数的规范签名：`(参数类型列表, 返回类型)`，类型已折算成同一套拼写。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sig {
    pub args: Vec<String>,
    pub ret: String,
}

/// C 类型 → 规范形（去掉全部空白；`const uint16_t *` → `constuint16_t*`）。
pub fn canon_c(t: &str) -> String {
    t.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Rust 类型 → 与 [`canon_c`] 同一套拼写。
pub fn canon_rust(t: &str) -> String {
    let t: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    let mapped = match t.as_str() {
        "u8" => "uint8_t",
        "i8" => "int8_t",
        "u16" => "uint16_t",
        "i16" => "int16_t",
        "u32" => "uint32_t",
        "i32" => "int32_t",
        "u64" => "uint64_t",
        "i64" => "int64_t",
        "usize" => "size_t",
        "isize" => "ptrdiff_t",
        "bool" => "bool",
        "()" => "void",
        "*constu16" => "constuint16_t*",
        other => other,
    };
    mapped.to_string()
}

// ---------- 解析：Rust 源码 ----------

/// 剥掉 `//` 行注释与 `/* */` 块注释（引号内的不处理 —— 本仓库的头文件里
/// 没有含注释符的字符串字面量，解析器失明时会由护栏测试报红）。
pub fn strip_c_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let b = src.as_bytes();
    let (mut i, mut block) = (0usize, false);
    while i < b.len() {
        if block {
            if b[i] == b'*' && i + 1 < b.len() && b[i + 1] == b'/' {
                block = false;
                i += 2;
                continue;
            }
        } else if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
            block = true;
            i += 2;
            continue;
        } else if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        } else {
            out.push(b[i] as char);
        }
        i += 1;
    }
    out
}

/// 从 `text` 的 `open` 位置（指向 `(`）取到配对 `)` 之间的内容。
pub fn balanced(text: &str, open: usize) -> Option<(String, usize)> {
    let b = text.as_bytes();
    debug_assert_eq!(b[open], b'(');
    let mut depth = 0i32;
    for (i, c) in b.iter().enumerate().skip(open) {
        match c {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((text[open + 1..i].to_string(), i + 1));
                }
            }
            _ => {}
        }
    }
    None
}

/// 顶层逗号切分（`*const u16, usize` → 两段；`f(a, b)` 只在本文件里不出现）。
pub fn split_args(s: &str) -> Vec<String> {
    let t = s.trim();
    if t.is_empty() || t == "void" {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    for c in t.chars() {
        match c {
            '(' | '[' | '<' => depth += 1,
            ')' | ']' | '>' => depth -= 1,
            ',' if depth == 0 => {
                out.push(std::mem::take(&mut cur));
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    out.push(cur);
    out
}

/// 解析 `pub unsafe extern "C" fn name(args) -> ret` 。
/// 返回 `(名字, 签名)` 列表。`#[unsafe(no_mangle)]` 的计数由护栏测试另行核对。
pub fn parse_rust_exports(src: &str) -> Vec<(String, Sig)> {
    const NEEDLE: &str = "pub unsafe extern \"C\" fn ";
    let mut out = Vec::new();
    for (off, _) in src.match_indices(NEEDLE) {
        let rest = &src[off + NEEDLE.len()..];
        let Some(paren_rel) = rest.find('(') else {
            continue;
        };
        let name = rest[..paren_rel].trim().to_string();
        let open = off + NEEDLE.len() + paren_rel;
        let Some((arg_text, after)) = balanced(src, open) else {
            continue;
        };
        let tail = src[after..].trim_start();
        let ret = match tail.strip_prefix("->") {
            Some(r) => {
                let end = r.find('{').expect("返回类型后应有 `{`");
                canon_rust(r[..end].trim())
            }
            // 无 `->` 即返回 `()` —— C 侧 `void`。
            None => "void".to_string(),
        };
        let args = split_args(&arg_text)
            .into_iter()
            .map(|a| {
                let (_, ty) = a
                    .split_once(':')
                    .unwrap_or_else(|| panic!("参数 `{}` 缺少 `: 类型`（解析失明？）", a.trim()));
                canon_rust(ty)
            })
            .collect();
        out.push((name, Sig { args, ret }));
    }
    out
}

/// 从 Rust 源码里取 `#[repr(C)] pub struct NAME { pub f: T, ... }` 的字段类型列表。
pub fn parse_rust_struct(src: &str, name: &str) -> Vec<String> {
    let head = format!("pub struct {name} {{");
    let start = src
        .find(&head)
        .unwrap_or_else(|| panic!("cabi.rs 里找不到 `{head}`"));
    let body_start = start + head.len();
    let end = src[body_start..].find("\n}").expect("结构体应有收尾 `}`");
    let mut fields = Vec::new();
    for line in src[body_start..body_start + end].lines() {
        let line = line.trim();
        // 跳过 `///` 文档行与空行；字段行形如 `pub ptr: *const u16,`
        let Some(rest) = line.strip_prefix("pub ") else {
            continue;
        };
        let Some((fname, ty)) = rest.split_once(':') else {
            continue;
        };
        let ty = ty.trim().trim_end_matches(',');
        fields.push(format!("{} {}", fname.trim(), canon_rust(ty)));
    }
    fields
}

// ---------- 解析：C 头文件 ----------

/// 剥掉预处理行（`#pragma` / `#include` / `#define` …）与注释，留下声明与 typedef。
pub fn c_decl_body(src: &str) -> String {
    strip_c_comments(src)
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 解析 `typedef struct { T f; ... } NAME;` → 字段规范类型列表（保序）。
pub fn parse_c_typedef_struct(src: &str, name: &str) -> Vec<String> {
    let body = c_decl_body(src);
    let needle = format!("}} {name};");
    let end = body
        .find(&needle)
        .unwrap_or_else(|| panic!("头文件里找不到 `typedef struct ... }} {name};`"));
    let start = body[..end].rfind("typedef struct {").expect("typedef 起始");
    let inner = &body[start + "typedef struct {".len()..end];
    let mut fields = Vec::new();
    // 字段以 `;` 分隔；每段形如 `const uint16_t *ptr`。
    for chunk in inner.split(';') {
        let cur = chunk.trim();
        if cur.is_empty() {
            continue;
        }
        let (fname, ty) = split_c_declarator(cur);
        assert!(!fname.is_empty(), "结构体字段解析不出名字：`{cur}`");
        fields.push(format!("{fname} {ty}"));
    }
    fields
}

/// 解析头文件里的函数声明 `RET NAME(args);`（只认 `opi_` 前缀，避免
/// `#define` 残留 / typedef 干扰）。返回 `(名字, 签名)` 列表。
pub fn parse_c_decls(src: &str) -> Vec<(String, Sig)> {
    let body = c_decl_body(src);
    let mut out = Vec::new();
    for stmt in body.split(';') {
        let Some(open) = stmt.find('(') else {
            continue;
        };
        if !stmt.trim_end().ends_with(')') {
            continue;
        }
        let Some((arg_text, _)) = balanced(stmt, open) else {
            continue;
        };
        let head = stmt[..open].split_whitespace().collect::<Vec<_>>();
        let [ret @ .., name] = head.as_slice() else {
            continue;
        };
        if !name.starts_with("opi_") || ret.is_empty() {
            continue;
        }
        // 名字与返回类型之间不能有别的标识符（防 `int foo bar(` 这类误吞）。
        let ret = canon_c(&ret.join(" "));
        let args = split_args(&arg_text)
            .into_iter()
            .map(|a| split_c_declarator(&a).1)
            .collect();
        out.push((name.to_string(), Sig { args, ret }));
    }
    out
}

pub fn is_ident(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// C 声明子 `const uint16_t *path` → (`path`, `constuint16_t*`)。
/// 无名参数（`uint32_t`）→ (`""`, `uint32_t`)：整串就是一个标识符，无可剥之名。
/// `*` 紧贴名字的写法（`*path`）要把 `*` 归到**类型**侧 —— 否则会解析出
/// `constuint16_t*path` 这种把参数名揉进类型的假签名（本解析器最初就栽在这）。
pub fn split_c_declarator(decl: &str) -> (String, String) {
    let a = decl.trim();
    let b = a.as_bytes();
    let mut i = b.len();
    while i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_') {
        i -= 1;
    }
    let name = &a[i..];
    if is_ident(name) && i > 0 && i < b.len() {
        // 名字前要有类型：往回跳过 `*` 与空白后仍应有内容。
        let mut j = i;
        while j > 0 && (b[j - 1] == b'*' || b[j - 1].is_ascii_whitespace()) {
            j -= 1;
        }
        if j > 0 {
            return (name.to_string(), canon_c(&a[..i]));
        }
    }
    (String::new(), canon_c(a))
}

// ---------- 检查器 ----------

/// 头文件 vs Rust 导出的**全部**差异（空 = 契约成立）。**不改文件**——
/// 吃字符串，故自检测试可以喂进去故意改坏的文本。
pub fn check_header(rust_src: &str, header_src: &str) -> Vec<String> {
    let rust = parse_rust_exports(rust_src);
    let c = parse_c_decls(header_src);
    let mut diffs = Vec::new();

    let rm: std::collections::BTreeMap<&str, &Sig> =
        rust.iter().map(|(n, s)| (n.as_str(), s)).collect();
    let cm: std::collections::BTreeMap<&str, &Sig> =
        c.iter().map(|(n, s)| (n.as_str(), s)).collect();

    for name in rm.keys() {
        if !cm.contains_key(name) {
            diffs.push(format!("头文件缺少 Rust 导出 `{name}`"));
        }
    }
    for name in cm.keys() {
        if !rm.contains_key(name) {
            diffs.push(format!(
                "头文件多出（或拼错）声明 `{name}`：cabi.rs 无此导出"
            ));
        }
    }
    for (name, rs) in &rm {
        let Some(cs) = cm.get(name) else { continue };
        if rs.ret != cs.ret {
            diffs.push(format!(
                "`{name}` 返回类型不一致：cabi.rs `{}` vs 头文件 `{}`",
                rs.ret, cs.ret
            ));
        }
        if rs.args.len() != cs.args.len() {
            diffs.push(format!(
                "`{name}` 参数个数不一致：cabi.rs {} 个 {:?} vs 头文件 {} 个 {:?}",
                rs.args.len(),
                rs.args,
                cs.args.len(),
                cs.args
            ));
            continue;
        }
        for (i, (ra, ca)) in rs.args.iter().zip(&cs.args).enumerate() {
            if ra != ca {
                diffs.push(format!(
                    "`{name}` 第 {} 个参数类型不一致：cabi.rs `{ra}` vs 头文件 `{ca}`",
                    i + 1
                ));
            }
        }
    }
    diffs
}

/// 结构体的字段规范形比对（`#[repr(C)]` 的字段顺序错了是**静默**读错位）。
pub fn check_struct(rust_src: &str, header_src: &str, name: &str) -> Vec<String> {
    let r = parse_rust_struct(rust_src, name);
    let c = parse_c_typedef_struct(header_src, name);
    let mut diffs = Vec::new();
    if r.len() != c.len() {
        diffs.push(format!(
            "`{name}` 字段个数不一致：cabi.rs {} vs 头文件 {}",
            r.len(),
            c.len()
        ));
        return diffs;
    }
    for (i, (a, b)) in r.iter().zip(&c).enumerate() {
        if a != b {
            diffs.push(format!(
                "`{name}` 第 {} 个字段不一致：cabi.rs `{a}` vs 头文件 `{b}`",
                i + 1
            ));
        }
    }
    diffs
}

// ---------- 读文件 ----------

pub const CABI: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/cabi.rs");
pub const MACOS_H: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../macos/OpiFFI.h");
pub const IOS_H: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../ios/OpiFFI.h");
pub const HARMONY_H: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../harmony/cpp/opi_ffi.h");
pub const HARMONY_BRIDGE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../harmony/cpp/napi_bridge.c"
);

pub fn read(p: &str) -> String {
    fs::read_to_string(p).unwrap_or_else(|e| panic!("读不到 {p}：{e}"))
}

pub fn report(diffs: Vec<String>) -> String {
    diffs.join("\n  ")
}
