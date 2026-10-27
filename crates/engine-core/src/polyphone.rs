// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **多音字补读音**纠偏层（2026-10-15 用户报障「为何单独打『么』候选词没有？」）。
//!
//! ## 是什么、为什么
//!
//! `么` 在 luna 词库里只挂了 `ma`/`me`/`yao` 三个键 —— 打 `mo`（`幺麽` 的 mó）一个字
//! 不出；同类还有 `绿` 缺 `lu`（绿林）、`咯` 缺 `ka`（咯血）。**这是词库缺读音，
//! 不是排序问题**：引擎侧实测 26 键 / 9 键、新旧两版引擎，`me` 与 `63` 的 `么` 都在第 0 位。
//!
//! 上游 `luna_pinyin.dict.yaml` 的读音列表本身就缺这些，改数据要动生成链与上游 pin；
//! 而这类缺口**有限、可枚举、能逐行被 Unihan 字段原文复核** —— 故做成一张小表
//! （`data/raw/polyphone.tsv`，生成器 `scripts/gen_polyphone.py`），
//! include_str! 进二进制。先例：`trad_chars.rs` 的繁体字表同一条路。
//!
//! ## 语义边界（改之前先读）
//!
//! · **补的条目按「精确匹配」算**（它确实是那个读音）：吃 `exact_bonus`，参与候选三档的
//!   常用单字档 —— 打 `mo` 时 `么` 与 `模 摸 默 …` 同档按词频排；
//! · **词频是词库量纲**：生成器取「该字在词库里的最大词频」（`opi-tools dump-keys` 的
//!   第三列）—— 词库词频是位置编码（十亿量级），Unihan 词次（几千）直接拿来会沉底。
//!   词库不认识那个字（罕见）时才退 Unihan 词次。
//!   ⚠️ **用之前要按活跃词库缩放**（[`scaled_freq`]）：表里的量纲是 luna 的，
//!   而引擎可以装别的库（夹具、繁体库、导入层）—— 不缩放的话，夹具词库（量纲几千）
//!   会被补进来的真库量级词频霸榜（2026-10-15 实测：`xian` 的首位被补进来的 `锬` 抢走，
//!   两条既有夹具门禁当场红）。
//! · **表里只有「词库真缺」的条目**（生成器拿取键转储做了差集）—— 本层不做二次判重，
//!   去重交给候选层既有的 `seen` 集合；
//! · **只在缓冲恰好等于表中的某个音节时命中**（表按音节建；前缀/模糊/简拼那些档不受影响）。
//!   表列的是**合法音节**（生成器按 `pinyin.rs::SYLLABLES` 过滤），不会引入打不出的音。

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::scheme::Scheme;

/// `字\t读音\t词频`（生成器写的，**勿手改**；含 Unihan pin 校验 + 词库取键差集）。
const TABLE: &str = include_str!("../../../data/raw/polyphone.tsv");

/// 表头里写的参考量纲（`# dict_max <N>`）—— 表内词频相对它表达。
fn reference_max() -> u64 {
    static M: OnceLock<u64> = OnceLock::new();
    *M.get_or_init(|| {
        TABLE
            .lines()
            .find_map(|l| l.strip_prefix("# dict_max "))
            // 只取**前导数字**：表头后面可能跟说明文字（写成 `4000000000（= …）` 时
            // `split_whitespace` 会把说明一起咬进来 ⇒ 解析失败 ⇒ 静默退化成「不适用」。
            // 2026-10-15 实测踩过这个坑（补读音整层不生效），两头都收紧。
            .map(|v| {
                v.chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
            })
            .and_then(|digits| digits.parse().ok())
            .unwrap_or(0)
    })
}

/// 这层**适不适用于**该词库（`dict_max` = `Dictionary::max_freq()`）。
///
/// 判据：量纲与参考词库**同档**（相差不到一个数量级）才适用。
/// 由来（2026-10-15 实测）：表是按 luna 建的 —— 「这个词库缺不缺这个读音」这条前提
/// 只对同量纲的库成立。夹具/异量纲库下那三个字在不在库里无从判断，补进去只是噪音，
/// 还会打翻既有的排序门禁（实测：`xian` 被补出 `锬`、`seng` 一族被补出 `锶`）。
///
/// ⚠️ 阈值（10×）是**启发式**：本仓的 luna 与 trad 词库都出自同一套生成器、量纲同档
/// （实测比值 ≈ 1.0000），所以这道门不会误伤生产路径；夹具词库（量纲几千）差六个
/// 数量级，稳定落在门外。若将来引入量纲完全不同的正规词库，改这里而不是改调用方。
pub(crate) fn applies(dict_max: u64) -> bool {
    let r = reference_max();
    r > 0 && dict_max.saturating_mul(10) >= r
}

/// 把表里的词频缩放到**活跃词库**的量纲（`dict_max` = `Dictionary::max_freq()`）。
///
/// 语料：表里词频是「该字在参考词库里的词频」；活跃词库与参考词库量纲相同（都是本仓
/// 生成器产出的 luna）时缩放比 = 1，一字不差。夹具/异量纲库下按比例缩小 —— 补的条目
/// 因此落在**该库自己的**分数区间里，不会凭参考库的量级霸榜。
/// 参考量纲未知（0，老表或坏表头）⇒ 返回 0：补的条目仍会出现，但排在同档末尾。
pub(crate) fn scaled_freq(freq: u32, dict_max: u64) -> u32 {
    let r = reference_max();
    if r == 0 {
        return 0;
    }
    let scaled = (freq as u128) * (dict_max as u128) / (r as u128);
    scaled.min(u32::MAX as u128) as u32
}

/// 读音 → [(字, 词频)]。首次查询时解析一次（~2k 行，微秒级），此后只读。
fn map() -> &'static HashMap<&'static str, Vec<(&'static str, u32)>> {
    static M: OnceLock<HashMap<&'static str, Vec<(&'static str, u32)>>> = OnceLock::new();
    M.get_or_init(|| {
        let mut m: HashMap<&'static str, Vec<(&'static str, u32)>> = HashMap::new();
        for line in TABLE.lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let mut it = line.split('\t');
            let (Some(ch), Some(reading), Some(freq)) = (it.next(), it.next(), it.next()) else {
                continue;
            };
            // 坏行跳过而不 panic：表是生成物，一条坏行不该让整个输入法起不来
            // （与 `trad_chars.rs` 的解析同一条姿态）。
            let Ok(freq) = freq.parse::<u32>() else {
                continue;
            };
            m.entry(reading).or_default().push((ch, freq));
        }
        m
    })
}

/// 9/14 键表的行：`(字, 词频, 拼音长度, 读音)`。**起别名是给 clippy 的 `type_complexity`
/// 让路**（发布前 `-D warnings` 实测：直写四元组会红）。
///
/// 读音是 2026-10-27 加的：此前这一列只有长度，候选层拿**输入数字串**冒充读数，
/// 9 键左列因此出现 `6'3` 这样的「拼音」（用户报障「打 ne 显示 63」）。
type DigitHit = (&'static str, u32, usize, &'static str);

/// 该读音在补读音表里的条目（**词库缺的才在表里** —— 生成器已做差集）。
/// 空切片 = 这个音没有缺口（绝大多数情况）。
pub(crate) fn hits(reading: &str) -> &'static [(&'static str, u32)] {
    map().get(reading).map(Vec::as_slice).unwrap_or(&[])
}

/// 9/14 键那一侧：按键串 → [(字, 词频, 拼音长度, 读音)]。
///
/// 为什么另开一张表：9/14 键的缓冲是**按键串**（9 键 `63`、14 键 `mo`），拿它去查
/// 按音节建的 `map()` 查不到。键序由 `Scheme::key_for_letter` 从**同一张类表**反查
/// （9 键 2=abc … 9=wxyz；14 键 `b`={b,n} …）—— 9 键原先只建了 T9 索引，14 键因此
/// 拿不到「么/mo」这类缺口补丁（2026-10-27 补）。拼音长度照样带出来：9/14 键那条路
/// 的「精确」判据是**键长相等**（见 `candidates_scheme.rs` 头注），编码后长度 1:1。
pub(crate) fn hits_for(scheme: Scheme, keys: &str) -> &'static [DigitHit] {
    fn build(scheme: Scheme) -> HashMap<&'static str, Vec<DigitHit>> {
        let mut m: HashMap<&'static str, Vec<DigitHit>> = HashMap::new();
        for (reading, items) in map() {
            let Some(encoded) = reading
                .chars()
                .map(|c| scheme.key_for_letter(c))
                .collect::<Option<String>>()
            else {
                continue;
            };
            // 借用泄漏**每个读音一次**（最多 410 次，进程内只建一次表）——
            // 键要活成 'static，而它由运行期拼出来（与 `map()` 同一条「一次解析、
            // 终身只读」的账）。
            let key: &'static str = Box::leak(encoded.into_boxed_str());
            for (ch, freq) in items {
                m.entry(key).or_default().push((
                    ch,
                    *freq,
                    reading.chars().count(),
                    // `reading` 是 `&&'static str`（map() 的键），解一层就是表里的
                    // 静态切片 —— 不额外分配。
                    *reading,
                ));
            }
        }
        m
    }
    match scheme {
        Scheme::Nine => {
            static NINE: OnceLock<HashMap<&'static str, Vec<DigitHit>>> = OnceLock::new();
            NINE.get_or_init(|| build(Scheme::Nine))
                .get(keys)
                .map(Vec::as_slice)
                .unwrap_or(&[])
        }
        Scheme::Fourteen => {
            static FOURTEEN: OnceLock<HashMap<&'static str, Vec<DigitHit>>> = OnceLock::new();
            FOURTEEN
                .get_or_init(|| build(Scheme::Fourteen))
                .get(keys)
                .map(Vec::as_slice)
                .unwrap_or(&[])
        }
        _ => &[],
    }
}
