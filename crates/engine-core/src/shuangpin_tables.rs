// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 双拼键位表（**生成物，勿手改**；生成器 `scripts/gen_shuangpin.py`）。
//!
//! 源：fcitx/libime `src/libime/pinyin/shuangpindata.h`，pin commit
//! `171edcf137001e8eb8274f53ec010058cda70b09`（装配语义见同仓 `shuangpinprofile.cpp`：零声母标记与
//! 单字符韵母自动补表都逐条镜像）。覆盖**搜狗同款**六套（自然码/微软/小鹤/
//! 智能ABC/紫光/拼音加加）；libime 另有中文之星与 GB，不在搜狗列表内，未收。
//!
//! 结构：`initials` = 声母键（只有 zh/ch/sh 需要列表，单字母声母即字母自身）；
//! `finals` = 韵母键（含自动补齐的 a/e/i/o/u/v）；`zero` = 零声母标记，
//! `*` 表示裸韵母键也接受（如小鹤）。

/// 一套双拼方案的表。键都是小写字母（宿主送进来的也只会是小写）。
pub struct ShuangpinTable {
    pub name: &'static str,
    /// (键, 韵母)：打进这个键 = 这个韵母。
    pub finals: &'static [(char, &'static str)],
    /// (键, 声母)：只在 zh/ch/sh 三个多字母声母上非空。
    pub initials: &'static [(char, &'static str)],
    /// 零声母标记串；含 `*` = 裸韵母键也可作零声母音节。
    pub zero: &'static str,
}

/// 全部方案（顺序 = 设置界面的展示序；索引即 FFI 的口径）。
pub static SHUANGPIN_TABLES: &[ShuangpinTable] = &[
    ShuangpinTable {
        name: "自然码",
        finals: &[('a', "a"), ('b', "ou"), ('c', "iao"), ('d', "iang"), ('d', "uang"), ('e', "e"), ('f', "en"), ('g', "eng"), ('h', "ang"), ('i', "i"), ('j', "an"), ('k', "ao"), ('l', "ai"), ('m', "ian"), ('n', "in"), ('o', "o"), ('o', "uo"), ('p', "un"), ('q', "iu"), ('r', "er"), ('r', "uan"), ('s', "iong"), ('s', "ong"), ('t', "ue"), ('t', "ve"), ('u', "u"), ('v', "ui"), ('v', "v"), ('w', "ia"), ('w', "ua"), ('x', "ie"), ('y', "ing"), ('y', "uai"), ('z', "ei")],
        initials: &[('i', "ch"), ('u', "sh"), ('v', "zh")],
        zero: "o*",
    },
    ShuangpinTable {
        name: "微软",
        finals: &[(';', "ing"), ('a', "a"), ('b', "ou"), ('c', "iao"), ('d', "iang"), ('d', "uang"), ('e', "e"), ('f', "en"), ('g', "eng"), ('h', "ang"), ('i', "i"), ('j', "an"), ('k', "ao"), ('l', "ai"), ('m', "ian"), ('n', "in"), ('o', "o"), ('o', "uo"), ('p', "un"), ('q', "iu"), ('r', "er"), ('r', "uan"), ('s', "iong"), ('s', "ong"), ('t', "ue"), ('u', "u"), ('v', "ui"), ('v', "v"), ('v', "ve"), ('w', "ia"), ('w', "ua"), ('x', "ie"), ('y', "uai"), ('y', "v"), ('z', "ei")],
        initials: &[('i', "ch"), ('u', "sh"), ('v', "zh")],
        zero: "o",
    },
    ShuangpinTable {
        name: "小鹤",
        finals: &[('a', "a"), ('b', "in"), ('c', "ao"), ('d', "ai"), ('e', "e"), ('f', "en"), ('g', "eng"), ('h', "ang"), ('i', "i"), ('j', "an"), ('k', "ing"), ('k', "uai"), ('l', "iang"), ('l', "uang"), ('m', "ian"), ('n', "iao"), ('o', "o"), ('o', "uo"), ('p', "ie"), ('q', "iu"), ('r', "uan"), ('s', "iong"), ('s', "ong"), ('t', "ue"), ('t', "ve"), ('u', "u"), ('v', "ui"), ('v', "v"), ('w', "ei"), ('x', "ia"), ('x', "ua"), ('y', "un"), ('z', "ou")],
        initials: &[('i', "ch"), ('u', "sh"), ('v', "zh")],
        zero: "*",
    },
    ShuangpinTable {
        name: "智能ABC",
        finals: &[('a', "a"), ('b', "ou"), ('c', "in"), ('c', "uai"), ('d', "ia"), ('d', "ua"), ('e', "e"), ('f', "en"), ('g', "eng"), ('h', "ang"), ('i', "i"), ('j', "an"), ('k', "ao"), ('l', "ai"), ('m', "ue"), ('m', "ui"), ('m', "ve"), ('n', "un"), ('o', "o"), ('o', "uo"), ('p', "uan"), ('q', "ei"), ('r', "er"), ('r', "iu"), ('s', "iong"), ('s', "ong"), ('t', "iang"), ('t', "uang"), ('u', "u"), ('v', "v"), ('w', "ian"), ('x', "ie"), ('y', "ing"), ('z', "iao")],
        initials: &[('e', "ch"), ('v', "sh"), ('a', "zh")],
        zero: "o",
    },
    ShuangpinTable {
        name: "紫光",
        finals: &[(';', "ing"), ('a', "a"), ('b', "iao"), ('d', "ie"), ('e', "e"), ('f', "ian"), ('g', "iang"), ('g', "uang"), ('h', "iong"), ('h', "ong"), ('i', "i"), ('j', "er"), ('j', "iu"), ('k', "ei"), ('l', "uan"), ('m', "un"), ('n', "ue"), ('n', "ui"), ('n', "ve"), ('o', "o"), ('o', "uo"), ('p', "ai"), ('q', "ao"), ('r', "an"), ('s', "ang"), ('t', "eng"), ('u', "u"), ('v', "v"), ('w', "en"), ('x', "ia"), ('x', "ua"), ('y', "in"), ('y', "uai"), ('z', "ou")],
        initials: &[('a', "ch"), ('i', "sh"), ('u', "zh")],
        zero: "o",
    },
    ShuangpinTable {
        name: "拼音加加",
        finals: &[('a', "a"), ('b', "ia"), ('b', "ua"), ('c', "uan"), ('d', "ao"), ('e', "e"), ('f', "an"), ('g', "ang"), ('h', "iang"), ('h', "uang"), ('i', "i"), ('j', "ian"), ('k', "iao"), ('l', "in"), ('m', "ie"), ('n', "iu"), ('o', "o"), ('o', "uo"), ('p', "ou"), ('q', "er"), ('q', "ing"), ('r', "en"), ('s', "ai"), ('t', "eng"), ('u', "u"), ('v', "ui"), ('v', "v"), ('w', "ei"), ('x', "uai"), ('x', "ue"), ('x', "ve"), ('y', "iong"), ('y', "ong"), ('z', "un")],
        initials: &[('u', "ch"), ('i', "sh"), ('v', "zh")],
        zero: "o*",
    },
];
