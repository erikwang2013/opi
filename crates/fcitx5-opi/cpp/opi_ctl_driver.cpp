// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// W1「直接确认」+ opi-ctl 动作分发驱动 —— **手工 harness，不进 CI**（CI 上没有
// fcitx5 的头/库）。跑法见同目录 run-harness.sh。
//
// 直接 #include 胶水源码（验的就是即将装进插件的那份代码）。三件事，都是**跨 agent
// 契约**而不是内部细节：
//   1. W1 覆盖面：切输入法 / 切英文 → **按模式分叉**：拼音/繁体提交缓冲原文、符号
//      模式不上关键字（2026-10-10 深夜用户裁决「切英文 = 默认是拼音，不要是候选
//      词」+ 同夜追加的模式分叉；白天那版「有候选上首候选」作废 —— 订正时间线、
//      正控与负对照见 §1/§2b）；焦点移出 / reset → **静默清空，绝不提交**
//      （用户裁决，与上一条成对照）。
//   2. **顺序契约**（lead 2026-10-10）：符号浏览进行中切走 ⇒ 先退浏览（恢复进入
//      浏览前的真缓冲）→ 再按 W1 提交。顺序反了就是把浏览态的标题串当用户输入
//      交出去 —— 本驱动用假 hook 把这条钉死（真引擎入口是弱符号，本分支未落地）。
//   3. opi-ctl 的动作分发面（handleCtlAction_，与 D-Bus 接收侧同一个函数）：五个
//      动作的返回值协议 "ok" / "err:<码>:<原因>"，以及「未接线 / 数据缺失」的区分。
//   4. opi-ctl state（只读查询，2026-10-10 用户裁决）：ctlState_ = D-Bus State 方法
//      回复出去的那一行 —— 冻结格式整行（五格；im 在本驱动恒为空串，真值面归
//      e2e 变体 ctl）、trad/lang 两格跟着状态变、以及**只读性**（全角格是双翻面
//      读的，得证明没把状态读漂）。
//   5. opi-ctl search（只读查询，2026-10-13）：**拆在 opi_ctl_search_driver.cpp**（本文件 500 行门禁）。handleCtlSearch_ = D-Bus Search 落到
//      的那一段（与 §7 调 handleCtlAction_ 同一先例）—— 空关键字 → err:2（用法错误）、
//      有/无命中的载荷类型、以及**关键字真的被传下去了**（命中数是全表真子集）。
//      搜索**语义**在 Rust 侧（engine-core 的 symbols 测试 + fcitx5_opi 的
//      cabi_search_tests），本段只验 C++ 协议层，故不写死任何具体符号（数据更新
//      不该让它红）。
//   6. opi-ctl mode（模式循环，2026-10-14 用户裁决）：**拆在 opi_ctl_mode_driver.cpp**
//      （同一个 500 行门禁）。token（pinyin/en26）与引擎读回逐条一致、双拼档的
//      收编与「进拼音摆回全拼」，都在那边。
//
// 局限（如实记，别把它读成更强的结论）：
//   · `OpiEngine engine(nullptr)` —— instance_ 为空：**D-Bus 注册本身**（真会话总线
//     上挂 /opi）与「作用于当前 IC」（mostRecentInputContext）测不到，归 e2e 变体
//     ctl（真守护进程 + 私有会话总线）；opi-ctl 客户端的退出码映射归它的 cargo 测试。
//   · state 的「没有 IC 也照常回一行」是**结构保证**（stateLine_ 不取 IC，见
//     opi_engine.h），本驱动有 IC，测不到「没有」那一半 —— 别再补一条假装测了。
//   · 符号浏览**引擎侧**未落地时弱符号是 nullptr：假 hook 顶替它验顺序契约，真 hook
//     只验「未接线时的一次性诊断」。引擎侧落地后那两条自动 SKIP（见 §7 FIN）。
#include "opi_fcitx5.cpp"

#include <fcitx/inputcontextmanager.h>

#include <cstdio>
#include <string>
#include <sys/stat.h> // chmod：§7d-2 的「写盘失败」形态
#include <unistd.h>   // dup/dup2：捕获 stderr（Pin A 的诊断断言）

#include "opi_driver_common.h" // 测试器具（TestIC/check/预编辑与候选读回）
#include "opi_ctl_search_driver.cpp" // §9 search 整段（500 行门禁拆出去的；要在器具之后）
#include "opi_ctl_mode_driver.cpp"   // §10 mode（模式循环）整段（同上，2026-10-14 拆出）

int main() {
    fcitx::InputContextManager manager;
    // 同 opi_panel_driver：IC 建在堆上不释放（~InputContext 断言 manager 已先标记
    // 它 destroyed）。一次性驱动，泄漏一个 IC 无所谓。
    TestIC &ic = *(new TestIC(manager));
    OpiEngine engine(nullptr);
    fcitx::InputMethodEntry entry("opi", "OPI", "zh_CN", "opi_fcitx5");

    auto press = [&](uint32_t sym) {
        fcitx::KeyEvent ev(&ic, fcitx::Key(static_cast<fcitx::KeySym>(sym)));
        engine.keyEvent(entry, ev);
    };
    auto type = [&](const char *seq) {
        for (const char *p = seq; *p; ++p) {
            press(static_cast<uint32_t>(*p));
        }
    };
    auto resetVia = [&]() {
        fcitx::ResetEvent r(&ic);
        engine.reset(entry, r);
    };
    auto takeEvent = [&](fcitx::EventType t) {
        fcitx::InputContextEvent ev(&ic, t);
        engine.deactivate(entry, ev);
    };
    auto switchIM = [&]() {
        takeEvent(fcitx::EventType::InputContextSwitchInputMethod);
    };
    auto focusOut = [&]() { takeEvent(fcitx::EventType::InputContextFocusOut); };
    auto bufferOf = [&]() { return take(opi_fcitx5_buffer()); };
    // 候选面**从 FFI 读**（经 JSON），不从面板读：§6 要拿它当「W1 提交了什么」的
    // 独立判据，两者同源会更不容易发现胶水接错。SIZE_MAX = 不切（同面板驱动的理由）。
    auto firstCand = [&]() -> std::string {
        auto v = parseJsonStringArray(take(opi_fcitx5_candidates(SIZE_MAX)));
        return v.empty() ? std::string("<none>") : v.front();
    };
    auto commitNow = [&]() {
        const std::string s = ic.committed_;
        ic.committed_.clear();
        return s;
    };
    // stderr 捕获（Pin A：未接线诊断里必须写清**期望的导出符号名**）。照 opi_status
    // 驱动之外的第一处用法：临时把 fd 2 接到 tmpfile，跑完还原 —— stderr 无缓冲，
    // 写下去即刻落文件。别的段落照常打真 stderr。
    auto captureStderr = [](auto &&fn) -> std::string {
        std::FILE *tmp = std::tmpfile();
        const int saved = dup(fileno(stderr));
        dup2(fileno(tmp), fileno(stderr));
        fn();
        std::fflush(stderr);
        dup2(saved, fileno(stderr));
        close(saved);
        std::rewind(tmp);
        std::string out;
        char buf[512];
        size_t n;
        while ((n = std::fread(buf, 1, sizeof buf, tmp)) > 0) {
            out.append(buf, n);
        }
        std::fclose(tmp);
        return out;
    };

    // §1..§6 一律用**空操作假 hook**：真 hook 未接线时每次调用都会打一次性诊断，
    // 那会让 §7 的 Pin A 断言（首次调用的诊断）失去意义 —— 而且 W1 的覆盖面本来就
    // 与浏览入口的实现无关。
    opiSymbolBrowseHook() = [](int32_t) { return 0; };

    std::printf("== 1. W1：切输入法 = 有候选也上**原文**（2026-10-10 深夜改写）==\n");
    // 订正时间线（本段原断言「提交首候选」）：2026-10-10 白天 lead 裁决「有候选上
    // 首候选」；同夜用户裁决「切英文 = 默认是拼音，不要是候选词」（移动端先改并
    // 实证 [英文] → 字段收 ni'hao），桌面统一为一律原文 ⇒ 本段随语义**有意**改写：
    // 正控 = 提交的就是缓冲原文（逐字），负对照 = **不是**首候选（把 select(0) 加
    // 回去只应红本段负对照与 §6 那两条，别的地方不该动）。同夜追加：一律原文
    // **按模式分叉** —— 本段是拼音面，符号模式那半（不上关键字）见 §2b。
    resetVia();
    commitNow();
    type("nihao");
    const std::string nihao0 = firstCand();
    const std::string raw0 = bufferOf();
    check(candidateCount(ic) > 0 && nihao0 != "<none>",
          "「nihao」有候选（被测前提）");
    check(raw0 == "nihao", "原文 = 打进去的串逐字（被测前提，实际 " + q(raw0) + "）");
    check(raw0 != nihao0, "前提：原文与首候选本就不同（否则负对照无分辨力；首候选 "
          + q(nihao0) + "）");
    switchIM();
    const std::string c1 = commitNow();
    check(c1 == raw0, "切输入法：提交缓冲原文 " + q(raw0) + "（不挑候选）");
    check(c1 != nihao0, "负对照：提交的**不是**首候选 " + q(nihao0));
    check(preeditOf(ic).empty() && candidateCount(ic) == 0 && bufferOf().empty(),
          "提交后引擎与面板都清干净（无缓冲、无预编辑、无候选）");

    std::printf("== 2. W1：无候选 = 提交原文（与 §1 同一条路 —— 分档已取消）==\n");
    // 哨兵 2026-10-12 换血：原 `zzzz` 现经简拼命中「自自在在」⇒ 前提翻面。
    // 换 `vvv`（v 非声母，探针实测 0 候选；同 opi_panel_driver.cpp §2b 与
    // cabi_fullwidth 的哨兵）。
    resetVia();
    commitNow();
    type("vvv");
    check(bufferOf() == "vvv" && candidateCount(ic) == 0,
          "「vvv」在缓冲里且零候选（被测前提）");
    switchIM();
    check(commitNow() == "vvv",
          "切输入法：提交原文（别把用户打的丢掉；无候选只是 §1 同一实现的退化情形）");
    check(preeditOf(ic).empty(), "提交后面板同样清干净");

    std::printf("== 2b. W1：符号模式 = 不上关键字（模式分叉的另一半，与 §1/§2 成对）==\n");
    // 同夜追加裁决（2026-10-10）：「一律原文」**按模式分叉** —— 拼音/繁体（字母
    // 缓冲）上原文（§1/§2 那两段），符号模式的关键字（`、,` 这类，缓冲里是
    // `dun`/`;sh` 这种）**不上屏**：切走只清缓冲、提交空串。理由：关键字不是待
    // 上屏文本，塞进文档比丢缓冲更坏。正控 = 提交空串；负对照 = 提交里**不许**
    // 出现关键字（把符号分支删掉只应红本段——`dun` 会被当原文交出去）。
    // 判据是**引擎当前 mode**（FFI 已有读口 opi_fcitx5_mode()，不需要新出口）。
    resetVia();
    commitNow();
    opi_fcitx5_switch_mode(kModeSymbol);
    type("dun");
    const std::string kw0 = bufferOf();
    check(kw0 == "dun", "符号模式：关键字进了缓冲（被测前提，实际 " + q(kw0) + "）");
    switchIM();
    const std::string kwCommit = commitNow();
    check(kwCommit.empty(),
          "切走：符号关键字**不上屏**（提交空串；实际 " + q(kwCommit)
              + "；塞进文档比丢缓冲更坏）");
    check(preeditOf(ic).empty() && bufferOf().empty() && candidateCount(ic) == 0,
          "切走：缓冲/候选/预编辑仍清干净");
    opi_fcitx5_switch_mode(kModePinyin); // 复位：§3 的「空缓冲」前提别沾上符号模式

    std::printf("== 3. W1：空缓冲 = 什么都不提交 ==\n");
    resetVia();
    commitNow();
    switchIM();
    check(commitNow().empty(), "空缓冲切走不提交（commitString(\"\") 没有意义）");

    std::printf("== 4. 焦点移出 = 静默清空（绝不提交 —— 与 §1 成对照）==\n");
    resetVia();
    commitNow();
    type("nihao");
    check(candidateCount(ic) > 0, "有候选 + 有缓冲（被测前提）");
    focusOut();
    check(commitNow().empty(), "焦点移出：一个字都不提交（用户裁决 2026-10-10）");
    check(preeditOf(ic).empty() && candidateCount(ic) == 0 && bufferOf().empty(),
          "焦点移出：面板与引擎都静默清空");

    std::printf("== 5. reset（InputContextReset）= 同 §4 ==\n");
    commitNow();
    type("nihao");
    resetVia();
    check(commitNow().empty(), "reset 不提交");
    check(preeditOf(ic).empty() && candidateCount(ic) == 0, "reset 静默清空");

    std::printf("== 6. 顺序契约：浏览中切走 ⇒ 先退浏览（恢复真缓冲）→ 再按 W1 提交 ==\n");
    // 假 hook 把真引擎的浏览态在**这一侧**模拟出来：进入时保存真缓冲、缓冲换成
    // 「标题串」（真引擎里浏览态的候选面就是类目标题），退出时同步恢复真缓冲
    // （opi_rust_ffi.h 的同步契约）。这样就能判两件事：调用**顺序**，以及提交出去
    // 的到底是恢复后的真缓冲还是浏览态那串。
    // 2026-10-10 深夜随 W1 语义订正改写：提交物从「真缓冲的首候选」改成「真缓冲
    // **原文**」；负对照留两条 —— 不是真缓冲首候选（select(0) 加回来这条红）、
    // 不是浏览态标题串的首候选（顺序反了这条红）。
    resetVia();
    type("hao");
    const std::string hao0 = firstCand();
    check(candidateCount(ic) > 0 && hao0 != "<none>", "用户真缓冲「hao」有候选（被测前提）");
    check(hao0 != "hao", "前提：首候选与缓冲原文不同（否则 §6 的负对照无分辨力）");
    std::string log;
    std::string browseSaved; // 浏览入口保存的真缓冲
    std::string exitSaw;     // 退出钩子**当时看到**的缓冲
    bool browsing = false;
    opiSymbolBrowseHook() = [&](int32_t action) -> int32_t {
        const bool toExit = action == kOpiSymbolBrowseExit
                            || (action == kOpiSymbolBrowseToggle && browsing);
        if (!toExit) {
            if (browsing) {
                return 1; // 已在浏览态又收到 Enter/Toggle：无动作
            }
            log += "enter;";
            browseSaved = bufferOf();
            browsing = true;
            opi_fcitx5_clear();
            type("fuhao"); // 浏览态缓冲 = 标题串（首候选 ≈「符号」）
            return 1;
        }
        if (!browsing) {
            return 0; // 幂等：非浏览态退出是 no-op
        }
        log += "exit;";
        exitSaw = bufferOf();
        browsing = false;
        opi_fcitx5_clear();
        type(browseSaved.c_str()); // 恢复真缓冲（同步完成）
        return 0;
    };
    // insert（2026-10-12 加）：本驱动 instance_ = nullptr ⇒「当前 IC」拿不到 ——
    // 这里钉两条**边界**（空文本 5 / 无焦点框 4）；成功路径（真提交进 IC）归 e2e 变体 ctl。
    {
        const std::string e1 = engine.handleCtlAction_("insert:");
        check(e1 == "err:5:insert 的文本为空", "insert 空文本 → err:5（实际 " + q(e1) + "）");
        const std::string e2 = engine.handleCtlAction_("insert:你好");
        check(e2.rfind("err:4:", 0) == 0,
              "无焦点 IC → err:4（实际 " + q(e2) + "）");
    }

    const std::string enterReply = engine.handleCtlAction_("symbols");
    check(enterReply == "ok", "进入浏览（symbols 动作，假 hook 返回 1）应答 ok");
    const std::string junk0 = firstCand();
    std::printf("  浏览态首候选=%s 真缓冲首候选=%s（负对照的两个靶子）\n",
                q(junk0).c_str(), q(hao0).c_str());
    switchIM(); // 切走：契约要求「先退浏览 → 再 W1 提交」
    const std::string committed = commitNow();
    check(log == "enter;exit;", "调用顺序 enter → exit（实际 [" + log + "]）");
    check(exitSaw == "fuhao",
          "退出钩子看到的仍是浏览态缓冲（证明 exit 发生在 W1 取缓冲**之前**）");
    check(committed == browseSaved,
          "提交的是恢复后的真缓冲**原文**「hao」（不挑候选）");
    check(committed != hao0, "负对照：提交的**不是**真缓冲的首候选 " + q(hao0));
    check(committed != junk0, "负对照：提交的**不是**浏览态标题串的首候选 " + q(junk0));
    check(preeditOf(ic).empty() && bufferOf().empty(), "提交后引擎与面板都清干净");

    // §7 的 lang（切英文）会走 opiDirectConfirm —— 换回空操作 hook，别让 §6 的
    // 假浏览态漏进下一段。
    opiSymbolBrowseHook() = [](int32_t) { return 0; };
    opi_fcitx5_switch_mode(kModePinyin); // 前面几段的模式残留归零

    std::printf("== 7. opi-ctl 动作分发（handleCtlAction_ = D-Bus 接收侧同一函数）==\n");
    // 状态项要 activate 才挂得上（core 在每次 activate 前清空该组）。驱动用
    // nullptr 构造引擎 ⇒ 下面 stderr 里会**预期地**出现两行「opi-ctl 通道没接上」
    // （拿不到 instance）：D-Bus 注册归 e2e 变体 ctl，这里不是故障。
    fcitx::InputContextEvent activateEvent(
        &ic, fcitx::EventType::InputContextSwitchInputMethod);
    engine.activate(entry, activateEvent);
    std::printf("  （上面 stderr 的「opi-ctl 通道没接上」是预期的：驱动无 instance）\n");
    auto findAction = [&](const char *a, const char *b) -> fcitx::Action * {
        for (auto *act : ic.statusArea().allActions()) {
            const std::string t = act->shortText(&ic);
            if (t == a || t == b) {
                return act;
            }
        }
        return nullptr;
    };

    std::printf("-- 7a 未知动作 = err:5（动作串是冻结契约，新增要先问 lead）--\n");
    const std::string bogus = engine.handleCtlAction_("bogus");
    check(bogus.rfind("err:5:", 0) == 0 && bogus.find("bogus") != std::string::npos,
          "未知动作回 err:5 且带上动作名：" + q(bogus));

    std::printf("-- 7b 简繁动作 = 与热键共用 opiToggleTraditional，含状态项刷新 --\n");
    check(opi_fcitx5_mode() == kModePinyin, "前置：当前是拼音模式");
    const std::string tradReply = engine.handleCtlAction_("trad");
    fcitx::Action *fanjian = findAction("简", "繁");
    check(tradReply == "ok" && opi_fcitx5_mode() == kModeTraditional,
          "trad：应答 ok 且模式切到繁体");
    check(fanjian != nullptr && fanjian->shortText(&ic) == "繁",
          "trad：「简/繁」状态项文案跟着变（「首版漏刷新」那个坑）");
    check(engine.handleCtlAction_("trad") == "ok" && opi_fcitx5_mode() == kModePinyin,
          "trad 再来一次切回拼音（来回切，不是单向）");
    check(fanjian != nullptr && fanjian->shortText(&ic) == "简", "「简/繁」文案切回「简」");

    std::printf("-- 7c lang = 切英文 + fullwidth 机械全角 --\n");
    resetVia();
    commitNow();
    type("nihao");
    check(bufferOf() == "nihao" && candidateCount(ic) > 0, "前置：有待定拼音");
    check(engine.handleCtlAction_("lang") == "ok" && opi_fcitx5_mode() == kModeEnglish,
          "lang：应答 ok 且模式切到英文（不是 fcitx5-remote -t，见 opi_engine.h）");
    // ⚠️ 「lang 顺带把待定拼音提交掉（W1）」这一半在**本驱动不可达**：
    // handleCtlAction_ 的「当前 IC」取自 instance_->mostRecentInputContext()，而这里
    // 用 nullptr 构造引擎 ⇒ opiDirectConfirm(ic) 在空指针守卫处就返回了。本段只验
    // 状态后果（切模式清缓冲）；**提交**那半归 e2e 变体 ctl —— 那边在真守护进程里
    // 先打 nihao、再跑 opi-ctl lang，断言 IC 真收到 CommitString「nihao」（缓冲
    // 原文；2026-10-10 深夜语义订正，见 opi_e2e_ctl.py 那条断言的订正注记）。
    check(bufferOf().empty(), "lang：切模式清了缓冲（提交那一半需要真 IC，见注释）");
    fcitx::Action *modeItem = findAction("中", "英");
    check(modeItem != nullptr && modeItem->shortText(&ic) == "英",
          "lang：「中/英」状态项文案跟着变");
    commitNow();
    press(0x2C); // ','
    check(commitNow() == ",", "英文模式半角直传：',' → \",\"");
    check(engine.handleCtlAction_("fullwidth") == "ok", "fullwidth：应答 ok");
    commitNow();
    press(0x2C);
    check(commitNow() == "，", "全角开关生效（英文模式是**机械全角**，不是中文标点）");
    check(engine.handleCtlAction_("fullwidth") == "ok", "fullwidth 再点一次关掉");
    commitNow();
    press(0x2C);
    check(commitNow() == ",", "关掉后回到半角");
    check(engine.handleCtlAction_("lang") == "ok" && opi_fcitx5_mode() == kModePinyin,
          "lang 再来一次切回拼音");
    check(modeItem != nullptr && modeItem->shortText(&ic) == "中", "「中/英」文案切回「中」");

    std::printf("-- 7d punct = 改配置面（设置窗同一条读路径）--\n");
    const char *cfgHome = std::getenv("XDG_CONFIG_HOME");
    if (cfgHome == nullptr || *cfgHome == '\0') {
        skip("未设 XDG_CONFIG_HOME（不拿用户真配置做实验）—— run-harness.sh 会指到临时目录");
    } else {
        OpiSettings readBack;
        readOpiSettings(readBack);
        const bool before = *readBack.chinesePunct;
        check(engine.handleCtlAction_("punct") == "ok", "punct：应答 ok");
        readOpiSettings(readBack);
        check(*readBack.chinesePunct != before, "punct：配置文件里的 ChinesePunct 翻面了");
        check(engine.handleCtlAction_("punct") == "ok", "punct 再点一次还原");
        readOpiSettings(readBack);
        check(*readBack.chinesePunct == before, "还原（本段不留副作用）");
        // 配置面翻面的**行为**后果（引擎侧标点表）在面板驱动与 e2e 里测，这里只管
        // 「改的是落盘的那个值」—— 内存里翻个 bool 才是错的（见 opi_engine.h）。
        //
        // 7d-2 写盘失败 = err:5 + 回滚（2026-10-10 裁决：失败要可见，别静默）。
        // 造失败：conf 文件与目录都改只读 —— safeSaveAsIni 无论「就地截断」
        // 还是「临时文件 + rename」形态都会 EACCES（两种都堵死，不押注实现）。
        const std::string confDir = std::string(cfgHome) + "/fcitx5/conf";
        const std::string conf = confDir + "/opi_fcitx5.conf";
        const std::string lineBefore = engine.ctlState_();
        const bool locked =
            chmod(conf.c_str(), 0444) == 0 && chmod(confDir.c_str(), 0555) == 0;
        if (!locked) {
            skip("chmod 只读失败（权限？）—— 写盘失败的那条路在本环境测不了");
        } else {
            const std::string failReply = engine.handleCtlAction_("punct");
            check(failReply.rfind("err:5:", 0) == 0
                      && failReply.find("保存失败") != std::string::npos,
                  "写盘失败 → err:5 且说清原因（不静默）：" + q(failReply));
            const std::string lineAfter = engine.ctlState_();
            check(lineAfter == lineBefore,
                  "写盘失败 → 状态回滚（conf 与活状态不许漂）：" + q(lineAfter));
            chmod(confDir.c_str(), 0755);
            chmod(conf.c_str(), 0644);
            readOpiSettings(readBack);
            check(*readBack.chinesePunct == before,
                  "失败臂没动 conf（回滚后文件与内存仍同值，§8 读它取 punctExp）");
        }
    }

    std::printf("-- 7e symbols：三态区分（未接线 / 数据缺失 / 正常）--\n");
    opiSymbolBrowseHook() = [](int32_t) { return -1; };
    const std::string notWired = engine.handleCtlAction_("symbols");
    check(notWired.rfind("err:4:", 0) == 0 && notWired.find("未接线") != std::string::npos,
          "入口未接线（hook 返回 -1）→ err:4：" + q(notWired));
    // 0 = 二义态：得看装载态。harness 把本驱动跑在**装载失败**的目录上（data-nosym：
    // 词库在、符号类目是零字节 tsv ⇒ Rust 侧返回 -2）—— 为什么不再用「没有 tsv」
    // 那个形态、以及为什么它在这类机器上不可复现，见 run-harness.sh 的 stage 4 段。
    opiSymbolBrowseHook() = [](int32_t) { return 0; };
    const std::string noData = engine.handleCtlAction_("symbols");
    check(noData.rfind("err:4:", 0) == 0 && noData.find("数据缺失") != std::string::npos,
          "返回 0 且数据未装载 → err:4（哑成功会让状态栏按钮「点了没反应还报成功」）");
    opiSymbolBrowseHook() = [](int32_t) { return 1; };
    check(engine.handleCtlAction_("symbols") == "ok", "返回 1（在浏览）→ ok");

    std::printf("-- 7f FIN 未接线的一次性诊断（Pin A：写清期望的导出符号名）--\n");
    if (opi_fcitx5_symbol_browse == nullptr) {
        opiSymbolBrowseHook() = opiSymbolBrowseCall; // 换回真入口
        const std::string diag = captureStderr([&]() { engine.handleCtlAction_("symbols"); });
        check(diag.find("opi_fcitx5_symbol_browse") != std::string::npos,
              "诊断里写明了期望的导出符号名（名字漂了时这是唯一看得见的地方）");
        const std::string diag2 = captureStderr([&]() { engine.handleCtlAction_("symbols"); });
        check(diag2.empty(), "诊断只打一次（不是每次动作都刷屏）");
    } else {
        skip("引擎侧符号浏览入口已接线（弱符号非空）—— 未接线诊断这条在本树不可达");
    }

    std::printf("== 8. state（只读查询；ctlState_ = D-Bus State 方法回复出去的那一行）==\n");
    // 前置：§7c 收尾把模式切回拼音，而 switch_mode 顺带把全角重置为**模式缺省**
    // （Mode::default_fullwidth：拼音/繁体 on，英文/符号 off）⇒ 开局是 fw=on；
    // §7d 把标点配置翻回原值、§7d-2 的写盘失败臂回滚后也没动它（落盘值 = 内存值，
    // 两条不会漂）。state 读的就是这六处既有状态，出处对照 opi_engine.h 的 stateLine_。
    // mode 那一格（2026-10-14 追加，行尾）在本驱动**有真值**：方案读口是弱符号，
    // 真 .so 在时 opiCtlSchemeRead() = 引擎实际方案（本段全程全拼 ⇒ 拼音档），
    // 符号缺失时它按全拼兜底 —— 两条在本段的取值相同（见 opi_ctl_mode.h）。
    check(opi_fcitx5_mode() == kModePinyin, "前置：拼音模式（§7c 收尾，全角随模式重置为 on）");
    std::string punctExp = "on";
    {
        OpiSettings readBack;
        readOpiSettings(readBack);
        punctExp = *readBack.chinesePunct ? "on" : "off";
    }
    const std::string s0 = engine.ctlState_();
    check(s0 == "lang=zh;trad=off;punct=" + punctExp + ";fw=on;im=;mode=pinyin",
          "冻结格式整行（键序固定 lang;trad;punct;fw;im;mode，桌面按位置解析）："
              + q(s0));
    // im 那一格在**本驱动恒为空串**：instance_ 是 nullptr ⇒ glue 直接接空串
    // （stateLine_ 里的 `instance_ != nullptr` 分支，没实例就没「最近 IC」可问）。
    // 前端约定：未知 = 维持现状。**真值面**（切 IM 后 im 跟着变；无 IC 时 im 依
    // ShareInputState：All 兜底给「下一扇窗的 IM」/ No 才是空串）归 e2e 变体 ctl
    // —— 那里有真守护进程 + 真 IC（本驱动假装测它就是假证据）。
    check(s0.find(";im=;") != std::string::npos,
          "im 格是空串（本驱动没实例可问，不是失败也不是别的值）");
    // mode **追加在行尾**（2026-10-14）：按位置读前五段的旧前端照旧 —— 这一条同时
    // 钉住「追加在末位」这个位置选择本身。
    check(s0.size() > 12 && s0.compare(s0.size() - 12, 12, ";mode=pinyin") == 0,
          "末格是 mode=pinyin（拼音 + 全拼 ⇒ pinyin 档；追加在行尾）");

    std::printf("-- 8a 切 trad 前后整行变化（off→on 与 on→off 两向都断）--\n");
    check(engine.handleCtlAction_("trad") == "ok" && opi_fcitx5_mode() == kModeTraditional,
          "trad（翻面走动作路径，state 只是读它）");
    const std::string s1 = engine.ctlState_();
    check(s1 == "lang=zh;trad=on;punct=" + punctExp + ";fw=on;im=;mode=pinyin",
          "lang/punct/fw/im 四段逐字不变，trad 翻 on，**mode 段原地不动**（繁体是"
          "面外态 ⇒ 收成 pinyin：读法仍是拼音，简繁由 trad 格自述。2026-10-14 改判，"
          "见 opi_ctl_mode.h 的 opiCtlStateModeToken）："
              + q(s1));
    check(s1 != s0, "切 trad 前后 state **不同**（状态栏「点了要有即时反馈」靠这一格）");
    check(engine.handleCtlAction_("trad") == "ok" && opi_fcitx5_mode() == kModePinyin,
          "trad 切回");
    const std::string s2 = engine.ctlState_();
    check(s2 == s0 && s1 != s2, "切回后整行逐字回到 s0（反方向也测到了）");

    std::printf("-- 8b lang 段跟着模式走（另一格；英文模式的 switch_mode 会把全角重置为缺省）--\n");
    check(engine.handleCtlAction_("lang") == "ok" && opi_fcitx5_mode() == kModeEnglish,
          "lang 切英文");
    const std::string s3 = engine.ctlState_();
    check(s3 == "lang=en;trad=off;punct=" + punctExp + ";fw=off;im=;mode=en26",
          "lang=en 且全角随英文缺省变 off，mode 格 = en26（英文档 —— lang 与 mode "
          "两格在英文态**同向**变）："
              + q(s3));

    std::printf("-- 8c 只读性：连读同值 + 行为面证明（全角那一格是双翻面读的，见 opi_engine.h）--\n");
    const std::string a = engine.ctlState_();
    const std::string b = engine.ctlState_();
    check(a == b && a == s3, "连读两次值相同 —— 双翻面读法的净效应为零（没在读侧翻走状态）");
    commitNow();
    press(0x2C); // ','
    check(commitNow() == ",",
          "读 state 之后英文半角直传仍成立（全角位没被读漂 —— 行为面证据，不只看返回值）");
    check(engine.handleCtlAction_("lang") == "ok" && opi_fcitx5_mode() == kModePinyin,
          "lang 切回");
    check(engine.ctlState_() == s0, "回到拼音后整行 == s0（本段不留副作用）");

    runCtlSearchChecks(engine, bufferOf); // §9 拆在 opi_ctl_search_driver.cpp（500 行门禁）
    runCtlModeChecks(engine, ic, bufferOf, findAction, type); // §10 同一个理由拆出

    std::printf("\n== 结果：%s ==\n", g_fail == 0 ? "全过" : "有失败");
    return g_fail == 0 ? 0 : 1;
}
