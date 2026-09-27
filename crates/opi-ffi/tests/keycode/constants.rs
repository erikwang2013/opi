// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 常量**值**契约的机械部分 —— 第五道门禁的解析与对账。用例在
//! `tests/keycode_contract.rs`。单独一个文件是 500 行硬规矩（同 `jni/checker.rs`）。
//!
//! ## 为什么值得单开一道
//!
//! 名字/签名门禁（`jni_contract` / `c_abi_contract`）看不见**值**。而值错的表现是
//! 「**不报错、不崩溃、静默失效**」：`states & KEY_STATE_REPEAT` 在位值上错一格就
//! 恒假，那条分支从此不再执行，编译期与运行期检查都不会响一声。本仓 iOS 侧的
//! `stateRepeat` 就是这样一颗雷。
//!
//! ## 四条规矩
//!
//! * **真源唯一** = `crates/engine-core/src/keys.rs`。别端只做「**子集且逐条值相等**」
//!   的核对 —— 不是多数表决（别端抄错不能反过来动真源），也不是集合相等。
//! * **子集是合法的**：鸿蒙 C / ETS **故意**不抄方向键（理由写在 `OpiEngine.ets`
//!   里：那是 use-site 声明面）。「少抄」不算缺陷，但必须与用例里钉住的清单一致 ——
//!   静默少抄一个仍要红。
//! * **求值后比**，不比字面串：`1 << 27` / `134217728` / `0x8000000` / `(1u << 27)`
//!   是同一个值，按字面比会制造假红。
//! * **认不出来就报错，不跳过**：本端多出一个没登记的常量 = 一条消息，不是静默忽略。
//!   静默忽略正是这批门禁要防的唯一失败模式。

use std::collections::BTreeMap;

/// 求值环境：**按本端书写的拼写**存已声明的常量（Swift 里是 `specialBase`，
/// C 里是 `OPI_KEY_SPECIAL_BASE`），这样表达式里的名字能自己解析出来。
pub type Env = BTreeMap<String, i64>;

/// 一条常量声明：求值结果 + **原文**。报文里两个都要有 —— 只有值的话，人还得回去
/// 猜是哪一行写错了。
#[derive(Debug, Clone)]
pub struct Const {
    pub value: i64,
    pub expr: String,
}

/// 一个**声明面**。macos 的键码与状态位分居两个 enum，算两面（`shift` 在两边
/// 分别是键码与状态位，合并成一个 map 会互相覆盖 → 假绿）。
#[derive(Debug, Clone)]
pub struct Surface {
    /// 报文里的「哪一方」，如 `harmony/ets · OpiEngine.ets`。
    pub label: String,
    /// 本端这一族解析到的常量：**本端拼写** → 值。
    pub consts: BTreeMap<String, Const>,
    /// 本端拼写 → 真源 `keys.rs` 拼写。命名惯例由**产生它的解析器**给出，
    /// 不在调用点手抄第二遍（手抄两遍必漂）。
    pub map: BTreeMap<String, String>,
    /// 本端这一族共出现多少条声明行。== `consts.len()`，不等就说明有同名重复
    /// （后一条静默覆盖前一条），那是假绿的经典入口。
    pub declared: usize,
}

// ---------- 五个对账方的路径 ----------

pub const KEYS_RS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../engine-core/src/keys.rs");
pub const HARMONY_C: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../harmony/cpp/opi_ffi.h");
pub const HARMONY_ETS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../harmony/ets/InputMethodExtensionAbility/OpiEngine.ets"
);
pub const IOS_SWIFT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../ios/OpiEngine.swift");
pub const MACOS_SWIFT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../macos/OpiEngine.swift");

// ---------- 求值器 ----------

/// 整数常量表达式求值：`|`、`<<`、括号、`0x`/十进制字面量（允许 `_` 分隔与
/// `u`/`U`/`l`/`L` 后缀），以及**同一段内先前已声明**的常量名。
///
/// 认不出来一律 `Err`。**不在解析器里放行任何东西** —— 放行一个拼错的名字，
/// 就等于让门禁对那一行失明。
pub fn eval(expr: &str, env: &Env) -> Result<i64, String> {
    let e = trim_parens(expr.trim());
    if e.is_empty() {
        return Err("空表达式".into());
    }
    let ors = split_at_depth(e, "|");
    if ors.len() > 1 {
        let mut acc = 0i64;
        for p in ors {
            acc |= eval(p, env)?;
        }
        return Ok(acc);
    }
    let sh = split_at_depth(e, "<<");
    match sh.len() {
        1 => atom(e, env),
        2 => {
            let a = eval(sh[0], env)?;
            let b = eval(sh[1], env)?;
            if !(0..63).contains(&b) {
                return Err(format!("位移量 {b} 越界：`{e}`"));
            }
            Ok(a << b)
        }
        n => Err(format!("`<<` 出现 {n} 段，本求值器只认两层：`{e}`")),
    }
}

fn atom(t: &str, env: &Env) -> Result<i64, String> {
    let s = t.trim();
    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        let d: String = h.chars().filter(|c| *c != '_').collect();
        return parse_with_suffix(&d, 16).ok_or_else(|| format!("十六进制字面量不认识：`{s}`"));
    }
    if s.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        let d: String = s.chars().filter(|c| *c != '_').collect();
        return parse_with_suffix(&d, 10).ok_or_else(|| format!("十进制字面量不认识：`{s}`"));
    }
    if let Some(v) = env.get(s) {
        return Ok(*v);
    }
    // `UInt32.max` 这类哨兵写法：值必须就是 u32::MAX，别放行成「随便什么数」。
    if matches!(s, ".max" | "UInt32.max" | "u32::MAX") {
        return Ok(u32::MAX as i64);
    }
    Err(format!(
        "不认识的标识符 `{s}`（既不在本段已声明的常量里，也不是字面量）。\
         别在解析器里放行：放行 = 门禁对这一行失明"
    ))
}

/// `0x0D` 这类字面量**末尾是合法十六进制数字**，不能无脑砍掉尾部字母当后缀；
/// 先按原样解析，只在失败时才逐个剥 `u`/`l` 再试。
fn parse_with_suffix(d: &str, radix: u32) -> Option<i64> {
    let mut cand = d.to_string();
    for _ in 0..3 {
        if let Ok(v) = i64::from_str_radix(&cand, radix) {
            return Some(v);
        }
        if cand.ends_with(['u', 'U', 'l', 'L']) {
            cand.pop();
        } else {
            break;
        }
    }
    None
}

/// 反复剥掉**配平**的最外层括号。`(a) | (b)` 不能被误剥成 `a) | (b`。
fn trim_parens(mut s: &str) -> &str {
    loop {
        let t = s.trim();
        if !t.starts_with('(') || !t.ends_with(')') {
            return t;
        }
        let mut depth = 0i32;
        let mut outer_ok = true;
        for (i, c) in t.char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 && i != t.len() - 1 {
                        outer_ok = false;
                        break;
                    }
                }
                _ => {}
            }
        }
        if !outer_ok {
            return t;
        }
        s = &t[1..t.len() - 1];
    }
}

/// 括号深度 0 处的运算符切分（`pat` 可多字符，如 `<<`）。
fn split_at_depth<'a>(s: &'a str, pat: &str) -> Vec<&'a str> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    let mut i = 0usize;
    while i < b.len() {
        match b[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            _ => {}
        }
        if depth == 0 && s[i..].starts_with(pat) {
            out.push(&s[start..i]);
            i += pat.len();
            start = i;
            continue;
        }
        i += 1;
    }
    out.push(&s[start..]);
    out
}

// ---------- 各端的解析器（都吃 `&str`，好让 fixture 走**同一条**代码路径） ----------

/// 收尾：一条都没解析出来 = 解析器失明 = 必须红（而不是「零差异 = 绿」）。
fn finish(
    label: &str,
    consts: BTreeMap<String, Const>,
    map: BTreeMap<String, String>,
    declared: usize,
) -> Result<Surface, String> {
    if consts.is_empty() {
        return Err(format!(
            "{label}：一条常量都没解析出来 —— 解析器失明 = 假绿，这里必须红"
        ));
    }
    Ok(Surface {
        label: label.into(),
        consts,
        map,
        declared,
    })
}

/// 真源：`pub const NAME: u32 = EXPR;`（真源自己映射自己）。
pub fn parse_rust(src: &str, label: &str, path: &str) -> Result<Surface, String> {
    let clean = super::checker::strip_comments(src);
    let mut consts = BTreeMap::new();
    let mut env = Env::new();
    let mut declared = 0usize;
    for line in clean.lines() {
        let Some(rest) = line.trim().strip_prefix("pub const ") else {
            continue;
        };
        let (name, rhs) = rest
            .split_once(':')
            .ok_or_else(|| format!("{path}：`{rest}` 不是 `名字: 类型 = 值` 的形状"))?;
        let (_, rhs) = rhs
            .split_once('=')
            .ok_or_else(|| format!("{path}：`{rest}` 缺 `=`"))?;
        let name = name.trim().to_string();
        let expr = rhs.trim().trim_end_matches(';').trim();
        declared += 1;
        let value = eval(expr, &env).map_err(|e| format!("{path} · {name}（`{expr}`）：{e}"))?;
        env.insert(name.clone(), value);
        consts.insert(
            name.clone(),
            Const {
                value,
                expr: expr.into(),
            },
        );
    }
    // 真源映射自己：`keys.rs` 的拼写就是真源拼写。
    let map = consts.keys().map(|k| (k.clone(), k.clone())).collect();
    finish(label, consts, map, declared)
}

/// `OPI_KEY_*` → 真源拼写：剥 `OPI_`；`KEY_SPECIAL_BASE` 在真源里叫 `SPECIAL_BASE`。
pub fn c_target(n: &str) -> String {
    match n.strip_prefix("OPI_").unwrap_or(n) {
        "KEY_SPECIAL_BASE" => "SPECIAL_BASE".into(),
        s => s.into(),
    }
}

/// 鸿蒙 C 头：`#define OPI_KEY_xxx (EXPR)`（只认这一族，别族的宏不在本契约里）。
pub fn parse_c(src: &str, label: &str, path: &str) -> Result<Surface, String> {
    let clean = super::checker::strip_comments(src);
    let mut consts = BTreeMap::new();
    let mut env = Env::new();
    let mut declared = 0usize;
    for line in clean.lines() {
        let Some(rest) = line.trim().strip_prefix("#define ") else {
            continue;
        };
        let Some((name, expr)) = rest.split_once(char::is_whitespace) else {
            continue;
        };
        if !name.starts_with("OPI_KEY_") {
            continue;
        }
        declared += 1;
        let expr = expr.trim();
        let value = eval(expr, &env).map_err(|e| format!("{path} · {name}（`{expr}`）：{e}"))?;
        env.insert(name.to_string(), value);
        consts.insert(
            name.to_string(),
            Const {
                value,
                expr: expr.into(),
            },
        );
    }
    let map = consts.keys().map(|k| (k.clone(), c_target(k))).collect();
    finish(label, consts, map, declared)
}

/// 鸿蒙 ETS：`export const KEY_xxx: number = EXPR;` / `STATE_xxx` / `const SPECIAL_BASE`。
pub fn ets_target(n: &str) -> String {
    match n.strip_prefix("STATE_") {
        Some(s) => format!("KEY_STATE_{s}"),
        None => n.into(),
    }
}

pub fn parse_ets(src: &str, label: &str, path: &str) -> Result<Surface, String> {
    let clean = super::checker::strip_comments(src);
    let mut consts = BTreeMap::new();
    let mut env = Env::new();
    let mut declared = 0usize;
    for line in clean.lines() {
        let t = line.trim();
        let (name, rest) = if let Some(r) = t.strip_prefix("export const ") {
            let name = r.split(':').next().unwrap_or("").trim().to_string();
            (name, r)
        } else if let Some(r) = t.strip_prefix("const SPECIAL_BASE") {
            ("SPECIAL_BASE".to_string(), r)
        } else {
            continue;
        };
        let keep = name.starts_with("KEY_") || name.starts_with("STATE_") || name == "SPECIAL_BASE";
        if !keep {
            continue;
        }
        let Some((_, rhs)) = rest.split_once('=') else {
            continue;
        };
        declared += 1;
        let expr = rhs.trim().trim_end_matches(';').trim();
        let value = eval(expr, &env).map_err(|e| format!("{path} · {name}（`{expr}`）：{e}"))?;
        env.insert(name.clone(), value);
        consts.insert(
            name,
            Const {
                value,
                expr: expr.into(),
            },
        );
    }
    let map = consts.keys().map(|k| (k.clone(), ets_target(k))).collect();
    finish(label, consts, map, declared)
}

/// `{` 起点 → 配对 `}` 的**内容**（不含花括号）。找不到头就 `None`。
pub fn block<'a>(src: &'a str, header: &str) -> Option<&'a str> {
    let at = src.find(header)?;
    let open = at + src[at..].find('{')?;
    let b = src.as_bytes();
    let mut depth = 0i32;
    let mut i = open;
    while i < b.len() {
        match b[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&src[open + 1..i]);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Swift 的一个 enum：`static let name[: UInt32] = EXPR`（行尾 `// 注释` 由
/// `strip_comments` 去掉）。名字与真源不同（`ret` / `del` / `repeatKey`），
/// 所以**由调用方给的映射表说清**，不靠驼峰转下划线的猜法 —— 猜法在 `shift`
/// 上会撞车（键码 `shift` 与状态位 `shift` 同名不同义）。
pub fn parse_swift(
    src: &str,
    header: &str,
    label: &str,
    path: &str,
    map: &[(&str, &str)],
) -> Result<Surface, String> {
    let clean = super::checker::strip_comments(src);
    let body = block(&clean, header).ok_or_else(|| format!("{path} 里找不到 `{header}`"))?;
    let mut consts = BTreeMap::new();
    let mut env = Env::new();
    let mut declared = 0usize;
    for line in body.lines() {
        let Some(rest) = line.trim().strip_prefix("static let ") else {
            continue;
        };
        let Some((lhs, rhs)) = rest.split_once('=') else {
            continue;
        };
        let name = lhs.split(':').next().unwrap_or("").trim().to_string();
        let expr = rhs.trim();
        declared += 1;
        let value = eval(expr, &env).map_err(|e| format!("{path} · {name}（`{expr}`）：{e}"))?;
        env.insert(name.clone(), value);
        consts.insert(
            name,
            Const {
                value,
                expr: expr.into(),
            },
        );
    }
    // 映射表整表收下（不是只收解析到的）——「登记了却没解析到」也要能报出来。
    let map = map
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
    finish(label, consts, map, declared)
}

// ---------- 对账 ----------

/// 一面 vs 真源。每条消息都点名**哪一方 + 哪个常量 + 两边的值与原写法**。
///
/// 四类问题共用一条通道（都是「与真源不符」）：值不等、本端多出未登记的常量、
/// 映射表登记了但本端没解析到（声明被删）、本地哨兵值不对。同名重复也算。
pub fn reconcile(surface: &Surface, truth: &Surface, locals: &[(&str, i64, &str)]) -> Vec<String> {
    let mut out = Vec::new();
    if surface.declared != surface.consts.len() {
        out.push(format!(
            "[{}] 声明 {} 条但只有 {} 个不同名字：有同名重复，后一条会静默覆盖前一条",
            surface.label,
            surface.declared,
            surface.consts.len()
        ));
    }
    let local: BTreeMap<&str, i64> = locals.iter().map(|(n, v, _)| (*n, *v)).collect();
    // 1. 本端多出来的（未登记的）——**必须报**，跳过它就等于门禁失明
    for (name, c) in &surface.consts {
        if surface.map.contains_key(name.as_str()) || local.contains_key(name.as_str()) {
            continue;
        }
        out.push(format!(
            "[{}] 本端多出一条未登记常量 `{name} = {}`（`{}`）：\
             要么它在真源里有对应（补映射表），要么它是本端本地哨兵（补 locals 并写明理由）",
            surface.label, c.value, c.expr
        ));
    }
    // 2. 逐条比值
    for (sname, target) in &surface.map {
        let Some(sc) = surface.consts.get(sname) else {
            out.push(format!(
                "[{}] 映射表登记了 `{sname}` → 真源 `{target}`，但本端**没解析到**这条声明",
                surface.label
            ));
            continue;
        };
        let Some(tc) = truth.consts.get(target) else {
            out.push(format!(
                "[{}] 真源 `keys.rs` 里没有 `{target}`（映射表 `{sname}` → `{target}` 已失效）",
                surface.label
            ));
            continue;
        };
        if sc.value != tc.value {
            out.push(format!(
                "[{}] `{sname}` = {}（`{}`），真源 `{target}` = {}（`{}`）—— 值不等",
                surface.label, sc.value, sc.expr, tc.value, tc.expr
            ));
        }
    }
    for (sname, want, why) in locals {
        match surface.consts.get(*sname) {
            Some(c) if c.value == *want => {}
            Some(c) => out.push(format!(
                "[{}] 本地哨兵 `{sname}` = {}（`{}`），应为 {want}（{why}）",
                surface.label, c.value, c.expr
            )),
            None => out.push(format!(
                "[{}] 本地哨兵 `{sname}` 没解析到（{why}）",
                surface.label
            )),
        }
    }
    out
}

/// 本面**没抄**的真源常量（子集是合法的，但用例把清单钉死）。
pub fn missing_from(surface: &Surface, truth: &Surface) -> Vec<String> {
    let have: std::collections::BTreeSet<&str> = surface.map.values().map(String::as_str).collect();
    truth
        .consts
        .keys()
        .filter(|k| !have.contains(k.as_str()))
        .cloned()
        .collect()
}
