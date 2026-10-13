// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 模糊拼音（**可开关**，四端共享）：核心 8 组模糊音 → 单音节/整串变体。
//!
//! **2026-10-13 起是逐对多选**（用户裁决「模糊音可多选类似 z=zh」）：入口加
//! `set_fuzzy_pairs(FuzzyMask)`；`set_fuzzy(bool)` 保留为全开/全关的包装。
//!
//! **2026-10-08 起有开关**：唯一入口是 `Engine::set_fuzzy`，门控在 `candidates.rs` 的
//! `RankOptions`；本模块仍无状态。配置来源按端：fcitx5 = 原生 addon 配置（`opi_config.h`）、
//! Android = 设置页、TSF = settings.ini；C ABI / JNI 侧另有对应出口。
//! ⚠️ 订正：原文写「常开而非开关：fcitx5（`Configurable=False`）与 TSF（无配置面）两端没有
//! 配置面」—— 那条前提已被「属性设置」这一轮推翻（两端都有了配置面），故改成开关。
//! ⚠️ 两处接入点（整串 + 逐音节兜底）**必须同开同关**：只关整串，`zongguo` 仍会从逐音节
//! 拿到模糊结果（门禁：`tests/fuzzy_jianpin_switches.rs`）。
//!
//! **变体靠按位置锚定的替换生成，不动 `pinyin::SYLLABLES`**：那张表被 410 条计数断言、
//! 贪婪切分断言、以及全 410 音节全前缀的二分单调性穷举锁着，扩表会连带推翻它们。
//!
//! 两个接入点（都在 `candidates.rs`），**两处都承重、缺一不可**：
//! - **逐音节回退循环内**（`merged.is_empty()` 时）：每音节查自身 + 变体，各取 top 3。
//!   实测（新 luna，2026-09-27）：打 `zongguo` 时 `query_all("zongguo")` 是 **0 行** ⇒
//!   回退成立，8 槽里 6 条（国/國/过/总/總/宗）**只能**由逐音节子查询给出。
//!   其中**变体**那半段另受 `wide_hits` 拦截（见下），故其可见性已被两库的连写词组压低：
//!   trad 仍承重（`nihao` 的「裏/裡」只能来自 `ni→li`），新 luna 的 47 个常用词实测里
//!   它一条候选都没贡献（常用词多已直接有条目键）；但远未失效，64×64 音节对穷举里
//!   仍有 1285 个输入以它为唯一模糊来源。
//! - **整串变体**（`input_variants`）：单音节输入与**连写词组**走这条，带 `MAX_VARIANTS` 上限
//!   （多音节是笛卡尔积，且这里在**每次击键**上）。简体侧同样靠它出词 —— 新 luna 的简体
//!   词组键是连写的（`zhongguo` 81 行、`zongguo` 0 行），`zongguo` 的 8 槽里「中国」正是
//!   变体 `zongguo→zhongguo` 带来的；trad 同理（38 行）。
//!
//! ⚠️ 本模块曾写「luna 词组键空格分隔 ⇒ 连写变体在简体库上是死代码」——那是**旧 luna**
//! （繁体词组段、空格键）的事实，随 2026-09-27 换 `rime-pinyin-simp` 简体词组源**已失效**。
//! 别再据此认为 `input_variants` 可以删。
//!
//! ⚠️ 两处都必须让**变体命中一律不算精确匹配**（`exact = false`）：
//! `n↔l` / `f↔h` 是**保长**替换，`e.pinyin_len == input_len` 对它们成立 —— 若据此吃满
//! `exact_bonus`，模糊命中就与真精确匹配同档硬碰词频。真实数据（`data/raw/trad_hanzi.tsv`）
//! 下 `li` 的精确首位「裏 3998755570」会被模糊命中「你 ni 3999128899」顶掉，
//! `ranking_quality.rs` 的 NEAR（`li`→裡 前 3）只剩 0 余量。
//! **加成必须是 0**：任何候选 ≤ `max_freq` ≤ `boost/2` = `exact_bonus` 是「精确 ≥ 前缀
//! 扩展」的结构性保证，且模糊档因此按**真实词频**排（自洽）。
//! ⚠️ 曾在此写「半额（`boost/4`）会让 `ranking_invariants.rs` 的 proptest 随机红」——
//! 实测**不复现**：把两处改成半额后该 proptest 连跑 10 次 10/10 绿（文本重叠的模糊副本被
//! `merged.retain` 先去重、逐音节路径又没有精确候选可违）。**别把那条当理由引用**；
//! 若在 CI 见过红，成因在别处。

use crate::pinyin::{is_syllable, segment};

/// 模糊音对（**可扩展**：加组只改这张表，逻辑不动）：
/// 平翘舌 zh/ch/sh、前后鼻 an/en/in、以及 n↔l、f↔h —— 共 8 组。
pub const FUZZY_PAIRS: &[(&str, &str)] = &[
    ("zh", "z"),
    ("ch", "c"),
    ("sh", "s"),
    ("an", "ang"),
    ("en", "eng"),
    ("in", "ing"),
    ("n", "l"),
    ("f", "h"),
];

/// 模糊音**启用集**（位掩码；第 i 位 = [`FUZZY_PAIRS`] 第 i 组）。
///
/// 2026-10-13 用户裁决（搜狗式设置面）：「模糊音（可多选类似 z=zh）」—— 从单一布尔
/// 开关升级为逐对可选。位序 = `FUZZY_PAIRS` 的数组序（那张表的顺序本身被
/// `table_is_the_core_eight` 钉着）；这个位序**是对外契约**（C ABI / JNI 的
/// `setFuzzyPairs(mask)`），改动必须四端同改。
pub type FuzzyMask = u8;

/// 全部 8 组都开（= 本层缺省值 = 既有行为，打开时逐字节不变）。
pub const FUZZY_ALL: FuzzyMask = ((1u16 << FUZZY_PAIRS.len()) - 1) as FuzzyMask;

/// 全部组名的**规范串**（逗号分隔，位序即 `FUZZY_PAIRS` 序）：
/// `"zh/z,ch/c,sh/s,an/ang,en/eng,in/ing,n/l,f/h"`。
///
/// 这是四端设置面的**唯一名字源**：宿主拿它做多选列表（或核对自绘的列表），
/// 别在各端手抄一遍名表 —— 加组时手抄必漂（本仓「优先消灭失败类别」的用例）。
pub fn pair_names() -> String {
    FUZZY_PAIRS
        .iter()
        .map(|(a, b)| format!("{a}/{b}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// 组名串 → 掩码（[`pair_names`] 的正向解析）。**唯一的解析入口**，
/// 供各端配置面共用（fcitx5 的 `FuzzyPairs=`、TSF 的 settings.ini、Android/Kotlin
/// 的偏好恢复都走它，别在宿主再写第二个解析器）。
///
/// 容错口径：按逗号切、两侧空白容忍；**认不得的名字整条忽略**（不回退、不报错 ——
/// 配置里多写了个未来组的名字，旧版本不该因此把别的组也关掉）；同名重复无害。
/// 空串 → `0`（全关；「键缺失」的缺省是全开，那是**调用方**的口径，见各端读侧）。
pub fn mask_from_names(names: &str) -> FuzzyMask {
    let mut mask: FuzzyMask = 0;
    for raw in names.split(',') {
        let name = raw.trim();
        if name.is_empty() {
            continue;
        }
        if let Some(i) = FUZZY_PAIRS
            .iter()
            .position(|(a, b)| name == format!("{a}/{b}") || name == format!("{b}/{a}"))
        {
            mask |= 1 << i;
        }
    }
    mask
}

/// 整串变体上限。多音节是**笛卡尔积**（k 个音节 × 每音节变体），而每次变体查询是一次
/// 词库前缀查询 —— 这条路在**每次击键**上，必须封顶（实测数据见 tests/fuzzy_ranking.rs）。
pub const MAX_VARIANTS: usize = 16;

/// 单音节的全部模糊变体（**含自身、自身排第一**，去重、顺序确定）。
///
/// 只对合法音节生成，且结果**必须仍是合法音节**：`song` 套 `s→sh` 得 `shong`，
/// 那不是音节，拿去查词库只会白费一次查询。
///
/// 同一音节多对可命中时枚举**非空子集**：`nan` → `{lan（换声母）, nang（换韵母）,
/// lang（都换）}`。逐对单独替换会漏掉 `lang`，全串替换连 `lan` 都给不出
/// （`n→l` 命中串尾的 n 得 `lal`，非法被滤掉）。
pub fn variants(syllable: &str, enabled: FuzzyMask) -> Vec<String> {
    let mut out = vec![syllable.to_string()];
    if !is_syllable(syllable) {
        return out;
    }
    let n = FUZZY_PAIRS.len();
    for mask in 1u32..(1 << n) {
        // 含被禁组的子集整支跳过（`enabled == FUZZY_ALL` 时恒不触发 ⇒ 旧行为逐字节不变）。
        if mask & u32::from(!enabled) != 0 {
            continue;
        }
        let mut cur = syllable.to_string();
        let mut ok = true;
        for (i, (a, b)) in FUZZY_PAIRS.iter().enumerate() {
            if mask & (1 << i) == 0 {
                continue;
            }
            match apply_pair(&cur, a, b) {
                Some(next) => cur = next,
                // 子集的含义就是「这几组都生效」；有一组作用不上就不成立。
                None => {
                    ok = false;
                    break;
                }
            }
        }
        // 非空 + 合法音节，二者缺一不可：空串会让替身词库（对空串照答）凭空造候选。
        if ok && cur != syllable && !cur.is_empty() && is_syllable(&cur) && !out.contains(&cur) {
            out.push(cur);
        }
    }
    out
}

/// 把一对模糊音作用到音节上（不适用则 `None`）。**位置锚定**见 [`apply_pair`]。
fn apply_pair(s: &str, a: &str, b: &str) -> Option<String> {
    if is_initial(a, b) {
        switch_prefix(s, a, b).or_else(|| switch_prefix(s, b, a))
    } else {
        switch_suffix(s, a, b).or_else(|| switch_suffix(s, b, a))
    }
}

fn switch_prefix(s: &str, from: &str, to: &str) -> Option<String> {
    s.strip_prefix(from).map(|rest| format!("{to}{rest}"))
}

fn switch_suffix(s: &str, from: &str, to: &str) -> Option<String> {
    s.strip_suffix(from).map(|head| format!("{head}{to}"))
}

/// 不含元音的对是**声母**对（zh/z、ch/c、sh/s、f/h、n/l）→ 只作用于位置 0；
/// 含元音的是**韵母**对（an/ang、en/eng、in/ing）→ 只作用于串尾。
///
/// 这条划分是必须的，不是优化：全串替换会把 `zhong` 变成 `zholg`（n→l 命中串尾）、
/// 把 `nin` 变成 `lil`、把 `zhuang` 变成 `zhuangg`。这些串都不是音节、会被合法性
/// 过滤掉，于是 `nin → lin`、`nan → lan` 这些**真实存在的模糊**反而一条都给不出来。
///
/// 每对只走**一个**方向，且先试长的一方（`zh` 先于 `z`、`ang` 先于 `an`），
/// 否则 `zhong` 会因 `z→zh` 变成 `zhhong`。
fn is_initial(a: &str, b: &str) -> bool {
    !a.chars().any(is_vowel) && !b.chars().any(is_vowel)
}

fn is_vowel(c: char) -> bool {
    matches!(c, 'a' | 'e' | 'i' | 'o' | 'u' | 'v')
}

/// 整串模糊变体（**不含自身**）：按音节替换后拼接。
/// 取前 [`MAX_VARIANTS`] 个，顺序确定（最左音节变化最快 → 优先改动靠前的音节）。
pub fn input_variants(input: &str, enabled: FuzzyMask) -> Vec<String> {
    // `'` 是用户手写的硬分隔（pinyin.rs）：它表达「就按这个切」，不是模糊音。
    // 且 segment 会吃掉分隔符，拼回来的 "xian" 恰好绕过用户的意图 —— 直接不做变体。
    if input.contains('\'') {
        return Vec::new();
    }
    let per: Vec<Vec<String>> = segment(input).iter().map(|s| variants(s, enabled)).collect();
    if per.iter().all(|v| v.len() < 2) {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut idx = vec![0usize; per.len()];
    loop {
        let cand: String = per.iter().zip(&idx).map(|(v, i)| v[*i].as_str()).collect();
        if cand != input && !out.contains(&cand) {
            out.push(cand);
            if out.len() >= MAX_VARIANTS {
                return out;
            }
        }
        // 里程表进位（最左最快）；进位溢出即穷尽。
        let mut k = 0;
        loop {
            if k == per.len() {
                return out;
            }
            idx[k] += 1;
            if idx[k] < per[k].len() {
                break;
            }
            idx[k] = 0;
            k += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pinyin::SYLLABLES;

    /// 名字串是四端唯一名源：规范串 ↔ 掩码必须**往返一致**（加组时这排会跟着长）。
    #[test]
    fn pair_names_round_trip() {
        let names = pair_names();
        assert_eq!(
            names,
            "zh/z,ch/c,sh/s,an/ang,en/eng,in/ing,n/l,f/h",
            "规范串是对外契约（各端多选列表与解析都拿它）"
        );
        assert_eq!(mask_from_names(&names), FUZZY_ALL, "规范串解析回全开");
    }

    /// 解析容错口径：认不得的忽略、空白容忍、反写（l/n）也认、空串 = 全关。
    #[test]
    fn mask_from_names_tolerates_noise() {
        assert_eq!(mask_from_names(""), 0, "空串 = 全关（键缺失的缺省是全开，那是调用方口径）");
        assert_eq!(mask_from_names("n/l,xx/yy"), 1 << 6, "认不得的整条忽略");
        assert_eq!(mask_from_names("l/n"), 1 << 6, "反写也认（两种写法同一组）");
        assert_eq!(
            mask_from_names("  zh/z , ch/c "),
            (1 << 0) | (1 << 1),
            "两侧空白容忍"
        );
        assert_eq!(mask_from_names("n/l,n/l"), 1 << 6, "重复无害");
    }

    #[test]
    fn table_is_the_core_eight() {
        assert_eq!(
            FUZZY_PAIRS,
            &[
                ("zh", "z"),
                ("ch", "c"),
                ("sh", "s"),
                ("an", "ang"),
                ("en", "eng"),
                ("in", "ing"),
                ("n", "l"),
                ("f", "h"),
            ]
        );
    }

    /// 每组**两个方向**都要生效：打 `zong` 出「中」与打 `zhong` 出「总」都是模糊音的用途。
    #[test]
    fn each_pair_applies_both_ways() {
        let cases = [
            ("zhong", "zong"),
            ("zong", "zhong"),
            ("chi", "ci"),
            ("ci", "chi"),
            ("shi", "si"),
            ("si", "shi"),
            ("an", "ang"),
            ("ang", "an"),
            ("shen", "sheng"),
            ("sheng", "shen"),
            ("xin", "xing"),
            ("xing", "xin"),
            ("ni", "li"),
            ("li", "ni"),
            ("fa", "ha"),
            ("ha", "fa"),
        ];
        for (from, to) in cases {
            let got = variants(from, FUZZY_ALL);
            assert!(
                got.contains(&from.to_string()),
                "{from} 必须含自身：{got:?}"
            );
            assert!(
                got.contains(&to.to_string()),
                "{from} 的变体里缺 {to}：{got:?}"
            );
        }
    }

    /// 声母对**只作用于位置 0**。全串替换会把 `zhong` 变成 `zholg`（n→l 命中串尾的 n）、
    /// 把 `nin` 变成 `lil`（l→n 命中第 3 个字母）—— 两者都不是音节，会被合法性过滤掉，
    /// 于是 `nin → lin` 这条**真实存在的模糊**反而给不出来。
    #[test]
    fn initial_pairs_only_touch_position_zero() {
        assert!(variants("zhong", FUZZY_ALL).contains(&"zong".to_string()));
        assert!(
            !variants("zhong", FUZZY_ALL).contains(&"zholg".to_string()),
            "声母对碰到了串尾：{:?}",
            variants("zhong", FUZZY_ALL)
        );
        for (syl, want) in [
            ("nin", "lin"),
            ("nan", "lan"),
            ("neng", "leng"),
            ("nv", "lv"),
        ] {
            assert!(
                variants(syl, FUZZY_ALL).contains(&want.to_string()),
                "{syl} 的变体里缺 {want}（声母 n↔l 只该动位置 0）：{:?}",
                variants(syl, FUZZY_ALL)
            );
        }
    }

    /// 韵母对**只作用于串尾**。全串替换会把 `zhuang` 变成 `zhuangg`。
    #[test]
    fn final_pairs_only_touch_the_tail() {
        assert!(variants("zhuang", FUZZY_ALL).contains(&"zhuan".to_string()));
        assert!(
            !variants("zhuang", FUZZY_ALL).contains(&"zhuangg".to_string()),
            "韵母对碰到了非串尾：{:?}",
            variants("zhuang", FUZZY_ALL)
        );
        assert!(variants("zhuan", FUZZY_ALL).contains(&"zhuang".to_string()));
        assert!(variants("shen", FUZZY_ALL).contains(&"sheng".to_string()));
    }

    /// 同一音节多对可命中时枚举**非空子集**：`nan` 既要 `lan`（换声母）、`nang`（换韵母），
    /// 也要 `lang`（两个都换）。逐个替换会漏掉最后那个。
    #[test]
    fn pairs_combine_as_subsets() {
        let got: Vec<String> = {
            let mut v = variants("nan", FUZZY_ALL);
            v.sort();
            v
        };
        assert_eq!(
            got,
            vec!["lan", "lang", "nan", "nang"],
            "nan 的变体应是 {{lan, lang, nan, nang}}"
        );
        let mut fang = variants("fang", FUZZY_ALL);
        fang.sort();
        assert_eq!(fang, vec!["fan", "fang", "han", "hang"], "fang 的变体");
    }

    /// 变体必须是**合法音节**（表里查不到的一律不生成）。
    ///
    /// 下表是评审跑出来的**真实非法组合**（410 音节逐对穷举的产物），当回归用例钉死：
    /// 只要变体生成放宽了过滤，这些串就会漏进词库查询。
    #[test]
    fn illegal_products_are_never_generated() {
        let forbidden: &[(&str, &str)] = &[
            // s→sh：唯独 -ong 非法
            ("song", "shong"),
            // f↔h 最脏
            ("fo", "ho"),
            ("huo", "fuo"),
            ("hai", "fai"),
            ("hao", "fao"),
            ("he", "fe"),
            ("hong", "fong"),
            ("hua", "fua"),
            ("huai", "fuai"),
            ("huan", "fuan"),
            ("huang", "fuang"),
            ("hui", "fui"),
            ("hun", "fun"),
            // zh/ch/sh → z/c/s + ua/uai/uang
            ("zhua", "zua"),
            ("zhuan", "zuang"),
            ("chuai", "cuai"),
            ("shuang", "suang"),
            // n↔l 边界：len / nia / no 都不是音节（`lia` 的反向 `nia` 给不出，
            // 表里只有 `lia`/`niao`）
            ("nen", "len"),
            ("leng", "len"),
            ("lo", "no"),
            ("lia", "nia"),
            // -ian → -iang 全灭
            ("bian", "biang"),
            ("dian", "diang"),
            ("tian", "tiang"),
            ("xuan", "xuang"),
        ];
        for (syl, bad) in forbidden {
            let got = variants(syl, FUZZY_ALL);
            assert!(
                !got.contains(&bad.to_string()),
                "{syl} 生成了非法变体 {bad}：{got:?}"
            );
        }
        // 反向：合法的必须给得出。`hou → fou` 是位置 0 的 h→f（合法）；
        // 而 `huo` 的 h→f 得 `fuo`（不是音节），故 `huo` 在 f↔h 上什么都给不出 ——
        // `fo` 本身也只能由 `hou` 这类音节来，不能由 `huo` 缩掉一个字母造出来。
        assert!(variants("hou", FUZZY_ALL).contains(&"fou".to_string()));
        assert_eq!(variants("huo", FUZZY_ALL), vec!["huo"], "fuo 不是音节，不得产生");
        assert!(variants("su", FUZZY_ALL).contains(&"shu".to_string()));
        assert!(variants("suan", FUZZY_ALL).contains(&"shuan".to_string()));
        // 全部产物一律合法
        for syl in SYLLABLES {
            for v in variants(syl, FUZZY_ALL) {
                assert!(is_syllable(&v), "{syl} 产生了非法音节变体 {v}");
            }
        }
    }

    /// 表本身无重复、自身恰好一次；非音节输入不生成变体（`segment` 兜底出的单字母）。
    #[test]
    fn variants_are_unique_and_identity_preserving() {
        for syl in ["zhong", "zong", "ni", "li", "zhang", "xin", "shen"] {
            let got = variants(syl, FUZZY_ALL);
            let mut uniq = got.clone();
            uniq.sort();
            uniq.dedup();
            assert_eq!(uniq.len(), got.len(), "{syl} 变体重复：{got:?}");
            assert_eq!(got.iter().filter(|v| *v == syl).count(), 1, "{syl}");
            assert_eq!(got[0], syl, "自身必须排第一（顺序确定）：{got:?}");
        }
        assert_eq!(variants("n", FUZZY_ALL), vec!["n"], "「n」不是音节，不生成变体");
        assert_eq!(
            variants("zh", FUZZY_ALL),
            vec!["zh"],
            "「zh」是前缀不是音节，不生成变体"
        );
        // 变体绝不能是空串（NaiveDict 那类替身对空串照答，会凭空造候选）
        for syl in SYLLABLES {
            for v in variants(syl, FUZZY_ALL) {
                assert!(!v.is_empty());
            }
        }
    }

    /// 整串变体按**音节**替换后拼接：`zongguo` → `zhongguo` 才能命中「中国」。
    /// 无变体可用的输入返回空（不是「返回自身」）。
    #[test]
    fn input_variants_cross_syllables() {
        assert_eq!(input_variants("zongguo", FUZZY_ALL), vec!["zhongguo"]);
        assert_eq!(input_variants("guoguo", FUZZY_ALL), Vec::<String>::new());
        assert_eq!(input_variants("", FUZZY_ALL), Vec::<String>::new());
    }

    /// `'` 是用户手写的切分意图，不做变体。
    #[test]
    fn apostrophe_opts_out() {
        assert_eq!(input_variants("xi'an", FUZZY_ALL), Vec::<String>::new());
    }

    /// 上限：`zhang×5` 的笛卡尔积是 3^5=243，必须封顶且不含自身。
    #[test]
    fn variant_count_is_capped() {
        let long = "zhang".repeat(5);
        let got = input_variants(&long, FUZZY_ALL);
        assert_eq!(got.len(), MAX_VARIANTS, "{got:?}");
        assert!(got.iter().all(|v| v != &long), "变体不得含自身");
    }
}
