// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **值契约**：`opi_switch_mode` / `opi_mode` 传递的那几个整数 ↔ 两侧的名字。
//!
//! 名字 + 类型签名门禁（`checker.rs`）对这里是**全盲**的：`switchMode(int)` 两侧都是
//! `(I)V`，把 `Traditional` 写成 `1` 也照样编译、照样注册、照样绿灯 —— **只会静默
//! 把「繁体」显示成「英文」**。所以整数参数的**取值约定**必须单独一条门禁。
//!
//! 陷阱是**声明序**：Rust `Mode` 的声明序是 `Pinyin, Traditional, English, Number,
//! Symbol`（照它推会得到 Traditional=1），Kotlin `EngineMode` 的声明序恰好与编码一致
//! （照它推是对的）。两侧注释都写着「错了不会编译失败」—— 这正是要机械锁住的原因：
//! **一条门禁比两处注释可靠。**
//!
//! ⚠️ 有一条**语义差异**故意不判红：Kotlin `fromInt` 越界回退 `PINYIN`，Rust
//! `mode_from_int` 越界给 `None`（`switchMode` 越界忽略）。两者不可达 —— 两侧调用点
//! 传的都只可能是引擎产出的 0..=4。写在这里是为了下一个人别再查一遍。

use std::collections::BTreeMap;

use super::checker::{read, strip_comments};

pub const CONVERT_RS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/api/convert.rs");
pub const KOTLIN_ENGINE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/kotlin/xyz/erik/opi/engine/EngineController.kt"
);

/// `{` 的下标 → 配对 `}` 的下标。
fn matching_brace(s: &str, open: usize) -> Result<usize, String> {
    let mut depth = 0usize;
    for (i, c) in s.as_bytes().iter().enumerate().skip(open) {
        match c {
            b'{' => depth += 1,
            b'}' => {
                if depth == 0 {
                    return Err(format!("`}}` 多于 `{{`：{s}"));
                }
                depth -= 1;
                if depth == 0 {
                    return Ok(i);
                }
            }
            _ => {}
        }
    }
    Err(format!("`{{` 没配平：{s}"))
}

/// `fn <name>(` 的函数体（花括号内文本）。
fn fn_body<'a>(src: &'a str, name: &str) -> Result<&'a str, String> {
    let at = src
        .find(&format!("fn {name}("))
        .ok_or_else(|| format!("找不到 fn {name}（值契约的锚点没了，门禁会失明）"))?;
    let rest = &src[at..];
    let open = rest
        .find('{')
        .ok_or_else(|| format!("fn {name} 没有函数体"))?;
    let close = matching_brace(rest, open)?;
    Ok(&rest[open + 1..close])
}

/// `A => B, C => D` → 配对的 arms（`arm.split_once("=>")`）。
fn arms(body: &str) -> Vec<(String, String)> {
    // 函数体是 `match m { 臂, 臂 }`：先切掉 match 头（连它的 `{`），否则第一个臂的
    // 左半边会粘上 `match m {`（实测踩过：报「臂左边不是 Mode::X」）。
    let body = match body.find('{') {
        Some(i) => &body[i + 1..],
        None => body,
    };
    body.split(',')
        .filter_map(|a| a.split_once("=>"))
        .map(|(l, r)| {
            (
                l.trim().to_string(),
                r.trim().trim_end_matches('}').trim().to_string(),
            )
        })
        .collect()
}

fn int_of(s: &str) -> Result<i32, String> {
    s.parse::<i32>()
        .map_err(|_| format!("`{s}` 不是整数：值契约的臂解析歪了"))
}

/// `mode_to_int`（`Mode::X => n`）与 `mode_from_int`（`n => Some(Mode::X)`）两张表。
///
/// 两张都要：只比一张的话，另一张把两个模式**对调**（`1` 与 `4` 互换）时，
/// `opi_mode` 出的数与 `opi_switch_mode` 收的数就会互相矛盾，而单张表看不出来。
/// `(mode_to_int, mode_from_int)`。
pub type ModeMaps = (BTreeMap<String, i32>, BTreeMap<String, i32>);

pub fn parse_rust_mode_maps(src: &str) -> Result<ModeMaps, String> {
    let text = strip_comments(src);

    let mut to_int = BTreeMap::new();
    for (l, r) in arms(fn_body(&text, "mode_to_int")?) {
        let name = l
            .strip_prefix("Mode::")
            .ok_or_else(|| format!("mode_to_int 的臂左边不是 `Mode::X`：`{l}`"))?;
        to_int.insert(name.to_uppercase(), int_of(&r)?);
    }

    let mut from_int = BTreeMap::new();
    for (l, r) in arms(fn_body(&text, "mode_from_int")?) {
        if l == "_" {
            continue; // 越界 → None，不是映射的一部分
        }
        let name = r
            .trim_start_matches("Some(")
            .trim_end_matches(')')
            .strip_prefix("Mode::")
            .ok_or_else(|| format!("mode_from_int 的臂右边不是 `Some(Mode::X)`：`{r}`"))?;
        from_int.insert(name.to_uppercase(), int_of(&l)?);
    }

    if to_int.is_empty() || from_int.is_empty() {
        return Err("模式映射解析成空 = 门禁失明".into());
    }
    Ok((to_int, from_int))
}

/// Kotlin `enum class EngineMode(val value: Int) { PINYIN(0), … }` 的 `名字 → 整数`。
pub fn parse_kotlin_mode_enum(src: &str) -> Result<BTreeMap<String, i32>, String> {
    let text = strip_comments(src);
    let at = text
        .find("enum class EngineMode(")
        .ok_or("EngineController.kt 里找不到 enum class EngineMode（被改名/搬家了？）")?;
    let rest = &text[at..];
    let open = rest.find('{').ok_or("EngineMode 没有枚举体")?;
    let close = matching_brace(rest, open)?;
    // 枚举体里 `;` 之后是成员声明（`companion object` 等），条目只在前半段
    let entries = rest[open + 1..close].split(';').next().unwrap_or("");

    let mut out = BTreeMap::new();
    for e in entries.split(',') {
        let e = e.trim();
        if e.is_empty() {
            continue;
        }
        let (name, v) = e
            .split_once('(')
            .ok_or_else(|| format!("枚举条目 `{e}` 没有显式 `(n)` —— 值契约要求写死整数"))?;
        let v = v
            .strip_suffix(')')
            .ok_or_else(|| format!("枚举条目 `{e}` 的括号没闭合"))?;
        out.insert(name.trim().to_uppercase(), int_of(v.trim())?);
    }
    if out.is_empty() {
        return Err("EngineMode 解析成空 = 门禁失明".into());
    }
    Ok(out)
}

/// 三张表必须**互相一致**：Rust 两个方向 + Kotlin 枚举。
pub fn check_mode_maps(
    to_int: &BTreeMap<String, i32>,
    from_int: &BTreeMap<String, i32>,
    kotlin: &BTreeMap<String, i32>,
    kotlin_label: &str,
) -> Vec<String> {
    let mut diffs = Vec::new();
    if to_int != from_int {
        diffs.push(format!(
            "Rust 内部不自洽：mode_to_int 与 mode_from_int 不是同一张表（对调某个模式时 \
             `opi_mode` 出的数与 `opi_switch_mode` 收的数会互相矛盾）\n    \
             mode_to_int   = {to_int:?}\n    mode_from_int = {from_int:?}"
        ));
    }
    for (name, v) in to_int {
        match kotlin.get(name) {
            None => diffs.push(format!(
                "`{name}`：Rust 是 {v}，{kotlin_label} 里没有这个名字"
            )),
            Some(k) if k != v => diffs.push(format!(
                "`{name}`：整数不一致 —— Rust {v} / {kotlin_label} {k}\
                 （⚠️ 照 `Mode` 的**声明序**推会得到 Traditional=1）"
            )),
            Some(_) => {}
        }
    }
    for name in kotlin.keys() {
        if !to_int.contains_key(name) {
            diffs.push(format!(
                "`{name}`：{kotlin_label} 有，Rust 的模式映射里没有"
            ));
        }
    }
    diffs.sort();
    diffs
}

pub fn rust_src() -> String {
    read(CONVERT_RS)
}

pub fn kotlin_src() -> String {
    read(KOTLIN_ENGINE)
}

// ---------- fixture：门禁自己的红与绿（不读仓里的活文件） ----------

pub const FIX_RS: &str = r#"
pub fn mode_from_int(m: i32) -> Option<Mode> {
    match m {
        0 => Some(Mode::Pinyin),
        1 => Some(Mode::English),
        2 => Some(Mode::Number),
        3 => Some(Mode::Symbol),
        4 => Some(Mode::Traditional),
        _ => None,
    }
}

pub fn mode_to_int(m: Mode) -> i32 {
    match m {
        Mode::Pinyin => 0,
        Mode::English => 1,
        Mode::Number => 2,
        Mode::Symbol => 3,
        Mode::Traditional => 4,
    }
}
"#;

/// KDoc 里放一个**同名概念**的假枚举：注释不剥掉的话，解析器可能读到 `A(9)`。
pub const FIX_KT: &str = r#"
/** 示例：enum class Ghost(val value: Int) { A(9), B(8) } */
enum class EngineMode(val value: Int) {
    PINYIN(0), ENGLISH(1), NUMBER(2), SYMBOL(3), TRADITIONAL(4);

    companion object {
        fun fromInt(v: Int) = entries.firstOrNull { it.value == v } ?: PINYIN
    }
}
"#;

/// 变异（Rust，**声明序陷阱**）：`Mode` 的声明序是 Pinyin, **Traditional**, English, …
/// 照它推就会把 Traditional 写成 1。
pub const FIX_RS_DECL_ORDER_MUTANT: &str = r#"
pub fn mode_from_int(m: i32) -> Option<Mode> {
    match m {
        0 => Some(Mode::Pinyin),
        1 => Some(Mode::English),
        2 => Some(Mode::Number),
        3 => Some(Mode::Symbol),
        4 => Some(Mode::Traditional),
        _ => None,
    }
}

pub fn mode_to_int(m: Mode) -> i32 {
    match m {
        Mode::Pinyin => 0,
        Mode::Traditional => 1,
        Mode::English => 2,
        Mode::Number => 3,
        Mode::Symbol => 4,
    }
}
"#;

/// 变异（Rust，反向表对调）：`mode_from_int` 里 1 与 4 互换 —— 两个方向互相矛盾。
pub const FIX_RS_FROM_INT_SWAP: &str = r#"
pub fn mode_from_int(m: i32) -> Option<Mode> {
    match m {
        0 => Some(Mode::Pinyin),
        1 => Some(Mode::Traditional),
        2 => Some(Mode::Number),
        3 => Some(Mode::Symbol),
        4 => Some(Mode::English),
        _ => None,
    }
}

pub fn mode_to_int(m: Mode) -> i32 {
    match m {
        Mode::Pinyin => 0,
        Mode::English => 1,
        Mode::Number => 2,
        Mode::Symbol => 3,
        Mode::Traditional => 4,
    }
}
"#;

/// 变异（Kotlin）：`TRADITIONAL(4)` → `TRADITIONAL(1)`。
pub const FIX_KT_TRAD_MUTANT: &str = r#"
enum class EngineMode(val value: Int) {
    PINYIN(0), ENGLISH(1), NUMBER(2), SYMBOL(3), TRADITIONAL(1);
}
"#;

/// 两份源码文本 → 差异列表。
pub fn diffs_for(rs: &str, kt: &str) -> Result<Vec<String>, String> {
    let (to_int, from_int) = parse_rust_mode_maps(rs)?;
    let kotlin = parse_kotlin_mode_enum(kt)?;
    Ok(check_mode_maps(&to_int, &from_int, &kotlin, "EngineMode"))
}
