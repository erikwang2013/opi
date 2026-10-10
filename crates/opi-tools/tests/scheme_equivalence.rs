// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 按键类匹配的两条实现在新库上的**等价门禁**（2026-10-11 加）。
//!
//! 来由：移动端把 `rank_and_pick_scheme` 的全表逐条 `matches` 换成
//! `Dictionary::scheme_entries` 的**区间枚举**（loader 覆写；实测单键 268ms → 16ms）。
//! 这条门禁钉「换掉的是算法、不是语义」：
//! ① 快路径返回的每一条 (词, 拼音) 都在全表参考实现的结果里；
//! ② 参考实现的**精确条目**（键长 == 输入键数）一条不少；
//! ③ 参考结果不超过上限时，两集合**逐条相等**（夹具小输入必须全等）。
//! pin 过滤走同一对实现比一遍（9 键左列读数钉选的路径）。
//!
//! 参考实现 = `engine_core::dictionary::scheme_entries_scan`（全表 + `matches`），
//! 快实现 = `MmapDictionary` 的 trait 覆写（`load_mmap` 拿到的就是它）。

use engine_core::dictionary::{Dictionary, scheme_entries_scan};
use engine_core::scheme::Scheme;
use engine_data::load_mmap;
use std::collections::HashSet;
use std::path::Path;

const LUNA_OPID: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/assets/luna.opid"
);
/// 与 `dictionary::SCHEME_MATCH_CAP` 同步的私有拷贝 —— 门禁要断言「上限之外一律
/// 不在快集里」，抄一份常量比引内部的旋钮更稳（旋钮改了这里会红，提醒复核本门禁）。
const CAP: usize = 2048;

type Row = (String, String); // (word, pinyin)

fn set_of(rows: Vec<(engine_core::Entry, String)>) -> HashSet<Row> {
    rows.into_iter().map(|(e, py)| (e.word, py)).collect()
}

#[test]
fn scheme_fast_path_matches_full_scan() {
    let dict = load_mmap(Path::new(LUNA_OPID)).expect("加载词库");
    let cases: &[(Scheme, &[&str], Option<&str>)] = &[
        (Scheme::Nine, &["6", "64", "644", "64426", "94", "96", "9"], None),
        (Scheme::Fourteen, &["b", "bu", "bug", "bugao", "bugaoml"], None),
        (Scheme::Nine, &["64", "644"], Some("ni")), // pin 过滤腿
    ];
    let mut checked = 0usize;
    for (scheme, inputs, pin) in cases {
        for input in *inputs {
            let reference = scheme_entries_scan(&dict, input, *scheme, *pin, usize::MAX);
            let reference_set = set_of(reference.clone());
            let fast = dict.scheme_entries(input, *scheme, *pin, CAP);
            let fast_set = set_of(fast.clone());
            assert!(
                fast_set.is_subset(&reference_set),
                "{scheme:?} {input:?} pin={pin:?}: 快路径返回了参考实现没有的条目：{:?}",
                fast_set.difference(&reference_set).take(5).collect::<Vec<_>>()
            );
            let exact: Vec<_> = reference_set
                .iter()
                .filter(|(_w, py)| py.len() == input.len())
                .cloned()
                .collect();
            for e in &exact {
                assert!(
                    fast_set.contains(e),
                    "{scheme:?} {input:?} pin={pin:?}: 精确条目漏了 {e:?}"
                );
            }
            if reference_set.len() <= CAP + exact.len() {
                assert_eq!(
                    fast_set, reference_set,
                    "{scheme:?} {input:?} pin={pin:?}: 未触发上限，集合应逐条相等"
                );
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 14, "用例数变了吗？");
}
