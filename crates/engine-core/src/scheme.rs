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
//! | [`Scheme::Wubi`] | 26 键 | 1 对 1（**换的是词典**，见 Engine::set_wubi_dict，2026-10-14） |
//!
//! 匹配语义是**前缀**（与全键盘的 `query_prefix` 同义）：`"64"` 命中
//! `ni`/`nihao`/`nu…` 之类以「按键类里包含 n、再包含 i」开头的键。
//!
//! ⚠️ 为什么匹配放在 engine-core 而不是各端：9 键的「64」**不是任何拼音键的
//! 前缀**（前缀查询表达不了按键类），必须遍历词库逐键比对；这份遍历与判定若四端
//! 各写一份必漂（本仓老毛病）。四端只负责「把键送进来」。
//!
//! ⚠️ 14 键布局是**本项目定义**（不是从主流输入法抄的键表）：两字母键取字母对的
//! **首个字母**为规范符号（`qw`→`q`），`l`/`m` 为单字母键。宿主按此布局画键盘、
//! 发符号；改这张表 = 改协议，四端要一起改（有单测钉着）。

use crate::dictionary::Dictionary;

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
    /// 双拼（2026-10-13 用户裁决「主流同款全套」）：键序按方案表**解码成全拼串**，
    /// 下游复用既有全拼管线（见 [`crate::shuangpin`]）。具体布局（自然码/微软/小鹤/
    /// 智能ABC/紫光/拼音加加）是 `Engine::shuangpin_layout` 的运行期状态，
    /// 不在本枚举里 —— 枚举是「匹配语义」的轴，布局只是那张表的索引。
    Shuangpin,
    /// 五笔（2026-10-14，M4 3b）：键是普通 ASCII 字母、**匹配即前缀** —— 与
    /// `Stroke` 同型（换的是词典，见 `Engine::set_wubi_dict`）。输入风格
    /// （纯五笔 / 拼音混输）是 `Engine::wubi_style` 的运行期状态，不在本枚举里
    /// （同 `Shuangpin` 与布局的切法：枚举管匹配语义，风格只是装配轴）。
    /// 候选装配（含混输合并与特殊习惯）见 `engine_wubi.rs`。
    Wubi,
}

impl Scheme {
    /// 方案**整数码**（对外协议）：0=全拼 1=拼音9键 2=拼音14键 3=笔画 4=双拼 5=五笔。
    ///
    /// 这是 JNI `scheme()`/`setScheme()` 与 C ABI `opi_scheme`/`opi_set_scheme` 的
    /// 线上口径，也是 fcitx5 配置枚举 `OpiPinyinMethod` 的取值来源（那边把枚举值
    /// 直接定义成这两个码）。**改这张表 = 改协议**：`api::scheme_from_int` 只是它的
    /// 转发（2026-10-14 前那张表在 opi-ffi 里，与这里的语义重复 —— 已收拢成一份），
    /// 两 ABI 的契约测试与四端注释都指着这里。
    pub fn code(self) -> i32 {
        match self {
            Scheme::Full => 0,
            Scheme::Nine => 1,
            Scheme::Fourteen => 2,
            Scheme::Stroke => 3,
            Scheme::Shuangpin => 4,
            Scheme::Wubi => 5,
        }
    }

    /// 整数码 → 方案。**越界 → None**（写侧忽略、读侧按 Full，与 `switch_mode`
    /// 的边界约定同款）。
    pub fn from_code(v: i32) -> Option<Scheme> {
        match v {
            0 => Some(Scheme::Full),
            1 => Some(Scheme::Nine),
            2 => Some(Scheme::Fourteen),
            3 => Some(Scheme::Stroke),
            4 => Some(Scheme::Shuangpin),
            5 => Some(Scheme::Wubi),
            _ => None,
        }
    }

    /// 配置面的**名字**（设置窗/ini 用；桌面两轨的「拼音方式」键存的就是它）。
    /// 与 Android 输入模式面板的中文标签刻意不同：那边是**键盘**的名字（拼音26键），
    /// 这里是**方案**的名字（全拼）—— 同一件事在两个语境下的自然叫法，别hard拉平。
    pub fn name(self) -> &'static str {
        match self {
            Scheme::Full => "全拼",
            Scheme::Nine => "拼音9键",
            Scheme::Fourteen => "拼音14键",
            Scheme::Stroke => "笔画",
            Scheme::Shuangpin => "双拼",
            Scheme::Wubi => "五笔",
        }
    }

    /// 名字 → 方案（**认不得 → None**；容错口径与 `fuzzy::mask_from_names` 一致：
    /// 调用方按「认不得就不动引擎」处理，别静默跳回默认）。
    pub fn from_name(name: &str) -> Option<Scheme> {
        match name.trim() {
            "全拼" => Some(Scheme::Full),
            "拼音9键" => Some(Scheme::Nine),
            "拼音14键" => Some(Scheme::Fourteen),
            "笔画" => Some(Scheme::Stroke),
            "双拼" => Some(Scheme::Shuangpin),
            "五笔" => Some(Scheme::Wubi),
            _ => None,
        }
    }

    /// 本方案下这个字符是不是**输入键**；是 → 返回该键覆盖的字母类。
    ///
    /// `Full`/`Stroke`/`Wubi` 恒 `None`：Full 走既有字母路径；Stroke 的五个键与
    /// 五笔的 26 个字母键都是普通字母，也走既有路径（换词典不换映射）。宿主画键盘时
    /// 用同一张表（9 键九个键、14 键十四个键），**别在宿主另抄一份**。
    pub fn key_class(self, ch: char) -> Option<&'static [char]> {
        match self {
            // 双拼的键也是普通字母（映射在引擎的解码层，不在键类层）。
            // 五笔同（映射就是字母本身，候选装配在 engine_wubi.rs）。
            Scheme::Full | Scheme::Stroke | Scheme::Shuangpin | Scheme::Wubi => None,
            Scheme::Nine => {
                // 2026-10-26：弹层选定的具体小写字母 = 单字母类（选 a 只匹配 a）。
                #[rustfmt::skip]
                static EXACT: [char; 26] = ['a','b','c','d','e','f','g','h','i','j','k','l','m','n','o','p','q','r','s','t','u','v','w','x','y','z'];
                match ch {
                    '2' => Some(&['a', 'b', 'c']),
                    '3' => Some(&['d', 'e', 'f']),
                    '4' => Some(&['g', 'h', 'i']),
                    '5' => Some(&['j', 'k', 'l']),
                    '6' => Some(&['m', 'n', 'o']),
                    '7' => Some(&['p', 'q', 'r', 's']),
                    '8' => Some(&['t', 'u', 'v']),
                    '9' => Some(&['w', 'x', 'y', 'z']),
                    'a'..='z' => Some(std::slice::from_ref(&EXACT[(ch as u8 - b'a') as usize])),
                    _ => None,
                }
            }
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

    /// 这个按键上**最前的字母**（= 键面第一字母：9 键 `'4'` → `'g'`；14 键首字母 == 键自身）。
    ///
    /// 唯一用途：宽松兜底档把读不出来的按键当「音节起头」时用它（用户裁决 2026-10-15
    /// 「没匹配的用按钮上最前的开始匹配字、词」；落点在
    /// [`crate::candidates_scheme::rank_and_pick_scheme`] 的尾档）。取不到类（`'0'`/`'1'`）
    /// ⇒ `None`，调用方据此**整档不启用**（与 [`Self::matches`]「取不到类即不匹配」同姿态）。
    pub fn key_leading_letter(self, ch: char) -> Option<char> {
        self.key_class(ch).and_then(|c| c.first()).copied()
    }

    /// 前缀语义匹配：`pinyin` 的前 `input.len()` 个字母逐个落在 `input` 按键的类里。
    ///
    /// `Full`/`Stroke`/`Wubi` 退化成正前缀（`starts_with`）—— 生产路径不会用它们走
    /// 这里（candidates 只在 Nine/Fourteen 下调本函数；五笔与笔画走各自词典的前缀
    /// 查询，见 `engine_wubi.rs`），但保持语义自洽，单测也拿它当基线。输入为空 →
    /// false（空缓冲没有候选，与各层同一条约定）。
    pub fn matches(self, input: &str, pinyin: &str) -> bool {
        if input.is_empty() {
            return false;
        }
        match self {
            Scheme::Full | Scheme::Stroke | Scheme::Wubi => pinyin.starts_with(input),
            // 双拼的键序不是拼音前缀（要先解码，见 Engine::candidates 的 Shuangpin 支）：
            // 本函数对它恒 false —— 真走到这里说明有人拿双拼键直查词库，是缺陷不是语义。
            Scheme::Shuangpin => false,
            Scheme::Nine | Scheme::Fourteen => {
                // 分词符（`'`，主流输入法「分词键」发它）：词库键**不含**分隔符（格式约定），
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

    /// 带**分词符**的匹配（2026-10-09，主流输入法「分词键」）：`input` 按 `'` 分段，
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

/// **9/14 键的数字空间回退**（2026-10-10 用户裁决「回退扩到 9 键」）：整串按键无命中时，
/// 丢掉尾部凑不满音节的键，返回**第一个字（最长单音节见证者）的键前缀**字节长度 ——
/// 候选层拿它重扫该前缀的整档（`candidates::rank_and_pick_scheme`），收尾层拿它定位
/// 留在缓冲里的尾部键（`Engine::commit_buffer_after_select`）。
///
/// 用户视角：9 键打长句「打超到下一音节的首键」（`64426`→`644269`）正是撞无候选的
/// 现场 —— 回退后出**第一个字** `64`（ni）的整档，选中只消耗第一个字的键、尾部键
/// 留着接着编（2026-10-18 用户裁决「先匹配前面的第一个字」）。
/// 语义与 26 键的分词回退（[`crate::candidates::segment_fallback`]，那边有逐条判据）
/// 对齐；判定只有这一处实现，两个调用点结论必同（纯函数、不存状态，与 26 键同一条
/// 纪律：结论由缓冲自证，不依赖「先取候选、再选中」的次序）。
///
/// 判据（全成立才回退）：
/// 1. 只对 9/14 键问 —— Full/Stroke 走自己的分词回退，别叠两套；
/// 2. 不含分词符 —— `'` 是用户**显式**的音节边界主张，`matches_segmented` 的严格语义
///    不受回退影响（`644'26` 断在 nih 后照旧谁都不匹配）；
/// 3. **整串（过 `pin` 滤后）一个精确命中都没有** —— 有精确命中 = 更具体的档，
///    不回退；**前缀扩展不算**（`scheme_find` 是前缀匹配，谓词里显式要求
///    `py.len() == input.len()` —— 2026-10-18 报障的另一半）；
/// 4. 存在**见证者**：词库键 `pinyin` 比整串**短**、音节对齐（[`boundary_aligned`]，
///    与 9 键读数链同一个 `pinyin::segment` 分段源）、且其键像恰是输入的类匹配前缀。
///    取**最长的单音节见证者**（第一个字的读音是一个完整拼音音节），返回其字节长度
///    `k` —— 「前缀重扫必命中」由见证者**自证**（它就是那次重扫里的一条），不需要
///    额外查询。为什么是「最长的单音节」而不是最短：9 键里一个键可能读成单字母
///    音节（`6` → `o`），也可能与下一键组成更明确的音节（`64` → `ni`）；取最长
///    单音节才符合用户打 `64426` 想要「你」而不是「哦」。多音节词（`nihao`）被
///    `is_syllable` 排除，逐字确认仍从第一个字开始。
///
/// 边界（与 26 键对齐）：单键输入没有更短的前缀 ⇒ 天然维持死（`5` 这类没有见证者）；
/// 单音节整串（如恰好一个音节的键）本来就命中、走不到这里；垃圾串（无见证者）同样维持
/// 死。⚠️ 尾部本身**可以**凑满音节（完整的音节键也会被丢）——判定是「丢到第一个音节
/// 查得到的前缀为止」，与 26 键「先匹配前面的第一个字」逐字同形，不是「尾部必须残缺」。
pub(crate) fn scheme_fallback_len<D: Dictionary + ?Sized>(
    dict: &D,
    input: &str,
    scheme: Scheme,
    pin: Option<&str>,
) -> Option<usize> {
    if input.is_empty()
        || input.contains('\'')
        || !matches!(scheme, Scheme::Nine | Scheme::Fourteen)
    {
        return None;
    }
    // 判据 3：整串（过 pin 滤后）没有**精确**命中 —— 区间枚举走存在性（**不许被
    // top-N 截断**，故用 `scheme_find` 而不是 `scheme_entries`）。`scheme_find` 的
    // 区间含比 input 更长的键（前缀扩展），必须按键长过滤，否则 `64426` 会被
    // `nihao...` 一类的长词冒充「整串命中」，逐字确认永远进不来。
    if dict
        .scheme_find(input, scheme, pin, &mut |py, _| {
            py.len() == input.len() && boundary_aligned(py)
        })
        .is_some()
    {
        return None;
    }
    // 判据 4：见证者 —— 取**最长的单音节**见证者（第一个字的读音是一个完整拼音音节；
    // 2026-10-18 用户裁决「先匹配前面的第一个字」）。从长到短试：多音节词被
    // `is_syllable` 排除，`644269` 不会一次吃掉 `nihao` 两字；单键的 `o` 会被
    // 更明确的 `ni`（`64`）压过。`scheme_find` 的区间含比 head 更长的键，故谓词里
    // 显式要求键长 == l。
    (1..input.len()).rev().find(|&l| {
        dict.scheme_find(&input[..l], scheme, pin, &mut |py, _| {
            py.len() == l && boundary_aligned(py) && crate::pinyin::is_syllable(py)
        })
        .is_some()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 整数码是**对外协议**（JNI/C ABI 两张整数表）：整表逐值钉，改一个数先在这里红。
    #[test]
    fn code_table_is_the_protocol() {
        let table: [(Scheme, i32); 6] = [
            (Scheme::Full, 0),
            (Scheme::Nine, 1),
            (Scheme::Fourteen, 2),
            (Scheme::Stroke, 3),
            (Scheme::Shuangpin, 4),
            (Scheme::Wubi, 5),
        ];
        for (s, c) in table {
            assert_eq!(s.code(), c, "{s:?} 的码");
            assert_eq!(Scheme::from_code(c), Some(s), "码 {c} 该读回 {s:?}");
        }
        // 越界 → None（写侧忽略、读侧按 Full —— 由 FFI 层落成各自哨兵）。
        assert_eq!(Scheme::from_code(6), None);
        assert_eq!(Scheme::from_code(-1), None);
    }

    /// 名字表（配置面）：往返 + 认不得 → None（**不是**回默认）。
    #[test]
    fn name_table_round_trips() {
        for s in [
            Scheme::Full,
            Scheme::Nine,
            Scheme::Fourteen,
            Scheme::Stroke,
            Scheme::Shuangpin,
            Scheme::Wubi,
        ] {
            assert_eq!(Scheme::from_name(s.name()), Some(s), "{s:?} 的名字往返");
        }
        assert_eq!(
            Scheme::from_name(" 双拼 "),
            Some(Scheme::Shuangpin),
            "两侧空白容错"
        );
        assert_eq!(
            Scheme::from_name("全拼含简拼"),
            None,
            "认不得 ⇒ None（简拼是另一个开关）"
        );
        assert_eq!(Scheme::from_name(""), None);
    }

    #[test]
    fn nine_key_class_table() {
        assert_eq!(Scheme::Nine.key_class('2'), Some(&['a', 'b', 'c'][..]));
        assert_eq!(Scheme::Nine.key_class('6'), Some(&['m', 'n', 'o'][..]));
        assert_eq!(Scheme::Nine.key_class('7'), Some(&['p', 'q', 'r', 's'][..]));
        // 0/1 不是输入键；字母是「单字母类」（2026-10-26 弹层精确选择）
        assert_eq!(Scheme::Nine.key_class('0'), None);
        assert_eq!(Scheme::Nine.key_class('1'), None);
        assert_eq!(Scheme::Nine.key_class('a'), Some(&['a'][..]));
        assert_eq!(Scheme::Nine.key_class('z'), Some(&['z'][..]));
        assert_eq!(Scheme::Nine.key_leading_letter('a'), Some('a'));
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
    fn key_leading_letter_is_the_first_of_the_class() {
        // 宽松兜底档（用户裁决 2026-10-15）只认这一个来源 —— 表动的字母就是它出的字母。
        assert_eq!(Scheme::Nine.key_leading_letter('2'), Some('a'));
        assert_eq!(Scheme::Nine.key_leading_letter('4'), Some('g'));
        assert_eq!(Scheme::Nine.key_leading_letter('7'), Some('p'));
        assert_eq!(Scheme::Nine.key_leading_letter('9'), Some('w'));
        assert_eq!(Scheme::Nine.key_leading_letter('0'), None);
        assert_eq!(Scheme::Nine.key_leading_letter('1'), None);
        // 2026-10-26：具体小写字母也有单字母类，首字母就是它自己。
        assert_eq!(Scheme::Nine.key_leading_letter('a'), Some('a')); // was None before
        // 14 键：键类首字母 == 键自身（退化成字面前缀，故 14 键本版不启用本档）
        assert_eq!(Scheme::Fourteen.key_leading_letter('b'), Some('b'));
        assert_eq!(Scheme::Fourteen.key_leading_letter('q'), Some('q'));
        // 非键方案恒 None
        assert_eq!(Scheme::Full.key_leading_letter('a'), None);
        assert_eq!(Scheme::Wubi.key_leading_letter('a'), None);
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
        // 2026-10-26：弹层选定具体小写字母 = 单字母类，不再按整格 T9 类匹配。
        assert!(Scheme::Nine.matches("a", "ai"));
        assert!(Scheme::Nine.matches("c", "ci"));
        assert!(!Scheme::Nine.matches("a", "bi"), "选 a 不能匹配 b 开头的词");
        assert!(!Scheme::Nine.matches("a", "ci"), "选 a 不能匹配 c 开头的词");
        assert!(Scheme::Nine.matches("ab", "abao"));
        assert!(!Scheme::Nine.matches("ab", "acao"));
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

    /// 五笔与 Full/Stroke 同为**纯前缀**（键是普通字母，无按键类展开、无解码）。
    /// 这条是协议级判据：改 semantics 先在这里红（生产候选走词典前缀查询，
    /// 与 matches 同判据，见 engine_wubi.rs）。
    #[test]
    fn wubi_matches_like_plain_prefix() {
        assert!(Scheme::Wubi.matches("wq", "wqvb"));
        assert!(!Scheme::Wubi.matches("wqvb", "wq"));
        assert!(!Scheme::Wubi.matches("", "wqvb"));
        assert_eq!(Scheme::Wubi.key_class('a'), None, "键是普通字母");
        assert_eq!(Scheme::Wubi.code(), 5);
        assert_eq!(Scheme::from_name("五笔"), Some(Scheme::Wubi));
    }
}
