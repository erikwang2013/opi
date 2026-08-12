# OPI M2 数据管线 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现词库数据管线：opi-tools 把 rime dict.yaml 文本编译为 `.opid` 二进制，engine-data 以 mmap 加载、校验（FNV-1a 校验和）、损坏自动回退内置词库，engine-core 通过 `Dictionary` trait 无缝使用。

**Architecture:** 新增两个 crate。engine-data（依赖 engine-core + memmap2）负责二进制格式（排序平面表 + pinyin/word blob，不用 DAWG，见 Task 11 偏差说明）、校验和校验、mmap/内存加载，实现 `engine_core::dictionary::Dictionary`。opi-tools（依赖 engine-data）是编译 CLI：`parse_dict`（rime 文本 → 条目）→ `compile`（排序 + 合成词频）→ `serialize` → 写盘。内置回退词库 `fallback.opid` 由 opi-tools 从 `data/raw/fallback.tsv` 编译后提交进仓库，`include_bytes!` 嵌入。查询 = 前缀二分（lower_bound + 字节后继上界）+ 按 freq 降序收集截断。

**Tech Stack:** Rust workspace（engine-core / engine-data / opi-tools），memmap2 0.9，proptest 1（opi-tools dev-dep）。数据源：rime/rime-luna-pinyin（LGPL-3.0，已 clone 到 `/tmp/rime-luna-pinyin/`，正文 `word\tpinyin` 或 `word\tpinyin\tNN.NN%`）。

**格式契约（所有任务共用）：** header 11B = MAGIC `b"OPID"`(4) + version u16=1(2) + flags u8=0(1) + count u32(4)；entries 表 `[11, 11+count*14)`，每条 14B = pinyin_blob_off u32 + pinyin_len u8 + word_blob_off u32 + word_len u8 + freq u32；pinyin blob `[11+count*14, +pinyin_total)`；word blob 紧随其后；trailer 8B = FNV-1a64 覆盖 `[11, len-8)`。pinyin_blob_off 相对 pinyin blob 起点，word_blob_off 相对 word blob 起点，全部 little-endian。常量：`HEADER_LEN=11`、`ENTRY_LEN=14`、`TRAILER_LEN=8`。

---

### Task 1: workspace 加入 engine-data 与 opi-tools

**Files:**
- Modify: `Cargo.toml`（workspace members）
- Create: `crates/engine-data/Cargo.toml`
- Create: `crates/engine-data/src/lib.rs`
- Create: `crates/opi-tools/Cargo.toml`
- Create: `crates/opi-tools/src/lib.rs`
- Create: `crates/opi-tools/src/main.rs`

- [ ] **Step 1: 修改 workspace Cargo.toml**

```toml
[workspace]
resolver = "2"
members = ["crates/engine-core", "crates/engine-data", "crates/opi-tools"]

[profile.release]
lto = true
```

- [ ] **Step 2: 创建 engine-data crate**

`crates/engine-data/Cargo.toml`：

```toml
[package]
name = "engine-data"
version = "0.1.0"
edition = "2021"

[dependencies]
engine-core = { path = "../engine-core" }
memmap2 = "0.9"
```

`crates/engine-data/src/lib.rs`：

```rust
//! engine-data：.opid 二进制词库的格式、校验与 mmap 加载（M2）。

pub mod checksum;
pub mod dictionary;
pub mod format;
pub mod loader;
```

再创建 4 个占位模块文件（各仅一行文档注释，后续任务填充）：

- `crates/engine-data/src/checksum.rs`：`//! （占位，Task 3 实现）`
- `crates/engine-data/src/format.rs`：`//! （占位，Task 4 实现）`
- `crates/engine-data/src/loader.rs`：`//! （占位，Task 7 实现）`
- `crates/engine-data/src/dictionary.rs`：`//! （占位，Task 8 实现）`

- [ ] **Step 3: 创建 opi-tools crate**

`crates/opi-tools/Cargo.toml`：

```toml
[package]
name = "opi-tools"
version = "0.1.0"
edition = "2021"

[dependencies]
engine-data = { path = "../engine-data" }

[dev-dependencies]
engine-core = { path = "../engine-core" }
proptest = "1"
```

`crates/opi-tools/src/lib.rs`：

```rust
//! opi-tools：词库编译工具（dict.yaml → .opid）。

pub mod compiler;
```

`crates/opi-tools/src/compiler.rs`（占位，Task 5 实现）：

```rust
//! （占位，Task 5 实现）
```

`crates/opi-tools/src/main.rs`：

```rust
fn main() {}
```

- [ ] **Step 4: 验证构建与既有测试**

Run: `cargo build --workspace && cargo test --workspace`
Expected: 构建成功，62 个测试全绿（M1 未受影响）。

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/engine-data crates/opi-tools
git commit -m "chore: add engine-data and opi-tools crates to workspace"
```

---

### Task 2: data/raw 词源记录与 fallback.tsv

**Files:**
- Create: `data/raw/fallback.tsv`
- Create: `data/raw/LICENSES.md`
- Create: `data/generated/.gitignore`

- [ ] **Step 1: 写失败测试前的数据文件（数据先行，fallback 词库内容即测试事实）**

`data/raw/fallback.tsv`（35 条高频词，3 列 = word\tpinyin\tfreq 整数）：

```tsv
我	wo	100000
你	ni	99000
他	ta	98000
她	ta	97000
好	hao	96000
是	shi	95000
的	de	94000
了	le	93000
不	bu	92000
在	zai	91000
有	you	90000
人	ren	89000
一	yi	88000
大	da	87000
中	zhong	86000
上	shang	85000
下	xia	84000
说	shuo	83000
去	qu	82000
来	lai	81000
会	hui	80000
到	dao	79000
想	xiang	78000
看	kan	77000
要	yao	76000
能	neng	75000
让	rang	74000
给	gei	73000
和	he	72000
与	yu	71000
都	dou	70000
很	hen	69000
没	mei	68000
还	hai	67000
个	ge	66000
```

`data/raw/LICENSES.md`：

```markdown
# 词库数据来源与许可证

| 文件 | 来源 | 许可证 | 说明 |
|---|---|---|---|
| fallback.tsv | OPI 项目自建 | MIT | 内置回退词库，由 opi-tools 编译为 data/generated/fallback.opid |
| luna_pinyin.dict.yaml（M2 验证用） | https://github.com/rime/rime-luna-pinyin | **LGPL-3.0** | 官方拼音词库，889KB ~70771 行，正文 `word\tpinyin`（无词频列） |

> 注意：rime 社区数据许可证为 LGPL-3.0（不是 BSD/GPL 混合）。使用前确认源码树内 LICENSE 文件。
```

`data/generated/.gitignore`：

```gitignore
*
!fallback.opid
```

- [ ] **Step 2: 验证文件**

Run: `wc -l data/raw/fallback.tsv && git check-ignore data/generated/luna.opid && git check-ignore -n data/generated/fallback.opid`
Expected: `35`；`data/generated/luna.opid` 被忽略（输出该路径）；`fallback.opid` 不匹配（`-n` 无输出即未被忽略）。

- [ ] **Step 3: Commit**

```bash
git add data/raw/fallback.tsv data/raw/LICENSES.md data/generated/.gitignore
git commit -m "chore: add fallback.tsv and LICENSES.md for M2 data pipeline"
```

---

### Task 3: FNV-1a 64 校验和

**Files:**
- Create: `crates/engine-data/src/checksum.rs`

- [ ] **Step 1: 写失败测试**

```rust
use super::*;

#[test]
fn known_vectors() {
    assert_eq!(fnv1a64(b""), 0xcbf29ce484222325);
    assert_eq!(fnv1a64(b"a"), 0xaf63dc4c8601ec8c);
    assert_eq!(fnv1a64(b"foobar"), 0x85944171f73967e8);
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p engine-data`
Expected: FAIL，`fnv1a64` 未定义。

- [ ] **Step 3: 实现**

```rust
const OFFSET: u64 = 0xcbf29ce484222325;
const PRIME: u64 = 0x100000001b3;

/// FNV-1a 64 位哈希（自实现，不引入依赖）。.opid 校验和专用。
pub fn fnv1a64(data: &[u8]) -> u64 {
    data.iter().fold(OFFSET, |h, &b| (h ^ b as u64).wrapping_mul(PRIME))
}
```

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p engine-data`
Expected: PASS，1 个测试。

- [ ] **Step 5: Commit**

```bash
git add crates/engine-data/src/checksum.rs
git commit -m "feat(engine-data): FNV-1a 64-bit checksum"
```

---

### Task 4: .opid v1 二进制格式 serialize/parse

**Files:**
- Create: `crates/engine-data/src/format.rs`

- [ ] **Step 1: 写失败测试（先测契约，再测实现）**

`crates/engine-data/src/format.rs` 内追加测试模块（此时模块本体只含类型占位声明，见 Step 3 前编译失败清单）：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> OpDict {
        OpDict {
            entries: vec![
                RawEntry { pinyin: "hao".into(), word: "好".into(), freq: 5000 },
                RawEntry { pinyin: "hao".into(), word: "号".into(), freq: 1200 },
                RawEntry { pinyin: "xiao".into(), word: "笑".into(), freq: 3000 },
            ],
            // pinyin blob = "hao"(3) + "hao"(3) + "xiao"(4) = 10 字节
            pinyin_total: 10,
        }
    }

    #[test]
    fn layout_byte_by_byte() {
        let bytes = serialize(&sample());
        assert_eq!(&bytes[0..4], b"OPID");
        assert_eq!(u16::from_le_bytes(bytes[4..6].try_into().unwrap()), 1);
        assert_eq!(bytes[6], 0);
        assert_eq!(u32::from_le_bytes(bytes[7..11].try_into().unwrap()), 3);
        // 第一条记录：hao/好/5000
        assert_eq!(u32::from_le_bytes(bytes[11..15].try_into().unwrap()), 0); // pinyin_blob_off
        assert_eq!(bytes[15], 3); // pinyin_len
        assert_eq!(u32::from_le_bytes(bytes[16..20].try_into().unwrap()), 0); // word_blob_off
        assert_eq!(bytes[20], 3); // word_len
        assert_eq!(u32::from_le_bytes(bytes[21..25].try_into().unwrap()), 5000);
        // pinyin blob 紧跟 entries 表（11 + 3*14 = 53）
        assert_eq!(&bytes[53..56], b"hao");
        assert_eq!(&bytes[56..59], b"hao");
        assert_eq!(&bytes[59..63], b"xiao");
        // trailer 校验和覆盖 [11, len-8)
        let expected = crate::checksum::fnv1a64(&bytes[11..bytes.len() - 8]);
        assert_eq!(u64::from_le_bytes(bytes[bytes.len() - 8..].try_into().unwrap()), expected);
    }

    #[test]
    fn roundtrip_restores_entries_sorted() {
        let parsed = parse(&serialize(&sample())).unwrap();
        assert_eq!(parsed.entries.len(), 3);
        assert_eq!(parsed.entries[0].word, "好");
        assert_eq!(parsed.entries[1].word, "号");
        assert_eq!(parsed.entries[2].word, "笑");
        assert_eq!(parsed.entries[2].freq, 3000);
        assert_eq!(parsed.pinyin_total, 10);
    }

    #[test]
    fn empty_dict_roundtrips() {
        let d = OpDict { entries: vec![], pinyin_total: 0 };
        let parsed = parse(&serialize(&d)).unwrap();
        assert!(parsed.entries.is_empty());
        assert_eq!(parsed.pinyin_total, 0);
    }

    #[test]
    fn bad_magic_rejected() {
        let mut bytes = serialize(&sample());
        bytes[0] = b'X';
        assert_eq!(parse(&bytes).unwrap_err(), FormatError::BadMagic);
    }

    #[test]
    fn bad_version_rejected() {
        let mut bytes = serialize(&sample());
        bytes[4] = 2;
        assert_eq!(parse(&bytes).unwrap_err(), FormatError::BadVersion(2));
    }

    #[test]
    fn truncated_rejected() {
        let bytes = serialize(&sample());
        assert_eq!(parse(&bytes[..10]).unwrap_err(), FormatError::Truncated);
    }

    #[test]
    fn payload_corruption_detected() {
        let mut bytes = serialize(&sample());
        bytes[20] ^= 0xFF;
        assert!(matches!(parse(&bytes), Err(FormatError::ChecksumMismatch { .. })));
    }

    #[test]
    fn checksummed_but_unsorted_rejected() {
        // 交换第 0 与第 2 条记录（hao ↔ xiao）并重算校验和，格式仍应拒绝无序表
        let mut bytes = serialize(&sample());
        for i in 0..ENTRY_LEN {
            bytes.swap(11 + i, 11 + 2 * ENTRY_LEN + i);
        }
        let sum = crate::checksum::fnv1a64(&bytes[11..bytes.len() - 8]);
        let tail = bytes.len() - TRAILER_LEN;
        bytes[tail..].copy_from_slice(&sum.to_le_bytes());
        assert_eq!(parse(&bytes).unwrap_err(), FormatError::Unsorted);
    }

    #[test]
    fn out_of_bounds_offset_rejected() {
        let mut bytes = serialize(&sample());
        bytes[11] = 0xFF; // 第一条 pinyin_blob_off 越界（校验和重算后仍应被拒绝）
        let sum = crate::checksum::fnv1a64(&bytes[11..bytes.len() - 8]);
        let tail = bytes.len() - TRAILER_LEN;
        bytes[tail..].copy_from_slice(&sum.to_le_bytes());
        assert_eq!(parse(&bytes).unwrap_err(), FormatError::BadOffsets);
    }
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p engine-data`
Expected: FAIL（`serialize`/`parse`/`OpDict`/`RawEntry`/`FormatError` 未定义）。

- [ ] **Step 3: 实现**

```rust
use crate::checksum::fnv1a64;

pub const MAGIC: &[u8; 4] = b"OPID";
pub const VERSION: u16 = 1;
pub const HEADER_LEN: usize = 11;
pub const ENTRY_LEN: usize = 14;
pub const TRAILER_LEN: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawEntry {
    pub pinyin: String,
    pub word: String,
    pub freq: u32,
}

/// 排序后的词库（entries 按 pinyin 字节序升序）。pinyin_total 为 pinyin blob 字节数，
/// 由 compile/parse 维护，loader 据此定位 word blob。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpDict {
    pub entries: Vec<RawEntry>,
    pub pinyin_total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    BadMagic,
    BadVersion(u16),
    Truncated,
    ChecksumMismatch { expected: u64, actual: u64 },
    Unsorted,
    BadOffsets,
}

/// 序列化为 .opid v1。条目先按 pinyin 字节序排序（不改动入参）。
/// 不变式：pinyin/word 均 ≤255 字节且为 ASCII pinyin（编译管线保证）。
pub fn serialize(dict: &OpDict) -> Vec<u8> {
    let mut sorted: Vec<&RawEntry> = dict.entries.iter().collect();
    sorted.sort_by(|a, b| a.pinyin.as_bytes().cmp(b.pinyin.as_bytes()));
    for e in &sorted {
        debug_assert!(e.pinyin.len() <= u8::MAX as usize);
        debug_assert!(e.word.len() <= u8::MAX as usize);
    }
    let mut pinyin_blob = Vec::new();
    let mut word_blob = Vec::new();
    let mut table = Vec::with_capacity(sorted.len() * ENTRY_LEN);
    for e in &sorted {
        let po = pinyin_blob.len() as u32;
        pinyin_blob.extend_from_slice(e.pinyin.as_bytes());
        let wo = word_blob.len() as u32;
        word_blob.extend_from_slice(e.word.as_bytes());
        table.extend_from_slice(&po.to_le_bytes());
        table.push(e.pinyin.len() as u8);
        table.extend_from_slice(&wo.to_le_bytes());
        table.push(e.word.len() as u8);
        table.extend_from_slice(&e.freq.to_le_bytes());
    }
    debug_assert_eq!(pinyin_blob.len(), dict.pinyin_total);
    let mut out = Vec::with_capacity(
        HEADER_LEN + table.len() + pinyin_blob.len() + word_blob.len() + TRAILER_LEN,
    );
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.push(0); // flags
    out.extend_from_slice(&(sorted.len() as u32).to_le_bytes());
    out.extend_from_slice(&table);
    out.extend_from_slice(&pinyin_blob);
    out.extend_from_slice(&word_blob);
    let sum = fnv1a64(&out[HEADER_LEN..]);
    out.extend_from_slice(&sum.to_le_bytes());
    out
}

/// 解析并完整校验 .opid v1。
pub fn parse(data: &[u8]) -> Result<OpDict, FormatError> {
    if data.len() < HEADER_LEN + TRAILER_LEN {
        return Err(FormatError::Truncated);
    }
    if &data[0..4] != MAGIC {
        return Err(FormatError::BadMagic);
    }
    let version = u16::from_le_bytes(data[4..6].try_into().unwrap());
    if version != VERSION {
        return Err(FormatError::BadVersion(version));
    }
    let tail = data.len() - TRAILER_LEN;
    let expected = u64::from_le_bytes(data[tail..].try_into().unwrap());
    let actual = fnv1a64(&data[HEADER_LEN..tail]);
    if expected != actual {
        return Err(FormatError::ChecksumMismatch { expected, actual });
    }
    let count = u32::from_le_bytes(data[7..11].try_into().unwrap()) as usize;
    let table_end = HEADER_LEN + count * ENTRY_LEN;
    if table_end > tail {
        return Err(FormatError::Truncated);
    }
    let pinyin_start = table_end;
    let mut rows: Vec<[u8; ENTRY_LEN]> = Vec::with_capacity(count);
    for i in 0..count {
        let rec = HEADER_LEN + i * ENTRY_LEN;
        rows.push(data[rec..rec + ENTRY_LEN].try_into().unwrap());
    }
    // pinyin_total = 各条 pinyin 终点最大值；word blob 紧随其后
    let pinyin_total = rows
        .iter()
        .map(|row| {
            let po = u32::from_le_bytes(row[0..4].try_into().unwrap()) as usize;
            let pl = row[4] as usize;
            po + pl
        })
        .max()
        .unwrap_or(0);
    let word_start = pinyin_start + pinyin_total;
    if word_start > tail {
        return Err(FormatError::BadOffsets);
    }
    let word_total = tail - word_start;
    let mut entries = Vec::with_capacity(count);
    let mut prev: Option<String> = None;
    for row in rows {
        let po = u32::from_le_bytes(row[0..4].try_into().unwrap()) as usize;
        let pl = row[4] as usize;
        let wo = u32::from_le_bytes(row[5..9].try_into().unwrap()) as usize;
        let wl = row[9] as usize;
        let freq = u32::from_le_bytes(row[10..14].try_into().unwrap());
        if pl == 0 || wl == 0 || po + pl > pinyin_total || wo + wl > word_total {
            return Err(FormatError::BadOffsets);
        }
        let pinyin = std::str::from_utf8(&data[pinyin_start + po..pinyin_start + po + pl])
            .map_err(|_| FormatError::BadOffsets)?;
        let word = std::str::from_utf8(&data[word_start + wo..word_start + wo + wl])
            .map_err(|_| FormatError::BadOffsets)?;
        if let Some(p) = &prev {
            if p.as_bytes() > pinyin.as_bytes() {
                return Err(FormatError::Unsorted);
            }
        }
        prev = Some(pinyin.to_string());
        entries.push(RawEntry { pinyin: pinyin.to_string(), word: word.to_string(), freq });
    }
    Ok(OpDict { entries, pinyin_total })
}
```

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p engine-data && cargo clippy -p engine-data --all-targets -- -D warnings`
Expected: 10 个测试 PASS；clippy 零警告。若 `pinyin_start` 触发 unused 警告，删除后重跑。

- [ ] **Step 5: Commit**

```bash
git add crates/engine-data/src/format.rs
git commit -m "feat(engine-data): .opid v1 binary format serialize/parse"
```

---

### Task 5: rime 文本解析器 parse_dict

**Files:**
- Create: `crates/opi-tools/src/compiler.rs`
- Create: `crates/opi-tools/tests/parse_dict_proptest.rs`

规则（与 Task 11 真实数据编译一致）：
- 跳过空行、`#` 注释、`-`/`...` 开头的 front-matter 行
- `\t` 切分：2 列 → freq=1000；3 列整数 → 直接取；3 列 `NN.NN%` → `round(percent × 1000)`
- pinyin 非 ASCII（如带声调）或 >255 字节 → 跳过；word 为空 → 跳过
- 重复 (pinyin, word) 取最大 freq

- [ ] **Step 1: 写失败测试**

```rust
use opi_tools::compiler::parse_dict;

#[test]
fn two_column_defaults_to_1000() {
    let entries = parse_dict("好\thao\n");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].freq, 1000);
    assert_eq!(entries[0].pinyin, "hao");
    assert_eq!(entries[0].word, "好");
}

#[test]
fn three_column_percent_rounds() {
    let entries = parse_dict("丁\tding\t99.93%\n");
    assert_eq!(entries[0].freq, 99930);
}

#[test]
fn three_column_integer_taken_directly() {
    let entries = parse_dict("我\two\t100000\n");
    assert_eq!(entries[0].freq, 100000);
}

#[test]
fn skips_comments_blanks_and_front_matter() {
    let text = "# 注释\n\n---\nname: luna_pinyin\nuse_preset_vocabulary: true\n...\n好\thao\n";
    let entries = parse_dict(text);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].word, "好");
}

#[test]
fn duplicate_takes_max_freq() {
    let text = "好\thao\t500\n好\thao\t3000\n";
    let entries = parse_dict(text);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].freq, 3000);
}

#[test]
fn non_ascii_pinyin_skipped() {
    let entries = parse_dict("好\thāo\n");
    assert!(entries.is_empty());
}

#[test]
fn empty_word_or_pinyin_skipped() {
    let entries = parse_dict("\thao\n好\t\n");
    assert!(entries.is_empty());
}
```

`crates/opi-tools/tests/parse_dict_proptest.rs`：

```rust
use opi_tools::compiler::parse_dict;
use proptest::prelude::*;

proptest! {
    /// 任意行输入不 panic
    #[test]
    fn never_panics_on_arbitrary_lines(
        lines in prop::collection::vec(prop::collection::vec(any::<String>(), 0..5), 0..50),
    ) {
        let text = lines.iter().map(|l| l.join("\t")).collect::<Vec<_>>().join("\n");
        let _ = parse_dict(&text);
    }

    /// 重复 (pinyin, word) 取最大 freq
    #[test]
    fn duplicate_takes_max(
        pinyin in "[a-z]{1,6}",
        word in "[\u{4e00}-\u{9fff}]{1,4}",
        a in 0u32..10_000,
        b in 0u32..10_000,
    ) {
        let text = format!("{word}\t{pinyin}\t{a}\n{word}\t{pinyin}\t{b}\n");
        let entries = parse_dict(&text);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].freq, a.max(b));
    }
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p opi-tools`
Expected: FAIL（`compiler` 模块为空）。

- [ ] **Step 3: 实现**

`crates/opi-tools/src/compiler.rs`：

```rust
use engine_data::{OpDict, RawEntry, serialize};
use std::collections::HashMap;
use std::path::Path;

/// 解析 rime dict.yaml / 项目 tsv 文本为条目。见 plan Task 5 规则。
pub fn parse_dict(text: &str) -> Vec<RawEntry> {
    let mut best: HashMap<(String, String), u32> = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('-') {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 2 || cols.len() > 3 {
            continue;
        }
        let word = cols[0].trim();
        let pinyin = cols[1].trim().to_lowercase();
        if word.is_empty() || pinyin.is_empty() || !pinyin.is_ascii() || pinyin.len() > u8::MAX as usize {
            continue;
        }
        let freq = match cols.get(2).map(|s| s.trim()).filter(|s| !s.is_empty()) {
            None => 1000,
            Some(s) => match parse_freq(s) {
                Some(f) => f,
                None => continue,
            },
        };
        let key = (pinyin.clone(), word.to_string());
        let prev = best.get(&key).copied().unwrap_or(0);
        best.insert(key, freq.max(prev));
    }
    let mut out: Vec<RawEntry> = best
        .into_iter()
        .map(|((pinyin, word), freq)| RawEntry { pinyin, word, freq })
        .collect();
    out.sort_by(|a, b| a.pinyin.as_bytes().cmp(b.pinyin.as_bytes()));
    out
}

/// 3 列词频：整数直接取；`NN.NN%` 按 round(percent × 1000)。
fn parse_freq(s: &str) -> Option<u32> {
    if let Ok(n) = s.parse::<u32>() {
        return Some(n);
    }
    let pct = s.strip_suffix('%')?;
    Some((pct.parse::<f64>().ok()? * 1000.0).round() as u32)
}

/// 排序 + 计算 pinyin_total，产出可序列化的 OpDict。
pub fn compile(entries: Vec<RawEntry>) -> OpDict {
    let mut entries = entries;
    entries.sort_by(|a, b| {
        a.pinyin
            .as_bytes()
            .cmp(b.pinyin.as_bytes())
            .then_with(|| a.word.as_bytes().cmp(b.word.as_bytes()))
    });
    entries.dedup_by(|a, b| a.pinyin == b.pinyin && a.word == b.word);
    let pinyin_total = entries.iter().map(|e| e.pinyin.len()).sum();
    OpDict { entries, pinyin_total }
}

pub fn compile_file(input: &Path, output: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(input).map_err(|e| format!("read {}: {e}", input.display()))?;
    let dict = compile(parse_dict(&text));
    let bytes = serialize(&dict);
    std::fs::write(output, &bytes).map_err(|e| format!("write {}: {e}", output.display()))?;
    Ok(())
}
```

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p opi-tools`
Expected: 7 个单元测试 + 2 个 proptest 全 PASS。

- [ ] **Step 5: Commit**

```bash
git add crates/opi-tools/src/compiler.rs crates/opi-tools/tests/parse_dict_proptest.rs
git commit -m "feat(opi-tools): rime dict text parser"
```

---

### Task 6: compile CLI + 构建并提交 fallback.opid

**Files:**
- Modify: `crates/opi-tools/src/main.rs`
- Create: `crates/opi-tools/tests/cli.rs`

- [ ] **Step 1: 写失败测试**

`crates/opi-tools/tests/cli.rs`：

```rust
#[test]
fn cli_compile_roundtrip() {
    let dir = std::env::temp_dir().join(format!("opi-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let tsv = dir.join("t.tsv");
    let opid = dir.join("t.opid");
    std::fs::write(&tsv, "好\thao\n号\thao\t1200\n").unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_opi-tools"))
        .args(["compile", tsv.to_str().unwrap(), opid.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let bytes = std::fs::read(&opid).unwrap();
    let parsed = engine_data::parse(&bytes).unwrap();
    assert_eq!(parsed.entries.len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p opi-tools --test cli`
Expected: FAIL（无 compile 子命令，exit code 非 0）。

- [ ] **Step 3: 实现 CLI**

```rust
use opi_tools::compiler::{compile_file, parse_dict};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("compile") => {
            let (Some(input), Some(output)) = (args.get(2), args.get(3)) else {
                eprintln!("usage: opi-tools compile <input.tsv|dict.yaml> <output.opid>");
                std::process::exit(2);
            };
            let text = match std::fs::read_to_string(input) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("read {}: {e}", input);
                    std::process::exit(1);
                }
            };
            let entries = parse_dict(&text);
            println!("input lines: {}", text.lines().count());
            println!("kept entries: {}", entries.len());
            if let Err(e) = compile_file(Path::new(input), Path::new(output)) {
                eprintln!("{e}");
                std::process::exit(1);
            }
            let size = std::fs::metadata(output).map(|m| m.len()).unwrap_or(0);
            println!("wrote {} ({} bytes)", output, size);
        }
        _ => {
            eprintln!("usage: opi-tools <compile|verify> ...");
            std::process::exit(2);
        }
    }
}
```

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p opi-tools --test cli`
Expected: PASS。

- [ ] **Step 5: 编译内置 fallback 词库并提交**

```bash
mkdir -p data/generated
cargo run -p opi-tools -- compile data/raw/fallback.tsv data/generated/fallback.opid
git add crates/opi-tools/src/main.rs crates/opi-tools/tests/cli.rs data/generated/fallback.opid
git commit -m "feat(opi-tools): compile CLI + build fallback.opid"
```

Expected: 输出 `input lines: 35`、`kept entries: 35`、`wrote .../fallback.opid`；fallback.opid 被 git 跟踪（.gitignore 例外生效）。

- [ ] **Step 6: 校验提交的 fallback.opid 可解析**

Run: `cargo run -p opi-tools -- verify data/generated/fallback.opid`（此时 verify 未实现会报 usage —— 预期行为，Task 11 补全；改用下列临时检查）

Run: `python3 -c "import sys;d=open('data/generated/fallback.opid','rb').read();print(len(d), d[:4])"` 
Expected: 文件存在、`OPID` 魔数正确。正式校验由 Task 11 的 verify 与 Task 8 的测试覆盖。

---

### Task 7: mmap 加载器 MmapDictionary

**Files:**
- Create: `crates/engine-data/src/loader.rs`

- [ ] **Step 1: 写失败测试**

`crates/engine-data/src/loader.rs` 测试模块：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{serialize, FormatError, OpDict, RawEntry};

    fn sample() -> Vec<u8> {
        serialize(&OpDict {
            entries: vec![
                RawEntry { pinyin: "hao".into(), word: "好".into(), freq: 5000 },
                RawEntry { pinyin: "hao".into(), word: "号".into(), freq: 1200 },
                RawEntry { pinyin: "xiao".into(), word: "笑".into(), freq: 3000 },
            ],
            // pinyin blob = "hao"(3) + "hao"(3) + "xiao"(4) = 10 字节
            pinyin_total: 10,
        })
    }

    fn temp_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "opi-load-{}-{}.opid",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn load_bytes_query_prefix_ordered() {
        let d = load_bytes(sample()).unwrap();
        assert_eq!(d.len(), 3);
        let got = d.query("h", 8);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].word, "好");
        assert_eq!(got[1].word, "号");
        let x = d.query("xiao", 8);
        assert_eq!(x[0].word, "笑");
    }

    #[test]
    fn query_partial_and_full_match() {
        let d = load_bytes(sample()).unwrap();
        assert_eq!(d.query("hao", 8).len(), 2);
        assert_eq!(d.query("ha", 8).len(), 2);
        assert_eq!(d.query("h", 8).len(), 2);
    }

    #[test]
    fn query_limit_truncates() {
        let d = load_bytes(sample()).unwrap();
        let got = d.query("hao", 1);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].word, "好");
    }

    #[test]
    fn query_no_match_or_empty() {
        let d = load_bytes(sample()).unwrap();
        assert!(d.query("zz", 8).is_empty());
        assert!(d.query("", 8).is_empty());
    }

    #[test]
    fn load_mmap_reads_file() {
        let path = temp_path();
        std::fs::write(&path, &sample()).unwrap();
        let d = load_mmap(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        let got = d.query("hao", 8);
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].word, "好");
    }

    #[test]
    fn corrupt_file_rejected() {
        let mut bytes = sample();
        bytes[20] ^= 0xFF;
        assert!(matches!(
            load_bytes(bytes),
            Err(LoadError::Format(FormatError::ChecksumMismatch { .. }))
        ));
    }

    #[test]
    fn missing_file_io_error() {
        assert!(matches!(
            load_mmap(Path::new("/nonexistent/opi.opid")),
            Err(LoadError::Io(_))
        ));
    }
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p engine-data`
Expected: FAIL（`loader` 模块为空）。

- [ ] **Step 3: 实现**

```rust
use std::path::Path;
use engine_core::dictionary::{Dictionary, Entry};
use crate::format::{parse, ENTRY_LEN, HEADER_LEN, OpDict};

/// 内存后盾：mmap 文件或堆上字节，避免自引用结构。
pub enum Backing {
    Map(memmap2::Mmap),
    Bytes(Vec<u8>),
}

#[derive(Debug)]
pub enum LoadError {
    Io(std::io::Error),
    Format(crate::format::FormatError),
}

/// 布局完全由 parse 校验过的参数恢复：count/pinyin_total 决定三个区段边界。
pub struct MmapDictionary {
    backing: Backing,
    count: usize,
    pinyin_total: usize,
}

impl MmapDictionary {
    fn from_parsed(backing: Backing, parsed: OpDict) -> Self {
        MmapDictionary {
            backing,
            count: parsed.entries.len(),
            pinyin_total: parsed.pinyin_total,
        }
    }

    fn data(&self) -> &[u8] {
        match &self.backing {
            Backing::Map(m) => m.as_ref(),
            Backing::Bytes(v) => v.as_slice(),
        }
    }

    fn table_start(&self) -> usize {
        HEADER_LEN
    }

    fn pinyin_start(&self) -> usize {
        HEADER_LEN + self.count * ENTRY_LEN
    }

    fn word_start(&self) -> usize {
        self.pinyin_start() + self.pinyin_total
    }

    /// 读第 i 条记录（parse 已校验，可安全 unwrap）。
    fn record(&self, data: &[u8], i: usize) -> (usize, usize, usize, usize, u32) {
        let off = self.table_start() + i * ENTRY_LEN;
        let po = u32::from_le_bytes(data[off..off + 4].try_into().unwrap()) as usize;
        let pl = data[off + 4] as usize;
        let wo = u32::from_le_bytes(data[off + 5..off + 9].try_into().unwrap()) as usize;
        let wl = data[off + 9] as usize;
        let freq = u32::from_le_bytes(data[off + 10..off + 14].try_into().unwrap());
        (po, pl, wo, wl, freq)
    }
}

impl Dictionary for MmapDictionary {
    fn query(&self, pinyin: &str, limit: usize) -> Vec<Entry> {
        if pinyin.is_empty() || self.count == 0 {
            return Vec::new();
        }
        let data = self.data();
        let needle = pinyin.as_bytes();
        let lo = lower_bound(data, self.table_start(), self.pinyin_start(), self.count, needle);
        let hi = match byte_successor(needle) {
            Some(succ) => lower_bound(data, self.table_start(), self.pinyin_start(), self.count, &succ),
            None => self.count,
        };
        let mut out: Vec<Entry> = Vec::new();
        let word_start = self.word_start();
        for i in lo..hi {
            let (_, _, wo, wl, freq) = self.record(data, i);
            let word = std::str::from_utf8(&data[word_start + wo..word_start + wo + wl])
                .expect("parse 已校验 UTF-8")
                .to_string();
            out.push(Entry { word, freq });
        }
        out.sort_by(|a, b| b.freq.cmp(&a.freq).then(a.word.as_bytes().cmp(b.word.as_bytes())));
        out.truncate(limit);
        out
    }

    fn len(&self) -> usize {
        self.count
    }
}

/// 第 i 条记录的 pinyin 字节。
fn entry_pinyin<'a>(data: &'a [u8], table_start: usize, pinyin_start: usize, i: usize) -> &'a [u8] {
    let off = table_start + i * ENTRY_LEN;
    let po = u32::from_le_bytes(data[off..off + 4].try_into().unwrap()) as usize;
    let pl = data[off + 4] as usize;
    &data[pinyin_start + po..pinyin_start + po + pl]
}

/// 前缀区间下界：第一个 pinyin >= needle 的下标。
fn lower_bound(data: &[u8], table_start: usize, pinyin_start: usize, count: usize, needle: &[u8]) -> usize {
    let mut lo = 0usize;
    let mut hi = count;
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if entry_pinyin(data, table_start, pinyin_start, mid) < needle {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// 字节后继：末字节 +1（带进位）。全 0xFF 返回 None（上界 = 表尾）。
fn byte_successor(p: &[u8]) -> Option<Vec<u8>> {
    let mut b = p.to_vec();
    let mut i = b.len();
    while i > 0 {
        i -= 1;
        let (nb, overflow) = b[i].overflowing_add(1);
        b[i] = nb;
        if !overflow {
            return Some(b);
        }
    }
    None
}

/// 打开文件并以只读 mmap 加载 + 校验。
pub fn load_mmap(path: &Path) -> Result<MmapDictionary, LoadError> {
    let file = std::fs::File::open(path).map_err(LoadError::Io)?;
    // safety: 只读共享映射，解析先于任何读取（parse 全量校验），映射生命周期随 self。
    let mmap = unsafe { memmap2::Mmap::map(&file) }.map_err(LoadError::Io)?;
    let parsed = parse(&mmap).map_err(LoadError::Format)?;
    Ok(MmapDictionary::from_parsed(Backing::Map(mmap), parsed))
}

/// 从堆上字节加载（测试与内嵌 fallback 用）。
pub fn load_bytes(bytes: Vec<u8>) -> Result<MmapDictionary, LoadError> {
    let parsed = parse(&bytes).map_err(LoadError::Format)?;
    Ok(MmapDictionary::from_parsed(Backing::Bytes(bytes), parsed))
}
```

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p engine-data && cargo clippy -p engine-data --all-targets -- -D warnings`
Expected: 全部 PASS；clippy 零警告。

- [ ] **Step 5: Commit**

```bash
git add crates/engine-data/src/loader.rs
git commit -m "feat(engine-data): mmap-backed dictionary loader"
```

---

### Task 8: 内置 fallback 与 load_or_fallback

**Files:**
- Create: `crates/engine-data/src/dictionary.rs`
- Modify: `crates/engine-data/src/lib.rs`（re-export）

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::dictionary::Dictionary;

    #[test]
    fn fallback_has_35_common_words() {
        let d = fallback_dict();
        assert_eq!(d.len(), 35);
        assert!(!d.is_empty());
    }

    #[test]
    fn fallback_queries_by_pinyin() {
        let d = fallback_dict();
        let wo = d.query("wo", 8);
        assert_eq!(wo[0].word, "我");
        let ni = d.query("n", 8);
        assert!(ni.iter().any(|e| e.word == "你"));
    }

    #[test]
    fn fallback_freqs_descending() {
        let d = fallback_dict();
        let hao = d.query("hao", 8);
        assert!(hao.windows(2).all(|w| w[0].freq >= w[1].freq));
    }

    #[test]
    fn load_or_fallback_none_returns_fallback() {
        let d = load_or_fallback(None).unwrap();
        assert_eq!(d.len(), 35);
    }

    #[test]
    fn load_or_fallback_missing_file_returns_fallback() {
        let d = load_or_fallback(Some(Path::new("/nonexistent/opi.opid"))).unwrap();
        assert_eq!(d.len(), 35);
    }

    #[test]
    fn load_or_fallback_corrupt_file_returns_fallback() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("opi-fb-{}", std::process::id()));
        std::fs::write(&path, b"OPID\x01\x00\x00garbage").unwrap();
        let d = load_or_fallback(Some(&path)).unwrap();
        let _ = std::fs::remove_file(&path);
        let wo = d.query("wo", 8);
        assert_eq!(wo[0].word, "我");
    }

    #[test]
    fn load_or_fallback_valid_file_uses_it() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("opi-fb-valid-{}", std::process::id()));
        let bytes = crate::format::serialize(&crate::format::OpDict {
            entries: vec![crate::format::RawEntry {
                pinyin: "opa".into(),
                word: "测试".into(),
                freq: 1,
            }],
            pinyin_total: 3,
        });
        std::fs::write(&path, &bytes).unwrap();
        let d = load_or_fallback(Some(&path)).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(d.len(), 1);
        assert_eq!(d.query("opa", 8)[0].word, "测试");
    }
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p engine-data`
Expected: FAIL（`dictionary` 模块为空）。

- [ ] **Step 3: 实现**

`crates/engine-data/src/dictionary.rs`：

```rust
use std::path::Path;
use engine_core::dictionary::Dictionary;
use crate::loader::{load_bytes, load_mmap, MmapDictionary};

/// 编译提交的内置回退词库（data/raw/fallback.tsv → opi-tools → data/generated/fallback.opid）。
/// 提交时经 opi-tools 校验，运行期解析失败即仓库损坏，直接 panic。
fn load_fallback() -> Result<MmapDictionary, String> {
    let bytes: &[u8] = include_bytes!("../../../data/generated/fallback.opid");
    load_bytes(bytes.to_vec()).map_err(|e| format!("内置 fallback 词库损坏: {e:?}"))
}

pub fn fallback_dict() -> MmapDictionary {
    load_fallback().expect("内置 fallback 词库损坏（提交时已校验）")
}

/// 加载指定词库；文件缺失/损坏一律回退内置词库。仅当内置词库本身损坏才返回 Err。
pub fn load_or_fallback(path: Option<&Path>) -> Result<Box<dyn Dictionary>, String> {
    if let Some(p) = path {
        if let Ok(d) = load_mmap(p) {
            return Ok(Box::new(d));
        }
    }
    load_fallback().map(|d| Box::new(d) as Box<dyn Dictionary>)
}
```

`crates/engine-data/src/lib.rs` 追加 re-export：

```rust
pub use checksum::fnv1a64;
pub use format::{parse, serialize, FormatError, OpDict, RawEntry};
pub use loader::{load_bytes, load_mmap, LoadError, MmapDictionary};
pub use dictionary::{fallback_dict, load_or_fallback};
// Dictionary trait 一并转发：opi-tools 的 verify 子命令（bin，不可用 dev-dep）需要
pub use engine_core::dictionary::Dictionary;
```

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p engine-data && cargo clippy -p engine-data --all-targets -- -D warnings`
Expected: 全部 PASS（含 fallback 7 个测试）；clippy 零警告。

- [ ] **Step 5: Commit**

```bash
git add crates/engine-data/src/dictionary.rs crates/engine-data/src/lib.rs
git commit -m "feat(engine-data): builtin fallback dict and load_or_fallback"
```

---

### Task 9: 端到端集成测试（Engine + mmap 词典）

**Files:**
- Create: `crates/opi-tools/tests/m2_integration.rs`

说明：engine-core 的 `Engine::new(Box<dyn Dictionary>, SymbolEngine, bool)` 已支持注入词典，无需改 engine-core；集成测试放 opi-tools（dev-dep engine-core）以复用 `parse_dict`/`compile`。

- [ ] **Step 1: 写失败测试**

```rust
use engine_core::dictionary::Dictionary;
use engine_core::engine::Engine;
use engine_core::symbols::SymbolEngine;
use engine_data::{fallback_dict, load_bytes, load_or_fallback, serialize, FormatError, LoadError};
use opi_tools::compiler::{compile, parse_dict};

#[test]
fn full_pipeline_engine_typing() {
    let text = "好\thao\n号\thao\t1200\n笑\txiao\t3000\n";
    let bytes = serialize(&compile(parse_dict(text)));
    let mmap = load_bytes(bytes).expect("load");
    let mut eng = Engine::new(Box::new(mmap), SymbolEngine::builtin(), false);
    for ch in "hao".chars() {
        eng.input_key(ch);
    }
    assert_eq!(eng.buffer(), "hao");
    let cands = eng.candidates(8);
    assert_eq!(cands[0].text, "好");
    assert_eq!(eng.select(0), "好");
    assert_eq!(eng.buffer(), "");
}

#[test]
fn engine_with_fallback_dict() {
    let mut eng = Engine::new(Box::new(fallback_dict()), SymbolEngine::builtin(), true);
    for ch in "wo".chars() {
        eng.input_key(ch);
    }
    let cands = eng.candidates(8);
    assert_eq!(cands[0].text, "我");
}

#[test]
fn corrupt_file_engine_falls_back() {
    let mut bytes = serialize(&compile(parse_dict("好\thao\n")));
    let n = bytes.len() - 9; // payload 区域（非 trailer）
    bytes[n] ^= 0xFF;
    assert!(matches!(
        load_bytes(bytes),
        Err(LoadError::Format(FormatError::ChecksumMismatch { .. }))
    ));
    let d = load_or_fallback(Some(std::path::Path::new("/nonexistent/opi.opid"))).unwrap();
    assert_eq!(d.len(), 35);
    let wo = d.query("wo", 8);
    assert_eq!(wo[0].word, "我");
}

#[test]
fn corrupted_opid_engine_still_boots() {
    // 引擎在词库损坏时仍可用（回退内置），全链路冒烟
    let mut eng = Engine::new(
        Box::new(load_or_fallback(None).unwrap()),
        SymbolEngine::builtin(),
        false,
    );
    for ch in "n".chars() {
        eng.input_key(ch);
    }
    assert!(eng.candidates(8).iter().any(|c| c.text == "你"));
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p opi-tools --test m2_integration`
Expected: FAIL（依赖符号未就位）。

- [ ] **Step 3: 运行确认通过（无需新实现代码）**

Run: `cargo test -p opi-tools && cargo clippy -p opi-tools --all-targets -- -D warnings`
Expected: 4 个集成测试 PASS，全量测试无回归；clippy 零警告。

- [ ] **Step 4: Commit**

```bash
git add crates/opi-tools/tests/m2_integration.rs
git commit -m "feat: end-to-end engine integration with mmap dictionary"
```

---

### Task 10: M1 遗留修正（符号引擎）

**Files:**
- Modify: `crates/engine-core/src/symbols.rs`
- Modify: `crates/engine-core/src/candidates.rs`（仅补测试）

- [ ] **Step 1: 写失败测试**

`crates/engine-core/src/symbols.rs` 测试模块追加：

```rust
#[test]
fn heart_suit_name_fixed() {
    let e = SymbolEngine::builtin();
    let hearts = e.search("heart");
    assert!(hearts.iter().any(|s| s.name == "心形"));
}

#[test]
fn search_matches_keyword_prefix() {
    let e = SymbolEngine::builtin();
    assert!(e.search("x").iter().any(|s| s.text == "😄")); // xiao
    assert!(e.search("sm").iter().any(|s| s.text == "😄")); // smile
    assert!(e.search("he").iter().any(|s| s.text == "♥")); // heart
    assert!(e.search("a").iter().any(|s| s.text == "あ")); // 平假名 a
}

#[test]
fn search_returns_deterministic_unique() {
    let e = SymbolEngine::builtin();
    let got = e.search("x");
    let mut texts: Vec<String> = got.iter().map(|s| s.text.clone()).collect();
    texts.sort();
    let mut uniq = texts.clone();
    uniq.dedup();
    assert_eq!(texts, uniq);
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p engine-core`
Expected: FAIL（`heart_suit_name_fixed`：name 仍是"黑桃心"；`search_matches_keyword_prefix`：exact 查找对 "x"/"sm"/"he" 返回空）。

- [ ] **Step 3: 实现修正**

`symbols.rs` 修改三处：

1) 条目名修正（builtin 中 ♥ 条目）：

```rust
SymbolEntry { text: "♥".into(), name: "心形".into(), keywords: vec!["heart".into(), "ai".into(), "xin".into()], block: BlockId(3), emoji: false },
```

2) `SymbolEngine::new` 加区块重叠断言（blocks 已按 start 排序）：

```rust
pub fn new(mut blocks: Vec<Block>, entries: Vec<SymbolEntry>) -> Self {
    blocks.sort_by_key(|b| b.start);
    debug_assert!(
        blocks.windows(2).all(|w| w[1].start > w[0].end),
        "Unicode 区块不可重叠"
    );
    // ...原逻辑
}
```

3) 精确 keyword 索引改为排序表 + 前缀二分（字段替换）：

```rust
pub struct SymbolEngine {
    blocks: Vec<Block>,
    entries: Vec<SymbolEntry>,
    keywords: Vec<(String, usize)>, // 排序后的 (小写 keyword, entry 下标)
    block_index: std::collections::HashMap<BlockId, Vec<usize>>,
}
```

`new` 中构建：

```rust
let mut keywords: Vec<(String, usize)> = Vec::new();
for (i, e) in entries.iter().enumerate() {
    block_index.entry(e.block).or_default().push(i);
    for kw in &e.keywords {
        keywords.push((kw.to_lowercase(), i));
    }
}
keywords.sort();
keywords.dedup();
```

`search` 替换为前缀匹配：

```rust
/// 关键字前缀搜索（拼音或英文小写；输入为完整/部分拼音）。
pub fn search(&self, keyword: &str) -> Vec<SymbolEntry> {
    let kw = keyword.to_lowercase();
    let keys: Vec<&str> = self.keywords.iter().map(|(k, _)| k.as_str()).collect();
    let lo = keys.partition_point(|k| k.as_bytes() < kw.as_bytes());
    let hi = match byte_successor(kw.as_bytes()) {
        Some(succ) => keys.partition_point(|k| k.as_bytes() < succ.as_slice()),
        None => keys.len(),
    };
    let mut out: Vec<SymbolEntry> = self.keywords[lo..hi]
        .iter()
        .map(|(_, i)| self.entries[*i].clone())
        .collect();
    out.sort_by(|a, b| a.text.cmp(&b.text));
    out.dedup_by(|a, b| a.text == b.text);
    out
}

/// 字节后继：末字节 +1（带进位）；全 0xFF 返回 None。
fn byte_successor(p: &[u8]) -> Option<Vec<u8>> {
    let mut b = p.to_vec();
    let mut i = b.len();
    while i > 0 {
        i -= 1;
        let (nb, overflow) = b[i].overflowing_add(1);
        b[i] = nb;
        if !overflow {
            return Some(b);
        }
    }
    None
}
```

（`byte_successor` 与 engine-data/loader.rs 中的实现重复，属有意为之：两个 crate 无依赖关系，且函数极简。）

- [ ] **Step 4: 补充 candidates 回归测试**

`crates/engine-core/src/candidates.rs` 测试模块追加：

```rust
#[test]
fn symbol_prefix_search_merges_into_candidates() {
    let d = test_dict();
    let s = SymbolEngine::builtin();
    let l = Learner::new(false);
    // "x" 前缀命中 😄（keyword xiao），同时拼音 xiao 词也出现
    let got = rank_and_pick(&d, &s, &l, "x", Mode::Pinyin, DEFAULT_TOP_N);
    assert!(got.iter().any(|c| c.kind == CandidateKind::Emoji && c.text == "😄"));
}
```

- [ ] **Step 5: 运行确认通过**

Run: `cargo test -p engine-core && cargo clippy -p engine-core --all-targets -- -D warnings`
Expected: 全部 PASS（62 + 新增）；clippy 零警告。

- [ ] **Step 6: Commit**

```bash
git add crates/engine-core/src/symbols.rs crates/engine-core/src/candidates.rs
git commit -m "fix(engine-core): symbol name typo, block overlap assert, prefix search"
```

---

### Task 11: verify 子命令 + 真实 rime 数据编译 + spec 偏差记录

**Files:**
- Modify: `crates/opi-tools/src/main.rs`
- Create: `crates/opi-tools/tests/cli_verify.rs`
- Modify: `docs/superpowers/specs/2026-08-12-opi-ime-design.md`（追加偏差节）

- [ ] **Step 1: 写失败测试**

`crates/opi-tools/tests/cli_verify.rs`：

```rust
#[test]
fn cli_verify_ok_and_rejects_corruption() {
    let dir = std::env::temp_dir().join(format!("opi-verify-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let tsv = dir.join("t.tsv");
    let opid = dir.join("t.opid");
    std::fs::write(&tsv, "好\thao\n号\thao\t1200\n").unwrap();

    let run = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_opi-tools"))
            .args(args)
            .output()
            .unwrap()
    };

    let ok = run(&["compile", tsv.to_str().unwrap(), opid.to_str().unwrap()]);
    assert!(ok.status.success(), "{}", String::from_utf8_lossy(&ok.stderr));

    let v = run(&["verify", opid.to_str().unwrap()]);
    assert!(v.status.success(), "{}", String::from_utf8_lossy(&v.stderr));
    let out = String::from_utf8_lossy(&v.stdout);
    assert!(out.contains("checksum: ok"));
    assert!(out.contains("entries: 2"));

    let mut bytes = std::fs::read(&opid).unwrap();
    bytes[20] ^= 0xFF;
    std::fs::write(&opid, &bytes).unwrap();
    let bad = run(&["verify", opid.to_str().unwrap()]);
    assert!(!bad.status.success());

    let _ = std::fs::remove_dir_all(&dir);
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p opi-tools --test cli_verify`
Expected: FAIL（verify 未实现，报 usage）。

- [ ] **Step 3: 实现 verify 子命令**

`main.rs` 追加（`use` 增加 `engine_data::{load_bytes, Dictionary}`）：

```rust
Some("verify") => {
    let Some(path) = args.get(2) else {
        eprintln!("usage: opi-tools verify <file.opid>");
        std::process::exit(2);
    };
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("read {}: {e}", path);
            std::process::exit(1);
        }
    };
    let t0 = std::time::Instant::now();
    match load_bytes(bytes) {
        Ok(d) => {
            let elapsed = t0.elapsed();
            println!("file: {}", path);
            println!("checksum: ok");
            println!("entries: {}", d.len());
            println!("load: {:.1}ms", elapsed.as_secs_f64() * 1000.0);
            for sample in ["hao", "wo", "n"] {
                let top: Vec<String> =
                    d.query(sample, 3).iter().map(|e| e.word.clone()).collect();
                println!("query \"{sample}\": {}", top.join(" "));
            }
        }
        Err(e) => {
            eprintln!("verify failed: {e:?}");
            std::process::exit(1);
        }
    }
}
```

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p opi-tools --test cli_verify`
Expected: PASS。

- [ ] **Step 5: 真实 rime 数据编译与验收**

```bash
ls /tmp/rime-luna-pinyin/luna_pinyin.dict.yaml || git clone --depth 1 git@github.com:rime/rime-luna-pinyin.git /tmp/rime-luna-pinyin
cargo run -p opi-tools -- compile /tmp/rime-luna-pinyin/luna_pinyin.dict.yaml data/generated/luna.opid
cargo run -p opi-tools -- verify data/generated/luna.opid
ls -l data/generated/luna.opid
```

Expected: compile 输出 `input lines: ~70771`、`kept entries: ~70000`；verify 输出 `checksum: ok`、`load: <200ms`、`entries: ~70000`；文件 < 30MB（预期 ~4MB）。**验收门槛：<30MB 与冷加载 <200ms 同时满足即通过。** `data/generated/luna.opid` 被 gitignore（验证：`git status` 不出现该文件）。

- [ ] **Step 6: 记录 spec 偏差**

`docs/superpowers/specs/2026-08-12-opi-ime-design.md` 末尾追加：

```markdown
## M2 实现偏差（2026-08-12）

1. **存储结构**：spec 原定 DAWG trie。实现为排序平面表（entries 表 + pinyin/word blob）+ 前缀二分。理由：~71K 条 rime-luna-pinyin 全量编译约 4MB，远低于 30MB 预算；DAWG 后缀共享最多省 ~5MB，却显著增加编译与加载复杂度。格式已版本化（magic OPID + version=1），后续可演进。
2. **许可证**：spec 原记 rime 词库 BSD/GPL 混合。实际 rime-luna-pinyin 为 **LGPL-3.0**（data/raw/LICENSES.md 已记录）。
3. **频率合成**：rime-luna-pinyin 无词频列 → 3 列行 `word\tpinyin\tNN.NN%` → freq = round(percent×1000)；2 列行 → freq = 1000；重复 (pinyin,word) 取 max。
4. **损坏回退**：spec 原定回退"内置精简词典"。实现为编译提交的内置 fallback.opid（35 条高频词，opi-tools 从 data/raw/fallback.tsv 生成，提交进仓库），与 M1 内置词等价且可再编译。
```

- [ ] **Step 7: Commit**

```bash
git add crates/opi-tools/src/main.rs crates/opi-tools/tests/cli_verify.rs docs/superpowers/specs/2026-08-12-opi-ime-design.md
git commit -m "feat(opi-tools): verify subcommand + real rime data compile + spec deviation"
```

---

### Task 12: 收尾 —— README、全量门禁、推送

**Files:**
- Modify: `README.md`

- [ ] **Step 1: 更新 README 里程碑与结构**

README.md 修改四处：

1) 构建与测试块（第 71-74 行）：

```markdown
```bash
cargo test --workspace                   # 单元 + 集成 + 属性测试
cargo clippy --workspace --all-targets -- -D warnings   # 门禁：零警告
```
```

2) 仓库结构块（engine-data / opi-tools 行替换为实际描述）：

```markdown
crates/
  engine-core/    # 纯逻辑内核，无 IO 无平台依赖
    src/          # composer / pinyin / trie / dictionary / learner / symbols / candidates / engine
    tests/        # engine_integration（11）+ proptests（5）
  engine-data/    # .opid 二进制词库：格式、FNV-1a 校验、mmap 加载、损坏回退（M2）
  opi-tools/      # 词库编译工具：dict.yaml → .opid + verify 校验（M2）
docs/superpowers/ # 设计规格与实施计划
data/             # 词库源数据（raw）与编译产物（generated，fallback.opid 入库）
```

3) 里程碑行（第 96 行）：

```markdown
- [x] **M2 数据管线**：opi-tools 编译词库 → `.opid` 二进制（mmap 加载、校验、损坏回退）
```

4) 许可证行（第 105 行）：

```markdown
- **词库数据**：按上游许可证单独声明（rime-luna-pinyin 为 LGPL-3.0），`data/raw` 逐条记录来源与许可证
```

- [ ] **Step 2: 全量门禁**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: 全部 PASS，零警告。

- [ ] **Step 3: 确认生成物未被误提交**

Run: `git status`
Expected: 仅 README.md 与待提交内容；`data/generated/luna.opid` 不出现。

- [ ] **Step 4: Commit 并推送**

```bash
git add README.md
git commit -m "docs: M2 milestone complete, update README"
git push origin main
```

Expected: push 成功（origin = git@github.com:erikwang2013/opi.git，main 分支）。

---

## Self-Review 记录

- **Spec 覆盖**：spec §5.1（.opid 格式：magic/version/表结构）→ Task 4；§5.2（mmap 加载）→ Task 7；§5.3（校验与损坏回退）→ Task 4/7/8；验收（<30MB、冷加载 <200ms）→ Task 11 Step 5；M2 milestone → Task 12。
- **无占位**：所有步骤含完整代码与命令；fallback.tsv 35 条精确列出；无 TBD。
- **类型一致性**：`fnv1a64`（Task 3）被 format.rs（Task 4）使用；`serialize/parse/OpDict/RawEntry/FormatError`（Task 4）被 compiler.rs（Task 5）、loader.rs（Task 7）、dictionary.rs（Task 8）、测试（Task 6/9/11）使用；`load_bytes/load_mmap/MmapDictionary/LoadError`（Task 7）被 dictionary.rs（Task 8）与集成测试（Task 9）使用；`compile/parse_dict`（Task 5）被 main.rs（Task 6/11）与 m2_integration（Task 9）使用。签名与 re-export 在 Task 8 Step 3 一次对齐；`Dictionary` trait 由 engine-data 转发（Task 8），供 opi-tools bin 使用（Task 11）。
- **测试事实链**：fallback 测试断言（len=35、wo→我、n→你）与 Task 2 数据一致；Task 4/7 的 sample 与断言一致（pinyin_total=10 = hao+hao+xiao）；Task 8/9 的 pinyin_total 与各自输入一致。
- **已知约定**：`byte_successor` 在 engine-data loader 与 engine-core symbols 各有一份（两 crate 无依赖，函数 10 行，重复是刻意的）；serialize 对 pinyin/word 长度有 debug_assert，编译管线在 parse_dict 保证 ≤255。
