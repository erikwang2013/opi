// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **C ABI 契约门禁**：`crates/opi-ffi/src/cabi.rs` 的导出 ↔ 手抄的 C 头文件。
//!
//! 为什么需要它：`macos/OpiFFI.h` 是**逐条手抄**的声明面，此前**没有任何测试在看**。
//! 名字抄错 → 编译不过（好）；**签名抄错**（`i32`↔`u32`、`size_t`↔`u32`、结构体字段
//! 顺序颠倒）→ 编译链接全绿、运行期读错内存位，是**静默 UB**。所以逐参比类型、
//! 比返回值、比 `#[repr(C)]` 结构体字段。名字集合的一致性只是其中一条。
//!
//! 比对方式：两侧各解析成**规范形**（去空白 + 折算 Rust 类型别名）后逐条比。
//! Rust 侧**读源码文本**，不引入 cbindgen、不依赖构建产物（契约是声明面之间的，
//! 不是「声明 vs 本机某次构建」——后者在 CI 上形状还不一样）。
//!
//! 三个**非空转护栏**（本项目教训：全绿只证明被断言的部分）：
//!   1. `parser_is_not_blind` —— 解析器若因源码改写而失明（匹配到 0 条），
//!      随后的「集合相等」会**假绿**。故先钉住锚点函数 + `no_mangle` 计数 == 解析计数。
//!   2. `checker_reports_next_mutant` 系列 —— 用**故意改坏的头文件文本**喂同一个
//!      检查器，断言它确实报错。红是本地可复现的，不靠「我手动改过一次」的证词。
//!   3. 头文件里每个**已声明但 Rust 侧没有**的名字都要报出来（防拼写漂移的单向漏检）。
//!
//! 三份头文件的关系（**有意设计，别改**）：
//!   * `macos/OpiFFI.h`  —— 唯一一份全量声明（本文件对它要求**集合相等**）。
//!   * `ios/OpiFFI.h`    —— 纯转发头，自己**不声明**任何函数（只 `#include`）。
//!   * `harmony/cpp/opi_ffi.h` —— **use-site 子集**：只声明 N-API 桥真正调用的出口。
//!     对它是「每条都对 + 全都被用到」，**不是**「等于全集」。

#[path = "c_abi/checker.rs"]
mod checker;

use checker::*;

use std::collections::BTreeSet;

use std::fs;
use std::path::Path;

// ---------- 1. 护栏：解析器不许失明 ----------

#[test]
fn parser_is_not_blind() {
    let src = read(CABI);
    let parsed = parse_rust_exports(&src);

    // 自检锚点：这三个若解析不出来，说明解析器（而非契约）坏了，
    // 后面的「集合相等」会变成两边都空 = 假绿。
    for anchor in ["opi_load", "opi_key_event", "opi_ffi_free_string"] {
        assert!(
            parsed.iter().any(|(n, _)| n == anchor),
            "解析器没解析出锚点 `{anchor}`：解析逻辑失明，其余断言不可信"
        );
    }
    let load = parsed.iter().find(|(n, _)| n == "opi_load").unwrap();
    assert_eq!(
        load.1,
        Sig {
            args: vec!["constuint16_t*".into(), "size_t".into()],
            ret: "bool".into()
        },
        "opi_load 的解析结果不对（解析器坏了，不是契约坏了）"
    );

    // `#[unsafe(no_mangle)]` 的条数必须等于解析出的导出条数 ——
    // 任何「加了导出但解析器看不见」的写法都会在这里亮。
    let no_mangle = src.matches("no_mangle").count();
    assert!(
        no_mangle > 0,
        "cabi.rs 里一条 `no_mangle` 都没有？解析器或文件路径错了"
    );
    assert_eq!(
        no_mangle,
        parsed.len(),
        "`no_mangle` 出现 {no_mangle} 次，但只解析出 {} 个 `pub unsafe extern \"C\" fn` —— \
         解析器失明或存在非标准写法的导出",
        parsed.len()
    );
    println!("[契约] cabi.rs 解析出 {} 个 C 导出", parsed.len());
}

// ---------- 2. 检查器自检：喂改坏的文本，必须报红 ----------

// 自检**不读真实头文件**：喂进去的是固定字面量。理由有两条 ——
//   ① 确定性：真头文件日后合法改动（如 `opi_switch_mode` 换签名）不该让自检假红；
//   ② 可读性：真文件有 160 行，变异没命中时 panic 会把整份文件倒进日志。
// 段落里刻意埋了注释干扰（`///` 里的假声明 `opi_ghost`）、`#define`、多行 typedef、
// 与 Rust 别名不同的拼写（`uint32_t` vs `u32`）—— 这些若解析器处理不了，
// `checker_accepts_matching_fixture`（绿色基线）会先红。
const FIX_RUST: &str = r#"
/// 注释里也有 ( 括号 ) 和 ; 分号
#[repr(C)]
pub struct OpiString {
    pub ptr: *const u16,
    pub len: usize,
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_demo(a: u32, b: *const u16) -> bool { true }

#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_demo2() {}
"#;

const FIX_H: &str = r#"
/// 注释里的假声明：bool opi_ghost(void); —— 剥注释后必须一字不剩
#define OPI_SOMETHING (1u << 3)
#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    const uint16_t *ptr;
    size_t len;
} OpiString;

bool opi_demo(uint32_t a, const uint16_t *b);
void opi_demo2(void);
"#;

/// 绿色基线：配对的一对 fixture 必须**零差异**。
/// 这条不是走过场 —— 下面的变异测试断言的是「有差异」，若解析器失明导致
/// 两边都解析成空，「有差异」同样成立（`头文件缺少 opi_demo` 也含子串）。
/// 只有这条绿了，那几条红的含义才是「检查器看见了变异」。
#[test]
fn checker_accepts_matching_fixture() {
    let diffs = check_header(FIX_RUST, FIX_H);
    assert!(
        diffs.is_empty(),
        "配对 fixture 被判有差异 —— 解析器或检查器坏了：\n  {}",
        report(diffs)
    );
    assert_eq!(
        parse_rust_exports(FIX_RUST).len(),
        2,
        "fixture 只该解析出 2 个导出"
    );
    assert_eq!(parse_c_decls(FIX_H).len(), 2, "fixture 只该解析出 2 条声明");
    // 注释里的假声明不许被当真。
    assert!(
        !parse_c_decls(FIX_H).iter().any(|(n, _)| n == "opi_ghost"),
        "注释里的 `opi_ghost` 被解析成了真声明 —— 注释剥离失效"
    );
    // 结构体基线：配对则零差异。
    assert!(check_struct(FIX_RUST, FIX_H, "OpiString").is_empty());
}

#[test]
fn checker_reports_parameter_type_mutant() {
    // i32 → u32：编译期看不出来、链接期也看不出来，运行期读错位。
    let mutant = FIX_H.replace("bool opi_demo(uint32_t a,", "bool opi_demo(int32_t a,");
    assert_ne!(mutant, FIX_H, "变异没生效");
    let diffs = check_header(FIX_RUST, &mutant);
    assert_eq!(
        diffs.len(),
        1,
        "对参数类型变异应恰好报 1 条：\n  {}",
        report(diffs)
    );
    assert!(diffs[0].contains("opi_demo") && diffs[0].contains("第 1 个参数"));
}

#[test]
fn checker_reports_deleted_declaration() {
    let mutant = FIX_H.replace("void opi_demo2(void);", "");
    assert_ne!(mutant, FIX_H, "变异没生效");
    let diffs = check_header(FIX_RUST, &mutant);
    assert_eq!(
        diffs.len(),
        1,
        "对删声明应恰好报 1 条：\n  {}",
        report(diffs)
    );
    assert!(diffs[0].contains("头文件缺少") && diffs[0].contains("opi_demo2"));
}

#[test]
fn checker_reports_return_type_mutant() {
    let mutant = FIX_H.replace("bool opi_demo(", "void opi_demo(");
    assert_ne!(mutant, FIX_H, "变异没生效");
    let diffs = check_header(FIX_RUST, &mutant);
    assert_eq!(
        diffs.len(),
        1,
        "对返回类型变异应恰好报 1 条：\n  {}",
        report(diffs)
    );
    assert!(diffs[0].contains("返回类型不一致"), "{}", diffs[0]);
}

#[test]
fn checker_reports_struct_field_mutant() {
    let mutant = FIX_H.replace(
        "    const uint16_t *ptr;\n    size_t len;\n} OpiString;",
        "    size_t len;\n    const uint16_t *ptr;\n} OpiString;",
    );
    assert_ne!(mutant, FIX_H, "变异没生效");
    let diffs = check_struct(FIX_RUST, &mutant, "OpiString");
    assert!(
        !diffs.is_empty(),
        "检查器对结构体字段**顺序**颠倒无反应 —— 这正是运行期静默读错位的那一类"
    );
}

/// **本轮最要紧的那个方向**：Rust 侧新增导出、头文件还没跟上
/// （历史 19 → 20 → 22 → 28，最近一次扩容是 `861b7a7`，每次都是这么发生的）。
#[test]
fn checker_reports_export_missing_from_header() {
    let mutant = format!(
        "{FIX_RUST}\n#[unsafe(no_mangle)]\npub unsafe extern \"C\" fn opi_new_export() -> bool {{ true }}\n"
    );
    let diffs = check_header(&mutant, FIX_H);
    assert_eq!(
        diffs.len(),
        1,
        "应恰好报 1 条「头文件缺少」：\n  {}",
        report(diffs)
    );
    assert!(diffs[0].contains("头文件缺少") && diffs[0].contains("opi_new_export"));
}

/// 反向：头文件单方面多写/拼错一条声明（Swift 会去找一个不存在的符号 ——
/// 这是**编译不过**，比签名错好，但同样该拦）。
#[test]
fn checker_reports_extra_header_declaration() {
    let mutant = FIX_H.replace(
        "void opi_demo2(void);",
        "void opi_demo2(void);\nOpiString opi_bufer(void);",
    );
    assert_ne!(mutant, FIX_H, "变异没生效");
    let diffs = check_header(FIX_RUST, &mutant);
    assert_eq!(
        diffs.len(),
        1,
        "应恰好报 1 条「多出声明」：\n  {}",
        report(diffs)
    );
    assert!(diffs[0].contains("opi_bufer"), "{}", diffs[0]);
}

// ---------- 3. 真正的门禁 ----------

/// Apple 侧唯一一份全量声明：要求**集合相等 + 逐参同型 + 结构体同布局**。
#[test]
fn macos_header_matches_cabi() {
    let rust = read(CABI);
    let h = read(MACOS_H);
    let mut diffs = check_header(&rust, &h);
    diffs.extend(check_struct(&rust, &h, "OpiString"));
    diffs.extend(check_struct(&rust, &h, "OpiKeyEventResult"));
    assert!(
        diffs.is_empty(),
        "crates/opi-ffi/src/cabi.rs 与 macos/OpiFFI.h 漂移（共 {} 条）：\n  {}",
        diffs.len(),
        report(diffs)
    );
    let n = parse_rust_exports(&rust).len();
    println!("[契约] macos/OpiFFI.h 与 cabi.rs 逐条一致（{n} 个导出）");
}

/// `ios/OpiFFI.h` 是**纯转发头**：自己一条函数都不许声明，
/// 且必须 `#include` 那份全量头（相对路径断了就没人看得见声明）。
#[test]
fn ios_header_is_pure_forwarder() {
    let h = read(IOS_H);
    let decls = parse_c_decls(&h);
    assert!(
        decls.is_empty(),
        "ios/OpiFFI.h 应当只转发、不声明，却出现了 {} 条：{:?}",
        decls.len(),
        decls.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>()
    );
    assert!(
        h.contains("#include \"../macos/OpiFFI.h\""),
        "ios/OpiFFI.h 的转发 `#include \"../macos/OpiFFI.h\"` 不见了（断链 = 静默失去全部声明）"
    );
    // 非空转护栏：「0 条」必须是**文件**的性质，不能是解析器对这个文件失明的假绿 ——
    // 在同一份文本上人为加一条声明，解析器必须看得见它。
    let probe = format!("{h}\nbool opi_probe(const uint16_t *p, size_t n);\n");
    assert_eq!(
        parse_c_decls(&probe).len(),
        1,
        "在 ios/OpiFFI.h 的文本上加一条声明后解析器仍看不见 —— 上面的「0 条」是假绿"
    );
    println!("[契约] ios/OpiFFI.h 是纯转发头（0 条自有声明，探针证明解析器不瞎）");
}

/// 鸿蒙侧是**有意设计的 use-site 子集**：每条声明都要与 `cabi.rs` 同型，
/// 且每条都必须真的被 N-API 桥调用（抄进来不用的就会变成第四份漂移源）。
/// **不要求等于全集** —— 那是有意设计，不是缺陷。
#[test]
fn harmony_header_is_consistent_use_site_subset() {
    let rust = read(CABI);
    let h = read(HARMONY_H);
    let decls = parse_c_decls(&h);
    assert!(
        !decls.is_empty(),
        "harmony 头文件解析出 0 条声明：解析器失明"
    );

    let full: std::collections::BTreeMap<String, Sig> =
        parse_rust_exports(&rust).into_iter().collect();
    let mut diffs = Vec::new();
    let mut names = BTreeSet::new();
    for (name, csig) in &decls {
        names.insert(name.clone());
        match full.get(name) {
            None => diffs.push(format!("`{name}` 在 cabi.rs 里不存在（拼错或已被删）")),
            Some(rsig) if rsig != csig => diffs.push(format!(
                "`{name}` 签名不一致：cabi.rs {:?} vs 头文件 {:?}",
                rsig, csig
            )),
            Some(_) => {}
        }
    }

    // 「声明了但桥里没用」—— 子集的意义就在于此，抄而不调用会积累漂移。
    let bridge = strip_c_comments(&read(HARMONY_BRIDGE));
    for name in &names {
        let hit = bridge
            .match_indices(name.as_str())
            .any(|(i, _)| bridge[i + name.len()..].trim_start().starts_with('('));
        if !hit {
            diffs.push(format!(
                "`{name}` 在 harmony/cpp/opi_ffi.h 里声明了，但 napi_bridge.c 从未调用"
            ));
        }
    }

    assert!(
        diffs.is_empty(),
        "harmony/cpp/opi_ffi.h 与 cabi.rs / napi_bridge.c 不一致：\n  {}",
        report(diffs)
    );
    println!(
        "[契约] harmony/cpp/opi_ffi.h：{} 条声明，全部与 cabi.rs 同型且都被桥调用（全集 {} 条，有意子集）",
        decls.len(),
        full.len()
    );
    // 允许子集 == 全集（有意扩桥时），但若真变成全集，说明「use-site」这个设计前提
    // 已经变了 —— 打印出来，不判红（判红会拦下一次合法的扩容）。
    if decls.len() == full.len() {
        println!(
            "[契约][注意] harmony 头文件已覆盖全部 {} 个导出，不再是子集",
            full.len()
        );
    }
}

/// 三份头文件里的 `OpiString` / `OpiKeyEventResult` 必须互相同布局 ——
/// 鸿蒙侧也自己抄了一份 typedef，布局漂了同样是静默读错位。
#[test]
fn harmony_struct_layouts_match_macos() {
    let macos = read(MACOS_H);
    let harmony = read(HARMONY_H);
    for name in ["OpiString", "OpiKeyEventResult"] {
        let m = parse_c_typedef_struct(&macos, name);
        // 两侧都解析出 0 个字段时 `assert_eq!` 会假绿 —— 先钉住非空。
        assert!(
            !m.is_empty(),
            "`{name}` 在 macos/OpiFFI.h 里解析出 0 个字段：解析器失明，下面的相等断言是空转"
        );
        assert_eq!(
            m,
            parse_c_typedef_struct(&harmony, name),
            "`{name}` 的 typedef 在 macos/ 与 harmony/ 两侧布局不同"
        );
    }
    // Rust 侧那两个 #[repr(C)] 结构体也要解析得出字段（失明则上面比的是空壳）。
    let src = read(CABI);
    assert!(!parse_rust_struct(&src, "OpiString").is_empty());
    assert!(!parse_rust_struct(&src, "OpiKeyEventResult").is_empty());
}

// ---------- 4. 头文件真的能被 C 编译器吃下去 ----------

/// 头文件 + 真 C 消费者**编得过**（`clang -fsyntax-only -Werror`，C 与 C++ 两种模式，
/// 再加一步真出目标文件的 `-c`）。
///
/// 与下面的 `run.sh` 分工：这里**不需要链接产物**，所以每次 `cargo test` 都会跑，
/// 拦「声明写错导致 C 编不过 / 消费者用错签名」；`run.sh` 才真链接真调用。
/// clang 不在 PATH 上就跳过（不是红 —— 本机以外的环境不该被这条卡住）。
#[test]
fn header_compiles_and_consumer_builds() {
    if std::process::Command::new("clang")
        .arg("--version")
        .output()
        .is_err()
    {
        println!("[契约][跳过] 本机没有 clang，跳过 C 编译验证");
        return;
    }
    let consumer = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/c_abi/consumer.c");
    let inc = concat!(env!("CARGO_MANIFEST_DIR"), "/../../macos");
    let obj = std::env::temp_dir().join("opi_abi_consumer_check.o");

    // ① 头文件单独过（C / C++ 两种模式）—— 转发的 ios 头也一并过。
    for extra in [Vec::new(), vec!["-x", "c++"]] {
        let out = std::process::Command::new("clang")
            .args(&extra)
            .args([
                "-fsyntax-only",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-I",
                inc,
                consumer,
            ])
            .output()
            .expect("跑 clang");
        assert!(
            out.status.success(),
            "clang{} 吃不下 macos/OpiFFI.h + 消费者：\n{}",
            if extra.is_empty() { "" } else { "++" },
            String::from_utf8_lossy(&out.stderr)
        );
    }
    // ② 真出目标文件（`-fsyntax-only` 不生成代码，这里是完整前端 + 代码生成）。
    let out = std::process::Command::new("clang")
        .args([
            "-std=c11", "-Wall", "-Wextra", "-Werror", "-c", "-I", inc, consumer,
        ])
        .arg("-o")
        .arg(&obj)
        .output()
        .expect("跑 clang");
    assert!(
        out.status.success(),
        "消费者编不出目标文件：\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = fs::remove_file(&obj);
    println!("[契约] macos/OpiFFI.h + 真 C 消费者：clang(C/C++)/clang -c 全过");
}

// ---------- 5. 交叉核对：导出符号确实在库里 ----------

/// 契约是「声明 ↔ 声明」，但**声明与产物**也要对上：`cabi.rs` 里解析出的名字
/// 必须是 `.so` 真正导出的符号（防 `#[no_mangle]` 被误删 / 被 feature 关掉）。
/// 构建产物缺失时**跳过**（不是红）—— CI 上大概率没跑过 `cargo build --release`。
#[test]
fn exported_symbols_exist_in_built_library() {
    let candidates = [
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/release/libopi_ffi.so"
        ),
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/debug/libopi_ffi.so"
        ),
    ];
    let Some(lib) = candidates.iter().find(|p| Path::new(p).exists()) else {
        println!(
            "[契约][跳过] 没有 libopi_ffi.so，跳过符号交叉核对（`cargo build -p opi_ffi --release` 后复跑）"
        );
        return;
    };
    let bytes = fs::read(lib).expect("读 .so");
    let text = String::from_utf8_lossy(&bytes);
    let expect = parse_rust_exports(&read(CABI));
    let missing: Vec<&str> = expect
        .iter()
        .map(|(n, _)| n.as_str())
        // 匹配 **NUL 结尾**的名字：`.dynstr` 里的符号名以 NUL 分隔，用裸子串会把
        // `opi_load` 在 `opi_load_trad` 里「找到」—— 少一个导出反而判绿。
        .filter(|n| !text.contains(&format!("{n}\0")))
        .collect();
    assert!(
        missing.is_empty(),
        "{} 里找不到这些导出符号（声明有、产物没有）：{missing:?}",
        lib
    );
    println!("[契约] {} 里核对到全部 {} 个导出符号", lib, expect.len());
}
