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
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <sstream>
#include <string>

#include "opi_driver_common.h"  // 测试器具（TestIC/check/预编辑与候选读回）
#include "opi_panel_page_check.cpp" // §3b UI 翻页（500 行门禁拆出的片段）

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
    check(preeditOf(ic) == "ni'hao", "预编辑串 == 显示串 \"ni'hao\"（2026-10-23 全键盘分词）");
    check(candidateCount(ic) > 0, "候选表非空");
    // 页大小**不写数字**（这里原本是 `<= 8`，第 5 份页大小拷贝）：面板收到的条数
    // 应当 == Rust 侧此刻返回的当前页条数 —— 同一个状态、经 FFI 独立取一次。
    // 写死 8 的两种坏法：Rust 把 PAGE_SIZE 改成 9 ⇒ 断言自己过期、红得**不像**
    // 断言坏了；胶水若把整页截断成 8 条（旧写法正是 take(8)），它**一声不响**。
    // 换成等式后两个方向都抓得住，且不必再跟任何常量走。
    // ⚠️ 这里必须用 SIZE_MAX（不切），**不能**用胶水那个 `kOpiCandidateFetchLimit`：
    // 两边共用同一个上限的话，谁把这个上限改小，面板条数与这里的读数会**一起变小**、
    // 等式照样成立 —— 断言对自己要防的那个改动瞎掉。用「不切」读，截断才看得见。
    const auto pageNow =
        parseJsonStringArray(take(opi_fcitx5_candidates(SIZE_MAX)));
    check(candidateCount(ic) == static_cast<int>(pageNow.size()),
          "候选表条数 == Rust 当前页条数（页大小由 Rust 定，此处不写死）");
    // 候选**序号**：插件调了 setSelectionKey 面板才有得画 —— 探针实测「不设」
    // 时 label 是**空串**（真库，见 docs/install/linux.md 的「候选序号」）。
    // 只钉「以页内数字开头」不钉整串（`"1. "` 的尾随空格是 fcitx5 自己拼的，
    // 跨版本会变）：改回不设 → 空串，第一条就红。逐条覆盖本页（不写死条数）。
    bool labelsOk = candidateCount(ic) > 0;
    for (int i = 0; i < candidateCount(ic); ++i) {
        if (candidateLabelAt(ic, i).rfind(std::to_string(i + 1), 0) != 0) {
            labelsOk = false;
            break;
        }
    }
    check(labelsOk, "候选序号按页内 1..n 编号（label 非空且以数字开头）");
    // 不写死字节数（2026-10-23 起显示串带分词撇号，"ni'hao" = 6 字节）：断言不变量。
    check(ic.inputPanel().preedit().cursor() == static_cast<int>(preeditOf(ic).size()),
          "预编辑光标在末尾（== 串字节长）");
    check(clientPreeditOf(ic) == "ni'hao", "clientPreedit == 显示串（内联预编辑通道）");
    check(candidateCursor(ic) == 0, "候选光标 = 0（候选栏有高亮）");

    // 上面那条等式在**短页**上照样成立，却测不出「胶水把整页截断」（实测：把胶水的
    // 上限改回 8、Rust 页 9 时，`nihao` 只有 4 条候选 ⇒ 等式 4==4 照绿）。判别力要靠
    // **满页**的输入，故另起一例：`ni` 在 luna 词库下满页（页多大就画多少条）。
    // 词库缺失/退化时这一条会变弱（打印里能看出 Rust 页只有几条），但不改判据。
    {
        fcitx::ResetEvent clearEvent(&ic);
        engine.reset(entry, clearEvent);
        type("ni");
        const auto full =
            parseJsonStringArray(take(opi_fcitx5_candidates(SIZE_MAX)));
        std::printf("  'ni' -> candidates=%d（Rust 当前页=%zu）\n",
                    candidateCount(ic), full.size());
        check(candidateCount(ic) == static_cast<int>(full.size()),
              "满页时面板条数 == Rust 当前页条数（胶水截断会在这里现形）");
    }

    std::printf("== 2b. 缓冲非空 + 候选 0 条（候选光标守卫）==\n");
    // 这一节是**变异测试**（实测过两个变异体）：去掉胶水里 `if (!texts.empty())`
    // 那个守卫，下面这步就会抛 std::invalid_argument（`CommonCandidateList: invalid
    // global index`）。**只**去守卫 —— 异常被 keyEvent 的 catch 吞掉，这个键作废、
    // 面板停在**上一次**的候选表上，于是下面两条断言失败（candidates=1 而非 0）。
    // 守卫和 catch **都**去掉 —— 异常就地 terminate，驱动 abort（exit 134），真
    // fcitx5 里的表现就是守护进程消失。而「缓冲非空 + 候选 0 条」是常态，
    // `vvv` 这类拼音本来就查不到候选。
    //
    // 哨兵 2026-10-12 换血（词库整合）：原 `zzzz` 现经**简拼**命中「自自在在」
    // （zzzz 恰是它的声母串）⇒ 前提翻面，本节两条断言红。换 `vvv` —— v 不是任何
    // 声母，简拼展开天然为空、音节回退也不成立（探针实测 0 候选）。同轮换掉
    // `opi_ctl_driver.cpp` §2 的同款哨兵。⚠️ 本文件行数贴着 500 门禁，注释从简。
    {
        fcitx::ResetEvent clearEvent(&ic);
        engine.reset(entry, clearEvent);
    }
    type("vvv");
    std::printf("  'vvv' -> preedit=%s clientPreedit=%s candidates=%d\n",
                q(preeditOf(ic)).c_str(), q(clientPreeditOf(ic)).c_str(),
                candidateCount(ic));
    check(preeditOf(ic) == "v'v'v", "无候选时缓冲照样进预编辑（逐字分词）");
    check(candidateCount(ic) == 0, "确实 0 条候选（走了守卫，没抛异常）");
    check(candidateCursor(ic) == -1, "无候选时不给候选光标（-1）");
    check(clientPreeditOf(ic) == "v'v'v", "无候选时 clientPreedit 照常");

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
    runUiPageChecks(ic, nFirst); // §3b 拆在 opi_panel_page_check.cpp（500 行门禁）
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
    check(preeditOf(ic) == "ni'hao", "翻页不改预编辑串");

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
    check(preeditOf(ic).empty(), "点选后预编辑清空");
    // 联想态（Spec §5）：有续接项时链式候选留面板（缓冲空 + 候选非空），无则清空 —— 都合法，按实际分支断言。
    const bool hasChain = candidateCount(ic) > 0;
    check(static_cast<bool>(ic.inputPanel().candidateList()) == hasChain,
          "点选后：联想态保留链式候选，无联想则面板清空");

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

    std::printf("== 8. 「属性设置」：简拼开关落到引擎了吗 ==\n");
    // 输入挑 **nh**：词库里没有任何以 nh 开头的键，整串/逐音节回退与模糊音都够不着
    // 「你好」—— 它只有简拼一条来路。于是这一段是开关的端到端门禁：
    // Rust `engine.set_jianpin` → C 出口 → 面板候选表。
    auto hasNiHao = [&]() {
        for (int i = 0; i < candidateCount(ic); ++i) {
            if (candidateAt(ic, i) == "你好") {
                return true;
            }
        }
        return false;
    };
    auto clearViaReset = [&]() {
        fcitx::ResetEvent r(&ic);
        engine.reset(entry, r);
    };
    type("nh");
    std::printf("  简拼开：nh -> candidates=%d\n", candidateCount(ic));
    check(hasNiHao(), "简拼开：nh 能查到「你好」");
    opi_fcitx5_set_jianpin(false);
    clearViaReset();
    type("nh");
    std::printf("  简拼关：nh -> candidates=%d\n", candidateCount(ic));
    check(!hasNiHao(), "简拼关：nh 查不到「你好」（开关真的落到了引擎）");
    opi_fcitx5_set_jianpin(true); // 还原（本段在末尾，仍还原，免得将来有人在后面加段）

    std::printf("== 9. 配置文件 → 引擎（设置窗写的就是这个文件）==\n");
    // 这一段走的是**设置窗那条路**：写 `$XDG_CONFIG_HOME/fcitx5/conf/opi_fcitx5.conf`
    // （键名与 desktop/.../Settings.kt 逐字相同）→ `readOpiSettings`（= reloadConfig
    // 调的那个函数）→ 引擎。⚠️ **只在 XDG_CONFIG_HOME 已指到临时目录时跑**：
    // 没设的话 StandardPath 会落到用户真实的 ~/.config —— 不拿用户的配置做实验。
    {
        const char *cfgHome = std::getenv("XDG_CONFIG_HOME");
        if (cfgHome == nullptr || *cfgHome == '\0') {
            skip("未设 XDG_CONFIG_HOME（不拿用户真配置做实验）—— run-harness.sh 会指到临时目录");
        } else {
            const std::string dir = std::string(cfgHome) + "/fcitx5/conf";
            const std::string path = dir + "/opi_fcitx5.conf";
            std::filesystem::create_directories(dir);
            auto writeConf = [&](const char *text) {
                std::ofstream f(path, std::ios::trunc);
                f << text;
            };
            writeConf("Jianpin=False\nFuzzy=True\n");
            OpiSettings s;
            readOpiSettings(s);
            applyOpiSettings(s);
            clearViaReset();
            type("nh");
            std::printf("  配置 Jianpin=False：nh -> candidates=%d\n", candidateCount(ic));
            check(!hasNiHao(), "配置文件里的 Jianpin=False 被读进来并生效了");
            writeConf("Jianpin=True\n");
            readOpiSettings(s);
            applyOpiSettings(s);
            clearViaReset();
            type("nh");
            check(hasNiHao(), "改回 True 同样生效（值是从文件读的，不是一次性）");
            std::filesystem::remove(path);
        }
    }

    std::printf("== 10. Shift 轻按切换中英文（超时那半在 e2e 真守护进程里测）==\n");
    // 直接以「按下/抬起」两个真 KeyEvent 驱动 opi_mode.h 的 handle()。驱动用
    // nullptr 构造引擎 ⇒ 查不到 GlobalConfig 的时限，按「不判超时」处理 ——
    // 超时那半（按住 >250ms 不许切）只能归 e2e 变体 shift（真守护进程里测）。
    auto shiftKey = [&](bool isRelease) {
        fcitx::KeyEvent ev(&ic, fcitx::Key(static_cast<fcitx::KeySym>(0xFFE1)),
                           isRelease);
        engine.keyEvent(entry, ev);
    };
    clearViaReset();
    shiftKey(false);
    shiftKey(true); // 之间没有别的键 ⇒ 轻按
    ic.committed_.clear();
    type("n");
    std::printf("  轻按后 n -> committed=%s\n", q(ic.committed_).c_str());
    check(ic.committed_ == "n", "轻按 Shift 切英文：'n' 直传上屏");
    clearViaReset();
    shiftKey(false);
    shiftKey(true);
    ic.committed_.clear();
    type("n");
    std::printf("  再轻按后 n -> preedit=%s committed=%s\n",
                q(preeditOf(ic)).c_str(), q(ic.committed_).c_str());
    check(ic.committed_.empty() && preeditOf(ic) == "n",
          "再轻按切回拼音：'n' 入缓冲不提交");
    // 组合键对照：中间有别的键按下 ⇒ 不算轻按。见证键 'n' 若提交即误判。
    clearViaReset();
    shiftKey(false);
    press(0x6E); // 'n' 按下（撤销判定）；它自己产生什么提交无所谓
    shiftKey(true);
    clearViaReset(); // 上一个 n 的残留（若有）清掉，只看见证键
    ic.committed_.clear();
    type("n");
    std::printf("  Shift+n 组合后见证键 -> committed=%s preedit=%s\n",
                q(ic.committed_).c_str(), q(preeditOf(ic)).c_str());
    check(ic.committed_.empty() && !preeditOf(ic).empty(),
          "Shift+字母 不算轻按：见证键入缓冲（若误切英文它会直传上屏）");

    std::printf("== 11. 「中/英」状态项：挂载与点击切换 ==\n");
    // activate 才挂状态项 —— core 在每次 activate 之前清空 StatusGroup::InputMethod，
    // 所以「挂上去」这件事只在 activate 里有意义（见 opi_mode.h 的 attach）。
    fcitx::InputContextEvent activateEvent(
        &ic, fcitx::EventType::InputContextSwitchInputMethod);
    engine.activate(entry, activateEvent);
    auto findModeAction = [&]() -> fcitx::Action * {
        for (auto *a : ic.statusArea().allActions()) {
            const std::string t = a->shortText(&ic);
            if (t == "中" || t == "英") {
                return a;
            }
        }
        return nullptr;
    };
    fcitx::Action *modeAction = findModeAction();
    check(modeAction != nullptr, "activate 后状态区里有「中/英」项");
    if (modeAction != nullptr) {
        check(modeAction->shortText(&ic) == "中", "初始显示「中」");
        clearViaReset();
        type("ni");
        modeAction->activate(&ic); // 点它 = 切换（与 Shift 轻按同一条 toggle）
        std::printf("  点击后 mode=%d preedit=%s 文案=%s\n",
                    static_cast<int>(opi_fcitx5_mode()), q(preeditOf(ic)).c_str(),
                    q(modeAction->shortText(&ic)).c_str());
        check(opi_fcitx5_mode() == 1, "点击状态项切到英文（mode==1）");
        check(modeAction->shortText(&ic) == "英", "状态项文案跟着变「英」");
        check(preeditOf(ic).empty(), "切换同时清掉了预编辑");
        modeAction->activate(&ic);
        check(opi_fcitx5_mode() == 0 && modeAction->shortText(&ic) == "中",
              "再点击切回「中」（能进能出）");
    }

    std::printf("== 12. 皮肤：classicui.conf 的改写（保留其它行 + 幂等）==\n");
    {
        const char *cfgHome = std::getenv("XDG_CONFIG_HOME");
        if (cfgHome == nullptr || *cfgHome == '\0') {
            skip("未设 XDG_CONFIG_HOME（不拿用户真配置做实验）—— run-harness.sh 会指到临时目录");
        } else {
            const std::string dir = std::string(cfgHome) + "/fcitx5/conf";
            const std::string path = dir + "/classicui.conf";
            std::filesystem::create_directories(dir);
            {
                std::ofstream f(path, std::ios::trunc);
                f << "Theme=deepin-light\n# 手加的注释\nDarkTheme=deepin-dark\n"
                     "UseDarkTheme=True\n";
            }
            check(opiSkinThemeName(OpiSkin::Keep) == nullptr,
                  "Keep 不映射主题（= 不介入）");
            check(std::string(opiSkinThemeName(OpiSkin::Cinnabar)) == "opi-cinnabar",
                  "枚举 → 主题目录名映射正确（四处同名契约的一处）");
            const bool ok = opiPatchClassicuiTheme(opiSkinThemeName(OpiSkin::Dark));
            std::ifstream in(path);
            std::stringstream ss;
            ss << in.rdbuf();
            const std::string got = ss.str();
            check(ok, "写入返回成功");
            check(got.find("Theme=opi-dark\n") != std::string::npos,
                  "Theme 改为 opi-dark");
            check(got.find("DarkTheme=opi-dark\n") != std::string::npos,
                  "DarkTheme 同步改（皮肤不随明暗跳回系统色）");
            check(got.find("UseDarkTheme=True\n") != std::string::npos &&
                      got.find("# 手加的注释\n") != std::string::npos,
                  "未知键与注释原样保留");
            opiPatchClassicuiTheme(opiSkinThemeName(OpiSkin::Dark));
            std::ifstream in2(path);
            std::stringstream ss2;
            ss2 << in2.rdbuf();
            check(ss2.str() == got, "重复应用同一皮肤幂等（内容逐字节不变）");
            // 文件不存在（首次应用）：两个键都该被补上。
            std::filesystem::remove(path);
            opiPatchClassicuiTheme(opiSkinThemeName(OpiSkin::Paper));
            std::ifstream in3(path);
            std::stringstream ss3;
            ss3 << in3.rdbuf();
            const std::string fresh = ss3.str();
            check(fresh.find("Theme=opi-paper\n") != std::string::npos &&
                      fresh.find("DarkTheme=opi-paper\n") != std::string::npos,
                  "文件不存在时新建并写入两个键");
            std::filesystem::remove(path);
        }
    }

    std::printf("== 13. 简繁切换（Ctrl+Shift+F）与真繁体词库 ==\n");
    // 繁体词库是**可选增强**：数据目录里有 opi/trad.opid 才测得到路由
    // （run-harness.sh 负责放）。缺了 SKIP 而不是拿「回退简体」冒充路由。
    {
        const char *dataHome = std::getenv("XDG_DATA_HOME");
        const std::string tradPath =
            std::string(dataHome != nullptr ? dataHome : "") + "/opi/trad.opid";
        if (dataHome == nullptr || *dataHome == '\0' ||
            !std::filesystem::exists(tradPath)) {
            skip("数据目录里没有 opi/trad.opid（不拿回退简体冒充繁体路由）");
        } else {
            // ctrl 组合键都按 rawKey 的原样构造（KeyEventBase 的构造只对 key()
            // 做 normalize，rawKey 原样保留）——与真前端一致：Ctrl+F 的 keysym
            // 是小写 'f'，Ctrl+Shift+F 是大写 'F'。
            auto hotkey = [&](uint32_t sym, bool shift, bool isRelease) {
                fcitx::KeyStates st(fcitx::KeyState::Ctrl);
                if (shift) {
                    st |= fcitx::KeyState::Shift;
                }
                fcitx::KeyEvent ev(
                    &ic,
                    fcitx::Key(static_cast<fcitx::KeySym>(sym), st), isRelease);
                engine.keyEvent(entry, ev);
            };
            auto ctrlShiftF = [&](bool isRelease) {
                hotkey(FcitxKey_F, true, isRelease);
            };
            auto topCands = [&]() {
                std::string out;
                for (int i = 0; i < std::min(4, candidateCount(ic)); ++i) {
                    out += candidateAt(ic, i);
                }
                return out;
            };
            // 基线：拼音下 hou 的候选与提交
            clearViaReset();
            ic.committed_.clear();
            type("hou");
            const std::string simpCands = topCands();
            std::printf("  拼音 hou 候选: %s\n", simpCands.c_str());
            press(0x20);
            std::printf("  拼音 hou+space -> %s\n", q(ic.committed_).c_str());
            check(ic.committed_ == "后", "基线：拼音下 hou 提交简体「后」");
            // Ctrl+Shift+F → 繁体
            clearViaReset();
            ctrlShiftF(false);
            ctrlShiftF(true);
            check(opi_fcitx5_mode() == 4, "Ctrl+Shift+F 切到繁体（mode==4）");
            clearViaReset();
            ic.committed_.clear();
            type("hou");
            const std::string tradCands = topCands();
            std::printf("  繁体 hou 候选: %s\n", tradCands.c_str());
            // 路由指纹用**候选表成员差异**：真词库下繁体有「後」而简体没有
            // （两库的 top 都可能是「后」——后/後 在繁体里都合法，故不断提交首项，
            // 断「後 只在繁体出现」这个更强的差异判据）。
            check(simpCands.find("後") == std::string::npos &&
                      tradCands.find("後") != std::string::npos,
                  "繁体路由生效：候选里「後」只在繁体模式出现");
            // 数字键选第 2 项：繁体提交「後」/（简体那侧对应是「候」）
            press('2');
            std::printf("  繁体 数字2 -> %s\n", q(ic.committed_).c_str());
            check(ic.committed_ == "後", "繁体模式数字选词提交「後」");
            // Ctrl+F（raw 是小写 'f'）**不许**切 —— 不抢应用的查找键
            {
                const int32_t before = opi_fcitx5_mode();
                hotkey(FcitxKey_f, false, false);
                check(opi_fcitx5_mode() == before,
                      "Ctrl+F 不触发（rawKey 的小写 f 分界生效）");
            }
            // 再按一次切回拼音
            clearViaReset();
            ctrlShiftF(false);
            ctrlShiftF(true);
            check(opi_fcitx5_mode() == 0, "再按 Ctrl+Shift+F 切回拼音");
        }
    }

    std::printf("\n%s（失败 %d 项）\n", g_fail == 0 ? "ALL PASS" : "HAS FAILURES",
                g_fail);
    return g_fail == 0 ? 0 : 1;
}
