// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 导入层的**每键查询预算**（2026-10-18 性能审计）。
//!
//! 旧实现把 `IMPORT_SCAN_BUDGET` 放在每次 `Dictionary` 调用里新建：一次按键的主查询、
//! 模糊变体、纠错、简拼展开、逐音节、分词回退各自都能再花 40ms —— 总账
//! 在结构上不成立。本模块把预算做成**每键一份**：`Engine::candidates` 在入口
//! `begin()`，所有查询家族共享同一截止；guard 析构恢复，**不粘滞**（下一键重新计时）。
//!
//! 总账：**30ms**（2026-10-22 用户裁决「所有端所有的响应要低于 30ms」，收紧自
//! 2026-10-11 的 50ms）。导入层的**初始**上限 20ms（`IMPORT_SCAN_BUDGET`，且被
//! 「总账 − `QUERY_RESERVE` 5ms」封顶），base 查询之后再按「总账 −
//! `QUERY_POST_RESERVE` 20ms − 已耗时」动态收紧（`import_deadline_dynamic`）——
//! 导入层必须在这之后提前停，才能把**整键**压在 30ms 内。超时按既有裁决**整层丢弃**、回纯内置。`None` = 不设限（测试/高级口，
//! 见 `Engine::set_import_scan_budget`）。

use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// 单键总预算（与 `docs/performance.md` 的 **30ms** 键反馈上限对齐，2026-10-22）。
pub(crate) const QUERY_TOTAL_BUDGET: Duration = Duration::from_millis(30);
/// 给内置查询留的余量（初始导入截止 = 总预算 − 本余量）。
pub(crate) const QUERY_RESERVE: Duration = Duration::from_millis(5);
/// 导入扫描结束后给合并 / 候选装配 / 排序留的硬余量。
///
/// 2026-10-23 实测：base 查询之后到返回（评分/排序/去重/简体优先/三档/纠错）
/// p50 ≈ 17ms（luna，`candidates(MAX)`），旧的 10ms 让导入层合法地超到总账之外
/// （20k 命中导入实测整键 71ms）。留 20ms 后导入层在 `总账 − 20ms` 处停：扫描完成
/// 的层照常合并，扫不完的层按既有语义**整层丢弃**。
pub(crate) const QUERY_POST_RESERVE: Duration = Duration::from_millis(20);

#[derive(Clone, Copy)]
pub(crate) struct QueryBudget {
    pub(crate) import_deadline: Instant,
    pub(crate) total_deadline: Instant,
}

impl QueryBudget {
    fn new(import: Duration) -> Self {
        let now = Instant::now();
        let total_deadline = now + QUERY_TOTAL_BUDGET;
        // 初始导入截止按「总账 − 5ms」封顶（缺省 import = 20ms）。
        let import_deadline = (now + import).min(total_deadline - QUERY_RESERVE);
        Self {
            import_deadline,
            total_deadline,
        }
    }
}

/// 查询槽的三态：没有查询在跑 / 跑着但不设限 / 跑着且有截止。
#[derive(Clone, Copy)]
pub(crate) enum QueryState {
    Inactive,
    Unlimited,
    Limited(QueryBudget),
}

/// 查询预算 guard：`Drop` 恢复进入前的状态（嵌套查询不覆盖外层截止）。
pub(crate) struct BudgetGuard<'a> {
    state: &'a Mutex<QueryState>,
    prev: QueryState,
}

impl Drop for BudgetGuard<'_> {
    fn drop(&mut self) {
        *lock(self.state) = self.prev;
    }
}

/// 锁中毒不 panic：预算只是一个可重建的瞬时标记（原则同 tsf 的锁策略）。
fn lock(state: &Mutex<QueryState>) -> MutexGuard<'_, QueryState> {
    state.lock().unwrap_or_else(|e| e.into_inner())
}

/// 在查询入口建立每键预算；已有预算（嵌套查询）不覆盖。
pub(crate) fn begin<'a>(
    state: &'a Mutex<QueryState>,
    import_scan_budget: Option<Duration>,
) -> BudgetGuard<'a> {
    let mut slot = lock(state);
    let prev = *slot;
    if matches!(prev, QueryState::Inactive) {
        *slot = match import_scan_budget {
            Some(b) => QueryState::Limited(QueryBudget::new(b)),
            None => QueryState::Unlimited,
        };
    }
    BudgetGuard { state, prev }
}

/// 当前导入层截止：`None` = 不设限。没有活动查询（直接调 `Dictionary` 的测试/旧口）
/// 时退化成一调用一份 —— 保持 `set_import_scan_budget` 的旧语义。
pub(crate) fn import_deadline(
    state: &Mutex<QueryState>,
    import_scan_budget: Option<Duration>,
) -> Option<Instant> {
    match *lock(state) {
        QueryState::Limited(b) => Some(b.import_deadline),
        QueryState::Unlimited => None,
        QueryState::Inactive => import_scan_budget.map(|b| Instant::now() + b),
    }
}

/// **动态导入截止**：在固定上限之外，再按「总账 − 20ms 后处理余量 − 当前已耗时」
/// 收紧。调用点必须在 base 查询**之后**调用（`now` 已包含 base 成本）；宽前缀
/// （如 `a`）base 本身很贵时，这里会把导入层提前停掉、整键仍在 30ms 内。
pub(crate) fn import_deadline_dynamic(
    state: &Mutex<QueryState>,
    import_scan_budget: Option<Duration>,
) -> Option<Instant> {
    let fixed = import_deadline(state, import_scan_budget);
    let total = match *lock(state) {
        QueryState::Limited(b) => b.total_deadline,
        _ => return fixed,
    };
    let cap = total - QUERY_POST_RESERVE;
    let remain = cap.saturating_duration_since(Instant::now());
    let dynamic = Instant::now() + remain; // remain = 0 ⇒ 立即超时
    Some(fixed.map_or(dynamic, |d| d.min(dynamic)))
}
