// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `opi-tools` 命令行的**边界**：参数、路径、I/O 失败、畸形 TSV。
//!
//! 与既有测试的分工：`cli.rs` 测正常往返与版本输出，`cli_verify.rs` 测
//! 「好文件通过 / 坏文件不通过」，`dict_crosslayer.rs` 测两层判定一致。
//! 本文件只测**边界上会不会有第三种结果**——不是成功也不是干净失败，
//! 而是 panic、静默写坏文件、或把源文件吃掉。
//!
//! 三条硬规矩（断言按这个写，不是按「现状是什么」写）：
//! 1. 退出码只有三档：0 成功、1 I/O 或格式错、2 用法错。**101（panic）不是档**。
//! 2. stderr 里出现 `panicked at` 就是崩溃，`catch_unwind` 兜住也不算干净失败。
//! 3. stdout 打出的 `kept entries: N` 必须等于写进文件里的条目数 ——
//!    这行数是操作员判断「我的 TSV 被认了多少」的**唯一**依据。

use engine_data::load_bytes;
use std::path::{Path, PathBuf};

struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!("opi-cli-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("建临时目录");
        Dir(p)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let p = self.path(name);
        std::fs::write(&p, bytes).expect("写临时文件");
        p
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(args: &[&str]) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_opi-tools"))
        .args(args)
        .output()
        .expect("跑 opi-tools")
}

fn s(p: &Path) -> &str {
    p.to_str().expect("临时路径是 UTF-8")
}

/// 退出码必须落在 {0,1,2}，且非 0 时 stderr 要有话说（不能只有 backtrace）。
/// 返回 `(退出码, stdout, stderr)`，供各用例继续断言。
fn code_and_streams(out: &std::process::Output, what: &str) -> (i32, String, String) {
    let code = out
        .status
        .code()
        .unwrap_or_else(|| panic!("{what}：进程被信号杀死（无退出码），这是崩溃不是失败"));
    let (so, se) = (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    );
    assert!(
        (0..=2).contains(&code),
        "{what}：退出码 {code} 不在 {{0,1,2}} 三档内（101 = panic）\nstderr: {se}"
    );
    assert!(
        !se.contains("panicked at"),
        "{what}：拒绝必须是干净错误，不能 panic\nstderr: {se}"
    );
    if code != 0 {
        assert!(
            !se.trim().is_empty(),
            "{what}：非 0 退出却不给任何错误说明（stdout: {so}）"
        );
    }
    (code, so, se)
}

/// 用法错一律 2：没有子命令、子命令不认识、子命令缺参数。
/// 用 2 而不是 1 是「壳脚本能区分『我调错了』与『文件坏了』」的前提。
#[test]
fn usage_errors_exit_2() {
    let dir = Dir::new("usage");
    let tsv = dir.write("a.tsv", b"\xe5\xa5\xbd\thao\n");
    for (what, args) in [
        ("无参数", vec![]),
        ("未知子命令", vec!["frobnicate"]),
        ("空子命令", vec![""]),
        ("compile 缺输入", vec!["compile"]),
        ("compile 缺输出", vec!["compile", s(&tsv)]),
        ("verify 缺路径", vec!["verify"]),
        ("大小写敏感", vec!["COMPILE", s(&tsv), "x.opid"]),
    ] {
        let out = run(&args);
        let (code, _, se) = code_and_streams(&out, what);
        assert_eq!(code, 2, "{what}：用法错必须是 2，stderr: {se}");
    }
}

/// I/O 错一律 1，且错误信息里要有**出错的那个路径**（否则用户不知道哪个参数写错了）。
///
/// 输出路径用「父目录不存在」而不是「chmod 500 的目录」：测试若以 root 跑，
/// 权限位形同虚设，那样这条断言会变成永远为真的假绿。
#[test]
fn io_errors_exit_1_and_name_the_path() {
    let dir = Dir::new("io");
    let tsv = dir.write("a.tsv", b"\xe5\xa5\xbd\thao\n");
    let missing = dir.path("nope.tsv");

    for (what, args, needle) in [
        (
            "compile 输入不存在",
            vec!["compile", s(&missing), "o.opid"],
            "nope.tsv",
        ),
        ("verify 文件不存在", vec!["verify", s(&missing)], "nope.tsv"),
        (
            "compile 输入是目录",
            vec!["compile", s(&dir.0), "o.opid"],
            "opi-cli-",
        ),
        ("verify 输入是目录", vec!["verify", s(&dir.0)], "opi-cli-"),
        (
            "compile 输出父目录不存在",
            vec!["compile", s(&tsv), s(&dir.path("no/such/dir/o.opid"))],
            "o.opid",
        ),
    ] {
        let out = run(&args);
        let (code, _, se) = code_and_streams(&out, what);
        assert_eq!(code, 1, "{what}：I/O 错必须是 1，stderr: {se}");
        assert!(
            se.contains(needle),
            "{what}：错误信息必须点出出错路径（找 {needle:?}）：{se}"
        );
    }
}

/// 路径**按字面取**：带空格/中文的路径照常工作；`--` 不是选项分隔符而是文件名。
///
/// `--` 这条是本项目 CLI 的**既成语义**（`main.rs` 只有 `args.get(1)` 一层匹配，
/// 没有选项解析）：写清楚是因为「`--` 被当成文件名 → 报『读不到 --』」与
/// 「`--` 被当成分隔符 → 把 `ok.tsv` 当输入」是两种完全不同的行为，
/// 而后者会静默编译错文件。断言钉住前者。
#[test]
fn paths_are_taken_literally() {
    let dir = Dir::new("paths");
    let tsv = dir.write("词 库 表.tsv", b"\xe5\xa5\xbd\thao\n");
    let out_p = dir.path("输 出.opid");
    let out = run(&["compile", s(&tsv), s(&out_p)]);
    let (code, so, se) = code_and_streams(&out, "中文+空格路径");
    assert_eq!(code, 0, "{so}{se}");
    assert!(out_p.exists(), "带空格的输出路径必须真的写出来");

    // 以 `-` 开头的文件名同样按字面取（会被 `--version` 之类的分支吃掉才是 bug）。
    let dash = dir.write("-dash.tsv", b"\xe5\xa5\xbd\thao\n");
    let dash_out = dir.path("-dash.opid");
    let out = run(&["compile", s(&dash), s(&dash_out)]);
    let (code, so, se) = code_and_streams(&out, "以 - 开头的路径");
    assert_eq!(code, 0, "{so}{se}");

    // `--` 是文件名，不是分隔符 —— 于是「读不到 `--`」，而不是拿 ok.tsv 去编译。
    let out = run(&["compile", "--", s(&tsv), "x.opid"]);
    let (code, _, se) = code_and_streams(&out, "`--` 前缀");
    assert_eq!(code, 1, "`--` 会被当路径读 → 1；stderr: {se}");
    assert!(
        se.contains("--"),
        "错误必须点名 `--`（说明它被当路径而非分隔符）：{se}"
    );
}

/// 畸形 TSV：非法 UTF-8 必须被整体拒绝（1），空行/CRLF/无末尾换行必须照常成功。
///
/// 「非法 UTF-8 整体拒绝」是正确行为里最该钉住的一条：反过来若某天改成
/// 「跳过坏行」，一个 Latin-1 存出来的词库会被编译成**缺了一部分词**的产物，
/// 而 exit 0，没有任何信号 —— 正是本项目反复踩的那类静默失效。
#[test]
fn malformed_tsv_is_rejected_loudly_or_parsed_strictly() {
    let dir = Dir::new("tsv");

    // 1) 非法 UTF-8：整文件拒绝，且要说清是编码问题。
    for (name, bytes) in [
        (
            "lone_ff.tsv",
            b"\xe5\xa5\xbd\thao\n\xff\xfe\thao\n".to_vec(),
        ),
        (
            "latin1.tsv",
            b"\xe5\xa5\xbd\thao\n\xc3\x28\tnihao\n".to_vec(),
        ),
        ("truncated_char.tsv", b"\xe5\xa5\thao\n".to_vec()),
    ] {
        let p = dir.write(name, &bytes);
        let out = run(&["compile", s(&p), s(&dir.path("x.opid"))]);
        let (code, so, se) = code_and_streams(&out, name);
        assert_eq!(code, 1, "{name} 必须被拒：{so}{se}");
        assert!(
            se.contains("UTF-8"),
            "{name}：错误信息应点明编码问题（否则用户只会以为是路径错）：{se}"
        );
        assert!(
            !dir.path("x.opid").exists(),
            "{name}：被拒的输入不得留下半个输出文件"
        );
    }

    // 2) 合法但「长得怪」：空行、纯空白行、CRLF、无末尾换行 —— 都该照常成功。
    let weird = dir.write(
        "weird.tsv",
        "# 注释\n\n   \n\r\n\u{597d}\thao\r\n\u{53f7}\thao\t1200".as_bytes(),
    );
    let outp = dir.path("weird.opid");
    let out = run(&["compile", s(&weird), s(&outp)]);
    let (code, so, se) = code_and_streams(&out, "空行/CRLF/无末尾换行");
    assert_eq!(code, 0, "{so}{se}");
    assert!(
        so.contains("kept entries: 2"),
        "CRLF 与空行不该吃掉条目（`好`/`号` 各 1）：{so}"
    );
}

/// stdout 的 `kept entries: N` 必须等于产物里**真正**的条目数。
///
/// 这行数字是操作员唯一的反馈：编译一个 40 万词的 rime 词库时，没人会去
/// `verify` 再核对一遍。它若与实际不符（少算、多算），用户拿到的就是
/// 「看起来编进去了、其实没有」的词库。
///
/// 顺带覆盖解析规则的合并语义：重复 (pinyin, word) 取 max freq，
/// 于是 5 行输入只产出 3 条 —— 与 `kept entries` 一致才算过。
#[test]
fn reported_entry_count_matches_the_written_file() {
    let dir = Dir::new("count");
    for (tag, text, expect) in [
        ("plain", "\u{597d}\thao\n\u{53f7}\thao\t1200\n", 2usize),
        (
            "dup_merged",
            "\u{597d}\thao\t500\n\u{597d}\thao\t3000\n\u{597d}\thao\n",
            1,
        ),
        (
            "four_cols",
            "\u{597d}\thao\textra\tcol4\n\u{53f7}\thao\t1200\n",
            1,
        ),
        (
            "bad_freq",
            "\u{597d}\thao\t\u{4e24}\n\u{53f7}\thao\t1200\n",
            1,
        ),
        ("nonascii_pinyin", "\u{597d}\th\u{e0}o\n\u{53f7}\thao\n", 1),
        (
            "long_pinyin",
            &format!("\u{597d}\t{}\n", "a".repeat(256)),
            0,
        ),
    ] {
        let tsv = dir.write(&format!("{tag}.tsv"), text.as_bytes());
        let opid = dir.path(&format!("{tag}.opid"));
        let out = run(&["compile", s(&tsv), s(&opid)]);
        let (code, so, se) = code_and_streams(&out, tag);
        assert_eq!(code, 0, "{tag}: {so}{se}");

        let reported: usize = so
            .lines()
            .find_map(|l| l.strip_prefix("kept entries: "))
            .and_then(|n| n.parse().ok())
            .unwrap_or_else(|| panic!("{tag}: stdout 必须有 kept entries 行：{so}"));
        assert_eq!(reported, expect, "{tag}: 解析规则变了？stdout: {so}");

        let dict = load_bytes(std::fs::read(&opid).expect("读产物"))
            .unwrap_or_else(|e| panic!("{tag}: 产物必须可加载：{e:?}"));
        assert_eq!(
            written_entries(&dict),
            reported,
            "{tag}: stdout 说 {reported} 条，文件里却是 {} 条 —— 报数与产物不符",
            written_entries(&dict)
        );
    }
}

/// 产物里的条目数。用几个刁钻前缀把整表扫一遍会让用例变慢，
/// 这里直接读加载器报的 `len()`（就是格式头里的 count，与 `verify` 打印的是同一个值）。
fn written_entries(d: &engine_data::MmapDictionary) -> usize {
    use engine_core::dictionary::Dictionary;
    d.len()
}

/// 大量输入不该崩、不该丢条、不该退化成 O(n²)：
/// 5 万行（含故意制造的重复键）必须在一次呼吸内编完，且报数与产物一致。
#[test]
fn large_input_stays_linear_and_consistent() {
    let dir = Dir::new("large");
    let mut text = String::with_capacity(1 << 21);
    for i in 0..50_000usize {
        // 字面量构造而非随机：用例必须可复现，红了要能一眼看出是哪一行。
        let word = char::from_u32(0x4e00 + (i % 3000) as u32).unwrap();
        let pinyin = ["hao", "ni", "wo", "zhong", "guo"][i % 5];
        text.push_str(&format!("{word}\t{pinyin}\t{}\n", 1000 + (i % 97)));
    }
    let tsv = dir.write("large.tsv", text.as_bytes());
    let opid = dir.path("large.opid");
    let t0 = std::time::Instant::now();
    let out = run(&["compile", s(&tsv), s(&opid)]);
    let (code, so, se) = code_and_streams(&out, "5 万行");
    assert_eq!(code, 0, "{so}{se}");
    assert!(
        t0.elapsed() < std::time::Duration::from_secs(60),
        "5 万行耗时 {:?}，疑似退化（本机实测约 0.5s）",
        t0.elapsed()
    );
    let reported: usize = so
        .lines()
        .find_map(|l| l.strip_prefix("kept entries: "))
        .and_then(|n| n.parse().ok())
        .expect("kept entries 行");
    let dict = load_bytes(std::fs::read(&opid).unwrap()).expect("产物可加载");
    assert_eq!(written_entries(&dict), reported, "报数与产物不符：{so}");
    use engine_core::dictionary::Dictionary;
    assert_eq!(
        dict.query("hao", 4).len(),
        4,
        "抽样查询要有结果（防「编了个空壳」）"
    );
}

/// **bug**：超过 255 字节的 word 让 `compile` 直接 panic（exit 101），
/// 而不是干净地跳过或报错。
///
/// 最小复现：
/// ```text
/// $ python3 -c "open('l.tsv','w').write('好'+'x'*300+'\thao\n')"
/// $ opi-tools compile l.tsv l.opid
/// input lines: 1
/// kept entries: 1
/// thread 'main' panicked at crates/engine-data/src/format.rs:65:47:
/// word > 255 bytes: TryFromIntError(PosOverflow)
/// $ echo $?
/// 101
/// ```
/// 边界实测：word = 255 字节通过、256 字节 panic（`… > u8::MAX` 处 `expect`）。
///
/// 根因：`parse_dict`（`crates/opi-tools/src/compiler.rs`）对 **pinyin** 有
/// `pinyin.len() > u8::MAX` 一条跳过规则，对 **word** 没有对称的一条；
/// 而 `serialize`（`crates/engine-data/src/format.rs:65`）写死 word ≤ 255 字节并
/// `expect`。两处限制只有一处被强制。中文 3 字节/字 → 85 字的短语词条就能触发，
/// rime 的短语词库（`.phrase.dict.yaml`）里很常见。
///
/// 期望：与 pinyin 同规则跳过（或整文件报错），但**不能 panic** ——
/// panic 信息里带的是 `format.rs` 行号，用户拿到的是一句无从下手的内部错误。
///
/// **已修（2026-09-27）**：`parse_dict` 补上对称的 `word.len() > u8::MAX` 跳过规则
/// （计入 `Parsed::unusable`，stderr 报一行统计），本用例由 pin 转为门禁。
#[test]
fn overlong_word_must_not_panic() {
    let dir = Dir::new("longword");
    let tsv = dir.write(
        "l.tsv",
        format!("{}\thao\n\u{53f7}\thao\n", "x".repeat(300)).as_bytes(),
    );
    let out = run(&["compile", s(&tsv), s(&dir.path("l.opid"))]);
    let se = String::from_utf8_lossy(&out.stderr);
    assert!(
        !se.contains("panicked at"),
        "超长 word 必须干净处理，不能 panic：{se}"
    );
    assert_ne!(
        out.status.code(),
        Some(101),
        "exit 101 = panic（TryFromIntError 从 format.rs 抛出的）"
    );
}

/// **bug**：带 UTF-8 BOM 的 TSV 会被编译进一个**词条带隐形前缀**的词库 ——
/// 首行的 word 变成 `"\u{FEFF}好"`，而不是 `"好"`。
///
/// 最小复现：
/// ```text
/// $ printf '\xef\xbb\xbf好\thao\n号\thao\t1200\n' > bom.tsv
/// $ opi-tools compile bom.tsv bom.opid && opi-tools verify bom.opid | grep hao
/// query "hao": 号 ﻿好          # ← 第二个是 U+FEFF + 好，肉眼在终端里看不出来
/// ```
/// 成因：`str::trim()` 按 `char::is_whitespace()` 裁空白，而 **U+FEFF 不是空白字符**
/// （Unicode 里它是 `Cf`，`is_whitespace()` 为 false），于是 BOM 留在首列 word 里，
/// 整行照常通过全部校验。条目数不变、校验和正常、exit 0 —— 没有任何信号。
///
/// 触发面很宽：Windows 记事本「UTF-8」、Excel 导出、PowerShell `>` 重定向写出的
/// TSV 默认都带 BOM，而「自己存一份词库给 opi-tools compile」是本项目文档里的正路。
/// 后果是该词条在候选窗里显示成一个空白的怪字，且**永远选不中「好」这个词**。
///
/// 期望：`parse_dict` 在切列前 `trim_start_matches('\u{FEFF}')`（BOM 只可能出现在
/// 文件首字节），或整文件拒绝。根因一处可修，不涉及解析规则。
///
/// **已修（2026-09-27）**：`parse_dict_report` 入口 `strip_prefix('\u{FEFF}')`，
/// 本用例由 pin 转为门禁。
#[test]
fn utf8_bom_must_not_leak_into_the_first_word() {
    let dir = Dir::new("bom");
    let with_bom = dir.write(
        "bom.tsv",
        "\u{FEFF}\u{597d}\thao\n\u{53f7}\thao\t1200\n".as_bytes(),
    );
    let without = dir.write(
        "plain.tsv",
        "\u{597d}\thao\n\u{53f7}\thao\t1200\n".as_bytes(),
    );
    let bom_opid = dir.path("bom.opid");
    let plain_opid = dir.path("plain.opid");
    for (inp, out) in [(&with_bom, &bom_opid), (&without, &plain_opid)] {
        let r = run(&["compile", s(inp), s(out)]);
        assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    }
    use engine_core::dictionary::Dictionary;
    let words = |p: &PathBuf| -> Vec<String> {
        load_bytes(std::fs::read(p).unwrap())
            .expect("产物可加载")
            .query("hao", 8)
            .iter()
            .map(|e| e.word.clone())
            .collect()
    };
    assert_eq!(
        words(&bom_opid),
        words(&plain_opid),
        "BOM 不该改变词条内容：带 BOM 与不带 BOM 的同一份表必须编出同样的词"
    );
    assert!(
        !words(&bom_opid).iter().any(|w| w.contains('\u{FEFF}')),
        "词条里不该出现 U+FEFF：{:?}",
        words(&bom_opid)
    );
}

/// **bug**：输入路径与输出路径相同时，`compile` 会把源 TSV **原地覆盖**成
/// 二进制 `.opid`，exit 0、不留备份、不提示。
///
/// 最小复现：
/// ```text
/// $ cp luna.tsv same.tsv && md5sum same.tsv
/// be6818bed4e55570c160bdf0972f1160  same.tsv
/// $ opi-tools compile same.tsv same.tsv ; echo $? ; file same.tsv
/// 0
/// same.tsv: data        # 头部已是 `O P I D`
/// ```
/// 触发条件极平常：`compile` 后面两个参数由 shell 补全补出同一个名字。
/// 后果不可逆（源词库没了），而 `parse_dict` 的输出对源文件唯一副本而言是
/// 单向转换 —— 这是「手工维护的 TSV 词库被毁」这一类事故里最便宜的一种。
///
/// 期望：两个路径指向同一文件（含 inode 相同 / `canonicalize` 相等）时，
/// 以非 0 退出并说明；或先写临时文件再原子替换（那时同路径也安全）。
/// 无论哪种，都不该在 exit 0 的情况下把用户唯一的源文件换成二进制。
///
/// **已修（2026-09-27）**：走的是「以非 0 退出并说明」——`compile_file` 在写盘前比对
/// `(dev, ino)`（`metadata` 跟进符号链接，`canonicalize` 看不出硬链接）。选它而不是
/// 「先写临时文件再原子替换」，是因为 `rename` 会把输出路径上的**符号链接换成一个
/// 普通文件**（部署时 `dict.opid -> /srv/...` 这类写法会被悄悄拆掉），而 `fs::write`
/// 保留原有 inode 与权限位。
#[test]
fn compile_must_not_overwrite_its_own_input() {
    let dir = Dir::new("inplace");
    let tsv = dir.write("same.tsv", b"\xe5\xa5\xbd\thao\n");
    let before = std::fs::read(&tsv).unwrap();
    let out = run(&["compile", s(&tsv), s(&tsv)]);
    let (code, so, se) = code_and_streams(&out, "原地覆盖");
    assert_ne!(
        code, 0,
        "输入输出同一文件必须拒绝；exit 0 意味着源 TSV 已被换成 `OPID` 二进制：{so}"
    );
    assert!(
        se.contains("same.tsv"),
        "拒绝必须点出那个文件（否则用户不知道哪两个参数撞了）：{se}"
    );
    assert_eq!(
        before,
        std::fs::read(&tsv).unwrap(),
        "输入文件必须原封不动（回归：被换成了 `OPID` 二进制头）"
    );

    // 硬链接是同一份数据的另一个名字：光比路径字符串或 canonicalize 是拦不住的。
    let alias = dir.path("alias.tsv");
    std::fs::hard_link(&tsv, &alias).expect("建硬链接");
    let out = run(&["compile", s(&alias), s(&tsv)]);
    let (code, _, se) = code_and_streams(&out, "硬链接原地覆盖");
    assert_ne!(code, 0, "硬链接指向同一个 inode，同样必须拒绝：{se}");
    assert_eq!(
        before,
        std::fs::read(&tsv).unwrap(),
        "经硬链接写同样会吃掉源文件"
    );
}
