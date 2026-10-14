// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

use engine_data::{Dictionary, load_bytes};
use opi_tools::compiler::{compile_file, parse_dict_report};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

/// 读者已经走了的那条流（`opi-tools … | head` 关掉读端之后）：置位后静默丢弃其后续输出。
static STDOUT_GONE: AtomicBool = AtomicBool::new(false);
static STDERR_GONE: AtomicBool = AtomicBool::new(false);

/// 往 stdout / stderr 写。三种结局，与 std 的 `println!` 只有第一种相同：
/// - 正常写出去；
/// - `BrokenPipe`：读者先退出，Unix 惯例里**不是错误** —— 但也**不能当场退出**：
///   `compile` 后面还有产物要落盘、`verify` 后面还有退出码要定，此刻 exit 会把
///   「没做的事」报成成功。所以只置位、丢弃这条流上后续输出，工作照做完、退出码照原样；
/// - 其它写失败（磁盘满 / EIO…）：照常报错 + exit 1 —— 不吞。
///   这一支在本机（rustc 1.97.1）没有测试覆盖：std 的 stdout/stderr 对只读 fd 的
///   EBADF 返回 `Ok(())`，失败到不了这里（详见 `tests/cli_pipe.rs` 头部说明）。
fn emit(to_stderr: bool, args: std::fmt::Arguments<'_>) {
    let gone = if to_stderr {
        &STDERR_GONE
    } else {
        &STDOUT_GONE
    };
    if gone.load(Ordering::Relaxed) {
        return;
    }
    let failed = if to_stderr {
        std::io::stderr().lock().write_fmt(args)
    } else {
        std::io::stdout().lock().write_fmt(args)
    };
    if let Err(e) = failed {
        if e.kind() == std::io::ErrorKind::BrokenPipe {
            gone.store(true, Ordering::Relaxed);
        } else {
            // 报错本身也可能写不出去（还是往 stderr），失败就算了 —— 但退出码要给。
            let _ = writeln!(std::io::stderr(), "opi-tools: write failed: {e}");
            std::process::exit(1);
        }
    }
}

/// [`emit`] 的两个语法糖，签名与 `println!` / `eprintln!` / `print!` 一致。
macro_rules! outln {
    ($($arg:tt)*) => { emit(false, format_args!("{}\n", format_args!($($arg)*))) };
}
macro_rules! out {
    ($($arg:tt)*) => { emit(false, format_args!($($arg)*)) };
}
macro_rules! errln {
    ($($arg:tt)*) => { emit(true, format_args!("{}\n", format_args!($($arg)*))) };
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        // 版本来自 workspace.package.version（单一版本源），与发布 tag 对齐。
        Some("-V" | "--version") => {
            outln!("opi-tools {}", env!("CARGO_PKG_VERSION"));
            out!("{}", opi_tools::OPI_PET);
        }
        Some("compile") => {
            let (Some(input), Some(output)) = (args.get(2), args.get(3)) else {
                errln!("usage: opi-tools compile <input.tsv|dict.yaml> <output.opid>");
                std::process::exit(2);
            };
            let text = match std::fs::read_to_string(input) {
                Ok(t) => t,
                Err(e) => {
                    errln!("read {}: {e}", input);
                    std::process::exit(1);
                }
            };
            let report = parse_dict_report(&text);
            outln!("input lines: {}", text.lines().count());
            outln!("kept entries: {}", report.entries.len());
            // 跳过本身是正常的（注释行、带声调的拼音行），**静默**才不正常：
            // 报一行统计，操作员对着 TSV 能自查。真正的格式错由 compile_file 拒绝。
            if !report.malformed.is_empty() || report.unusable > 0 {
                let first = match report.malformed.first() {
                    Some((n, s)) => format!("，首个列数不符在第 {n} 行：{s:?}"),
                    None => String::new(),
                };
                errln!(
                    "warning: 跳过 {} 行（列数不符 {}、词条不可用 {}）{first}",
                    report.malformed.len() + report.unusable,
                    report.malformed.len(),
                    report.unusable
                );
            }
            if let Err(e) = compile_file(report, Path::new(input), Path::new(output)) {
                errln!("{e}");
                std::process::exit(1);
            }
            let size = std::fs::metadata(output).map(|m| m.len()).unwrap_or(0);
            outln!("wrote {} ({} bytes)", output, size);
            out!("{}", opi_tools::OPI_PET);
        }
        Some("verify") => {
            let Some(path) = args.get(2) else {
                errln!("usage: opi-tools verify <file.opid>");
                std::process::exit(2);
            };
            let bytes = match std::fs::read(path) {
                Ok(b) => b,
                Err(e) => {
                    errln!("read {}: {e}", path);
                    std::process::exit(1);
                }
            };
            let t0 = std::time::Instant::now();
            match load_bytes(bytes) {
                Ok(d) => {
                    let elapsed = t0.elapsed();
                    outln!("file: {}", path);
                    outln!("checksum: ok");
                    outln!("entries: {}", d.len());
                    outln!("load: {:.1}ms", elapsed.as_secs_f64() * 1000.0);
                    for sample in ["hao", "wo", "n"] {
                        let top: Vec<String> =
                            d.query(sample, 3).iter().map(|e| e.word.clone()).collect();
                        outln!("query \"{sample}\": {}", top.join(" "));
                    }
                    out!("{}", opi_tools::OPI_PET);
                }
                Err(e) => {
                    errln!("verify failed: {e:?}");
                    std::process::exit(1);
                }
            }
        }
        // 取键转储（2026-10-15 加）：`opi-tools dump-keys <file.opid>` →
        // 每行 `字<TAB>键1,键2,…`（只列**单字**条目，键去重后字典序）。
        // 用途：`scripts/gen_polyphone.py --dump <本命令的输出>` —— 多音字补读音表要
        // 「词库里缺哪个读音」这个差集，而 `.opid` 是二进制、python 读不了；
        // 放在本工具里是为了让那条生成链**可复现**（跑一次命令就能重生成表）。
        Some("dump-keys") => {
            let Some(path) = args.get(2) else {
                errln!("usage: opi-tools dump-keys <file.opid>");
                std::process::exit(2);
            };
            let bytes = match std::fs::read(path) {
                Ok(b) => b,
                Err(e) => {
                    errln!("read {}: {e}", path);
                    std::process::exit(1);
                }
            };
            let d = match load_bytes(bytes) {
                Ok(d) => d,
                Err(e) => {
                    errln!("load failed: {e:?}");
                    std::process::exit(1);
                }
            };
            // 字 → （键集合, 最大词频）。**词频列是给 gen_polyphone.py 用的**：
            // 词库的词频是位置编码（十亿量级），Unihan 的词次（几千）不同量纲 ——
            // 补读音的候选要按**该字在词库里的常用度**落位，拿这一列当量纲桥。
            let mut rev: std::collections::BTreeMap<
                String,
                (std::collections::BTreeSet<String>, u32),
            > = std::collections::BTreeMap::new();
            d.visit_entries(&mut |key: &str, word: &str, f: u32| {
                if word.chars().count() == 1 {
                    let e = rev.entry(word.to_string()).or_default();
                    e.0.insert(key.to_string());
                    e.1 = e.1.max(f);
                }
            });
            for (ch, (keys, freq)) in &rev {
                outln!(
                    "{ch}\t{}\t{freq}",
                    keys.iter().cloned().collect::<Vec<_>>().join(",")
                );
            }
            errln!(
                "（单字 {} 个；本转储仅供 gen_polyphone.py 取差集）",
                rev.len()
            );
        }
        _ => {
            errln!("usage: opi-tools <compile|verify|dump-keys> ...");
            errln!("       opi-tools -V | --version");
            std::process::exit(2);
        }
    }
}
