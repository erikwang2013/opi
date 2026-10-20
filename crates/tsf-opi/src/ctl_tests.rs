// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 控制面语义（`ctl.rs`）：协议解析、状态行、动作分派、失败语义。
//! 不测管道/COM —— 那两层只在 Windows 上存在（本机门禁覆盖不到，交付时如实列）。

use super::*;
use crate::logic::{KeyOutcome, ShiftState, TsfLogic};
use engine_core::composer::Mode;
use engine_core::dictionary::InMemoryDictionary;

fn state() -> TsfLogic {
    let mut d = InMemoryDictionary::new();
    d.insert("ni", "你", 100);
    d.insert("nihao", "你好", 90);
    let mut s = TsfLogic::with_dict(Box::new(d));
    s.switch_mode(Mode::Pinyin); // 构造器不假设引擎默认态
    s
}

fn english() -> TsfLogic {
    let mut s = state();
    s.switch_mode(Mode::English);
    s
}

fn act(s: &mut TsfLogic, name: &str) -> Result<CtlReply, String> {
    execute(s, &CtlOp::Action(name.into()), None)
}

fn token(reply: &CtlReply) -> Option<&str> {
    match reply {
        CtlReply::Action { token, .. } => token.as_deref(),
        _ => None,
    }
}

fn commit(reply: &CtlReply) -> Option<&str> {
    match reply {
        CtlReply::Action { commit, .. } => commit.as_deref(),
        _ => None,
    }
}

// ---- 协议解析 ----

#[test]
fn parses_the_four_ops() {
    let state_req = parse(r#"{"type":"ctl","id":7,"op":"state"}"#).unwrap();
    assert_eq!(state_req.id, 7);
    assert_eq!(state_req.op, CtlOp::State);
    assert_eq!(
        parse(r#"{"op":"action","action":"punct","id":8,"type":"ctl"}"#)
            .unwrap()
            .op,
        CtlOp::Action("punct".into())
    );
    assert_eq!(
        parse(r#"{"type":"ctl","id":9,"op":"search","query":"smile"}"#)
            .unwrap()
            .op,
        CtlOp::Search("smile".into())
    );
    assert_eq!(
        parse(r#"{"type":"ctl","id":10,"op":"commit","text":"😄"}"#)
            .unwrap()
            .op,
        CtlOp::Commit("😄".into())
    );
}

#[test]
fn rejects_malformed_or_foreign_lines() {
    // 非 ctl 类型（候选协议自己的行）绝不能误当控制请求
    assert!(parse(r#"{"type":"show","id":1,"op":"state"}"#).is_none());
    assert!(parse(r#"{"type":"ctl","op":"state"}"#).is_none()); // 缺 id
    assert!(parse(r#"{"type":"ctl","id":1}"#).is_none()); // 缺 op
    assert!(parse(r#"{"type":"ctl","id":1,"op":"nope"}"#).is_none());
    assert!(parse(r#"{"type":"ctl","id":1,"op":"action"}"#).is_none());
    assert!(parse(r#"{"type":"ctl","id":1,"op":"action","action":""}"#).is_none());
    assert!(parse(r#"{"type":"ctl","id":1,"op":"search","query":""}"#).is_none());
    assert!(parse(r#"{"type":"ctl","id":1,"op":"commit","text":""}"#).is_none());
    assert!(parse("not json").is_none());
    // 上限（动作名/关键字/直插文本）——超长一律拒绝，不截断
    let long = "a".repeat(MAX_QUERY + 1);
    assert!(
        parse(&format!(
            r#"{{"type":"ctl","id":1,"op":"search","query":"{long}"}}"#
        ))
        .is_none()
    );
    let long = "x".repeat(MAX_TEXT + 1);
    assert!(
        parse(&format!(
            r#"{{"type":"ctl","id":1,"op":"commit","text":"{long}"}}"#
        ))
        .is_none()
    );
    let long = "a".repeat(MAX_ACTION + 1);
    assert!(
        parse(&format!(
            r#"{{"type":"ctl","id":1,"op":"action","action":"{long}"}}"#
        ))
        .is_none()
    );
    // 负数 id 不是 u64
    assert!(parse(r#"{"type":"ctl","id":-1,"op":"state"}"#).is_none());
}

// ---- 状态行（与 Linux `opi-ctl state` 逐字同格式） ----

#[test]
fn state_line_matches_linux_format() {
    assert_eq!(
        state_line(&state()),
        "lang=zh;trad=off;punct=on;fw=on;im=opi;mode=pinyin;enpro=off"
    );
}

#[test]
fn state_line_english_reports_en_and_effective_punct_off() {
    // 生效标点值在英文档恒 off（配置值仍是 on，动作翻的是配置值）
    let line = state_line(&english());
    assert!(line.contains("lang=en;"), "{line}");
    assert!(line.contains(";punct=off;"), "{line}");
    assert!(line.contains(";mode=en26;"), "{line}");
}

#[test]
fn state_line_maps_traditional_and_wubi() {
    let mut s = state();
    s.switch_mode(Mode::Traditional);
    let line = state_line(&s);
    assert!(line.contains("lang=zh;trad=on;"), "{line}");
    assert!(line.contains(";mode=pinyin;"), "{line}"); // 面外态收成拼音读法

    let mut s = state();
    s.set_scheme(engine_core::Scheme::Wubi);
    assert!(state_line(&s).contains(";mode=wubi;"));
    assert_eq!(mode_token(&s), "wubi");
}

// ---- 动作：lang ----

#[test]
fn lang_entering_english_confirms_raw_buffer_and_resets_shift() {
    let mut s = state();
    s.input_key('n' as u32, 0);
    s.input_key('i' as u32, 0);
    s.shift_state = ShiftState::Single; // ⇧ 残留不该被带进英文档
    let reply = act(&mut s, "lang").unwrap();
    assert_eq!(commit(&reply), Some("ni")); // W1：先交原文，再切模式
    assert_eq!(s.mode(), Mode::English);
    assert_eq!(s.shift_state, ShiftState::Off);
}

#[test]
fn lang_leaving_english_switches_without_commit() {
    let mut s = english();
    let reply = act(&mut s, "lang").unwrap();
    assert_eq!(commit(&reply), None);
    assert_eq!(s.mode(), Mode::Pinyin);
    assert_eq!(token(&reply), None);
}

// ---- 动作：trad / fullwidth / englishpro ----

#[test]
fn trad_is_a_plain_switch() {
    let mut s = state();
    act(&mut s, "trad").unwrap();
    assert_eq!(s.mode(), Mode::Traditional);
    act(&mut s, "trad").unwrap();
    assert_eq!(s.mode(), Mode::Pinyin);
}

#[test]
fn fullwidth_toggles() {
    let mut s = state();
    assert!(s.fullwidth()); // 全拼模式默认全角（Engine 既有语义，与 Linux 轨同源）
    act(&mut s, "fullwidth").unwrap();
    assert!(!s.fullwidth());
    assert!(state_line(&s).contains(";fw=off;"));
}

#[test]
fn englishpro_toggles_and_clears_the_buffer() {
    let mut s = english();
    // 普通档：字母直出，不查词库
    assert_eq!(s.input_key('n' as u32, 0), KeyOutcome::Commit("n".into()));
    act(&mut s, "englishpro").unwrap();
    assert!(s.english_pro());
    assert_eq!(s.buffer(), "");
    // 专业档：字母进缓冲走前缀联想（此处只钉路由，不钉候选内容）
    assert_eq!(s.input_key('n' as u32, 0), KeyOutcome::CompositionChanged);
    act(&mut s, "englishpro").unwrap();
    assert!(!s.english_pro());
    assert_eq!(s.buffer(), "");
}

// ---- 动作：mode 循环 ----

#[test]
fn mode_cycle_is_pinyin_wubi_en26_pinyin() {
    let mut s = state();
    s.input_key('n' as u32, 0);
    s.input_key('i' as u32, 0);

    let reply = act(&mut s, "mode").unwrap();
    assert_eq!(token(&reply), Some("wubi"));
    assert_eq!(commit(&reply), Some("ni")); // 切五笔前先交待定拼音
    assert_eq!(s.mode(), Mode::Pinyin);
    assert_eq!(s.scheme(), engine_core::Scheme::Wubi);

    let reply = act(&mut s, "mode").unwrap();
    assert_eq!(token(&reply), Some("en26"));
    assert_eq!(commit(&reply), None);
    assert_eq!(s.mode(), Mode::English);

    let reply = act(&mut s, "mode").unwrap();
    assert_eq!(token(&reply), Some("pinyin"));
    assert_eq!(s.mode(), Mode::Pinyin);
    assert_eq!(s.scheme(), engine_core::Scheme::Full); // 回拼音档必须归全拼
    assert_eq!(state_line(&s).split(';').nth(5), Some("mode=pinyin"));
}

#[test]
fn mode_from_traditional_goes_to_wubi_and_normalizes_mode() {
    let mut s = state();
    s.switch_mode(Mode::Traditional);
    let reply = act(&mut s, "mode").unwrap();
    assert_eq!(token(&reply), Some("wubi"));
    assert_eq!(s.mode(), Mode::Pinyin); // 面外态 -> Pinyin + Wubi，token 才不空
    assert_eq!(s.scheme(), engine_core::Scheme::Wubi);
}

// ---- 动作：punct（落盘优先，失败不动引擎） ----

fn temp_settings(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("opi-ctl-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("settings.ini")
}

#[test]
fn punct_persists_only_that_key_and_then_toggles_engine() {
    let path = temp_settings("persist");
    std::fs::write(&path, "Learn=False\nChinesePunct=true\nCorrection=0\n").unwrap();
    let mut s = state();

    execute(&mut s, &CtlOp::Action("punct".into()), Some(&path)).unwrap();
    assert!(!s.chinese_punct());
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("ChinesePunct=false\n"), "{text:?}");
    assert!(
        text.contains("Learn=False\n"),
        "其余行必须原样保留: {text:?}"
    );
    assert!(text.contains("Correction=0\n"), "{text:?}");

    execute(&mut s, &CtlOp::Action("punct".into()), Some(&path)).unwrap();
    assert!(s.chinese_punct());
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .contains("ChinesePunct=true\n")
    );
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

#[test]
fn punct_write_failure_leaves_engine_untouched() {
    // 目标是已存在的目录：rename 必然失败
    let dir = temp_settings("fail");
    let dir = dir.parent().unwrap().to_path_buf();
    let mut s = state();
    let err = execute(&mut s, &CtlOp::Action("punct".into()), Some(&dir)).unwrap_err();
    assert!(err.contains("保存失败"), "{err}");
    assert!(s.chinese_punct(), "写失败必须回滚到原值");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn punct_without_settings_path_is_rejected() {
    let mut s = state();
    let err = execute(&mut s, &CtlOp::Action("punct".into()), None).unwrap_err();
    assert!(err.contains("LOCALAPPDATA"), "{err}");
    assert!(s.chinese_punct());
}

// ---- 动作：search / commit / 未知 ----

#[test]
fn search_returns_matching_symbol_texts() {
    let mut s = state();
    match execute(&mut s, &CtlOp::Search("smile".into()), None).unwrap() {
        CtlReply::Texts(texts) => assert!(texts.iter().any(|t| t == "😄"), "{texts:?}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(
        execute(&mut s, &CtlOp::Search("qqqqq".into()), None).unwrap(),
        CtlReply::Texts(vec![])
    );
}

#[test]
fn commit_is_echoed_for_the_caller_to_insert() {
    let mut s = state();
    assert_eq!(
        execute(&mut s, &CtlOp::Commit("你好".into()), None).unwrap(),
        CtlReply::Commit("你好".into())
    );
}

#[test]
fn unknown_action_is_an_error_not_a_silent_ok() {
    let mut s = state();
    let err = act(&mut s, "symbols").unwrap_err();
    assert!(err.contains("未知动作"), "{err}");
}

// ---- 应答 JSON（键序 = serde_json 字典序；桌面用真实行做契约测试） ----

#[test]
fn reply_json_state_shape() {
    let line = reply_json(7, Ok(CtlReply::State(state_line(&state()))));
    let v: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(v["type"], "ctl_result");
    assert_eq!(v["id"], 7);
    assert_eq!(v["ok"], true);
    assert_eq!(
        v["state"],
        "lang=zh;trad=off;punct=on;fw=on;im=opi;mode=pinyin;enpro=off"
    );
    assert!(line.starts_with(r#"{"id":7,"ok":true,"state":"#), "{line}");
}

#[test]
fn reply_json_action_token_and_error_shapes() {
    let line = reply_json(8, Ok(action_reply(Some("wubi".into()))));
    assert_eq!(
        line,
        r#"{"id":8,"ok":true,"token":"wubi","type":"ctl_result"}"#
    );
    let line = reply_json(9, Ok(action_reply(None)));
    assert_eq!(line, r#"{"id":9,"ok":true,"type":"ctl_result"}"#);
    let line = reply_json(10, Err("未知动作：x".into()));
    assert_eq!(
        line,
        r#"{"error":"未知动作：x","id":10,"ok":false,"type":"ctl_result"}"#
    );
}

#[test]
fn reply_json_texts_shape() {
    let line = reply_json(11, Ok(CtlReply::Texts(vec!["😄".into()])));
    let v: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(v["texts"][0], "😄");
    // 直插应答不带文本字段（调用方自己知道发了什么）
    let line = reply_json(12, Ok(CtlReply::Commit("x".into())));
    assert_eq!(line, r#"{"id":12,"ok":true,"type":"ctl_result"}"#);
}
