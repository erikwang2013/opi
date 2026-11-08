// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 候选分页（从 router.rs 拆出守 500 行）。

use super::*;

/// 单键快路径的抓取深度：首屏 2 页，之后每翻一页多 1 页（见
/// [`INITIAL_FETCH_PAGES`]）。抽成函数是为了让「2 页首屏 + 增量」这条裁决有
/// 一个可断言的形状，而不是散在 `candidates()` 里的算术。
pub(super) fn page_fetch_limit(page: usize) -> usize {
    (page + INITIAL_FETCH_PAGES) * PAGE_SIZE
}

impl KeyRouter {
    /// 当前页候选文本（最多 PAGE_SIZE 条）。**分页只此一份** —— 经 `opi_candidates_page`
    /// 出口给 UI，前端因此不需要知道 PAGE_SIZE（自己切全局列表 = 常量抄第二份）。
    pub fn candidates(&self) -> Vec<String> {
        // Full/Stroke/Wubi 的 `select` 用全局下标重算，单页取 `page_fetch_limit`
        // （首屏 2 页 + 每翻一页增量 1 页）与全量列表逐条同前缀
        // （`candidates_finite` 的单键快路径也借此生效）；
        // Nine/Fourteen/Shuangpin 的读数缓存依赖 limit，必须保持全量。
        let limit = match self.engine.scheme() {
            crate::scheme::Scheme::Full
            | crate::scheme::Scheme::Stroke
            | crate::scheme::Scheme::Wubi => page_fetch_limit(self.page),
            _ => FETCH_LIMIT,
        };
        self.engine
            .candidates(limit)
            .iter()
            .skip(self.page * PAGE_SIZE)
            .take(PAGE_SIZE)
            .map(|c| c.text.clone())
            .collect()
    }

    pub fn page(&self) -> usize {
        self.page
    }

    /// 总页数（无候选 → 0）。公开给 `opi_page_count` 出口（UI 的「共 N 页」）——
    /// 与 [`Self::candidates`] 同源，两者不会各算一套。
    pub fn page_count(&self) -> usize {
        self.fetched().len().div_ceil(PAGE_SIZE)
    }
}
