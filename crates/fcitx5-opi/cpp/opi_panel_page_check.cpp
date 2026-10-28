// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// §3b UI 翻页接口 —— 从 `opi_panel_driver.cpp` 拆出的**片段**，不是独立驱动
// （那边加上本段后 522 行，撞 500 行门禁）。由驱动主文件 `#include`，编译行只列
// 主文件 —— 别把它加成第二个 TU（同 `opi_ctl_search_driver.cpp` 的先例）。
//
// 验的是 classicui 箭头 / D-Bus NextPage 走的那组 PageableCandidateList 虚函数：
// 它们必须落到与键盘翻页同一份 Rust 页码（`opi_fcitx5_page_step`），并原地重建
// 当前页候选。输入面板刷新由前端在 next()/prev() 后自行 updateUserInterface。

static void runUiPageChecks(TestIC &ic, const std::string &nFirst) {
    std::printf("== 3b. UI 翻页接口（classicui 箭头 / D-Bus NextPage 同一组虚函数）==\n");
    auto list = ic.inputPanel().candidateList();
    auto *pageable = list ? list->toPageable() : nullptr;
    check(pageable != nullptr, "候选表实现了 PageableCandidateList");
    if (pageable != nullptr) {
        const bool hasNext = pageable->hasNext();
        check(!pageable->hasPrev(), "首页 hasPrev() == false（不画上一页箭头）");
        pageable->next();
        const std::string uiAfter = candidateAt(ic, 0);
        std::printf("  UI next(): [0]=%s -> [0]=%s candidates=%d\n",
                    q(nFirst).c_str(), q(uiAfter).c_str(),
                    candidateCount(ic));
        check(hasNext && uiAfter != nFirst,
              "UI next() 换页（与键盘翻页同一 Rust 页码）");
        check(candidateCursor(ic) == 0, "换页后高亮位回到本页首条");
        pageable->prev();
        check(candidateAt(ic, 0) == nFirst, "UI prev() 回到首页首项");
        check(!pageable->hasPrev(), "回到首页后 hasPrev() == false");
    }
}
