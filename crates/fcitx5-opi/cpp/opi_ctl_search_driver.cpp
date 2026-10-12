// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// §9 search（只读查询，2026-10-13）—— 从 `opi_ctl_driver.cpp` 拆出来的**片段**，不是
// 独立驱动：那边加上本段后到 507 行顶破了 500 行门禁（`crates/opi-ffi/tests/line_limit.rs`），
// 按职责切开。由驱动主文件 `#include`（与它 `#include opi_fcitx5.cpp` 同一先例）；
// `run-harness.sh` 的编译行只列驱动主文件，不列本文件 —— 别把它加成第二个 TU。
//
// 本段验的是**协议层**（C++ 的 err 分类 + JSON 载荷的类型），搜索**语义**在 Rust 侧
// （engine-core 的 symbols 测试 + fcitx5_opi 的 cabi_search_tests），所以这里不写死任何
// 具体符号（数据更新不该让它红）。前提与局限见驱动主文件的头注释。

// [bufferOf] 由调用方传进来：它是驱动 main 里的 lambda（本节原本就在 main 内），
// 这里用模板参数接住，免得把它的实现复制一份（单一真源）。
template <class BufferOf>
static void runCtlSearchChecks(OpiEngine &engine, BufferOf bufferOf) {
    std::printf("== 9. search（只读查询；handleCtlSearch_ = D-Bus Search 落到的那一段）==\n");
    // 本段验的是**协议层**（C++ 的 err 分类 + JSON 载荷的类型），不是搜索语义
    // （语义在 Rust 侧：engine-core 的 symbols 测试 + fcitx5_opi 的 cabi_search_tests
    // 逐条比对引擎）。所以关键字只用来分「有命中 / 无命中」，不写死任何具体符号 ——
    // 数据更新（symbols.tsv 重导）不该让本段变红。
    const std::string stateBefore9 = engine.ctlState_();
    const std::string emptyKw = engine.handleCtlSearch_("");
    check(emptyKw.rfind("err:2:", 0) == 0,
          "空关键字 → err:2（opi-ctl 的 2 = 用法错误；引擎侧 search(\"\") 是「全部条目」，"
          "当搜索结果回出去就是答非所问）：" + q(emptyKw));
    if (opi_fcitx5_search_symbols == nullptr) {
        skip("引擎侧搜索入口未接线（弱符号 nullptr）—— 载荷那几条在本树不可达");
    } else {
        // 正控关键字：§2b 已经证过 "dun" 是活的符号关键字（引擎认领了这三个字母）。
        const std::string hitJson = engine.handleCtlSearch_("dun");
        const auto hits = parseJsonStringArray(hitJson);
        check(!hits.empty(), "有命中的关键字 → 非空 JSON 文本数组：" + q(hitJson));
        // 逐条非空：载荷是「文本数组」，不是「条目数组」（后者会带 name/keywords）。
        bool allNonEmpty = true;
        for (const auto &t : hits) {
            if (t.empty()) allNonEmpty = false;
        }
        check(allNonEmpty, "数组元素全是非空文本（不是条目对象/空串）");
        // 负对照：无命中 = 空数组，**不是** err（「没搜到」与「没搜到东西」是两回事，
        // 但都不是失败 —— 桌面对空数组显示「没有匹配」）。
        const std::string missJson = engine.handleCtlSearch_("zzzznosuchkeywordzzz");
        check(parseJsonStringArray(missJson).empty() && missJson.find("err:") == std::string::npos,
              "无命中 → 空数组而不是 err（实际 " + q(missJson) + "）");
        // 分辨力证明：「关键字真的被传下去了」。空关键字在**本层被拒**（上面那条），
        // 但在 FFI 面是「全部条目」—— 于是「dun 的命中数是全表真子集」证明关键字
        // 在链路上没被丢掉（否则上面那条「非空」可能只是在看全表，全绿却零信息）。
        const auto allTexts = parseJsonStringArray(take(opi_fcitx5_search_symbols(
            reinterpret_cast<const uint8_t *>(""), 0)));
        check(hits.size() < allTexts.size(),
              "命中数是全表的真子集（关键字在 FFI 链路上没被丢）：" +
                  std::to_string(hits.size()) + " < " + std::to_string(allTexts.size()));
        // 只读性：与 §8c 同一条纪律 —— 搜索不碰引擎状态、不碰 IC（表是进程级单例里的
        // 编译内嵌数据），所以没有焦点窗口也能搜；本驱动有 IC，验的是「没动它」。
        check(engine.ctlState_() == stateBefore9, "搜索是只读的：state 整行逐字不变");
        check(bufferOf().empty(), "搜索不碰缓冲/面板");
    }
}
