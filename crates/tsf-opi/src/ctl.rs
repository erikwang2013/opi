// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! Windows 状态栏/设置 → TSF 的**控制面**（平台中立部分）。
//!
//! 通道拓扑：候选窗进程（`desktop/`）是 broker；状态栏/设置进程连
//! `\\.\pipe\opi-control`，broker 把请求转发到它和 TSF 之间已有的候选管道
//! （`candidate_io.rs` 的 `{"type":"ctl"}` / `{"type":"ctl_result"}`）。
//! 本模块只管**请求/应答的语义**，不碰管道/COM —— 故 Linux 主机可单测。
//!
//! 请求（broker → TSF，NDJSON 一行）：
//! ```text
//! {"id":7,"op":"state","type":"ctl"}
//! {"action":"punct","id":8,"op":"action","type":"ctl"}
//! {"id":9,"op":"search","query":"smile","type":"ctl"}
//! {"id":10,"op":"commit","text":"😄","type":"ctl"}
//! {"id":11,"op":"reload","type":"ctl"}                       // 设置窗保存后让 TSF 重读
//! ```
//! 应答（TSF → broker → 状态栏/设置，原样转发；键序 = serde_json 字典序）：
//! ```text
//! {"id":7,"ok":true,"state":"lang=zh;trad=off;punct=on;fw=off;im=opi;mode=pinyin;enpro=off","type":"ctl_result"}
//! {"id":8,"ok":true,"type":"ctl_result"}                       // 动作成功
//! {"id":8,"ok":true,"token":"wubi","type":"ctl_result"}        // action=mode 的应答
//! {"id":9,"ok":true,"texts":["😄",...],"type":"ctl_result"}
//! {"id":10,"ok":true,"type":"ctl_result"}                      // 已入上屏队列
//! {"id":11,"ok":true,"type":"ctl_result"}                      // 已重读设置/词库
//! {"error":"未知动作：x","id":8,"ok":false,"type":"ctl_result"}
//! ```
//! `state` 的格式与 Linux `opi-ctl state` **逐字一致**（含行尾 `mode=`/`enpro=`），
//! 桌面解析器只有一份。`im=` 恒为 `opi`：本通道只在 OPI 的 TSF 客户端连着时才存在。
//!
//! 线程边界：执行发生在候选管道的**读线程**上（`SharedAction::on_ctl`）。这里只动
//! 引擎锁内的状态；上屏文本一律经既有 `enqueue_commit_text` 排队，由按键线程执行
//! （见 `state.rs` 的线程模型）——读线程不碰 COM，宿主 UI 线程不被阻塞。
//! 标点动作的落盘是唯一 IO：目标文件由调用方传入（测试可换临时路径）。

use std::path::{Path, PathBuf};

use engine_core::Scheme;
use engine_core::composer::Mode;
use serde_json::{Value, json};

use crate::logic::TsfLogic;

/// 通道里能代表「本输入法」的唯一名（桌面据此判断状态栏该不该显示）。
pub const IM_NAME: &str = "opi";
/// 动作名上限（当前最长 `clearuserwords`，留余量）。
const MAX_ACTION: usize = 64;
/// 搜索关键字上限（与 Linux 桌面搜索框的实际输入同量级）。
const MAX_QUERY: usize = 256;
/// 一次直插文本上限（符号页多选可能拼长串）。
const MAX_TEXT: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CtlOp {
    State,
    Action(String),
    Search(String),
    Commit(String),
    /// 重读 settings.ini + 词库目录（设置窗保存/导入词库后调用）。
    Reload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CtlRequest {
    pub id: u64,
    pub op: CtlOp,
}

/// 执行结果。`Action` 可能既带新档位 token、又带要上屏的待确认文本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CtlReply {
    State(String),
    Action {
        token: Option<String>,
        commit: Option<String>,
    },
    Texts(Vec<String>),
    Commit(String),
    /// 设置/词库已重读（无载荷）。
    Reload,
}

/// 解析一行请求。任何不合契约的行 → None（缺 id/op、类型错、超长）。
pub fn parse(line: &str) -> Option<CtlRequest> {
    let v: Value = serde_json::from_str(line).ok()?;
    if v.get("type").and_then(Value::as_str) != Some("ctl") {
        return None;
    }
    let id = v.get("id").and_then(Value::as_u64)?;
    let op = match v.get("op").and_then(Value::as_str)? {
        "state" => CtlOp::State,
        "action" => {
            let action = v.get("action").and_then(Value::as_str)?;
            if action.is_empty() || action.len() > MAX_ACTION {
                return None;
            }
            CtlOp::Action(action.to_string())
        }
        "search" => {
            let query = v.get("query").and_then(Value::as_str)?;
            if query.is_empty() || query.len() > MAX_QUERY {
                return None;
            }
            CtlOp::Search(query.to_string())
        }
        "commit" => {
            let text = v.get("text").and_then(Value::as_str)?;
            if text.is_empty() || text.len() > MAX_TEXT {
                return None;
            }
            CtlOp::Commit(text.to_string())
        }
        "reload" => CtlOp::Reload,
        _ => return None,
    };
    Some(CtlRequest { id, op })
}

/// 档位 token（与 fcitx5 的 `opiCtlModeToken` 同一张表）：English→en26、
/// 全拼+Pinyin→pinyin、五笔+Pinyin→wubi，其余空串（面外态）。
pub fn mode_token(logic: &TsfLogic) -> String {
    match (logic.mode(), logic.scheme()) {
        (Mode::English, _) => "en26".into(),
        (Mode::Pinyin, Scheme::Full) => "pinyin".into(),
        (Mode::Pinyin, Scheme::Wubi) => "wubi".into(),
        _ => String::new(),
    }
}

/// `state` 行末的档位格：面外态（繁体/数字/符号/双拼）收成 `pinyin`（读法仍是拼音），
/// 与 fcitx5 的 `opiCtlStateModeToken` 同一条改判。
fn state_mode_token(logic: &TsfLogic) -> String {
    let t = mode_token(logic);
    if t.is_empty() { "pinyin".into() } else { t }
}

/// 与 Linux `opi-ctl state` 逐字同格式的一行。
pub fn state_line(logic: &TsfLogic) -> String {
    format!(
        "lang={};trad={};punct={};fw={};im={};mode={};enpro={}",
        if logic.mode() == Mode::English {
            "en"
        } else {
            "zh"
        },
        on_off(logic.mode() == Mode::Traditional),
        // 生效值：英文/数字/符号恒 off（与 fcitx5 2026-10-15 订正一致）。
        on_off(logic.engine.effective_chinese_punct()),
        on_off(logic.fullwidth()),
        IM_NAME,
        state_mode_token(logic),
        on_off(logic.english_pro()),
    )
}

fn on_off(v: bool) -> &'static str {
    if v { "on" } else { "off" }
}

/// 执行一个请求（**读线程**上、引擎锁内调用；不做 COM）。
///
/// `settings_path` 给 `punct`（先落盘、后改引擎）与 `reload` 用；
/// `dicts_dir` 只给 `reload` 用（与 `Activate` 同一条 `apply_settings`）。
/// 其余动作是会话内状态，不落配置。
pub fn execute(
    logic: &mut TsfLogic,
    op: &CtlOp,
    settings_path: Option<&Path>,
    dicts_dir: &Path,
) -> Result<CtlReply, String> {
    match op {
        CtlOp::State => Ok(CtlReply::State(state_line(logic))),
        CtlOp::Search(q) => Ok(CtlReply::Texts(logic.search_symbol_texts(q))),
        CtlOp::Commit(text) => Ok(CtlReply::Commit(text.clone())),
        // 与 Activate 完全同一条套用路径（设置 + 五笔表 + 导入词库）。
        // 代价：若此刻正在组字，apply_settings 的 set_scheme 会清缓冲 —— 与激活时一致。
        CtlOp::Reload => {
            let path = settings_path.ok_or("设置文件路径不可用（LOCALAPPDATA 未设置）")?;
            logic.apply_settings(crate::settings::load_file(path), dicts_dir);
            logic.apply_custom_phrases(crate::custom_phrases::load(dicts_dir));
            logic.apply_english_custom_phrases(crate::custom_phrases::load_english(dicts_dir));
            Ok(CtlReply::Reload)
        }
        CtlOp::Action(action) => match action.as_str() {
            // 中/英：与 Linux `lang` 同一语义（Shift 轻按）——语义本体在
            // `TsfLogic::toggle_language`（Shift 轻按走同一个函数，两处不能各写一份：
            // 一处改了「切英文要先确认缓冲」、另一处漏改就是静默漂移）。
            "lang" => Ok(CtlReply::Action {
                token: None,
                commit: non_empty(logic.toggle_language()),
            }),
            // 简繁：与 Linux `opiToggleTraditional` 一致（plain switch，不确认缓冲）。
            "trad" => {
                let next = if logic.mode() == Mode::Traditional {
                    Mode::Pinyin
                } else {
                    Mode::Traditional
                };
                logic.switch_mode(next);
                Ok(action_reply(None))
            }
            // 标点：翻的是**配置值**（不是按模式折算的生效值）。先落盘再改引擎。
            "punct" => {
                let want = !logic.chinese_punct();
                let path = settings_path.ok_or("设置文件路径不可用（LOCALAPPDATA 未设置）")?;
                crate::settings::set_chinese_punct(path, want)
                    .map_err(|e| format!("中文标点配置保存失败：{e}"))?;
                logic.set_chinese_punct(want);
                Ok(action_reply(None))
            }
            "fullwidth" => {
                logic.toggle_fullwidth();
                Ok(action_reply(None))
            }
            "englishpro" => {
                let on = !logic.english_pro();
                logic.set_english_pro(on);
                Ok(action_reply(None))
            }
            // 清空用户词（学习记忆）；候选集会变，`on_ctl` 按 Action 自动刷新。
            "clearuserwords" => {
                logic.clear_user_words();
                Ok(action_reply(None))
            }
            // 模式循环：拼音(全拼) → 五笔 → en26 → 拼音（与 `opiCtlCycleMode` 同序）。
            // 切五笔/英文前先确认待定拼音（W1）；从英文回来时方案归全拼。
            "mode" => {
                if logic.mode() == Mode::English {
                    logic.set_scheme(Scheme::Full);
                    logic.switch_mode(Mode::Pinyin);
                    Ok(action_reply(Some(mode_token(logic))))
                } else if state_mode_token(logic) == "pinyin" {
                    let commit = non_empty(logic.switch_mode_confirmed(Mode::Pinyin));
                    logic.set_scheme(Scheme::Wubi);
                    Ok(CtlReply::Action {
                        token: non_empty(mode_token(logic)),
                        commit,
                    })
                } else {
                    let commit = non_empty(logic.switch_mode_confirmed(Mode::English));
                    logic.reset_shift();
                    Ok(CtlReply::Action {
                        token: non_empty(mode_token(logic)),
                        commit,
                    })
                }
            }
            other => Err(format!("未知动作：{other}")),
        },
    }
}

fn action_reply(token: Option<String>) -> CtlReply {
    CtlReply::Action {
        token,
        commit: None,
    }
}

fn non_empty(s: String) -> Option<String> {
    if s.is_empty() { None } else { Some(s) }
}

/// 应答 JSON（发回 broker）。`Err` 是给用户看的原因，原样进 `error` 字段。
pub fn reply_json(id: u64, reply: Result<CtlReply, String>) -> String {
    let mut out = json!({"type": "ctl_result", "id": id, "ok": reply.is_ok()});
    match reply {
        Ok(CtlReply::State(state)) => out["state"] = Value::String(state),
        Ok(CtlReply::Action { token, .. }) => {
            if let Some(t) = token {
                out["token"] = Value::String(t);
            }
        }
        Ok(CtlReply::Texts(texts)) => out["texts"] = json!(texts),
        Ok(CtlReply::Commit(_) | CtlReply::Reload) => {}
        Err(why) => out["error"] = Value::String(why),
    }
    out.to_string()
}

/// 默认 settings.ini（与 `settings::load` 同一条路径推导）。
pub fn default_settings_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|v| crate::settings::path_in(Some(v.as_os_str())))
}

/// 默认导入词库目录（与 `dict_path::dicts_dir` 同一条路径推导）。
pub fn default_dicts_dir() -> PathBuf {
    crate::dict_path::dicts_dir(std::env::var_os("LOCALAPPDATA").as_deref())
}

#[cfg(test)]
#[path = "ctl_tests.rs"]
mod tests;
