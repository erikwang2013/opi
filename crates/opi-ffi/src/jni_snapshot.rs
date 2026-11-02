// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **状态快照**出口（2026-11-02，性能项 1+2）：Android 每次 `refresh` 原先要打约 13 次
//! JNI（buffer / 六个标量开关 / candidates / candidateReadings / displayBuffer /
//! pendingText …）。本出口在**同一把引擎锁**内一次取回刷新所需的全部状态；宿主优先用它，
//! 旧 so 没有本出口时照旧走原逐出口路径（原出口一个不删）。
//!
//! # 返回布局（`String[]`：定长前缀 + 两段带计数的变长段）
//!
//! 用固定下标而不是分隔符拼接 —— 候选/读音本身是任意 UTF-16 串，任何分隔符都有歧义。
//! 每个槽位都是独立 Java String，转换复用 `jni_util::string_array`（`NewString`，
//! emoji 安全，不经过 modified UTF-8）。下标与 Android 侧 `EngineReadings.kt` 的
//! `SNAP_*` 常量逐项对应：
//!
//! | 下标 | 内容 |
//! |------|------|
//! | 0 | buffer |
//! | 1 | bufferIsCarried（"1"/"0"） |
//! | 2 | mode（整数，`mode_to_int`） |
//! | 3 | scheme（整数，`scheme_to_int`） |
//! | 4 | learnerEnabled（"1"/"0"） |
//! | 5 | fuzzyEnabled（"1"/"0"） |
//! | 6 | jianpinEnabled（"1"/"0"） |
//! | 7 | fullwidthState（"1"/"0"） |
//! | 8 | wubiNextKeys（非五笔 / 空缓冲 = 空串） |
//! | 9 | displayBuffer |
//! | 10 | pendingText |
//! | 11 | 候选条数 N |
//! | 12 .. 12+N | 候选文本 |
//! | 12+N | 读数条数 M |
//! | 13+N .. 13+N+M | 候选读数（已去重保序，引擎出口排序） |
//!
//! # 读序契约
//!
//! 引擎的 `scheme_readings` 是 `candidates()` 的副产物、且与算它时的缓冲绑存 ——
//! 本函数内**必须**按 candidates → candidateReadings → displayBuffer 的顺序取
//! （displayBuffer 也读这份缓存）；先取 display 会拿到空串/旧值。Android 的
//! `EngineController.refresh` 依赖同一顺序（有单测钉住）。
//!
//! # 读数深度（性能项 2）
//!
//! 9/14/双拼中文态的左列读音来自**整份候选排名**的副产物。折叠候选栏只显示一页，
//! 但读数要覆盖整份排名 ⇒ 排名深度仍取 512（与旧口径一致），**只把 limit 条候选
//! 回传给 Java** —— 读数不再为了显示搬 512 条候选字符串。平铺模式宿主传 limit=512，
//! 两者一致。非读数方案只排名 limit 条（普通候选栏仍只取一页）。
//!
//! ⚠️ 读数方案守卫与 `engine-core/src/engine_readings.rs` 的 `candidate_readings()`
//! 开头两道守卫同义；Android 的 `ReadingsSchemesLockstepTest` 钉 Kotlin ↔ 引擎，
//! 本文件由 `readings_guard_matches_engine_sets` 单测钉同一组取值。

#![allow(clippy::missing_safety_doc)]

use jni::sys::{self, jint};

use crate::api;
use crate::jni::{JEnv, guard};
use crate::jni_util;

/// 快照定长前缀的槽位（与 Android `EngineReadings.kt` 的 `SNAP_*` 逐项对应）。
pub(crate) const SLOT_BUFFER: usize = 0;
pub(crate) const SLOT_BUFFER_CARRIED: usize = 1;
pub(crate) const SLOT_MODE: usize = 2;
pub(crate) const SLOT_SCHEME: usize = 3;
pub(crate) const SLOT_LEARNER: usize = 4;
pub(crate) const SLOT_FUZZY: usize = 5;
pub(crate) const SLOT_JIANPIN: usize = 6;
pub(crate) const SLOT_FULLWIDTH: usize = 7;
pub(crate) const SLOT_WUBI_HINT: usize = 8;
pub(crate) const SLOT_DISPLAY: usize = 9;
pub(crate) const SLOT_PENDING: usize = 10;
pub(crate) const SLOT_CAND_COUNT: usize = 11;
pub(crate) const SLOT_CAND_START: usize = 12;

/// 布局顺序自查：12 个前缀槽位逐个入列 —— 重号/漏改在编译期就红。
const _: [usize; SLOT_CAND_START] = [
    SLOT_BUFFER,
    SLOT_BUFFER_CARRIED,
    SLOT_MODE,
    SLOT_SCHEME,
    SLOT_LEARNER,
    SLOT_FUZZY,
    SLOT_JIANPIN,
    SLOT_FULLWIDTH,
    SLOT_WUBI_HINT,
    SLOT_DISPLAY,
    SLOT_PENDING,
    SLOT_CAND_COUNT,
];

/// 单次快照回传的候选上限。与 Android 平铺取回窗口一致（`fetchLimit` 的 512）；
/// 同时是入参上限（系统边界校验：宿主传负数/天文数字都不至于把 JNI 数组撑爆）。
pub(crate) const MAX_SNAPSHOT_CANDIDATES: usize = 512;

/// 读数方案要覆盖整份排名的历史深度（旧 Android `fetchLimit=512` 的口径）。
const READINGS_RANK: usize = 512;

/// 读数守卫：中文模式 + 9/14/双拼（与 `engine_readings.rs::candidate_readings` 同义）。
pub(crate) fn needs_readings(mode: api::ApiMode, scheme: engine_core::Scheme) -> bool {
    matches!(mode, api::ApiMode::Pinyin | api::ApiMode::Traditional)
        && matches!(
            scheme,
            engine_core::Scheme::Nine
                | engine_core::Scheme::Fourteen
                | engine_core::Scheme::Shuangpin
        )
}

fn flag(on: bool) -> String {
    if on { "1" } else { "0" }.to_string()
}

/// 组装快照（不含 JNI 转换，纯函数便于单测）。
///
/// `limit` = 回传给宿主的候选条数（折叠 pageSize / 平铺 512）；读数方案内部排名深度
/// 至少 512，非读数方案就是 `limit`。
pub(crate) fn build_snapshot(e: &api::Api, limit: usize) -> Vec<String> {
    let limit = limit.min(MAX_SNAPSHOT_CANDIDATES);
    let buffer = e.buffer();
    let mode = e.mode();
    let scheme = e.scheme();
    let readings_needed = needs_readings(mode, scheme);
    // 性能项 2：读数要整份排名，但只回 limit 条候选。
    let rank_limit = if readings_needed {
        limit.max(READINGS_RANK)
    } else {
        limit
    };
    // 编码提示只在五笔态 + 缓冲非空时算（引擎侧是全量重算，别的方案连问都不问）。
    let wubi_hint = if scheme == engine_core::Scheme::Wubi && !buffer.is_empty() {
        e.wubi_next_keys()
    } else {
        String::new()
    };
    // 读序契约（见模块头）：candidates 先、readings 紧接、displayBuffer 最后。
    let mut candidates = e.candidates(rank_limit);
    candidates.truncate(limit);
    let readings = if readings_needed {
        e.candidate_readings()
    } else {
        Vec::new()
    };
    let display = e.display_buffer();
    let pending = e.pending_text();

    let mut out = Vec::with_capacity(SLOT_CAND_START + candidates.len() + 1 + readings.len());
    out.push(buffer);
    out.push(flag(e.buffer_is_carried()));
    out.push(api::mode_to_int(mode.into()).to_string());
    out.push(api::scheme_to_int(scheme).to_string());
    out.push(flag(e.learner_enabled()));
    out.push(flag(e.fuzzy_enabled()));
    out.push(flag(e.jianpin_enabled()));
    out.push(flag(e.fullwidth()));
    out.push(wubi_hint);
    out.push(display);
    out.push(pending);
    out.push(candidates.len().to_string());
    out.extend(candidates.into_iter().map(|c| c.text));
    out.push(readings.len().to_string());
    out.extend(readings);
    out
}

/// stateSnapshot(limit: int) -> String[]。未装载 → 空数组（宿主按长度校验后回退）。
///
/// 固定下标布局见模块头；各槽位都是独立 String（无分隔符歧义）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_state_snapshot(
    env: JEnv,
    _class: sys::jclass,
    limit: jint,
) -> sys::jobjectArray {
    let items = guard(|| {
        // 系统边界：负数归 0；超大钳到回传窗口上限。
        let limit = (limit.max(0) as usize).min(MAX_SNAPSHOT_CANDIDATES);
        api::with_engine(|e| build_snapshot(e, limit)).unwrap_or_default()
    });
    unsafe { jni_util::string_array(env, items) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::Scheme;

    fn api_with(scheme: Scheme, keys: &[&str]) -> api::Api {
        let mut a = api::Api::load_fallback_sync().expect("fallback load");
        a.set_scheme(scheme);
        for k in keys {
            a.input_key((*k).to_string());
        }
        a
    }

    fn count(s: &[String], at: usize) -> usize {
        s[at].parse().expect("计数字段应为十进制整数")
    }

    /// 布局自洽：定长前缀 + N 条候选 + M 条读数，长度恰好。
    #[test]
    fn snapshot_layout_counts_are_consistent() {
        let a = api_with(Scheme::Nine, &["6", "4"]);
        let s = build_snapshot(&a, 8);
        assert!(s.len() >= SLOT_CAND_START);
        assert_eq!(s[SLOT_BUFFER], "64");
        assert_eq!(s[SLOT_MODE], "0");
        assert_eq!(s[SLOT_SCHEME], "1");
        let n = count(&s, SLOT_CAND_COUNT);
        let m_at = SLOT_CAND_START + n;
        let m = count(&s, m_at);
        assert_eq!(s.len(), m_at + 1 + m, "布局长度必须与两段计数吻合");
        assert!(n <= 8, "候选只回 limit 条（实得 {n}）");
        assert!(!s[SLOT_DISPLAY].is_empty(), "9 键中文态 displayBuffer 应有读数");
        assert!(m > 0, "9 键中文态应回读数");
    }

    /// 性能项 2：折叠栏 limit 很小，但排名按读数口径 512 做 —— 候选只搬 limit 条，
    /// 读数照样覆盖整份排名（limit=0 也能拿到读数）。
    #[test]
    fn readings_rank_deep_but_candidates_stay_at_limit() {
        let a = api_with(Scheme::Nine, &["6", "4"]);
        let s = build_snapshot(&a, 0);
        assert_eq!(count(&s, SLOT_CAND_COUNT), 0, "limit=0 不回候选");
        let m = count(&s, SLOT_CAND_START);
        assert!(m > 0, "读数仍在（排名深度 512，与回传条数无关）");
    }

    /// 非读数方案（全键盘拼音）排名深度 = limit，读数段为空 —— 普通候选栏不多花。
    #[test]
    fn non_readings_scheme_has_no_readings_segment() {
        let a = api_with(Scheme::Full, &["n", "i"]);
        let s = build_snapshot(&a, 8);
        let n = count(&s, SLOT_CAND_COUNT);
        assert_eq!(count(&s, SLOT_CAND_START + n), 0);
    }

    /// 守卫取值与引擎 `candidate_readings` 的两道 matches 一致（Kotlin 侧另有锁步门禁）。
    #[test]
    fn readings_guard_matches_engine_sets() {
        use api::ApiMode;
        for mode in [ApiMode::Pinyin, ApiMode::Traditional] {
            for scheme in [Scheme::Nine, Scheme::Fourteen, Scheme::Shuangpin] {
                assert!(needs_readings(mode, scheme), "{mode:?}/{scheme:?} 应有读数");
            }
        }
        assert!(!needs_readings(ApiMode::English, Scheme::Nine));
        assert!(!needs_readings(ApiMode::Pinyin, Scheme::Full));
        assert!(!needs_readings(ApiMode::Pinyin, Scheme::Stroke));
        assert!(!needs_readings(ApiMode::Pinyin, Scheme::Wubi));
    }
}
