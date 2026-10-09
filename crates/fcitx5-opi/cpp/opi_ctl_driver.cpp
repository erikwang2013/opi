// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// W1「直接确认」+ opi-ctl 动作分发驱动 —— **手工 harness，不进 CI**（CI 上没有
// fcitx5 的头/库）。跑法见同目录 run-harness.sh。
//
// 直接 #include 胶水源码（验的就是即将装进插件的那份代码）。三件事，都是**跨 agent
// 契约**而不是内部细节：
//   1. W1 覆盖面：切输入法 / 切英文 → 有候选上首候选、无候选提交原文；
//      焦点移出 / reset → **静默清空，绝不提交**（用户裁决，与上一条成对照）。
//   2. **顺序契约**（lead 2026-10-10）：符号浏览进行中切走 ⇒ 先退浏览（恢复进入
//      浏览前的真缓冲）→ 再按 W1 提交。顺序反了就是把浏览态的标题串当用户输入
//      交出去 —— 本驱动用假 hook 把这条钉死（真引擎入口是弱符号，本分支未落地）。
//   3. opi-ctl 的动作分发面（handleCtlAction_，与 D-Bus 接收侧同一个函数）：五个
//      动作的返回值协议 "ok" / "err:<码>:<原因>"，以及「未接线 / 数据缺失」的区分。
//   4. opi-ctl state（只读查询，2026-10-10 用户裁决）：ctlState_ = D-Bus State 方法
//      回复出去的那一行 —— 冻结格式整行、trad/lang 两格跟着状态变、以及**只读性**
//      （全角格是双翻面读的，得证明没把状态读漂）。
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
#include <unistd.h> // dup/dup2：捕获 stderr（Pin A 的诊断断言）

#include "opi_driver_common.h" // 测试器具（TestIC/check/预编辑与候选读回）

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

    std::printf("== 1. W1：切输入法 = 有候选上首候选 ==\n");
    resetVia();
    commitNow();
    type("nihao");
    const std::string nihao0 = firstCand();
    check(candidateCount(ic) > 0 && nihao0 != "<none>",
          "「nihao」有候选（被测前提；没候选时本段测的是§2 那条路）");
    switchIM();
    check(commitNow() == nihao0, "切输入法：提交首候选 " + q(nihao0));
    check(preeditOf(ic).empty() && candidateCount(ic) == 0 && bufferOf().empty(),
          "提交后引擎与面板都清干净（无缓冲、无预编辑、无候选）");

    std::printf("== 2. W1：无候选 = 提交原文 ==\n");
    resetVia();
    commitNow();
    type("zzzz");
    check(bufferOf() == "zzzz" && candidateCount(ic) == 0,
          "「zzzz」在缓冲里且零候选（被测前提）");
    switchIM();
    check(commitNow() == "zzzz", "切输入法：无候选时提交原文（别把用户打的丢掉）");
    check(preeditOf(ic).empty(), "提交后面板同样清干净");

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
    resetVia();
    type("hao");
    const std::string hao0 = firstCand();
    check(candidateCount(ic) > 0, "用户真缓冲「hao」有候选（被测前提）");
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
    const std::string enterReply = engine.handleCtlAction_("symbols");
    check(enterReply == "ok", "进入浏览（symbols 动作，假 hook 返回 1）应答 ok");
    const std::string junk0 = firstCand();
    std::printf("  浏览态首候选=%s 真缓冲首候选=%s\n", q(junk0).c_str(),
                q(hao0).c_str());
    switchIM(); // 切走：契约要求「先退浏览 → 再 W1 提交」
    const std::string committed = commitNow();
    check(log == "enter;exit;", "调用顺序 enter → exit（实际 [" + log + "]）");
    check(exitSaw == "fuhao",
          "退出钩子看到的仍是浏览态缓冲（证明 exit 发生在 W1 取缓冲**之前**）");
    check(committed == hao0, "提交的是真缓冲「hao」的首候选 " + q(hao0));
    check(committed != junk0, "提交的**不是**浏览态标题串的首候选 " + q(junk0));
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
    // 先打 nihao、再跑 opi-ctl lang，断言 IC 真收到 CommitString「你好」。
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
    // §7d 把标点配置翻回了原值（落盘值 = 内存值，两条不会漂）。state 读的就是这
    // 四处既有状态，出处对照 opi_engine.h 的 stateLine_。
    check(opi_fcitx5_mode() == kModePinyin, "前置：拼音模式（§7c 收尾，全角随模式重置为 on）");
    std::string punctExp = "on";
    {
        OpiSettings readBack;
        readOpiSettings(readBack);
        punctExp = *readBack.chinesePunct ? "on" : "off";
    }
    const std::string s0 = engine.ctlState_();
    check(s0 == "lang=zh;trad=off;punct=" + punctExp + ";fw=on",
          "冻结格式整行（键序固定 lang;trad;punct;fw，桌面按位置解析）：" + q(s0));

    std::printf("-- 8a 切 trad 前后整行变化（off→on 与 on→off 两向都断）--\n");
    check(engine.handleCtlAction_("trad") == "ok" && opi_fcitx5_mode() == kModeTraditional,
          "trad（翻面走动作路径，state 只是读它）");
    const std::string s1 = engine.ctlState_();
    check(s1 == "lang=zh;trad=on;punct=" + punctExp + ";fw=on",
          "只有 trad 段翻成 on，其余三段逐字不变：" + q(s1));
    check(s1 != s0, "切 trad 前后 state **不同**（状态栏「点了要有即时反馈」靠这一格）");
    check(engine.handleCtlAction_("trad") == "ok" && opi_fcitx5_mode() == kModePinyin,
          "trad 切回");
    const std::string s2 = engine.ctlState_();
    check(s2 == s0 && s1 != s2, "切回后整行逐字回到 s0（反方向也测到了）");

    std::printf("-- 8b lang 段跟着模式走（另一格；英文模式的 switch_mode 会把全角重置为缺省）--\n");
    check(engine.handleCtlAction_("lang") == "ok" && opi_fcitx5_mode() == kModeEnglish,
          "lang 切英文");
    const std::string s3 = engine.ctlState_();
    check(s3 == "lang=en;trad=off;punct=" + punctExp + ";fw=off",
          "lang=en 且全角随英文缺省变 off：" + q(s3));

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

    std::printf("\n== 结果：%s ==\n", g_fail == 0 ? "全过" : "有失败");
    return g_fail == 0 ? 0 : 1;
}
