// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `.opid` v1 解析器 / mmap 加载器的健壮性与差分测试。
//!
//! 契约（`format.rs` 头注释 + `loader.rs` 模块注释）：**坏文件 → `Err`，绝不 panic、
//! 绝不越界读**。本文件用四类输入逼近这条契约：
//!
//! 1. **截断**：每一个字节位置。裸截断一律停在 `ChecksumMismatch`（校验和先失败），
//!    更深的分支一次都跑不到 —— 所以还要做**重算校验和的截断**，让 `Truncated`/
//!    `BadOffsets` 这些分支真被走到。
//! 2. **改写 + 重算校验和**：只改内容不改长度，逼出偏移/有序性检查；2000 个变异体要求
//!    Ok 与 Err 两条路都出现过，否则「没 panic」只覆盖了一半。
//! 3. **长度/偏移字段的极端值**：`count = u32::MAX`（若退回裸 `count * ENTRY_LEN`
//!    在 32 位 target 上会回绕并绕过边界检查 —— 出货的 armeabi-v7a 就是 32 位）、
//!    `po/wo = u32::MAX`（若退回裸 `+` 会让越界切片落回合法范围）。
//! 4. **差分**：`MmapDictionary::query` 必须与「遍历 entries 线性过滤」逐条一致 ——
//!    `lower_bound` + `byte_successor` 是手写二分，最值得上 oracle；而
//!    `InMemoryDictionary`（Trie，所有单测用的那个）与 mmap 版在**引擎输出层面**
//!    也必须完全一致，否则「测试全绿」证明的是另一套实现。
//!
//! 注：`query("")` 按实现返回空（`pinyin.is_empty()` 早退），线性前缀过滤会返回全部 ——
//! 这是**有意**的空查询早退，差分里显式排除，不当缺陷。

use engine_core::Engine;
use engine_core::composer::Mode;
use engine_core::dictionary::{Dictionary, InMemoryDictionary};
use engine_core::symbols::SymbolEngine;
use engine_data::checksum::fnv1a64;
use engine_data::format::{
    FormatError, HEADER_LEN, OpDict, RawEntry, TRAILER_LEN, parse, serialize,
};
use engine_data::loader::{load_bytes, load_mmap};

// ---------- 构造与工具 ----------

fn raw(pinyin: &str, word: &str, freq: u32) -> RawEntry {
    RawEntry {
        pinyin: pinyin.into(),
        word: word.into(),
        freq,
    }
}

/// 序列化（`serialize` 内部会按拼音字节序排，pinyin_total = pinyin blob 长度）。
fn bytes_of(entries: &[RawEntry]) -> Vec<u8> {
    let total: usize = entries.iter().map(|e| e.pinyin.len()).sum();
    serialize(&OpDict {
        entries: entries.to_vec(),
        pinyin_total: total,
    })
}

fn sample_entries() -> Vec<RawEntry> {
    vec![
        raw("hao", "好", 5000),
        raw("hao", "号", 1200),
        raw("xiao", "笑", 3000),
        raw("xi", "西", 400),
        raw("xian", "先", 900),
        raw("n", "嗯", 20),
    ]
}

/// 重算尾部校验和（让「只改内容、不改长度」的变异体能过校验，走到更深的检查）。
fn reseal(bytes: &mut [u8]) {
    let tail = bytes.len() - TRAILER_LEN;
    let sum = fnv1a64(&bytes[HEADER_LEN..tail]);
    bytes[tail..].copy_from_slice(&sum.to_le_bytes());
}

/// xorshift64*：确定性 PRNG，避免为 fuzz 引入新依赖。
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Ok 的文件必须自洽：每条 pinyin/word 非空、pinyin ≤ 255 字节、表按拼音字节序非递减，
/// 且**加载器不得 panic**、按拼音查回来必须能找到这条词。
fn check_ok_file(bytes: &[u8], parsed: &OpDict) {
    let mut prev: Option<&[u8]> = None;
    for e in &parsed.entries {
        assert!(!e.pinyin.is_empty() && !e.word.is_empty(), "空词条: {e:?}");
        assert!(
            e.pinyin.len() <= 255 && e.word.len() <= 255,
            "长度字段溢出: {e:?}"
        );
        if let Some(p) = prev {
            assert!(p <= e.pinyin.as_bytes(), "表未按拼音字节序: {e:?}");
        }
        prev = Some(e.pinyin.as_bytes());
    }
    let d = load_bytes(bytes.to_vec()).expect("parse 通过但 load_bytes 失败");
    for e in &parsed.entries {
        let hits = d.query(&e.pinyin, usize::MAX);
        // 同一条记录必须查得回来，且 pinyin_len 等于该记录存储的拼音字节长度
        // （精确/前缀扩展的分层判据）。注意不能要求「所有同名词都同长」——
        // 变异体可能让两条不同拼音的记录解出同一个词。
        assert!(
            hits.iter()
                .any(|h| h.word == e.word && h.pinyin_len == e.pinyin.len()),
            "自己解析出的词条查不回来或 pinyin_len 不符: {e:?}"
        );
    }
}

// ---------- 1. 截断 ----------

#[test]
fn truncation_at_every_byte_is_rejected() {
    let full = bytes_of(&sample_entries());
    for len in 0..full.len() {
        let got = parse(&full[..len]);
        assert!(got.is_err(), "截断到 {len}/{} 字节竟被接受", full.len());
        assert!(
            load_bytes(full[..len].to_vec()).is_err(),
            "截断到 {len} 字节竟然加载成功"
        );
    }
    assert!(parse(&full).is_ok(), "前置：完整文件必须通过");
}

/// 重算校验和的截断：这样才走得到 Truncated / BadOffsets，而不是被校验和一把挡掉。
#[test]
fn resealed_truncation_is_rejected() {
    let full = bytes_of(&sample_entries());
    let mut reached = 0usize;
    for len in HEADER_LEN + TRAILER_LEN..full.len() {
        let mut b = full[..len].to_vec();
        reseal(&mut b);
        let err = parse(&b).unwrap_err();
        // 头部完好、count 仍是 6，编码区被砍 → 必然是结构性拒绝，而非校验和
        assert!(
            matches!(err, FormatError::Truncated | FormatError::BadOffsets),
            "截断到 {len} 字节应报结构错误，实际 {err:?}"
        );
        reached += 1;
    }
    assert!(reached > 50, "覆盖不足：只检查了 {reached} 个截断点");
}

// ---------- 2. 改写 + 重算校验和 ----------

#[test]
fn mutated_files_never_panic_and_ok_ones_are_self_consistent() {
    let full = bytes_of(&sample_entries());
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let (mut ok, mut err) = (0usize, 0usize);
    for _ in 0..3000 {
        let mut b = full.clone();
        for _ in 0..1 + rng.below(3) {
            let pos = rng.below(b.len());
            b[pos] = rng.next() as u8;
        }
        reseal(&mut b);
        match parse(&b) {
            Ok(d) => {
                ok += 1;
                check_ok_file(&b, &d);
            }
            Err(_) => err += 1,
        }
    }
    assert!(ok > 0, "3000 个变异体没有一个通过 —— fuzz 只覆盖了拒绝路径");
    assert!(err > 0, "3000 个变异体全部通过 —— 改坏的是同一处且没被检查");
}

// ---------- 3. 长度/偏移字段的极端值 ----------

#[test]
fn extreme_count_field_is_rejected_without_allocating() {
    let base = bytes_of(&sample_entries());
    for count in [0xFFFF_FFFFu32, 0x8000_0000, 0x1000_0000, 0x0000_0004] {
        let mut b = base.clone();
        b[7..11].copy_from_slice(&count.to_le_bytes());
        reseal(&mut b);
        let got = parse(&b);
        // count 与真实表长不符：要么表尾越界（Truncated），要么后面每一条都越界（BadOffsets/Unsorted）
        assert!(got.is_err(), "count={count} 竟然被接受");
        assert!(
            matches!(
                got.unwrap_err(),
                FormatError::Truncated | FormatError::BadOffsets | FormatError::Unsorted
            ),
            "count={count} 的错误类型不在预料内"
        );
    }
}

#[test]
fn extreme_offsets_are_rejected() {
    let base = bytes_of(&sample_entries());
    // 每条记录的 (字段下标, 值)：po / wo 取 u32::MAX（裸 `+` 回绕会让越界切片落回合法范围）
    for (off, label) in [(11usize, "po"), (16, "wo")] {
        for v in [u32::MAX, u32::MAX - 3, 0x8000_0000] {
            let mut b = base.clone();
            b[off..off + 4].copy_from_slice(&v.to_le_bytes());
            reseal(&mut b);
            let got = parse(&b);
            assert!(got.is_err(), "{label}={v:#x} 竟然被接受");
        }
    }
    // 长度字段取 0xFF：pinyin/word 长度上限 255，越界即 BadOffsets
    for (off, label) in [(15usize, "pl"), (20, "wl")] {
        let mut b = base.clone();
        b[off] = 0xFF;
        reseal(&mut b);
        assert!(parse(&b).is_err(), "{label}=0xFF 竟然被接受");
    }
}

#[test]
fn degenerate_files_are_rejected_cleanly() {
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("空文件", vec![]),
        ("只有一个字节", vec![0]),
        ("魔数都不够", b"OPI".to_vec()),
        ("全零 19 字节", vec![0u8; HEADER_LEN + TRAILER_LEN]),
        ("全零 64 字节", vec![0u8; 64]),
        ("只有魔数", b"OPID".to_vec()),
        ("魔数+版本+空表", {
            let mut v = b"OPID".to_vec();
            v.extend_from_slice(&1u16.to_le_bytes());
            v.push(0);
            v.extend_from_slice(&0u32.to_le_bytes());
            v.extend_from_slice(&[0u8; TRAILER_LEN]);
            v
        }),
    ];
    for (what, b) in cases {
        let got = parse(&b);
        assert!(got.is_err(), "{what}: 竟被接受");
        assert!(load_bytes(b.clone()).is_err(), "{what}: 加载器竟接受");
    }
    // 全零文件不能报成 BadVersion（魔数先于版本检查）
    assert_eq!(
        parse(&[0u8; 64]).unwrap_err(),
        FormatError::BadMagic,
        "全零文件应先被魔数挡下"
    );
}

// ---------- 4. 差分：手写二分 vs 线性过滤 ----------

/// `query` 必须与「遍历 entries 线性前缀过滤 + (词频降序, 词升序) 截断」逐条一致。
#[test]
fn prefix_query_matches_brute_force_scan() {
    let mut rng = Rng(0xDEAD_BEEF_1234_5678);
    let alphabet = *b"abczhnx";
    let words = ["好", "号", "笑", "西", "先", "嗯", "词", "多字词"];
    for trial in 0..300 {
        // 生成条目：短拼音 + 大量前缀关系（字母表小 → 冲突密集）
        let n = 1 + rng.below(12);
        let mut entries: Vec<RawEntry> = Vec::new();
        for i in 0..n {
            let len = 1 + rng.below(4);
            let pinyin: String = (0..len)
                .map(|_| alphabet[rng.below(alphabet.len())] as char)
                .collect();
            if entries.iter().any(|e| e.pinyin == pinyin) {
                continue; // 拼音不重复：显式覆盖「同一前缀下多词」由词区分
            }
            entries.push(raw(&pinyin, words[i % words.len()], rng.next() as u32));
        }
        let bytes = bytes_of(&entries);
        let parsed = parse(&bytes).expect("自产文件必须可解析");
        let dict = load_bytes(bytes).expect("自产文件必须可加载");

        // 探针：全体拼音的全部前缀 + 再添一字符的超集 + 非 ASCII + 单字符
        let mut needles: Vec<String> = Vec::new();
        for e in &parsed.entries {
            for k in 0..=e.pinyin.len() {
                needles.push(e.pinyin[..k].to_string());
            }
            needles.push(format!("{}a", e.pinyin));
            needles.push(format!("{}z", e.pinyin));
        }
        needles.push("中".into());
        needles.push("😄".into());
        needles.push("zzz".into());

        for needle in &needles {
            if needle.is_empty() {
                continue; // 空查询早退是有意行为，见文件头注释
            }
            let got: Vec<(String, u32)> = dict
                .query(needle, usize::MAX)
                .into_iter()
                .map(|e| (e.word, e.freq))
                .collect();
            let mut want: Vec<(String, u32)> = parsed
                .entries
                .iter()
                .filter(|e| e.pinyin.starts_with(needle.as_str()))
                .map(|e| (e.word.clone(), e.freq))
                .collect();
            want.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.as_bytes().cmp(b.0.as_bytes())));
            assert_eq!(
                got, want,
                "trial {trial}: 查询 {needle:?} 与线性过滤不一致（条目 {:?}）",
                parsed.entries
            );

            // limit 截断必须发生在排序之后：取前 k 条 == 完整结果的头部
            if tried_limit(trial) {
                let k = rng.below(3);
                let got_k: Vec<String> =
                    dict.query(needle, k).into_iter().map(|e| e.word).collect();
                let want_k: Vec<String> = want.iter().take(k).map(|p| p.0.clone()).collect();
                assert_eq!(got_k, want_k, "limit {k} 截断顺序错误（needle {needle:?}）");
            }
        }
    }
}

/// 只在部分 trial 上跑 limit 分支，省一半时间又不丢覆盖。
fn tried_limit(trial: usize) -> bool {
    trial.is_multiple_of(2)
}

// ---------- 5. 差分：两个 Dictionary 实现必须给出同一份候选 ----------

/// `InMemoryDictionary`（Trie）与 `MmapDictionary`（mmap + 手写二分）是同一 trait 的
/// 两个实现，**引擎层输出必须逐字相同**。所有单测跑的是前者、出货跑的是后者 ——
/// 两者一旦分叉，「测试全绿」证明的就是另一套实现。
#[test]
fn both_dictionary_impls_yield_identical_candidates() {
    let entries = vec![
        raw("xi", "西", 400),
        raw("xian", "先", 900),
        raw("xiang", "想", 4640), // 前缀扩展 + 静态词频最高
        raw("xiao", "笑", 3000),
        raw("hao", "好", 5000),
        raw("ha", "哈", 800),
        raw("n", "嗯", 20),
        raw("ni", "你", 2000),
    ];
    let bytes = bytes_of(&entries);
    let mmap: Box<dyn Dictionary> = Box::new(load_bytes(bytes.clone()).unwrap());
    let mut mem = InMemoryDictionary::new();
    for e in parse(&bytes).unwrap().entries {
        mem.insert(&e.pinyin, &e.word, e.freq);
    }
    assert_eq!(
        mem.max_freq(),
        mmap.max_freq(),
        "最大词频决定 boost，必须先一致"
    );

    for input in [
        "xi", "xian", "xia", "x", "hao", "ha", "n", "ni", "nihao", "zz",
    ] {
        for limit in [1usize, 3, 8, 64] {
            let a = candidates_of(mem_ref(&mem), input, limit);
            let b = candidates_of(&*mmap, input, limit);
            assert_eq!(a, b, "input={input} limit={limit}: 两个词库实现候选不一致");
        }
    }
}

/// `InMemoryDictionary` 借用一下（`Engine` 要 Box，这里只需 &dyn）。
fn mem_ref(d: &InMemoryDictionary) -> &dyn Dictionary {
    d
}

fn candidates_of(d: &dyn Dictionary, input: &str, limit: usize) -> Vec<String> {
    // Engine 需要 Box<dyn Dictionary>，这里换成直接调 rank_and_pick 的同源路径：
    // engine_core::candidates::rank_and_pick 是 Engine::candidates 的唯一实现。
    use engine_core::candidates::{USER_BOOST, rank_and_pick};
    use engine_core::learner::Learner;
    let boost = USER_BOOST.max(d.max_freq().saturating_mul(2));
    let l = Learner::new(false);
    let s = SymbolEngine::new(Vec::new(), Vec::new());
    rank_and_pick(d, &s, &l, input, Mode::Pinyin, limit, boost)
        .into_iter()
        .map(|c| c.text)
        .collect()
}

/// 文件路径（`load_mmap`）与内存路径（`load_bytes`）必须给出同一份候选 ——
/// 出货走前者，测试与内嵌 fallback 走后者。
#[test]
fn file_load_matches_memory_load() {
    let entries = vec![
        raw("hao", "好", 5000),
        raw("hao", "号", 1200),
        raw("ha", "哈", 800),
        raw("xian", "先", 900),
        raw("xiang", "想", 4640),
    ];
    let bytes = bytes_of(&entries);
    let path = std::env::temp_dir().join(format!("opi-robust-{}.opid", std::process::id()));
    std::fs::write(&path, &bytes).unwrap();
    let from_file = load_mmap(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    let from_bytes = load_bytes(bytes).unwrap();
    assert_eq!(from_file.len(), from_bytes.len());
    assert_eq!(from_file.max_freq(), from_bytes.max_freq());

    for input in ["h", "hao", "ha", "xian", "xiang", "xi", "zz"] {
        assert_eq!(
            candidates_of(&from_file, input, 8),
            candidates_of(&from_bytes, input, 8),
            "input={input}: 文件加载与内存加载候选不一致"
        );
    }
}

/// `Engine` 门面端到端：同一个 `.opid`，Trie 版与 mmap 版的候选必须逐字相同（含模式路由）。
#[test]
fn engine_candidates_identical_across_loaders() {
    let bytes = bytes_of(&sample_entries());
    for mode in [Mode::Pinyin, Mode::Traditional] {
        for input in ["h", "hao", "xi", "xian", "n", "nihao"] {
            let a = engine_cands(Box::new(inmemory_from(&bytes)), mode, input);
            let b = engine_cands(Box::new(load_bytes(bytes.clone()).unwrap()), mode, input);
            assert_eq!(a, b, "{mode:?}/{input}: Trie 与 mmap 的候选不一致");
        }
    }
}

/// 用 `.opid` 解析出来的条目重建 Trie 词库（测试与出货两条路的同源对照）。
fn inmemory_from(bytes: &[u8]) -> InMemoryDictionary {
    let mut d = InMemoryDictionary::new();
    for e in parse(bytes).unwrap().entries {
        d.insert(&e.pinyin, &e.word, e.freq);
    }
    d
}

fn engine_cands(d: Box<dyn Dictionary>, mode: Mode, input: &str) -> Vec<String> {
    let mut e = Engine::new(d, SymbolEngine::new(Vec::new(), Vec::new()), false);
    e.switch_mode(mode);
    for c in input.chars() {
        e.input_key(c);
    }
    e.candidates(8).into_iter().map(|c| c.text).collect()
}
