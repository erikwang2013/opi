// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 词库产物的**跨层一致性**：同一批字节，CLI `verify`（走 `engine_data::load_bytes`）
//! 与 FFI `opi_load`（走 `engine_data::load_mmap`）必须给出同一个判定。
//!
//! 为什么值得单独测：这两条路用的是**两个不同的加载器**
//! （`crates/engine-data/src/loader.rs:184` 的 mmap 版 vs `:193` 的堆字节版），
//! 共用同一个 `parse`。共用的部分一改就同时变，**不共用的部分（文件打开、映射、
//! 空文件、目录、权限）才是会分叉的地方**——而分叉的后果是本项目已经踩过的那个坑：
//! 校验工具说「没问题」，实际运行的引擎却装不上（见 `fcitx5-loaddictionary-silent-failure`
//! 记忆：词库损坏 → 引擎完全不出字、无报错）。所以这里逐个字节地比对两层判定，
//! 而且要求**拒绝必须是干净的错误**（CLI 不能 panic 出 backtrace、FFI 不能靠
//! `catch_unwind` 兜住 panic 后返 false —— 那是「有 bug 但被吞了」）。
//!
//! 依赖 `opi_ffi` 需要在 `crates/opi-tools/Cargo.toml` 的 dev-dependencies 里，
//! 因为本文件同时需要 `CARGO_BIN_EXE_opi-tools`（只有本包自己的二进制才有）。

use engine_data::format::serialize;
use opi_ffi::cabi::{
    OpiString, opi_buffer, opi_candidates, opi_clear, opi_ffi_free_string, opi_input_key, opi_load,
    opi_load_trad, opi_set_learner, opi_switch_mode,
};
use opi_tools::compiler::{compile, parse_dict};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// `SINGLETON` 是进程级的，而 `cargo test` 默认多线程 —— 碰它的用例要串行。
static SERIAL: Mutex<()> = Mutex::new(());

const TSV: &str = "我\two\t100000\n好\thao\n号\thao\t1200\n你\tni\n";

fn good_bytes() -> Vec<u8> {
    serialize(&compile(parse_dict(TSV)))
}

/// 每个用例一个独立目录（同一个测试二进制里并行跑，名字不能撞）。
struct Dir(PathBuf);

impl Dir {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!("opi-xlayer-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("建临时目录");
        Dir(p)
    }
    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let p = self.0.join(name);
        std::fs::write(&p, bytes).expect("写临时文件");
        p
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run_cli(args: &[&str]) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_opi-tools"))
        .args(args)
        .output()
        .expect("跑 opi-tools")
}

fn as_units(s: &Path) -> Vec<u16> {
    s.to_str()
        .expect("临时路径是 UTF-8")
        .encode_utf16()
        .collect()
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

/// 两侧对同一份字节的判定：`(cli 接受, ffi 接受, cli stderr 是否 panic)`。
///
/// CLI 的 stderr 也要看：`load_bytes` 是**没有** `catch_unwind` 的裸调用，
/// 一个越界切片就是整个进程带 backtrace 崩掉 —— exit code 非 0，于是
/// 「判定一致」这条断言会**照样通过**。所以 panic 必须单独判。
fn verdicts(path: &Path, units: &[u16]) -> (bool, bool, bool) {
    let out = run_cli(&["verify", path.to_str().unwrap()]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let panicked = stderr.contains("panicked at") || stderr.contains("RUST_BACKTRACE");
    let ffi = unsafe { opi_load(units.as_ptr(), units.len()) };
    (out.status.success(), ffi, panicked)
}

fn assert_agree(path: &Path, units: &[u16], what: &str) {
    let (cli, ffi, panicked) = verdicts(path, units);
    assert!(
        !panicked,
        "{what}：拒绝必须是干净错误，不能 panic 出 backtrace"
    );
    assert_eq!(
        cli, ffi,
        "{what}：CLI verify={cli} 与 FFI opi_load={ffi} 判定分叉（同一份字节，两层必须同判）"
    );
}

/// 截断到每一个长度：从 0 字节到完整文件，两层同判、且都不 panic。
///
/// 截断是最廉价也最像真实故障的损坏（下载中断、磁盘满、`cp` 到一半断电），
/// 而它踩的正是「长度字段与实际不符」这一类边界。
#[test]
fn cli_verify_and_ffi_load_agree_on_every_truncation() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let dir = Dir::new("trunc");
    let good = good_bytes();
    let mut accepted = 0;
    for n in 0..=good.len() {
        let p = dir.write("t.opid", &good[..n]);
        assert_agree(
            &p,
            &as_units(&p),
            &format!("截断到 {n}/{} 字节", good.len()),
        );
        if run_cli(&["verify", p.to_str().unwrap()]).status.success() {
            accepted += 1;
        }
    }
    // 正控：只有「一个都没截」的那份该被两层同时接受。
    // 少了这条，把 `parse` 改成永远返回 Err 也能让上面的断言全绿。
    assert_eq!(
        accepted, 1,
        "只有完整文件该通过 verify（当前通过 {accepted} 个长度）"
    );
}

/// 逐字节翻转（每个偏移量各试一对低位/高位）：两层同判、且都不 panic。
///
/// 单字节翻转覆盖了「校验和覆盖不到的位置」——**头部 11 字节不在校验和范围内**
/// （`format.rs` 里 `fnv1a64(&bytes[11..len-8])`），魔数/版本/count 全靠 `parse`
/// 自己校验。所以这一组正是「校验和说没事、格式校验说有事」的分界线。
#[test]
fn cli_verify_and_ffi_load_agree_on_every_single_byte_mutation() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let dir = Dir::new("mutate");
    let good = good_bytes();
    let mut rejected = 0;
    for i in 0..good.len() {
        for mask in [0x01u8, 0x80u8] {
            let mut b = good.clone();
            b[i] ^= mask;
            let p = dir.write("m.opid", &b);
            assert_agree(&p, &as_units(&p), &format!("第 {i} 字节翻转 {mask:#04x}"));
            if !run_cli(&["verify", p.to_str().unwrap()]).status.success() {
                rejected += 1;
            }
        }
    }
    // 正控：绝大多数翻转必须被拒。若哪天变成「几乎全接受」，说明校验和/格式校验
    // 不再生效，而「两层判定一致」这条断言对**双双放行**是看不出来的。
    let total = good.len() * 2;
    assert!(
        rejected * 10 >= total * 9,
        "单字节翻转应至少九成被拒：{} / {} 被拒",
        rejected,
        total
    );
}

/// 正控的正面：被两层接受的那份文件，**词条必须真的能用**。
///
/// `verify` 打印的 `entries:` 和 `opi_load` 的 `true` 都可能为真而引擎查不到东西
/// （比如词库装上了但索引空）。这条把「判定为真」与「用户真的能打出字」接上：
/// 少了它，一个「永远返回成功但不装词库」的实现能让上两个用例全绿。
#[test]
fn dictionary_accepted_by_both_layers_is_actually_queryable() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let dir = Dir::new("usable");
    let p = dir.write("good.opid", &good_bytes());
    let out = run_cli(&["verify", p.to_str().unwrap()]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{stdout}");
    assert!(
        stdout.contains("entries: 4"),
        "4 条 TSV 应产出 4 个条目：{stdout}"
    );

    unsafe {
        opi_set_learner(false);
        opi_switch_mode(0);
        opi_clear();
        assert!(
            opi_load(as_units(&p).as_ptr(), as_units(&p).len()),
            "两层都接受的文件必须装载成功"
        );
    }
    // `opi_input_key` 一次只吃**一个**字符（多字符返回空串、不入缓冲），
    // 所以按字符喂 —— 直接喂 "hao" 会让缓冲保持为空，这条用例就变成了假绿。
    for c in "hao".chars() {
        let one = c.to_string().encode_utf16().collect::<Vec<_>>();
        assert_eq!(
            read(unsafe { opi_input_key(one.as_ptr(), one.len()) }),
            "",
            "拼音模式下单字符入缓冲，不该提交"
        );
    }
    let cands = read(unsafe { opi_candidates(8) });
    assert!(
        cands.contains('好') && cands.contains('号'),
        "TSV 里的词条必须出现在候选里（否则「装载成功」是空话）：{cands}"
    );
    assert_eq!(
        read(unsafe { opi_buffer() }),
        "hao",
        "输入不得被吞：{cands}"
    );
}

/// 繁体路径是**另一个** C 导出、走同一条 `load_mmap`：坏字节同样必须 false。
/// 单测 `opi_load` 不够 —— 两个导出各自有一份参数解码，坏文件不该只在一条路上被拦。
#[test]
fn trad_export_uses_the_same_loader_and_rejects_corruption() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    let dir = Dir::new("trad");
    let good = good_bytes();
    let mut bad = good.clone();
    bad[good.len() / 2] ^= 0xFF;
    let badp = dir.write("bad.opid", &bad);
    let goodp = dir.write("good.opid", &good);
    // 引擎要先装载（load_trad 挂在已装载的引擎上）。
    assert!(unsafe { opi_load(as_units(&goodp).as_ptr(), as_units(&goodp).len()) });
    assert!(
        !unsafe { opi_load_trad(as_units(&badp).as_ptr(), as_units(&badp).len()) },
        "损坏的繁体词库必须 false"
    );
    assert!(
        unsafe { opi_load_trad(as_units(&goodp).as_ptr(), as_units(&goodp).len()) },
        "同一份字节在 opi_load 为真、opi_load_trad 也必须为真（同一加载器）"
    );
}

/// `compile` 的确定性：同一输入跑两次必须**字节相同**，且等于库内
/// `serialize(compile(parse_dict(...)))`。
///
/// 不确定的编译产物会让「校验和」失去意义（每次构建都变），也会让
/// 「二进制里嵌的 fallback 词库与仓库源不一致」这类问题无法复现。
#[test]
fn compile_is_deterministic_and_matches_the_library() {
    let dir = Dir::new("determ");
    let tsv = dir.write("d.tsv", TSV.as_bytes());
    let a = dir.write("a.opid", b"");
    let b = dir.write("b.opid", b"");
    for out in [&a, &b] {
        let r = run_cli(&["compile", tsv.to_str().unwrap(), out.to_str().unwrap()]);
        assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    }
    let (ba, bb) = (std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
    assert_eq!(ba, bb, "两次编译必须字节相同");
    assert_eq!(
        ba,
        good_bytes(),
        "CLI 产物必须等于库内 serialize(compile(parse_dict))"
    );
}

/// **bug**：列数不符的行被静默丢弃，于是「格式用错」与「词库本来就空」在工具链里
/// 完全无法区分 —— exit 0、`checksum: ok`、`entries: 0`，一路无错，最后用户拿到
/// 一个打不出任何字的输入法。
///
/// 最小复现（本机已跑，见报告）：
/// ```text
/// $ printf '好 hao\n号 hao\n' > spaces.tsv      # 用空格而不是 TAB（从表格粘贴的常见形态）
/// $ opi-tools compile spaces.tsv spaces.tsv.opid
/// input lines: 2
/// kept entries: 0
/// wrote spaces.tsv.opid (19 bytes)          ← exit 0
/// $ opi-tools verify spaces.tsv.opid
/// checksum: ok
/// entries: 0                                ← exit 0
/// ```
/// 同类输入：CSV（`好,hao`）、4 列及以上、单列 —— 全部走 `cols.len() < 2 || > 3`
/// 这一条 `continue`。`parse_dict` 的文档只写了「跳过空行/注释/front-matter」，
/// 没写「列数不符整行丢弃且不报告」。
///
/// 期望：0 条产出时 `compile` 必须以非 0 退出（或至少 stderr 明确告警）；
/// 根因一处可修（`compiler.rs` 的 `parse_dict` 统计被丢行数 / `main.rs` 加 3 行门禁），
/// 不涉及解析规则本身。
///
/// **已修（2026-09-27）**，失败语义界定如下（这是本条的关键，别过度收紧）：
/// - **该报的**：列数不在 2..=3 的行 —— 这行**不像表的一行**（空格 / 逗号分隔、
///   少列多列）。「一行都没解析出来、却有这类行」= 列分隔符用错了，拒绝写出空壳。
/// - **正常的跳过**（不报错，只统计）：空行、`#` 注释、`-` 开头（front-matter），
///   以及 2..=3 列但词条按文档规则不可用的行（空字段、非 ASCII 或超长 pinyin、
///   超长 word、词频解析失败）—— rime 词库里带声调的拼音行就靠这条滤掉，
///   `cli_boundary.rs` 的 `reported_entry_count_matches_the_written_file` 里
///   `long_pinyin` 一例明确要求这种输入仍然 exit 0。
///
/// 界限就是「这行是不是表的一行」：不是 → 格式错；是但内容不合规 → 跳过。
#[test]
fn compile_must_not_silently_write_an_empty_dictionary() {
    let dir = Dir::new("empty");
    let tsv = dir.write("spaces.tsv", b"\xe5\xa5\xbd hao\n\xe5\x8f\xb7 hao\n");
    let out = dir.write("e.opid", b"");
    let r = run_cli(&["compile", tsv.to_str().unwrap(), out.to_str().unwrap()]);
    let so = String::from_utf8_lossy(&r.stdout);
    let se = String::from_utf8_lossy(&r.stderr);
    assert!(
        !r.status.success(),
        "0 条产出必须非 0 退出（回归：又被当成成功写了个空壳）stdout: {so}"
    );
    assert!(
        se.contains("spaces.tsv") && se.contains("TAB"),
        "报的必须是「列分隔符用错了」，并点名文件 —— 否则用户只会以为词库本来就空：{se}"
    );
    assert_eq!(
        std::fs::read(&out).unwrap(),
        b"",
        "拒绝时不该在输出路径留下半个产物"
    );
}

// ---------- 入库产物与源数据的绑定 ----------
//
// 上面测的是「同一份字节，两条加载路径同判」；这里测的是**入库的那份字节
// 是不是这份源的产物**。入库产物有两个下游会用到：
// `data/generated/fallback.opid` 被 `include_bytes!` 编进每个二进制
// （`crates/engine-data/src/dictionary.rs`），`trad.opid` 是繁体路径的部署数据。
// 两者都是「提交进仓库的二进制」，一旦源改了而产物没重编，**没有任何编译期
// 或运行期报错**：引擎照样装载、照样能查，只是查的是旧词表。

const RAW_FALLBACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/raw/fallback.tsv");
const RAW_TRAD_HANZI: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/raw/trad_hanzi.tsv");
const RAW_TRAD_PHRASES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../data/raw/trad_phrases.tsv"
);
const OPID_FALLBACK: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../data/generated/fallback.opid"
);
const OPID_TRAD: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../data/generated/trad.opid"
);

/// 入库产物必须等于**用仓库里的源现编**出来的字节。
///
/// 牙齿在于比的是两个各自独立的 2.5 MB / 715 B 字节串：左边由 `parse_dict →
/// compile → serialize` 现场算出，右边从磁盘读；改动解析规则、排序规则、
/// FNV 实现、词条布局中的**任何一个**，而忘了重编产物，这条就红。
/// 这正是本项目怕的那类故障：格式变了、产物没跟上，产物读出来是另一个东西。
#[test]
fn committed_dictionaries_match_their_sources() {
    let fallback_src = std::fs::read_to_string(RAW_FALLBACK).expect("读 fallback.tsv");
    let rebuilt = serialize(&compile(parse_dict(&fallback_src)));
    let committed = std::fs::read(OPID_FALLBACK).expect("读 fallback.opid");
    assert!(
        committed.len() > 100,
        "fallback.opid 只有 {} 字节 —— 空产物比错产物更危险（回退库静默失效）",
        committed.len()
    );
    assert_eq!(
        rebuilt, committed,
        "fallback.opid 与 data/raw/fallback.tsv 现编结果不一致：改了源就用 \
         `opi-tools compile data/raw/fallback.tsv data/generated/fallback.opid` 重编并提交"
    );

    // 繁体：CONTRIBUTING 的配方是 `cat trad_hanzi.tsv trad_phrases.tsv`，
    // 直接拼接、中间不插换行 —— 所以先证明 hanzi 以换行结尾，否则 `cat`
    // 会把最后一行汉字和第一行短语粘成一行（那一行多半列数不符 → 静默丢弃）。
    let hanzi = std::fs::read_to_string(RAW_TRAD_HANZI).expect("读 trad_hanzi.tsv");
    let phrases = std::fs::read_to_string(RAW_TRAD_PHRASES).expect("读 trad_phrases.tsv");
    assert!(
        hanzi.ends_with('\n'),
        "trad_hanzi.tsv 末尾没有换行 —— `cat` 拼接会把两行粘起来（配方见 CONTRIBUTING.md）"
    );
    let merged = format!("{hanzi}{phrases}");
    let rebuilt = serialize(&compile(parse_dict(&merged)));
    let committed = std::fs::read(OPID_TRAD).expect("读 trad.opid");
    assert!(
        committed.len() > 1_000_000,
        "trad.opid 只有 {} 字节，不像 9.6 万条的产物",
        committed.len()
    );
    assert_eq!(
        rebuilt, committed,
        "trad.opid 与两份 raw TSV 现编结果不一致：改了源就用 CONTRIBUTING 的 \
         `cat` + `opi-tools compile` 重编，并同步 android/app/src/main/assets/trad.opid"
    );
}

/// 源文件里**没有一行**是被静默丢掉的。
///
/// `parse_dict` 有四条静默 `continue`（列数不是 2..=3、空 word/pinyin、
/// 非 ASCII 或超 255 字节的 pinyin、第三列解析不出词频），加一条去重
/// （`(pinyin, word)` 相同者合并）。这几条没有一条会报告 —— 现实里表现为
/// 「词库编出来就是少几个词」，谁也不会发现。所以这里用「文档写明的跳过规则」
/// 反推应有的条数，要求恰好相等。
///
/// 牙齿：往任何一份 raw TSV 里加一行 4 列的、或加一行第三列写成 `abc` 的，
/// 这条立刻红（而 `verify`、`compile`、引擎装载全都照常成功）。
#[test]
fn no_source_line_is_silently_dropped() {
    for path in [RAW_FALLBACK, RAW_TRAD_HANZI, RAW_TRAD_PHRASES] {
        let text = std::fs::read_to_string(path).expect("读 raw TSV");
        // 文档写明的跳过：空行、`#` 注释、front-matter（`-` 开头）。其余都该留下来。
        let expected = text
            .lines()
            .filter(|l| {
                let l = l.trim();
                !l.is_empty() && !l.starts_with('#') && !l.starts_with('-')
            })
            .count();
        let got = parse_dict(&text).len();
        assert_eq!(
            got, expected,
            "{path}: 解析出 {got} 条、非跳过行 {expected} 行 —— 有行被静默丢弃。\
             可能是列数不是 2..=3、word/pinyin 为空、pinyin 非 ASCII 或超 255 字节、\
             第三列词频解析失败，或出现了重复的 (pinyin, word) 对被去重合并"
        );
    }
}
