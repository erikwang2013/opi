// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// OPI fcitx5 面板推进验证驱动 —— **手工 harness，不进 CI**（CI 上没有 fcitx5
// 的头/库）。跑法见同目录 run-harness.sh 或 README「验证 harness」一节。
//
// 直接 #include 胶水源码 —— 验的就是即将装进插件的那份代码，不是它的复制品。
// 用真实 fcitx5 5.1.12 头与库，构造真实 fcitx::InputContext 子类，
// 按键后读回 ic.inputPanel() 的预编辑串与候选表。不需要 dbus、不需要守护进程。
//
// 前置条件：
//   - fcitx5 5.1.12 头与库（FCITX5_HEADERS / FCITX5_LIBS，见 run-harness.sh）
//   - Rust cdylib 已构建：cargo build --release -p fcitx5_opi
//   - $XDG_DATA_HOME/opi/luna.opid（可选；缺失时回退 Rust 内置 35 词库，
//     候选会明显变少，翻页那节会打印 SKIP 而不是 FAIL）
//
// 局限（如实记录）：这只证明「引擎把状态推进了面板」，不证明某个具体前端能
// 把它画出来 —— 那需要真前端，见同目录 opi_e2e.py 与 README 的 dbus 限制。

#include "opi_fcitx5.cpp"

#include <fcitx/inputcontextmanager.h>

#include <cstdio>
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

int main() {
    fcitx::InputContextManager manager;
    // IC 故意建在堆上且不释放：~InputContext 断言 manager 已先把它标记为
    // destroyed（实测 `Assertion d_ptr->destroyed_ failed`），栈对象会先于
    // manager 析构而踩断言。一次性驱动，泄漏一个 IC 无所谓。
    TestIC &ic = *(new TestIC(manager));
    OpiEngine engine(nullptr); // instance_ 在胶水里只存不用
    fcitx::InputMethodEntry entry("opi", "OPI", "zh_CN", "opi_fcitx5");

    auto press = [&](uint32_t sym) {
        fcitx::KeyEvent event(&ic, fcitx::Key(static_cast<fcitx::KeySym>(sym)));
        engine.keyEvent(entry, event);
    };
    auto type = [&](const char *seq) {
        for (const char *p = seq; *p; ++p) {
            press(static_cast<uint32_t>(*p));
        }
    };

    std::printf("== 1. 初始面板 ==\n");
    std::printf("  preedit=%s candidates=%d committed=%s\n",
                q(preeditOf(ic)).c_str(), candidateCount(ic),
                q(ic.committed_).c_str());
    check(preeditOf(ic).empty() && candidateCount(ic) == 0,
          "初始面板为空（预编辑空 + 无候选）");
    check(ic.inputPanel().empty(), "InputPanel::empty() 为真");

    std::printf("== 2. 逐键输入 n i h a o ==\n");
    for (const char *p = "nihao"; *p; ++p) {
        press(static_cast<uint32_t>(*p));
        std::printf("  '%c' -> preedit=%s candidates=%d cand[0]=%s\n", *p,
                    q(preeditOf(ic)).c_str(), candidateCount(ic),
                    q(candidateAt(ic, 0)).c_str());
    }
    check(preeditOf(ic) == "nihao", "预编辑串 == 缓冲 \"nihao\"");
    check(candidateCount(ic) > 0, "候选表非空");
    check(candidateCount(ic) <= 8, "候选表 <= 一页 8 条");
    check(ic.inputPanel().preedit().cursor() == 5, "预编辑光标在末尾（字节 5）");
    check(clientPreeditOf(ic) == "nihao", "clientPreedit == 缓冲（内联预编辑通道）");
    check(candidateCursor(ic) == 0, "候选光标 = 0（候选栏有高亮）");

    std::printf("== 2b. 缓冲非空 + 候选 0 条（候选光标守卫）==\n");
    // 这一节是**变异测试**（实测过两个变异体）：去掉胶水里 `if (!texts.empty())`
    // 那个守卫，下面这步就会抛 std::invalid_argument（`CommonCandidateList: invalid
    // global index`）。**只**去守卫 —— 异常被 keyEvent 的 catch 吞掉，这个键作废、
    // 面板停在**上一次**的候选表上，于是下面两条断言失败（candidates=1 而非 0）。
    // 守卫和 catch **都**去掉 —— 异常就地 terminate，驱动 abort（exit 134），真
    // fcitx5 里的表现就是守护进程消失。而「缓冲非空 + 候选 0 条」是常态，
    // `v` / `zzzz` 这类拼音本来就查不到候选。
    {
        fcitx::ResetEvent clearEvent(&ic);
        engine.reset(entry, clearEvent);
    }
    type("zzzz");
    std::printf("  'zzzz' -> preedit=%s clientPreedit=%s candidates=%d\n",
                q(preeditOf(ic)).c_str(), q(clientPreeditOf(ic)).c_str(),
                candidateCount(ic));
    check(preeditOf(ic) == "zzzz", "无候选时缓冲照样进预编辑");
    check(candidateCount(ic) == 0, "确实 0 条候选（走了守卫，没抛异常）");
    check(candidateCursor(ic) == -1, "无候选时不给候选光标（-1）");
    check(clientPreeditOf(ic) == "zzzz", "无候选时 clientPreedit 照常");

    std::printf("== 3. 翻页（PageDown/PageUp 走 Rust 页码）==\n");
    // "nihao" 只有一页；换单字母缓冲 "n"（满 8 条，说明还有下一页）。
    {
        fcitx::ResetEvent clearEvent(&ic);
        engine.reset(entry, clearEvent);
    }
    press(static_cast<uint32_t>('n'));
    std::printf("  缓冲 n: candidates=%d cand[0]=%s\n", candidateCount(ic),
                q(candidateAt(ic, 0)).c_str());
    const std::string nFirst = candidateAt(ic, 0);
    const int nCount = candidateCount(ic);
    press(0xff56 /* PageDown */);
    std::printf("  PageDown: [0]=%s -> [0]=%s\n", q(nFirst).c_str(),
                q(candidateAt(ic, 0)).c_str());
    check(candidateAt(ic, 0) != nFirst, "PageDown 换页（首个候选变化）");
    press(0xff55 /* PageUp */);
    std::printf("  PageUp:   -> [0]=%s\n", q(candidateAt(ic, 0)).c_str());
    check(candidateAt(ic, 0) == nFirst, "PageUp 回到首页首个候选");
    check(candidateCount(ic) == nCount, "翻页后本页条数不变");

    std::printf("  --- 以下为缓冲 nihao 的翻页（单页时应无变化）---\n");
    {
        fcitx::ResetEvent clearEvent(&ic);
        engine.reset(entry, clearEvent);
    }
    type("nihao");
    std::printf("  缓冲 nihao: candidates=%d\n", candidateCount(ic));
    const std::string beforeFirst = candidateAt(ic, 0);
    const int beforeCount = candidateCount(ic);
    press(0xff56 /* PageDown */);
    const std::string afterFirst = candidateAt(ic, 0);
    std::printf("  PageDown: [0]=%s (%d 条) -> [0]=%s (%d 条)\n",
                q(beforeFirst).c_str(), beforeCount, q(afterFirst).c_str(),
                candidateCount(ic));
    if (beforeCount == 8 && afterFirst != beforeFirst) {
        check(true, "PageDown 换页（候选首项变化，Rust 页码推进）");
        press(0xff55 /* PageUp */);
        std::printf("  PageUp:   -> [0]=%s\n", q(candidateAt(ic, 0)).c_str());
        check(candidateAt(ic, 0) == beforeFirst, "PageUp 回到首页首项");
    } else {
        skip("缓冲 \"nihao\" 在本次词库下只有一页（候选 <8 条），翻页无从观察");
    }
    check(preeditOf(ic) == "nihao", "翻页不改预编辑串");

    std::printf("== 4. 点击候选（CandidateWord::select）==\n");
    ic.committed_.clear();
    auto list = ic.inputPanel().candidateList();
    const std::string clicked = candidateAt(ic, 0);
    check(static_cast<bool>(list), "面板上有候选表对象");
    list->candidate(0).select(&ic);
    std::printf("  点选 %s -> committed=%s preedit=%s candidates=%d\n",
                q(clicked).c_str(), q(ic.committed_).c_str(),
                q(preeditOf(ic)).c_str(), candidateCount(ic));
    check(ic.committed_ == clicked, "点选提交了该候选文本");
    check(preeditOf(ic).empty() && candidateCount(ic) == 0, "点选后面板清空");

    std::printf("== 5. 空格提交 ==\n");
    ic.committed_.clear();
    type("hao");
    std::printf("  'hao' -> preedit=%s candidates=%d\n", q(preeditOf(ic)).c_str(),
                candidateCount(ic));
    press(0x20 /* space */);
    std::printf("  space -> committed=%s preedit=%s candidates=%d\n",
                q(ic.committed_).c_str(), q(preeditOf(ic)).c_str(),
                candidateCount(ic));
    check(!ic.committed_.empty(), "空格提交了文本");
    check(preeditOf(ic).empty() && candidateCount(ic) == 0, "提交后面板清空");
    check(clientPreeditOf(ic).empty(), "提交后 clientPreedit 也清空");

    std::printf("== 6. reset（失焦/切输入法）==\n");
    type("ni");
    std::printf("  输入后 preedit=%s candidates=%d\n", q(preeditOf(ic)).c_str(),
                candidateCount(ic));
    check(!preeditOf(ic).empty(), "reset 前面板确有内容");
    fcitx::ResetEvent resetEvent(&ic);
    engine.reset(entry, resetEvent);
    std::printf("  reset 后 preedit=%s candidates=%d panelEmpty=%d\n",
                q(preeditOf(ic)).c_str(), candidateCount(ic),
                ic.inputPanel().empty() ? 1 : 0);
    check(preeditOf(ic).empty() && candidateCount(ic) == 0, "reset 后面板清空");
    check(clientPreeditOf(ic).empty(), "reset 后 clientPreedit 也清空");
    check(ic.inputPanel().empty(), "reset 后 InputPanel::empty() 为真");

    std::printf("== 7. 直通键不污染面板 ==\n");
    ic.committed_.clear();
    press(0x20 /* 空缓冲空格：直传客户端 */);
    std::printf("  空缓冲 space -> committed=%s preedit=%s candidates=%d\n",
                q(ic.committed_).c_str(), q(preeditOf(ic)).c_str(),
                candidateCount(ic));
    check(ic.committed_ == " ", "空缓冲空格直传（提交一个空格）");
    check(preeditOf(ic).empty() && candidateCount(ic) == 0, "直通后面板仍为空");

    std::printf("\n%s（失败 %d 项）\n", g_fail == 0 ? "ALL PASS" : "HAS FAILURES",
                g_fail);
    return g_fail == 0 ? 0 : 1;
}
