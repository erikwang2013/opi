// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! JNI 声明面契约的**机械**部分：`jni.rs` 的 `RegisterNatives` 表 ↔ 宿主类里的
//! `native` 声明。单独一个文件是 500 行硬规矩（同 `c_abi/checker.rs`）：
//! `jni_contract.rs` 装用例、本文件装解析与比对；`mod checker;` 不会被当独立测试目标。

use std::fs;

// ---------- 规范形 ----------

/// 一条 native 方法：JNI **名字** + JNI **描述符** `(参数...)返回`。
///
/// 两者都必须对：`RegisterNatives` 按 (名字, 描述符) 二元组在目标类里找方法，
/// 任一不符 → **整张表注册失败** → `JNI_OnLoad` 返 0 → `System.load` 抛
/// `UnsatisfiedLinkError`。整表连带失败这一点是**实测**的（21 条里缺 1 条即全盘失败），
/// 见 `jni_contract.rs` 头注里的复现步骤。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Method {
    pub name: String,
    pub desc: String,
}

impl Method {
    pub fn new(name: &str, desc: &str) -> Self {
        Self {
            name: name.into(),
            desc: desc.into(),
        }
    }
}

/// Kotlin / Java 类型 → JNI 描述符。**认不出来就报错，不猜** —— 猜错会变成
/// 「两侧都折算成同一个错误值 = 假绿」，那正是本门禁要防的事。
pub fn desc_of(ty: &str) -> Result<String, String> {
    let flat: String = ty.chars().filter(|c| !c.is_whitespace()).collect();
    let t = flat.to_lowercase();
    let t = t.trim_end_matches('?');
    Ok(match t {
        "string" => "Ljava/lang/String;".into(),
        "boolean" | "bool" => "Z".into(),
        "byte" => "B".into(),
        "char" => "C".into(),
        "short" => "S".into(),
        "int" => "I".into(),
        "long" => "J".into(),
        "float" => "F".into(),
        "double" => "D".into(),
        "unit" | "void" => "V".into(),
        // Kotlin `Array<String>` 与 Java `String[]` 小写后落这里
        "array<string>" | "string[]" => "[Ljava/lang/String;".into(),
        other => {
            return Err(format!(
                "不认识的类型 `{other}`：desc_of 只认已列出的拼写，\
                 遇到新类型请**扩展 desc_of**，别在调用点绕过（绕过 = 门禁看不见它）"
            ));
        }
    })
}

/// 去掉 `//` 行注释与 `/* */` 块注释（KDoc / 文档注释里常有示例代码，
/// 不剥掉会把注释里的 `external fun` 当声明）。
pub fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < src.len() {
        let rest = &src[i..];
        if rest.starts_with("/*") {
            match rest.find("*/") {
                Some(j) => i += j + 2,
                None => break, // 未闭合：后面全是注释
            }
        } else if rest.starts_with("//") {
            match rest.find('\n') {
                Some(j) => {
                    out.push('\n');
                    i += j + 1;
                }
                None => break,
            }
        } else {
            let ch = rest.chars().next().expect("非空");
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

fn is_ident(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// `(` 的下标 → 配对 `)` 的下标。调用方保证 `open` 处是 `(`。
fn matching_paren(s: &str, open: usize) -> Result<usize, String> {
    let b = s.as_bytes();
    let mut depth = 0usize;
    for (i, c) in b.iter().enumerate().skip(open) {
        match c {
            b'(' => depth += 1,
            b')' => {
                if depth == 0 {
                    return Err(format!("`)` 多于 `(`：`{s}`"));
                }
                depth -= 1;
                if depth == 0 {
                    return Ok(i);
                }
            }
            _ => {}
        }
    }
    Err(format!("括号没配平：`{s}`"))
}

fn parens_balanced(s: &str) -> bool {
    let mut depth = 0i32;
    for c in s.bytes() {
        match c {
            b'(' => depth += 1,
            b')' => depth -= 1,
            _ => {}
        }
    }
    depth == 0
}

/// 顶层逗号切分（`<>` 与 `()` 内不算）。
pub fn split_top_level(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match c {
            '<' | '(' => depth += 1,
            '>' | ')' => depth -= 1,
            ',' if depth == 0 => {
                out.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

/// `name: Type, name2: Type2` → JNI 参数描述符串。参数名本身不参与描述符。
fn params_desc(params: &str) -> Result<String, String> {
    let mut args = String::new();
    for p in split_top_level(params) {
        let p = p.trim();
        if p.is_empty() {
            continue;
        }
        let Some(colon) = p.rfind(':') else {
            return Err(format!("参数 `{p}` 没有类型标注（`name: Type`）：{params}"));
        };
        args.push_str(&desc_of(&p[colon + 1..])?);
    }
    Ok(args)
}

/// Java 的参数是 `Type name`（**与 Kotlin 的 `name: Type` 相反**），类型在前。
fn java_params_desc(params: &str) -> Result<String, String> {
    let mut args = String::new();
    for p in split_top_level(params) {
        let p = p.trim();
        if p.is_empty() {
            continue;
        }
        let Some(at) = p.rfind(char::is_whitespace) else {
            return Err(format!(
                "Java 参数 `{p}` 缺类型（应为 `Type name`）：{params}"
            ));
        };
        args.push_str(&desc_of(&p[..at])?);
    }
    Ok(args)
}

/// JNI 描述符的结构自检（`(...)X`）。不深校验类型字母 —— 值不值得信由比对决定，
/// 但**空串/没解析出东西**必须在这里挡住。
fn assert_descriptor(desc: &str, what: &str) -> Result<(), String> {
    if !desc.starts_with('(') {
        return Err(format!("{what} 的描述符不以 `(` 开头：`{desc}`"));
    }
    let close = matching_paren(desc, 0)?;
    if close + 1 >= desc.len() {
        return Err(format!("{what} 的描述符缺返回类型：`{desc}`"));
    }
    Ok(())
}

// ---------- 解析：Rust 侧注册表 ----------

/// 从 `jni.rs` 里抽 `NativeMethod::from_raw_parts(jni_str!("名字"), jni_str!("描述符"), …)`。
///
/// 只认这个形状：每一处的两个 `jni_str!` 必须出现在**下一个** `from_raw_parts(` 之前，
/// 否则报错 —— 否则解析器会「借」下一条的名字，把两边都算成对的（假绿）。
pub fn parse_rust_registrations(src: &str) -> Result<Vec<Method>, String> {
    let text = strip_comments(src);
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(i) = text[pos..].find("from_raw_parts(") {
        let after = &text[pos + i + "from_raw_parts(".len()..];
        let seg_end = after.find("from_raw_parts(").unwrap_or(after.len());
        let seg = &after[..seg_end];
        let mut lits: Vec<String> = Vec::new();
        let mut cur = 0;
        while lits.len() < 2 {
            let Some(j) = seg[cur..].find("jni_str!(") else {
                return Err(format!(
                    "第 {} 条 from_raw_parts 只找到 {} 个 jni_str!：应为「名字, 描述符」两个",
                    out.len() + 1,
                    lits.len()
                ));
            };
            let s = &seg[cur + j + "jni_str!(".len()..];
            // 先按引号取字面量，**不能**找第一个 `)` —— 描述符里就有 `)`
            // （`(I)V`），那样会截成 `(Ljava/lang/String;`。
            let q1 = s
                .find('"')
                .ok_or_else(|| format!("jni_str! 里不是字符串字面量：`{}`", s.trim()))?;
            let q2 = s[q1 + 1..]
                .find('"')
                .ok_or_else(|| format!("jni_str! 的字符串没闭合：`{}`", s.trim()))?
                + q1
                + 1;
            // 字面量后必须紧跟 `)`：否则是 `jni_str!(concat!(…))` 之类别的形状，
            // 只按第一个字面量解析就会读歪。
            if !s[q2 + 1..].trim_start().starts_with(')') {
                return Err(format!("jni_str! 的字面量后面不是 `)`：`{}`", s.trim()));
            }
            lits.push(s[q1 + 1..q2].to_string());
            cur += j + "jni_str!(".len() + q2 + 1;
        }
        assert_descriptor(&lits[1], &format!("注册表条目 `{}`", lits[0]))?;
        out.push(Method::new(&lits[0], &lits[1]));
        pos += i + "from_raw_parts(".len();
    }
    Ok(out)
}

/// `JNI_OnLoad` 里 `find_class` 的宿主类名（`xyz/erik/opi/jni/OpiEngine`）。
pub fn parse_host_class(src: &str) -> Result<String, String> {
    let text = strip_comments(src);
    let i = text
        .find("find_class(")
        .ok_or("jni.rs 里找不到 find_class（宿主类名解析不出来）")?;
    let after = &text[i + "find_class(".len()..];
    let s = &after[after
        .find("jni_str!(")
        .ok_or("find_class 里没有 jni_str!")?..];
    let lit = &s["jni_str!(".len()..];
    let end = lit.find(')').ok_or("jni_str! 没有右括号")?;
    let lit = lit[..end].trim();
    lit.strip_prefix('"')
        .and_then(|x| x.strip_suffix('"'))
        .map(|x| x.to_string())
        .ok_or_else(|| format!("find_class 里不是字符串字面量：`{lit}`"))
}

/// `jni.rs` 里 `#[unsafe(no_mangle)]` 的条数（护栏用：应 == 注册条数 + 1 个 `JNI_OnLoad`）。
pub fn count_no_mangle(src: &str) -> usize {
    strip_comments(src).matches("#[unsafe(no_mangle)]").count()
}

// ---------- 解析：Kotlin 侧 `external fun` ----------

/// 从 Kotlin 源码里抽 `external fun`（`override` 等修饰符可有可无，声明可折行）。
pub fn parse_kotlin_externals(src: &str) -> Result<Vec<Method>, String> {
    let text = strip_comments(src);
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let Some(at) = lines[i].find("external fun ") else {
            i += 1;
            continue;
        };
        let mut decl = lines[i][at + "external fun ".len()..].to_string();
        while !parens_balanced(&decl) {
            i += 1;
            if i >= lines.len() {
                return Err(format!("`{decl}` 的参数表没配平（已到文件末尾）"));
            }
            decl.push(' ');
            decl.push_str(lines[i]);
        }
        out.push(parse_decl(&decl)?);
        i += 1;
    }
    let markers = text.matches("external fun ").count();
    if markers != out.len() {
        return Err(format!(
            "有 {markers} 处 `external fun ` 但只解析出 {} 条：解析器漏读了，\
             别让它静默跳过（漏掉的那条正是门禁的盲区）",
            out.len()
        ));
    }
    Ok(out)
}

/// `名字(参数): 返回` → [`Method`]。返回类型省略 = Kotlin `Unit` = JNI `V`。
fn parse_decl(decl: &str) -> Result<Method, String> {
    let open = decl
        .find('(')
        .ok_or_else(|| format!("`{decl}` 没有参数表"))?;
    let name = decl[..open].trim();
    if !is_ident(name) {
        return Err(format!("`{decl}` 的函数名不像标识符：`{name}`"));
    }
    let close = matching_paren(decl, open)?;
    let args = params_desc(&decl[open + 1..close])?;
    let rest = decl[close + 1..].trim();
    let ret = if rest.is_empty() {
        "V".to_string()
    } else {
        let ty = rest
            .strip_prefix(':')
            .ok_or_else(|| format!("`{decl}` 的参数表后面既不是结尾也不是 `: 返回类型`"))?;
        desc_of(ty)?
    };
    Ok(Method::new(name, &format!("({args}){ret}")))
}

// ---------- 解析：Java 宿主类 `static native` ----------

/// 从 Java 源码里抽 `static native <返回> <名字>(<参数>);`。
pub fn parse_java_natives(src: &str) -> Result<Vec<Method>, String> {
    let text = strip_comments(src);
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if !line.ends_with(';') {
            continue;
        }
        let Some(at) = line.find("native ") else {
            continue;
        };
        let decl = &line[at + "native ".len()..];
        let open = decl
            .find('(')
            .ok_or_else(|| format!("`{line}` 没有参数表"))?;
        let close = matching_paren(decl, open)?;
        let head = decl[..open].trim();
        let name_at = head
            .rfind(|c: char| c.is_whitespace())
            .map(|i| i + 1)
            .ok_or_else(|| format!("`{line}` 缺返回类型"))?;
        let name = head[name_at..].trim();
        if !is_ident(name) {
            return Err(format!("`{line}` 的方法名不像标识符：`{name}`"));
        }
        let ret = desc_of(&head[..name_at])?;
        let args = java_params_desc(&decl[open + 1..close])?;
        out.push(Method::new(name, &format!("({args}){ret}")));
    }
    let markers = text.matches("native ").count();
    if markers != out.len() {
        return Err(format!(
            "有 {markers} 处 `native ` 但只解析出 {} 条（同上：不许静默跳过）",
            out.len()
        ));
    }
    Ok(out)
}

// ---------- 比对 ----------

fn sorted(v: &[Method]) -> Vec<&Method> {
    let mut r: Vec<&Method> = v.iter().collect();
    r.sort();
    r
}

/// 两侧必须**集合相等**：名字与描述符逐一相同，且双向无剩余。
pub fn check_exact(reg: &[Method], host: &[Method], host_label: &str) -> Vec<String> {
    let mut diffs = check_host_declares(reg, host, host_label);
    let reg_names: Vec<&str> = reg.iter().map(|m| m.name.as_str()).collect();
    for m in sorted(host) {
        if !reg_names.contains(&m.name.as_str()) {
            diffs.push(format!(
                "`{}`：{host_label} 声明了但 RegisterNatives 没注册（宿主会多出永远解析不到的 native）",
                m.name
            ));
        }
    }
    diffs.sort();
    diffs
}

/// `RegisterNatives` 的每一条都必须能在宿主类里找到**同名同描述符**的声明。
///
/// 这是 JNI 真正的约束方向（不是「集合相等」）：表里的每条都要命中，宿主多声明几条
/// 不会在装载期报错（只在被调用时才 `UnsatisfiedLinkError`）。
pub fn check_host_declares(reg: &[Method], host: &[Method], host_label: &str) -> Vec<String> {
    let mut diffs = Vec::new();
    for m in sorted(reg) {
        match host.iter().find(|h| h.name == m.name) {
            None => diffs.push(format!(
                "`{}`：RegisterNatives 注册了 `{}`，但 {host_label} 没有声明 —— \
                 **整张表会注册失败**（JNI_OnLoad 返 0 → System.load 抛 UnsatisfiedLinkError）",
                m.name, m.desc
            )),
            Some(h) if h.desc != m.desc => diffs.push(format!(
                "`{}`：描述符不一致 —— 注册表 `{}` / {host_label} `{}`",
                m.name, m.desc, h.desc
            )),
            Some(_) => {}
        }
    }
    diffs
}

// ---------- 读文件 ----------

pub const JNI_RS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/jni.rs");
pub const KOTLIN_HOST: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/kotlin/xyz/erik/opi/jni/OpiEngine.kt"
);
pub const JAVA_SMOKE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/jni_smoke/Main.java"
);

pub fn read(p: &str) -> String {
    fs::read_to_string(p).unwrap_or_else(|e| panic!("读不到 {p}：{e}"))
}

pub fn report(diffs: Vec<String>) -> String {
    diffs.join("\n  ")
}
