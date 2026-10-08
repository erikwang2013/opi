// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
//
// opi_panel_driver / 后续驱动程序共用的测试器具（2026-10-09 从 opi_panel_driver.cpp
// 拆出：那个文件加「Shift/皮肤/简繁」几段后越了 500 行门禁 —— 与 opi_json.h 同一套
// 拆分先例）。**只放器具，不放断言**：各驱动的用例在自己的 main 里。
//
// 用法：驱动先 `#include "opi_fcitx5.cpp"`（真胶水源码），再 include 本文件。

#include <fcitx/inputcontextmanager.h>

#include <cstdio>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <sstream>
#include <string>

namespace {

int g_fail = 0;

struct TestIC : fcitx::InputContext {
    explicit TestIC(fcitx::InputContextManager &m)
        : fcitx::InputContext(m, "opi-test") {
        // 声明 Preedit 能力 —— 胶水的 clientPreedit 通道就挂在这一位上。
        // 不声明的话下面第 2 节的断言测不到东西（守卫会把通道整个关掉）。
        setCapabilityFlags(fcitx::CapabilityFlag::Preedit);
    }
    const char *frontend() const override { return "test"; }
    void commitStringImpl(const std::string &text) override {
        committed_ += text;
    }
    void deleteSurroundingTextImpl(int, unsigned int) override {}
    void forwardKeyImpl(const fcitx::ForwardKeyEvent &) override {}
    void updatePreeditImpl() override {}
    std::string committed_;
};

std::string q(const std::string &s) { return "\"" + s + "\""; }

std::string preeditOf(fcitx::InputContext &ic) {
    return ic.inputPanel().preedit().toString();
}

// 内联预编辑（clientPreedit）——与面板 preedit 是**两条**通道，见胶水里的说明。
std::string clientPreeditOf(fcitx::InputContext &ic) {
    return ic.inputPanel().clientPreedit().toString();
}

// 候选栏高亮位：-1 = 无高亮。对**空**候选表调 setGlobalCursorIndex 会抛
// std::invalid_argument，所以这个值必须只在有候选时被设置。
int candidateCursor(fcitx::InputContext &ic) {
    auto list = ic.inputPanel().candidateList();
    return list ? list->cursorIndex() : -1;
}

// 无候选表 = 0 条（panel.reset() 会把候选表置空，candidateList() 返回 nullptr）。
int candidateCount(fcitx::InputContext &ic) {
    auto list = ic.inputPanel().candidateList();
    return list ? list->size() : 0;
}

std::string candidateAt(fcitx::InputContext &ic, int idx) {
    auto list = ic.inputPanel().candidateList();
    if (!list || idx < 0 || idx >= list->size()) {
        return "<none>";
    }
    return list->candidate(idx).text().toString();
}

// 候选**序号**（label）：面板画的「1. 」来自它，不是候选文本的一部分。
// 插件的 `setSelectionKey` 是它唯一的赋值点（见 opi_fcitx5.cpp 那段注释）。
std::string candidateLabelAt(fcitx::InputContext &ic, int idx) {
    auto list = ic.inputPanel().candidateList();
    if (!list || idx < 0 || idx >= list->size()) {
        return "<none>";
    }
    return list->label(idx).toString();
}

void check(bool ok, const std::string &what) {
    std::printf("  [%s] %s\n", ok ? "PASS" : "FAIL", what.c_str());
    if (!ok) {
        ++g_fail;
    }
}

void skip(const std::string &what) {
    std::printf("  [SKIP] %s\n", what.c_str());
}

} // namespace
