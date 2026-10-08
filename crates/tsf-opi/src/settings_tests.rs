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
            jianpin: false,
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
