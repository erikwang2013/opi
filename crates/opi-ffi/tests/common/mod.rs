// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 两轨对照测试的**共用机械**：键码折算（`K`）、动作归一（`Act`）、
//! 全状态快照（`Snap`）、脚本驱动器（`run_script`）。
//!
//! 单独一个文件是因为 500 行硬规矩：`two_track_parity.rs` 与
//! `two_track_keycodes.rs` 两个测试二进制共用它，各自只装自己的用例。
//! `mod common;` 不会被当独立测试目标编译，所以这里没有 `#[test]`。
#![allow(dead_code)]

pub use engine_core::composer::Mode;
pub use fcitx5_opi::candidate::CandidateState;
pub use tsf_opi::logic::TsfLogic;

use fcitx5_opi::candidate::ShiftState as FShift;
use fcitx5_opi::input_method::{KeyAction, handle_key};
use tsf_opi::logic::{KeyOutcome, ShiftState as TShift};

/// 取 android 部署副本：`data/generated/luna.opid` 是 gitignore 的构建中间物
/// （`data/generated/.gitignore`），全新 clone / CI 上不存在，指着它会让本文件在
/// 干净检出里失败。两份在本机逐字节相同（sha256 3c625138…，见报告）。
pub const LUNA: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../android/app/src/main/assets/luna.opid"
);

/// 一条**逻辑**按键：与两轨各自的键码空间无关。
#[derive(Debug, Clone, Copy)]
pub enum K {
    Ch(char),
    Back,
    Del,
    Tab,
    Esc,
    Return,
    Space,
    PageUp,
    PageDown,
    Shift,
}

impl K {
    /// fcitx5 侧键码（常量取自该轨自身，避免测试里抄一份会漂移的数字）。
    pub fn fcitx5(self) -> u32 {
        use fcitx5_opi::input_method as im;
        match self {
            K::Ch(c) => c as u32,
            K::Back => im::KEY_BACK_SPACE,
            K::Del => im::KEY_DELETE,
            K::Tab => im::KEY_TAB,
            K::Esc => im::KEY_ESCAPE,
            K::Return => im::KEY_RETURN,
            K::Space => im::KEY_SPACE,
            K::PageUp => im::KEY_PAGE_UP,
            K::PageDown => im::KEY_PAGE_DOWN,
            // 左 ⇧：TSF 轨只有一码，这里取左码为代表。
            K::Shift => im::KEY_SHIFT_L,
        }
    }

    /// TSF 侧键码（`SPECIAL_BASE | VK`）。
    pub fn tsf(self) -> u32 {
        use tsf_opi::logic as lg;
        match self {
            K::Ch(c) => c as u32,
            K::Back => lg::KEY_BACK_SPACE,
            K::Del => lg::KEY_DELETE,
            K::Tab => lg::KEY_TAB,
            K::Esc => lg::KEY_ESCAPE,
            K::Return => lg::KEY_RETURN,
            K::Space => lg::KEY_SPACE,
            K::PageUp => lg::KEY_PAGE_UP,
            K::PageDown => lg::KEY_PAGE_DOWN,
            K::Shift => lg::KEY_SHIFT,
        }
    }

    pub fn name(self) -> String {
        match self {
            K::Ch(c) => format!("字符 {c:?}({:#x})", c as u32),
            other => format!("{other:?}"),
        }
    }
}

/// 归一后的动作：两轨的枚举只在这里对齐一次。
#[derive(Debug, PartialEq, Eq)]
pub enum Act {
    /// 不消费，交系统/客户端。
    Pass,
    /// 已接管（缓冲/候选/页码变化）。
    Handled,
    /// 提交文本。**空文本不算提交**：`Commit("")` 与 `Handled` 在宿主看来一样。
    Commit(String),
}

pub fn fcitx5_act(a: KeyAction) -> Act {
    match a {
        KeyAction::Input(s) if s.is_empty() => Act::Handled,
        KeyAction::Input(s) => Act::Commit(s),
        KeyAction::EngineHandled => Act::Handled,
        KeyAction::PassThrough => Act::Pass,
    }
}

pub fn tsf_act(o: KeyOutcome) -> Act {
    match o {
        KeyOutcome::Commit(s) if s.is_empty() => Act::Handled,
        KeyOutcome::Commit(s) => Act::Commit(s),
        // TSF 多一档 Consumed（消费但无需刷新候选窗）；fcitx5 无对应值，
        // 归一到 Handled —— 对宿主而言都是「已接管」。
        KeyOutcome::CompositionChanged | KeyOutcome::Consumed => Act::Handled,
        KeyOutcome::Unhandled => Act::Pass,
    }
}

/// 一次事件之后两轨的全部可观测状态。**逐字段比**，不只比动作：
/// 「两边都返回 Handled 但一边删了一个码点」这种分歧只有靠状态才看得见。
#[derive(Debug, PartialEq, Eq)]
pub struct Snap {
    pub act: Act,
    pub buffer: String,
    pub mode: Mode,
    pub cands: Vec<String>,
    pub page: usize,
    pub pages: usize,
    pub shift: &'static str,
}

pub fn fcitx5_snap(s: &mut CandidateState, keyval: u32, states: u32) -> Snap {
    let act = fcitx5_act(handle_key(s, keyval, states));
    Snap {
        act,
        buffer: s.buffer(),
        mode: s.mode(),
        cands: s.candidates(),
        page: s.page(),
        pages: s.page_count(),
        shift: match s.shift_state() {
            FShift::Off => "off",
            FShift::Single => "single",
            FShift::Lock => "lock",
        },
    }
}

pub fn tsf_snap(s: &mut TsfLogic, keyval: u32, states: u32) -> Snap {
    let act = tsf_act(s.input_key(keyval, states));
    Snap {
        act,
        buffer: s.buffer(),
        mode: s.mode(),
        cands: s.candidates(),
        page: s.page(),
        pages: s.page_count(),
        shift: match s.shift_state() {
            TShift::Off => "off",
            TShift::Single => "single",
            TShift::Lock => "lock",
        },
    }
}

/// 脚本的一步。
#[derive(Debug, Clone, Copy)]
pub enum Step {
    Key(K, u32),
    Mode(Mode),
    /// 前提断言：此刻缓冲必须非空（见 `run_script` 里的说明）。
    NonEmptyBuffer,
}

/// 逐步跑一段覆盖全部分支的脚本，每一步都比对全部可观测状态。
pub fn run_script(events: &[Step]) {
    let mut f = CandidateState::load(Some(LUNA)).expect("fcitx5 轨装载 luna");
    let mut t = TsfLogic::load(Some(LUNA)).expect("tsf 轨装载 luna");
    assert_eq!(f.buffer(), t.buffer(), "装载语义两轨必须一致（空缓冲）");
    for (i, step) in events.iter().enumerate() {
        let (fs, ts) = match *step {
            Step::Mode(m) => {
                f.switch_mode(m);
                t.switch_mode(m);
                (None, None)
            }
            // 前提断言：脚本作者声明「此刻缓冲非空」，就在**这里**核对 ——
            // 特殊键在空缓冲 / 非空缓冲下走的是两条分支，前提不成立时后面的比较
            // 会静默退化成「都在空缓冲下直通」，分歧被掩盖（实见下面的注释）。
            Step::NonEmptyBuffer => {
                assert!(
                    !f.buffer().is_empty() && !t.buffer().is_empty(),
                    "第 {i} 步前提不成立：缓冲为空（fcitx5 {:?} / tsf {:?}）—— \
                     紧跟其后的特殊键会落到空缓冲分支，比较失去意义",
                    f.buffer(),
                    t.buffer()
                );
                (None, None)
            }
            Step::Key(k, states) => (
                Some(fcitx5_snap(&mut f, k.fcitx5(), states)),
                Some(tsf_snap(&mut t, k.tsf(), states)),
            ),
        };
        if let (Some(fs), Some(ts)) = (fs, ts) {
            let what = match step {
                Step::Key(k, states) => format!("{} + states {states:#x}", k.name()),
                Step::Mode(_) | Step::NonEmptyBuffer => unreachable!(),
            };
            assert_eq!(
                fs, ts,
                "第 {i} 步（{what}）两轨行为分叉：\n fcitx5: {fs:?}\n tsf   : {ts:?}"
            );
        } else {
            // 模式切换后也要同步：否则下一步的差异会被误判成按键差异。
            assert_eq!(f.mode(), t.mode(), "第 {i} 步模式不同步");
        }
    }
}
