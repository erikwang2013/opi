// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **500 行规矩的门禁** —— 此前这条规矩**零工具把守**：全仓 `wc -l` 只出现在
//! `crates/opi-ffi/tests/c_abi/run.sh`（那里数的是导出符号），CI / scripts / tests
//! 都没有行数检查 ⇒ 越线**没有任何工具会发现**，全靠人记得量。本会话手工量出过两次实锤
//! （`fcitx5-opi/src/lib.rs` 516、`tsf-opi/src/vk.rs` 520）。
//!
//! ## 口径（三条，都是踩过的形状）
//!
//! * **按文件系统遍历，不用 `git ls-files`**：本仓大量**未跟踪但真实存在**的源码
//!   （拆分产物：`crates/fcitx5-opi/cpp/opi_json.h`、`crates/tsf-opi/src/vk_tests.rs` …）。
//!   `git ls-files` 会**整个漏掉**它们，而它们正是最该被看住的那批。
//! * **行数 = 文件有多少行**，不是 `wc -l`（那数的是换行符）：**末尾没有换行符的最后一行照样计入**。
//!   差别正好落在 500 这一格上 —— 501 行且末行无换行符时 `wc -l` 报 500（**放过**），
//!   本门禁报 501（**红**）。所以这里不采用 `wc -l` 语义。
//! * 上限**写死 500**，不做成可配置的（明文规矩，不是参数）。
//!
//! ## 排除项（每条都得说得出理由）
//!
//! | 排除 | 理由 |
//! |---|---|
//! | `*.md` | 本仓明文「500 行只管**源码**」；`plans/` 里本来就有 2000+ 行的文档 |
//! | `.` 开头的目录 | 版本控制/工具目录（`.git` `.github` `.agents` `.claude-flow` `.gradle`） |
//! | `target/` `build/` `__pycache__/` `node_modules/` | 构建与缓存产物 |
//! | `data/generated/` `data/raw/` | 生成物与原始数据（`.tsv` 等） |
//! | `Cargo.lock`（按文件名） | cargo 生成并重写的锁文件（本仓 730 行），不是源码 —— 这是**唯一**按名字放行的 |
//! | 非 UTF-8 文件 | 二进制（`.opid` `.so` `.a` `.class` `.deb` `ruvector.db` …）—— 按**内容**判，不列扩展名 |
//!
//! ## 覆盖边界（本门禁**不**保证的）
//!
//! * **隐藏目录整个不扫**：`.github/workflows/*.yml` 之类不在内。它们不是源码，
//!   但**如实记**：若哪天 CI 配置里塞了长文件，本门禁看不见。
//! * **只数行数，不看内容**：500 行以内的文件里写了什么，本门禁一句话都不说
//!   （压缩成一行的一万字符文件它也照绿）。
//! * **生成物不在内**：某个手工源文件若被生成器改写成超线，它会被抓到；但生成器
//!   **自己**的产物目录已被排除。
//! * 它是**文本扫描**，判据只是「行数」与「能不能 UTF-8 解码」，与 `cfg`、宏、门禁无关。

use std::path::{Path, PathBuf};

/// 明文规矩，写死。
const LIMIT: usize = 500;

/// 目录级排除：`.` 开头的一律不看（VCS/工具/缓存），其余按名。
fn dir_is_excluded(name: &str) -> bool {
    name.starts_with('.') || matches!(name, "target" | "build" | "__pycache__" | "node_modules")
}

/// 路径级排除：生成物与原始数据（按**路径前缀**，不是按目录名 ——
/// `crates/fcitx5-opi/data/` 是词的库数据，不在排除之列）。
fn path_is_excluded(rel: &str) -> bool {
    rel.starts_with("data/generated/") || rel.starts_with("data/raw/")
}

/// 生成物（按**文件名**，任何目录下都算）。`Cargo.lock` 是 cargo 生成并**重写**的锁文件
/// （本仓 730 行，行数由依赖图决定，手改无意义）—— 它不是源码，归「生成物」那一类。
/// ⚠️ 这是本门禁**唯一**按名字放行的文件；再有超线的**源码**不许往这里加，得拆文件。
fn is_generated(name: &str) -> bool {
    name == "Cargo.lock"
}

fn walk(dir: &Path, root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        // 符号链接一律不跟：避免环，也避免把仓库外的树拉进来。
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        if ft.is_dir() {
            if !dir_is_excluded(&name) {
                walk(&p, root, out);
            }
        } else if let Ok(rel) = p.strip_prefix(root) {
            let rel = rel.to_string_lossy().replace('\\', "/");
            if !name.ends_with(".md") && !is_generated(&name) && !path_is_excluded(&rel) {
                out.push(p);
            }
        }
    }
}

/// **行数**（不是 `wc -l`）：末尾无换行符的最后一行照样计入。
///
/// ⚠️ `str::lines()` **本来就是这个语义**（末尾的换行符不额外产生一行，末行缺换行符
/// 也照样算一行）—— 所以直接数就对了，**不要再补 `+1`**：我第一版顺手补了，
/// 结果「500 行且末行无换行符」这个**合法**文件被报成 501 而假红（M2b 边界探针抓到的）。
fn line_count(src: &str) -> usize {
    src.lines().count()
}

#[test]
fn no_source_file_exceeds_500_lines() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/opi-ffi → 仓库根")
        .to_path_buf();
    let mut files = Vec::new();
    walk(&root, &root, &mut files);

    let mut over: Vec<(usize, String)> = Vec::new();
    let (mut text, mut binary) = (0usize, 0usize);
    let mut seen: Vec<String> = Vec::new();
    for p in &files {
        let Ok(bytes) = std::fs::read(p) else {
            continue;
        };
        let Ok(src) = String::from_utf8(bytes) else {
            binary += 1;
            continue;
        };
        text += 1;
        let rel = p
            .strip_prefix(&root)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('\\', "/");
        let n = line_count(&src);
        if n > LIMIT {
            over.push((n, rel.clone()));
        }
        seen.push(rel);
    }
    over.sort_by(|a, b| b.cmp(a));
    println!("\n500 行门禁：扫到文本源码 {text} 个（跳过二进制 {binary} 个）");

    // 非空转护栏：遍历真的走到了每一个角落 —— 少了文件就是「门禁对它们失明」。
    // 这几个是**跨目录**挑的，含一个**未跟踪**的拆分产物（`git ls-files` 会漏的那种）。
    for must in [
        "crates/engine-core/src/keys.rs",
        "crates/opi-ffi/src/cabi.rs",
        "crates/fcitx5-opi/cpp/opi_json.h",
        "harmony/cpp/opi_ffi.h",
        "ios/OpiEngine.swift",
        "macos/OpiEngine.swift",
        "shared/pet/OpiPet.kt",
    ] {
        assert!(
            seen.iter().any(|s| s == must),
            "{must} 没被扫到 —— 遍历坏了（或它被挪走/改名了），先修遍历再谈行数"
        );
    }
    // 再一条：**每一棵源码树都得有文件被扫到** —— 这条比数字更准，专门抓
    // 「某条排除规则把整棵树吃掉了」（那才是真失明，且不会随文件增删误报）。
    for tree in [
        "crates/", "android/", "ios/", "macos/", "harmony/", "desktop/", "shared/", "scripts/",
    ] {
        assert!(
            seen.iter().any(|s| s.starts_with(tree)),
            "{tree} 下一个文件都没扫到 —— 排除规则吃掉了整棵树"
        );
    }
    // 兜底下限：只用来抓「遍历整个坏掉」。实测 268 个（2026-09-28，与独立 find 口径
    // 259 文本 + 12 二进制交叉核对过，本遍历还多出 9 个点文件）；正常增删文件不该碰它。
    assert!(
        text >= 200,
        "只扫到 {text} 个文本源码 —— 遍历坏了（正常是两百多个）"
    );

    assert!(
        over.is_empty(),
        "有 {} 个源码文件超过 {LIMIT} 行：\n  {}\n拆文件（按职责/测试模块切），别改这条上限",
        over.len(),
        over.iter()
            .map(|(n, f)| format!("{n:>5} 行  {f}"))
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}
