// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// 候选表 + 翻页箭头（2026-10-28 从 opi_fcitx5.cpp 拆出：那份文件贴着 500 行
// 硬规矩，与 opi_json.h / opi_engine.h 同一拆分先例）。
//
// 页码真源 = Rust（CandidateState::page）：列表里只装**当前页**的候选，
// hasPrev/hasNext/prev/next 转发给 opi_fcitx5_page_state / opi_fcitx5_page_step，
// 再把新一页**原地**重建进自己。于是 classicui / D-Bus 的翻页箭头可点，而页码
// 仍只有一份（不把整份候选交给 CommonCandidateList 自己翻，避免两份页码漂移）。
#pragma once

#include <fcitx/candidatelist.h>
#include <fcitx/inputcontext.h>
#include <fcitx/text.h>

#include <cstdint>
#include <cstdio>
#include <exception>
#include <memory>
#include <string>
#include <utility>
#include <vector>

#include "opi_json.h"
#include "opi_rust_ffi.h"

// 定义在 opi_fcitx5.cpp；声明也在 opi_engine.h（这里不 include 它：会带入
// 整个 addon 声明面，没必要）。
void refreshingPanelFor(fcitx::InputContext *ic);

// 取回候选的条数上限：**不要**再往这里写一个「Rust PAGE_SIZE 的镜像」。
// Rust 的 `opi_fcitx5_candidates(limit)` 返回的已经是**切好的整页**（`candidate.rs`
// 按它自己的 PAGE_SIZE `.skip().take()`，这里的 limit 是在那之后再切一刀），
// 所以本文件根本不需要知道页大小；页大小由**取回的条数**决定。
static constexpr size_t kOpiCandidateFetchLimit = SIZE_MAX;

// 候选词：点选后走 Rust 侧 select(**页内**索引) 并提交。
class OpiCandidateWord : public fcitx::CandidateWord {
public:
    OpiCandidateWord(std::string text, size_t index)
        : fcitx::CandidateWord(fcitx::Text(std::move(text))), index_(index) {}
    void select(fcitx::InputContext *ic) const override;

private:
    // 页内索引（与 Rust 侧 CandidateState::select 同语义 = page*PAGE_SIZE+index）。
    size_t index_;
};

inline void OpiCandidateWord::select(fcitx::InputContext *ic) const {
    // 点选是从 UI 线程调进来的第三个入口，同样要挡异常（见 keyEvent 的说明）。
    try {
        const std::string out = take(opi_fcitx5_select(index_));
        if (ic != nullptr && !out.empty()) {
            ic->commitString(out);
        }
        // 选完 Rust 侧缓冲/候选都变了，面板必须同步 —— 否则候选栏会留在屏幕上。
        refreshingPanelFor(ic);
    } catch (const std::exception &e) {
        fprintf(stderr, "fcitx5-opi: select 异常，本次点选作废: %s\n", e.what());
    } catch (...) {
        // 非 std::exception 的异常若逃进 fcitx5 主循环会带走整个守护进程（语言栏一起没）。
        fprintf(stderr, "fcitx5-opi: select 未知异常，本次点选作废\n");
    }
}

/// 当前页候选表：内容永远只有一页（Rust 切好的），但分页接口转发 Rust。
class OpiCandidateList : public fcitx::CommonCandidateList {
public:
    OpiCandidateList(const std::vector<std::string> &texts, int page, int pages) {
        fill(texts);
        page_ = page;
        pages_ = pages;
    }

    // UI 只查这两个 + 调 prev/next（classicui 点击/滚轮、D-Bus NextPage）。
    // totalPages/currentPage/setPage 仍用基类（内部恒单页）：不对外假称有绝对跳页。
    bool hasPrev() const override { return page_ > 0; }
    bool hasNext() const override { return page_ + 1 < pages_; }
    void prev() override { step(-1); }
    void next() override { step(1); }

private:
    void step(int delta);
    void fill(const std::vector<std::string> &texts);

    int page_ = 0;
    int pages_ = 0;
};

/// 翻页：先问 Rust（页码真源），再重建本页。宿主（classicui / dbusfrontend）
/// 调完 prev/next 会自己 `updateUserInterface(InputPanel)`，所以这里不改面板。
inline void OpiCandidateList::step(int delta) {
    try {
        const OpiPageState st = opi_fcitx5_page_step(delta);
        page_ = st.page;
        pages_ = st.pages;
        // 紧跟 page_step 调 candidates 命中它留下的全量缓存（一次取回）。
        fill(parseJsonStringArray(take(opi_fcitx5_candidates(kOpiCandidateFetchLimit))));
    } catch (const std::exception &e) {
        fprintf(stderr, "fcitx5-opi: 翻页异常，本次翻页作废: %s\n", e.what());
    } catch (...) {
        fprintf(stderr, "fcitx5-opi: 翻页未知异常，本次翻页作废\n");
    }
}

/// 普通候选与联想候选共用：页大小 = 实际推过去的条数；选择键 1..n 让 fcitx5
/// 画序号并把数字路由到 OpiCandidateWord::select（**页内索引**）。空表不设
/// pageSize / 高亮位（setGlobalCursorIndex(0) 对空表抛 invalid_argument）。
inline void OpiCandidateList::fill(const std::vector<std::string> &texts) {
    clear();
    if (texts.empty()) {
        return;
    }
    setPageSize(static_cast<int>(texts.size()));
    for (size_t i = 0; i < texts.size(); ++i) {
        append<OpiCandidateWord>(texts[i], i);
    }
    fcitx::KeyList selectionKeys;
    for (size_t i = 0; i < texts.size(); ++i) {
        selectionKeys.push_back(fcitx::Key(std::to_string(i + 1)));
    }
    setSelectionKey(selectionKeys);
    setGlobalCursorIndex(0);
}

static std::unique_ptr<fcitx::CommonCandidateList>
makeCandidateList(const std::vector<std::string> &texts, int page, int pages) {
    return std::make_unique<OpiCandidateList>(texts, page, pages);
}
