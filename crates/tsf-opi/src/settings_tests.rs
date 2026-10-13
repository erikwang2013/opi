// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `settings.rs` 的主机单测（Linux 可跑 —— 正是把它从 Windows 胶水里搬出来的理由）。
//!
//! 夹具分两半：**手写的 ini 文本**（容错口径逐条钉住）+ 真实文件系统（`path_in`/`load`
//! 的落点与「读不到 = 全默认」）。键名那半是**跨语言契约**（Kotlin 写侧），
//! 写死在这里 —— 那边改一个字母，这里是第一道会红的地方。

use super::*;

#[test]
fn defaults_are_all_on() {
    assert_eq!(Settings::default(), parse(""));
    assert_eq!(Settings::default(), parse("# 只有注释\n\n"));
}

#[test]
fn parses_every_key_false() {
    let s = parse("Learn=False\nChinesePunct=False\nFuzzy=False\nJianpin=False\n");
    assert_eq!(
        s,
        Settings {
            learn: false,
            chinese_punct: false,
            fuzzy: false,
            fuzzy_pairs: String::new(),
            jianpin: false,
            correction: true,
            shuangpin_layout: String::new(),
            pinyin_method: String::new(),
        }
    );
}

#[test]
fn only_exact_key_names_are_recognised() {
    // 键名**区分大小写**（跨语言契约：desktop 的 Settings.kt 写的就是
    // Learn/ChinesePunct/Fuzzy/Jianpin 这四个）。拼错/换大小写 = 该键**静默**不生效
    // ——不报错、只是没改到，所以这里钉住"小写不算"。
    assert!(
        parse("learn=False\n").learn,
        "小写 learn 不该生效（否则会静默串键）"
    );
    assert!(!parse("Fuzzy=false\n").fuzzy, "键名对、值大小写不敏感");
}

#[test]
fn tolerant_values_and_comments() {
    let s = parse(
        "Fuzzy = false   # 行内注释\n\
         Jianpin=OFF\n\
         Learn=1\n\
         ChinesePunct = yes\n",
    );
    assert!(!s.fuzzy);
    assert!(!s.jianpin);
    assert!(s.learn);
    assert!(s.chinese_punct);
}

#[test]
fn unknown_keys_and_bad_values_are_ignored() {
    let s = parse("未来才有的键=whatever\nFuzzy=maybe\nJianpin=\nNotAKey\n");
    assert_eq!(s, Settings::default());
}

#[test]
fn path_is_localappdata_opi_settings_ini() {
    // 与 desktop Settings.kt 的 Windows 分支逐字对应（写侧写这里、读侧读这里）。
    assert_eq!(
        path_in(Some(OsStr::new("C:\\Users\\u\\AppData\\Local"))),
        PathBuf::from("C:\\Users\\u\\AppData\\Local")
            .join("opi")
            .join("settings.ini")
    );
    // 缺/空环境变量：不 panic，退化成相对路径（读不到 → 全默认）。
    assert_eq!(path_in(None), PathBuf::from("opi/settings.ini"));
    assert_eq!(
        path_in(Some(OsStr::new(""))),
        PathBuf::from("opi/settings.ini")
    );
}

#[test]
fn load_reads_the_real_file_and_falls_back() {
    // 真文件系统：写一份到临时目录 → 读回来；目录不存在 ⇒ 全默认（不是错误）。
    let dir = std::env::temp_dir().join(format!("opi-settings-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let opi = dir.join("opi");
    std::fs::create_dir_all(&opi).unwrap();
    std::fs::write(
        opi.join(SETTINGS_FILE_NAME),
        "# 由 opi-candidates --settings 写入\nLearn=False\nFuzzy=False\n",
    )
    .unwrap();
    let s = load(Some(dir.as_os_str()));
    assert!(!s.learn);
    assert!(!s.fuzzy);
    assert!(s.chinese_punct, "缺键回默认");
    assert!(s.jianpin);

    // 指向一个不存在的目录：读不到 = 全默认。
    let missing = dir.join("nope");
    assert_eq!(load(Some(missing.as_os_str())), Settings::default());

    let _ = std::fs::remove_dir_all(&dir);
}

/// 模糊音逐对（2026-10-13）：字符串键 `FuzzyPairs` 的解析与缺省口径。
#[test]
fn parses_fuzzy_pairs_string() {
    // 缺省 = 空 = 未设置（调用方回落到旧 `Fuzzy` 布尔）——老配置行为一字不变。
    assert_eq!(parse("").fuzzy_pairs, "");
    assert_eq!(parse("Fuzzy=True\n").fuzzy_pairs, "");
    let s = parse("FuzzyPairs=zh/z,n/l  # 注释剥掉\n");
    assert_eq!(s.fuzzy_pairs, "zh/z,n/l", "两侧空白与注释都剥掉");
}

/// 模糊音逐对（2026-10-13）：`FuzzyPairs` 非空 ⇒ 以列表为准；空 ⇒ 回落布尔。
/// 断言走**引擎读侧**（`TsfLogic.engine.fuzzy_pairs()`），不是只测解析。
/// 自足夹具（不复用 logic_tests 的 state()：那边已顶 500 行门禁，见 [path] 拆法）。
#[test]
fn apply_settings_prefers_the_pairs_list() {
    use engine_core::dictionary::InMemoryDictionary;
    use engine_core::fuzzy::FUZZY_ALL;
    let mut s = crate::logic::TsfLogic::with_dict(Box::new(InMemoryDictionary::new()));
    s.apply_settings(parse("Fuzzy=False\nFuzzyPairs=n/l\n"));
    assert_eq!(
        s.engine.fuzzy_pairs(),
        1 << 6,
        "列表为准：只开 n/l（bit6 = n/l）"
    );
    s.apply_settings(parse("Fuzzy=True\n"));
    assert_eq!(s.engine.fuzzy_pairs(), FUZZY_ALL, "无列表 → 回落布尔全开");
    s.apply_settings(parse("Fuzzy=False\n"));
    assert_eq!(s.engine.fuzzy_pairs(), 0, "无列表 + 布尔关 → 全关");
}

/// 拼音纠错（2026-10-13）：键 `Correction`，缺省开（对齐搜狗）。
#[test]
fn parses_correction_switch() {
    assert!(parse("").correction, "缺省开");
    assert!(!parse("Correction=False\n").correction);
    assert!(parse("Correction=true\n").correction);
}

/// 双拼布局（2026-10-13）：字符串键 `ShuangpinLayout` 的解析 + **套到引擎读侧**。
/// 两条语义各一断言：认得的名字真的改布局；空/认不得的名字**不动**引擎（不是把
/// 用户的布局静默跳回自然码）。
#[test]
fn parses_and_applies_shuangpin_layout() {
    use engine_core::dictionary::InMemoryDictionary;
    // 解析：名字原样收，注释/空白剥掉；缺省 = 空串（未设置）。
    assert_eq!(parse("").shuangpin_layout, "");
    assert_eq!(parse("ShuangpinLayout=微软\n").shuangpin_layout, "微软");
    assert_eq!(
        parse("ShuangpinLayout=小鹤  # 注释\n").shuangpin_layout,
        "小鹤"
    );

    let mut s = crate::logic::TsfLogic::with_dict(Box::new(InMemoryDictionary::new()));
    assert_eq!(s.engine.shuangpin_layout(), 0, "缺省 = 自然码（引擎初值）");
    s.apply_settings(parse("ShuangpinLayout=紫光\n"));
    assert_eq!(s.engine.shuangpin_layout(), 4, "紫光 = 表序 4");
    // 空串（未设置）与认不得的名字都不动引擎。
    s.apply_settings(parse("Learn=True\n"));
    assert_eq!(s.engine.shuangpin_layout(), 4, "未设置 ⇒ 保持");
    s.apply_settings(parse("ShuangpinLayout=打错了\n"));
    assert_eq!(
        s.engine.shuangpin_layout(),
        4,
        "认不得的名字 ⇒ 保持（不静默跳回 0）"
    );
    s.apply_settings(parse("ShuangpinLayout=拼音加加\n"));
    assert_eq!(s.engine.shuangpin_layout(), 5, "拼音加加 = 表序 5");
}

/// 拼音方式（2026-10-14）：字符串键 `PinyinMethod` 的解析 + **套到引擎读侧**。
/// 缺省/认不得都**不动**引擎（老配置行为一字不变：全拼）。
#[test]
fn parses_and_applies_pinyin_method() {
    use engine_core::Scheme;
    use engine_core::dictionary::InMemoryDictionary;
    assert_eq!(parse("").pinyin_method, "", "缺省 = 空 = 未设置");
    assert_eq!(parse("PinyinMethod=双拼\n").pinyin_method, "双拼");
    assert_eq!(parse("PinyinMethod=全拼 # 注释\n").pinyin_method, "全拼");

    let mut s = crate::logic::TsfLogic::with_dict(Box::new(InMemoryDictionary::new()));
    assert_eq!(s.engine.scheme(), Scheme::Full, "缺省 = 全拼（引擎初值）");
    s.apply_settings(parse("PinyinMethod=双拼\n"));
    assert_eq!(s.engine.scheme(), Scheme::Shuangpin, "双拼真的进了引擎");
    // 认不得的名字/缺键都不动引擎（不静默跳回全拼）。
    s.apply_settings(parse("Learn=True\n"));
    assert_eq!(s.engine.scheme(), Scheme::Shuangpin, "未设置 ⇒ 保持");
    s.apply_settings(parse("PinyinMethod=五笔\n"));
    assert_eq!(
        s.engine.scheme(),
        Scheme::Shuangpin,
        "认不得 ⇒ 保持（不静默跳回）"
    );
    s.apply_settings(parse("PinyinMethod=全拼\n"));
    assert_eq!(s.engine.scheme(), Scheme::Full, "切回全拼");
}
