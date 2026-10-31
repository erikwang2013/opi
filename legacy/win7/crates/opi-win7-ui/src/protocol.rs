// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! NDJSON wire protocol shared with the Windows 10 client.
//!
//! Source of truth: the module header of `crates/tsf-opi/src/candidate_io.rs`.
//! Pipe name `\\.\pipe\opi-v2` is fixed for the v2 protocol; incompatible
//! changes must bump to v3 (both ends move together).
//!
//! Phase 1 only consumes server -> window messages (show/hide/position);
//! select/page/commit variants are parsed so the outbound side can be added
//! without changing the wire shape.

use serde_json::Value;

pub const PIPE_NAME: &str = r"\\.\pipe\opi-v2";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Msg {
    Show {
        buffer: String,
        candidates: Vec<String>,
        page: usize,
        page_count: usize,
        mode: String,
    },
    Hide,
    Position {
        x: i32,
        y: i32,
    },
    StateChanged,
    Select {
        index: usize,
    },
    NextPage,
    PrevPage,
    CommitText {
        text: String,
    },
}

/// Parse one NDJSON frame. Unknown/invalid lines return `None` and are ignored
/// by the caller: a newer peer may send fields this build does not know.
pub fn parse_line(line: &str) -> Option<Msg> {
    let v: Value = serde_json::from_str(line.trim_end_matches(['\r', '\n'])).ok()?;
    let field = |k: &str| v.get(k);
    match field("type")?.as_str()? {
        "show" => Some(Msg::Show {
            buffer: field("buffer")?.as_str()?.to_owned(),
            candidates: field("candidates")?
                .as_array()?
                .iter()
                .map(|c| c.as_str().unwrap_or_default().to_owned())
                .collect(),
            page: field("page").and_then(Value::as_u64).unwrap_or(1).max(1) as usize,
            page_count: field("page_count")
                .and_then(Value::as_u64)
                .unwrap_or(1)
                .max(1) as usize,
            mode: field("mode")
                .and_then(Value::as_str)
                .unwrap_or("pinyin")
                .to_owned(),
        }),
        "hide" => Some(Msg::Hide),
        "position" => Some(Msg::Position {
            x: field("x")?.as_i64()? as i32,
            y: field("y")?.as_i64()? as i32,
        }),
        "state_changed" => Some(Msg::StateChanged),
        "select" => Some(Msg::Select {
            index: field("index")?.as_u64()? as usize,
        }),
        "next_page" => Some(Msg::NextPage),
        "prev_page" => Some(Msg::PrevPage),
        "commit_text" => Some(Msg::CommitText {
            text: field("text")?.as_str()?.to_owned(),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipe_name_is_v2() {
        assert_eq!(PIPE_NAME, r"\\.\pipe\opi-v2");
    }

    #[test]
    fn parses_show_frame() {
        let line = r#"{"type":"show","buffer":"ni","candidates":["你","尼"],"page":2,"page_count":3,"mode":"pinyin"}"#;
        assert_eq!(
            parse_line(line),
            Some(Msg::Show {
                buffer: "ni".into(),
                candidates: vec!["你".into(), "尼".into()],
                page: 2,
                page_count: 3,
                mode: "pinyin".into(),
            })
        );
    }

    #[test]
    fn show_defaults_page_and_mode_and_tolerates_crlf() {
        let line = "{\"type\":\"show\",\"buffer\":\"a\",\"candidates\":[]}\r\n";
        assert_eq!(
            parse_line(line),
            Some(Msg::Show {
                buffer: "a".into(),
                candidates: vec![],
                page: 1,
                page_count: 1,
                mode: "pinyin".into(),
            })
        );
    }

    #[test]
    fn parses_position_and_replies() {
        assert_eq!(
            parse_line(r#"{"type":"position","x":120,"y":340}"#),
            Some(Msg::Position { x: 120, y: 340 })
        );
        assert_eq!(
            parse_line(r#"{"type":"select","index":3}"#),
            Some(Msg::Select { index: 3 })
        );
        assert_eq!(
            parse_line(r#"{"type":"commit_text","text":"，"}"#),
            Some(Msg::CommitText { text: "，".into() })
        );
    }

    #[test]
    fn rejects_malformed_and_unknown_frames() {
        assert_eq!(parse_line("not json"), None);
        assert_eq!(parse_line(r#"{"type":"future_message"}"#), None);
        // show without candidates is a protocol error, not an empty popup.
        assert_eq!(parse_line(r#"{"type":"show","buffer":"a"}"#), None);
    }
}
