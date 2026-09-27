// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 中文标点表 + 全角映射（用户裁决 2026-09-27：「引擎层加标点表，四端走同一条路」）。
//!
//! 表是**代码里的 const 而不是 `data/raw/*.tsv`**：约 20 行、无出处无 license
//! （`data/raw/` 那几套是带出处/许可的语料），且每行都要写「为什么是这个字」的
//! 注释 —— 值得做成数据文件的是「可增长、有来源」的东西。四端共用的**单一真相**
//! 由 `Engine::punct_text` 这一处判定保证，与表放在哪种文件里无关。
//!
//! 两种目标字符集（**两个独立开关管**，见 `Engine::punct_text`）：
//! - **中文标点**（[`chinese`]）：`,` → `，`、`.` → `。`、`\` → `、`（中文专用字形），
//!   受 `Engine::chinese_punct` 管，**本函数只出表命中、不兜底**；
//! - **机械全角**（[`ascii_fullwidth`]）：`.` → `．`(U+FF0E)，受 `Engine::fullwidth` 管，
//!   同时充当中文模式表未命中时的兜底。西文文本里冒出中文句号是错的。

/// ASCII 键 → 中文标点。成对引号在表里写**左**引号，右引号由 [`Quotes`] 交替给出。
///
/// 只放有中文专用字形的键：`.` 是 `。`(U+3002) 而不是全角的 `．`(U+FF0E)，
/// `[`/`]` 是直角引号 `【】`、`<`/`>` 是书名号 `《》`。其余 ASCII 标点在
/// `Engine::fullwidth` 开着时走 [`ascii_fullwidth`] 机械全角兜底 —— 兜底与表命中
/// 分属两个开关，关掉全角只让兜底直通，表里这些键照出中文标点。
///
/// **来源/约定**：照 fcitx5 的中文标点约定（`fcitx://config/addon/punctuation/
/// punctuationmap/zh_CN`；本机对应文件 `/usr/share/fcitx5/punctuation/punc.mb.zh_CN`）。
/// 2026-09-27 与该文件**逐条实测对照**：本表 16 条的**目标字形**与它的**首个候选**
/// 完全相同（含 `"`→`“`、`'`→`‘` 取左引号——与 [`Quotes`] 的交替模型一致）。
/// 它收录而本表未收录的键（`$ % ^ _ ` { }`）在全角开关开着时走机械全角兜底，
/// **行为与它不同**：
/// 它 `` ` ``→`·`、`^`→`……`、`_`→`——`、`$`→`$`（原样，循环里还有 `￥`），
/// 本表分别出 `｀`、`＾`、`＿`、`＄`。**team-lead 2026-09-28 裁决：保持机械全角、
/// 不跟随**（理由：表内容变更不属「拆两个开关」那一轮；且它的 `^`→`……`、`_`→`——`
/// 是**多字符**替换，跟随要改本表的数据模型，不是加 4 行）。这 4 键已作为**开放项**
/// 上报用户；用户若要跟，是独立一轮 + 改模型，**别再当悬案评估**。
/// 它一个键有多个候选（重复按循环），本表是单目标 + 引号交替，**有意不同**。
pub const CHINESE_PUNCT: &[(char, char)] = &[
    (',', '，'),
    ('.', '。'),
    ('?', '？'),
    ('!', '！'),
    (':', '：'),
    (';', '；'),
    // 顿号：中文最常用的标点之一，主流 IME 都挂在反斜杠上（HK/TW 亦然）
    ('\\', '、'),
    ('(', '（'),
    (')', '）'),
    ('[', '【'),
    (']', '】'),
    ('<', '《'),
    ('>', '》'),
    ('~', '～'),
    ('"', '“'),
    ('\'', '‘'),
];

/// 成对引号的交替状态：同键交替出左/右引号。
///
/// **复位时机 = 切模式**（调用方是 `Engine::switch_mode`）。**不随提交/清缓冲复位**：
/// 引号交替是**排版状态**，跟着「正在写的这段文本」走 —— 提交一个词再打右引号是
/// 常态，而缓冲在每次选词后都会清空，跟着缓冲复位等于右引号永远打不出来。
/// 全角开关也不复位：它只管宽度，不管此前打过什么。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Quotes {
    /// 下一对 `"` 出右引号。
    double_closed: bool,
    /// 下一对 `'` 出右引号。
    single_closed: bool,
}

impl Quotes {
    /// 复位到「下一对出左引号」。
    pub fn reset(&mut self) {
        *self = Quotes::default();
    }

    /// 取 `ch` 该出的引号并翻转状态；`ch` 不是成对引号 → `None`。
    pub fn next(&mut self, ch: char) -> Option<char> {
        let (slot, open, close) = match ch {
            '"' => (&mut self.double_closed, '“', '”'),
            '\'' => (&mut self.single_closed, '‘', '’'),
            _ => return None,
        };
        let out = if *slot { close } else { open };
        *slot = !*slot;
        Some(out)
    }
}

/// 中文标点表查找：成对引号 → 表。
/// `None` = 这个键**不在表里**（控制符、非 ASCII、字母数字，以及表未收录的 ASCII 标点）。
///
/// **只出表命中，不兜底**（用户裁决 2026-09-28 拆开关）：表未收录的键（`^` `/` `@`…）
/// 此前在这里被机械全角兜底，拆开后兜底归 `Engine::punct_text` 里 `fullwidth` 那一档
/// —— 表命中受 `chinese_punct` 管、兜底受 `fullwidth` 管，两层各自有闸，不能混在
/// 一个函数里出结果（混在一起，「关全角」必然连表一起关）。
pub fn chinese(ch: char, quotes: &mut Quotes) -> Option<char> {
    if let Some(q) = quotes.next(ch) {
        return Some(q);
    }
    CHINESE_PUNCT
        .iter()
        .find(|&&(k, _)| k == ch)
        .map(|&(_, target)| target)
}

/// 机械全角：ASCII 标点平移 +0xFEE0。
///
/// Unicode 全角区的 U+FF01..U+FF5E 与 ASCII 的 0x21..0x7E **一一连续对应**，
/// 所以这里不需要表。字母数字**不在内**：全角字母表是另一件事（本表只管标点，
/// 也不该改掉英文直传路径的大小写行为）。
pub fn ascii_fullwidth(ch: char) -> Option<char> {
    if !ch.is_ascii_punctuation() {
        return None;
    }
    char::from_u32(ch as u32 + 0xFEE0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_keys_are_unique_ascii_punct() {
        let mut seen = Vec::new();
        for &(k, _) in CHINESE_PUNCT {
            assert!(k.is_ascii_punctuation(), "表键 {k:?} 不是 ASCII 标点");
            assert!(!seen.contains(&k), "表键 {k:?} 重复");
            seen.push(k);
        }
    }

    /// 表里的目标必须是**非 ASCII**（ASCII 目标 = 等于没映射）且单字符。
    #[test]
    fn table_targets_are_non_ascii() {
        for &(k, t) in CHINESE_PUNCT {
            assert!(!t.is_ascii(), "{k:?} → {t:?} 是 ASCII，等于没映射");
        }
    }

    /// [`chinese`] **只出表命中**：表未收录的键返回 `None`，机械全角兜底不在这层
    /// —— 兜底归 `Engine::punct_text` 的 `fullwidth` 那一档（拆开关的前提）。
    /// 这里若把 `/` 兜底成 `／`，`Engine` 那道独立的闸就永远看不到 `None`，
    /// 「关全角」又会连表一起关。
    #[test]
    fn chinese_lookup_does_not_fall_back_to_mechanical_fullwidth() {
        let mut q = Quotes::default();
        for c in ['/', '^', '@', 'a', '1', '中'] {
            assert_eq!(chinese(c, &mut q), None, "{c:?} 不在表里 ⇒ 不归这层出结果");
        }
        assert_eq!(chinese(',', &mut q), Some('，'), "表命中照出");
    }

    /// `.` 是这张表存在的理由：中文模式 `。`(U+3002) 与全角的 `．`(U+FF0E) **不同**，
    /// 英文模式取后者 —— 两者混用就是「西文里冒出中文句号」。
    #[test]
    fn cjk_full_stop_differs_from_mechanical_fullwidth() {
        let mut q = Quotes::default();
        assert_eq!(chinese('.', &mut q), Some('。'));
        assert_eq!(ascii_fullwidth('.'), Some('．'));
        assert_ne!(chinese('.', &mut q), ascii_fullwidth('.'));
    }

    /// 机械全角在 0x21..=0x7E 上是**双射**（全角区连续对应），且不碰字母数字。
    #[test]
    fn mechanical_fullwidth_is_a_bijection_on_punctuation() {
        let mut n = 0;
        for c in (0x21u8..=0x7e).map(char::from) {
            match ascii_fullwidth(c) {
                Some(f) => {
                    assert!(c.is_ascii_punctuation(), "字母数字 {c:?} 被全角化了");
                    assert_eq!(f as u32, c as u32 + 0xFEE0);
                    assert_eq!(ascii_fullwidth(f), None, "{f:?} 不该再被全角化一次");
                    n += 1;
                }
                // 反函数：全角区里每个字符都找得到一个 ASCII 前身
                None => assert!(!c.is_ascii_punctuation(), "标点 {c:?} 没有全角形"),
            }
        }
        assert_eq!(n, 32, "ASCII 标点共 32 个");
    }

    #[test]
    fn quotes_alternate_per_pair_and_reset() {
        let mut q = Quotes::default();
        assert_eq!(q.next('"'), Some('“'));
        assert_eq!(q.next('"'), Some('”'));
        assert_eq!(q.next('\''), Some('‘'), "单引号另有状态，不跟双引号走");
        assert_eq!(q.next('\''), Some('’'));
        assert_eq!(q.next('a'), None);
        q.reset();
        assert_eq!(q.next('"'), Some('“'));
        assert_eq!(q.next('\''), Some('‘'));
    }
}
