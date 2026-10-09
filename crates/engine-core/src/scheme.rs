// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 输入方案：**按键 → 字母类**的映射（2026-10-09，移动端「输入模式」）。
//!
//! `Mode` 管「语言变体」（简/繁/英/数字/符号），本模块管「键盘怎么摊开」：
//!
//! | 方案 | 键 | 每键覆盖的字母 |
//! |---|---|---|
//! | [`Scheme::Full`] | 26 键 | 1 对 1（既有行为） |
//! | [`Scheme::Nine`] | `2`–`9` | `abc` / `def` / … / `wxyz`（T9 式） |
//! | [`Scheme::Fourteen`] | `q e t u o / a d g j l / z c b m` | 见 `key_class` 的表 |
//! | [`Scheme::Stroke`] | `h s p n z` | 1 对 1（**换的是词典**，见 Engine::set_stroke_dict） |
//!
//! 匹配语义是**前缀**（与全键盘的 `query_prefix` 同义）：`"64"` 命中
//! `ni`/`nihao`/`nu…` 之类以「按键类里包含 n、再包含 i」开头的键。
//!
//! ⚠️ 为什么匹配放在 engine-core 而不是各端：9 键的「64」**不是任何拼音键的
//! 前缀**（前缀查询表达不了按键类），必须遍历词库逐键比对；这份遍历与判定若四端
//! 各写一份必漂（本仓老毛病）。四端只负责「把键送进来」。
//!
//! ⚠️ 14 键布局是**本项目定义**（不是从搜狗抄的键表）：两字母键取字母对的
//! **首个字母**为规范符号（`qw`→`q`），`l`/`m` 为单字母键。宿主按此布局画键盘、
//! 发符号；改这张表 = 改协议，四端要一起改（有单测钉着）。

/// 输入方案。**默认 Full = 既有行为**（老调用方一行不动）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scheme {
    /// 全键盘（26 键）。
    #[default]
    Full,
    /// 拼音 9 键（T9）。数字键在宿主侧**不再是选词键**（路由门在 KeyRouter / 三轨
    /// router；引擎侧只保证「送进来的数字」在 Nine 下进缓冲而不是被丢弃）。
    Nine,
    /// 拼音 14 键（两字母一组的紧凑布局）。
    Fourteen,
    /// 笔画键盘：键就是 `h/s/p/n/z` 五个普通字母，走既有字母路径 ——
    /// 本方案改变的是 [`crate::Engine::candidates`] 查哪个词典（`stroke_dict`）
    /// 以及**关掉模糊音/简拼**（对笔画串没有意义）。
    Stroke,
}

impl Scheme {
    /// 本方案下这个字符是不是**输入键**；是 → 返回该键覆盖的字母类。
    ///
    /// `Full`/`Stroke` 恒 `None`：Full 走既有字母路径；Stroke 的五个键是普通字母，
    /// 也走既有路径（换词典不换映射）。宿主画键盘时用同一张表（9 键九个键、
    /// 14 键十四个键），**别在宿主另抄一份**。
    pub fn key_class(self, ch: char) -> Option<&'static [char]> {
        match self {
            Scheme::Full | Scheme::Stroke => None,
            Scheme::Nine => Some(match ch {
                '2' => &['a', 'b', 'c'],
                '3' => &['d', 'e', 'f'],
                '4' => &['g', 'h', 'i'],
                '5' => &['j', 'k', 'l'],
                '6' => &['m', 'n', 'o'],
                '7' => &['p', 'q', 'r', 's'],
                '8' => &['t', 'u', 'v'],
                '9' => &['w', 'x', 'y', 'z'],
                _ => return None,
            }),
            Scheme::Fourteen => Some(match ch {
                'q' => &['q', 'w'],
                'e' => &['e', 'r'],
                't' => &['t', 'y'],
                'u' => &['u', 'i'],
                'o' => &['o', 'p'],
                'a' => &['a', 's'],
                'd' => &['d', 'f'],
                'g' => &['g', 'h'],
                'j' => &['j', 'k'],
                'l' => &['l'],
                'z' => &['z', 'x'],
                'c' => &['c', 'v'],
                'b' => &['b', 'n'],
                'm' => &['m'],
                _ => return None,
            }),
        }
    }

    /// 前缀语义匹配：`pinyin` 的前 `input.len()` 个字母逐个落在 `input` 按键的类里。
    ///
    /// `Full`/`Stroke` 退化成正前缀（`starts_with`）—— 生产路径不会用它们走这里
    /// （candidates 只在 Nine/Fourteen 下调本函数），但保持语义自洽，单测也拿它
    /// 当基线。输入为空 → false（空缓冲没有候选，与各层同一条约定）。
    pub fn matches(self, input: &str, pinyin: &str) -> bool {
        if input.is_empty() {
            return false;
        }
        match self {
            Scheme::Full | Scheme::Stroke => pinyin.starts_with(input),
            Scheme::Nine | Scheme::Fourteen => {
                // 分词符（`'`，搜狗「分词键」发它）：词库键**不含**分隔符（格式约定），
                // 按段长切开比 —— 语义见 matches_segmented。
                if input.contains('\'') {
                    return self.matches_segmented(input, pinyin);
                }
                if pinyin.len() < input.len() {
                    return false;
                }
                input.bytes().zip(pinyin.bytes()).all(|(k, p)| {
                    // 类表只覆盖 ASCII 字母；词库键是 ASCII 小写（格式约定），
                    // 但遍历时仍防御非 ASCII 键：取不到类即不匹配。
                    self.key_class(char::from(k))
                        .is_some_and(|class| class.contains(&char::from(p)))
                })
            }
        }
    }

    /// 带**分词符**的匹配（2026-10-09，搜狗「分词键」）：`input` 按 `'` 分段，
    /// 词库拼音按段长切开逐段比对（词库键不含分隔符）。
    ///
    /// - 每段仍走按键类（9/14 键），逐字节比 —— 与无分隔那条**同一条规则**；
    /// - **非末段必须落在音节边界上**（`segment()` 的每块都是完整音节）：分词的
    ///   意义就是「这里断」——`64'26` 要求拼音前两字母恰是一个音节（ni），
    ///   才让「nian」按 ni|an 命中；`644'26`（断在 nih 后）不对齐，谁也不匹配。
    /// - 末段可只到前缀（正在打，与全键盘「打一半」同义）。
    /// - 空段（开头/结尾/连续 `'`）视为无效输入，不匹配。
    fn matches_segmented(self, input: &str, pinyin: &str) -> bool {
        let mut rest = pinyin;
        let segs: Vec<&str> = input.split('\'').collect();
        for (i, seg) in segs.iter().enumerate() {
            if seg.is_empty() {
                // 末段为空（`64'`：刚点分词还没继续打）= 与不带分词同义（前面的段
                // 已验过音节边界）；开头/连续 `'` 的空段不是有效输入。
                return i + 1 == segs.len();
            }
            if rest.len() < seg.len() {
                return false;
            }
            let (head, tail) = rest.split_at(seg.len());
            if !seg.bytes().zip(head.bytes()).all(|(k, p)| {
                self.key_class(char::from(k))
                    .is_some_and(|class| class.contains(&char::from(p)))
            }) {
                return false;
            }
            if i + 1 == segs.len() {
                return true;
            }
            if !boundary_aligned(head) {
                return false;
            }
            rest = tail;
        }
        false
    }
}

/// `head` 是否落在**音节边界**上：贪婪切分后每块都是完整音节且拼回原串
/// （`ni` ✓、`nihao` ✓；`nih` 切出 ["ni","h"]，`h` 不是音节 ⇒ ✗）。
fn boundary_aligned(head: &str) -> bool {
    let segs = crate::pinyin::segment(head);
    !segs.is_empty() && segs.iter().all(|s| crate::pinyin::is_syllable(s)) && segs.concat() == head
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nine_key_class_table() {
        assert_eq!(Scheme::Nine.key_class('2'), Some(&['a', 'b', 'c'][..]));
        assert_eq!(Scheme::Nine.key_class('6'), Some(&['m', 'n', 'o'][..]));
        assert_eq!(Scheme::Nine.key_class('7'), Some(&['p', 'q', 'r', 's'][..]));
        // 0/1 与字母都不是 9 键的输入键
        assert_eq!(Scheme::Nine.key_class('0'), None);
        assert_eq!(Scheme::Nine.key_class('1'), None);
        assert_eq!(Scheme::Nine.key_class('a'), None);
    }

    #[test]
    fn fourteen_key_class_table_is_pinned() {
        // 这张表是**协议**（宿主画键盘、发符号都用它）：钉住整表，改一个字母
        // 都要在这里先红。
        let table: Vec<(char, &[char])> = vec![
            ('q', &['q', 'w']),
            ('e', &['e', 'r']),
            ('t', &['t', 'y']),
            ('u', &['u', 'i']),
            ('o', &['o', 'p']),
            ('a', &['a', 's']),
            ('d', &['d', 'f']),
            ('g', &['g', 'h']),
            ('j', &['j', 'k']),
            ('l', &['l']),
            ('z', &['z', 'x']),
            ('c', &['c', 'v']),
            ('b', &['b', 'n']),
            ('m', &['m']),
        ];
        // 14 个键，且并集恰好是全部 26 个字母（无遗漏、无重复）
        assert_eq!(table.len(), 14);
        let mut all: Vec<char> = table
            .iter()
            .flat_map(|(_, cs)| cs.iter().copied())
            .collect();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), 26, "字母并集不是 26 个（有重复或遗漏）：{all:?}");
        for (k, cs) in table {
            assert_eq!(Scheme::Fourteen.key_class(k), Some(cs), "键 {k} 的类不符");
        }
        assert_eq!(
            Scheme::Fourteen.key_class('x'),
            None,
            "x 不是 14 键的键（属 z 键的类）"
        );
        assert_eq!(Scheme::Fourteen.key_class('2'), None);
    }

    #[test]
    fn nine_matches_prefix_semantics() {
        assert!(Scheme::Nine.matches("64", "ni"));
        assert!(
            Scheme::Nine.matches("64", "nihao"),
            "前缀：ni 的扩展 nihao 也该命中"
        );
        assert!(Scheme::Nine.matches("64426", "nihao"));
        assert!(!Scheme::Nine.matches("64", "li"), "l 不在 6 的类里");
        assert!(!Scheme::Nine.matches("64426", "ni"), "输入比键长 → 不匹配");
        assert!(!Scheme::Nine.matches("", "ni"));
        assert!(!Scheme::Nine.matches("64", ""), "空键不匹配");
    }

    #[test]
    fn fourteen_matches_prefix_semantics() {
        // n→b 键、i→u 键 ⇒ "bu" 命中 "ni"
        assert!(Scheme::Fourteen.matches("bu", "ni"));
        assert!(Scheme::Fourteen.matches("bu", "nihao"));
        // q 键类 {q,w}：输入 q 命中 qiang/wang
        assert!(Scheme::Fourteen.matches("q", "qiang"));
        assert!(Scheme::Fourteen.matches("q", "wang"));
        assert!(!Scheme::Fourteen.matches("q", "ni"));
        assert!(!Scheme::Fourteen.matches("", "ni"));
    }

    #[test]
    fn full_and_stroke_are_plain_prefix() {
        assert!(Scheme::Full.matches("ni", "nihao"));
        assert!(Scheme::Stroke.matches("hsp", "hspnz"));
        assert!(!Scheme::Stroke.matches("hsp", "hs"));
    }
}
