// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 导入层的**每键查询预算**（2026-10-18 性能审计）。
//!
//! 旧实现把 `IMPORT_SCAN_BUDGET` 放在每次 `Dictionary` 调用里新建：一次按键的主查询、
//! 模糊变体、纠错、简拼展开、逐音节、分词回退各自都能再花 40ms —— 50ms 总账
//! 在结构上不成立。本模块把预算做成**每键一份**：`Engine::candidates` 在入口
//! `begin()`，所有查询家族共享同一截止；guard 析构恢复，**不粘滞**（下一键重新计时）。
//!
//! 总账：50ms 键反馈。导入层的**初始**上限 40ms（总账 − `QUERY_RESERVE` 10ms），
//! 但 base 查询之后会再按「总账 − `QUERY_POST_RESERVE` 15ms − 已耗时」动态收紧
//! （`import_deadline_dynamic`）—— 实测宽前缀 `a` 的 base 查询 + 组装本身可达
//! 30–40ms，固定 40ms 额度在数学上无法把**整键**压在 50ms 内。超时按既有裁决
//! **整层丢弃**、回纯内置。`None` = 不设限（测试/高级口，见
//! `Engine::set_import_scan_budget`）。

use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// 单键总预算（与 `docs/performance.md` 的 50ms 键反馈上限对齐）。
pub(crate) const QUERY_TOTAL_BUDGET: Duration = Duration::from_millis(50);
/// 给内置查询留的余量（初始导入截止 = 总预算 − 本余量）。
pub(crate) const QUERY_RESERVE: Duration = Duration::from_millis(10);
/// 导入扫描结束后给合并 / 候选装配 / 排序留的硬余量（2026-10-18 实测：
/// `a` 这类宽前缀的 base 查询 + 组装本身可达 30–40ms，10ms 余量远远不够；
/// 宽前缀下导入层必须提前停，才能把**整键**压在 50ms 内）。
pub(crate) const QUERY_POST_RESERVE: Duration = Duration::from_millis(15);

#[derive(Clone, Copy)]
pub(crate) struct QueryBudget {
    pub(crate) import_deadline: Instant,
    pub(crate) total_deadline: Instant,
}

impl QueryBudget {
    fn new(import: Duration) -> Self {
        let now = Instant::now();
        let total_deadline = now + QUERY_TOTAL_BUDGET;
        // 初始导入截止仍按「总账 − 10ms」的上限（缺省 40ms）。
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

/// **动态导入截止**：在固定上限之外，再按「总账 − 15ms 后处理余量 − 当前已耗时」
/// 收紧。调用点必须在 base 查询**之后**调用（`now` 已包含 base 成本）；宽前缀
/// （如 `a`）base 本身很贵时，这里会把导入层提前停掉、整键仍在 50ms 内。
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
