// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// §10 `mode`（模式循环，用户裁决 2026-10-14）—— 从 `opi_ctl_driver.cpp` 拆出来的
// **片段**，不是独立驱动（同 `opi_ctl_search_driver.cpp` 的先例：主文件加上本段
// 会顶破 500 行门禁 `crates/opi-ffi/tests/line_limit.rs`）。由驱动主文件
// `#include`；`run-harness.sh` 的编译行只列主文件 —— 别把它加成第二个 TU。
//
// 本段验的（走 `handleCtlAction_` = D-Bus Trigger 落到的那一段，与 opi-ctl 回复
// 出去的是同一个返回值）：
//   · 循环顺序 拼 → 英 → 拼…，且每步的应答 **token**（pinyin/en26 —— 唯一一个
//     成功应答不是 "ok" 的动作）与**引擎读回**（opi_fcitx5_mode / 方案）逐条一致；
//   · 同一张 token 表：`opi-ctl state` 行末的 `mode=` 格与应答 token 在**循环面上**
//     逐条相等（2026-10-14 加，两处共用 opi_ctl_mode.h 的派生 —— 谁另写一套这里就
//     红）。面外态两者**刻意不同**：state 报读法（pinyin），应答报下一步（en26），
//     §10c/§10d 把这两面成对钉住；
//   · 「进拼音档把方案摆回全拼」这条契约（它的可见后果是：双拼用户点这个按钮会
//     落到全拼，见 opi_ctl_mode.h 的副作用注记）—— 读侧 opi_fcitx5_scheme() 取证；
//   · 面外态（繁体/符号/双拼）：state 那格报 **pinyin**（读法仍是拼音 —— 不是英文；
//     2026-10-14 改判），点一下就进 en26（正常循环腿，没有「收编跳」）；
//   · 两个状态项文案跟着刷（「中/英」在 §7c 已有先例：漏刷是首版踩过的坑）。
//
// 局限（如实记）：W1「切英文先把待定拼音交出去」的**提交**那一半在本驱动**不可达**
// —— handleCtlAction_ 的「当前 IC」取自 instance_->mostRecentInputContext()，而本
// 驱动用 nullptr 构造引擎 ⇒ opiDirectConfirm 在空指针守卫处就返回。那半归 e2e 变体
// ctl（真守护进程 + 真 IC）；这里只验「缓冲被切模式清掉」这一条。

// [bufferOf] / [findItem] / [typeSeq] 由调用方传进来：都是驱动 main 里的 lambda
// （本节原本就在 main 内），模板参数接住，免得把实现复制一份（单一真源；同 search
// 段的先例）。
template <class BufferOf, class FindItem, class TypeSeq>
static void runCtlModeChecks(OpiEngine &engine, TestIC &ic, BufferOf bufferOf,
                             FindItem findItem, TypeSeq typeSeq) {
    std::printf("== 10. mode（模式循环 拼 → 英 →（未来五笔）；应答 = 状态 token）==\n");
    if (opi_fcitx5_set_scheme == nullptr || opi_fcitx5_scheme == nullptr) {
        // 方案读/写侧都是弱符号：.so 比本树旧（没重编 target/release）时它们是
        // nullptr，本段全部断言都会退化成「测了个缺省」—— 明确 skip，别假绿。
        skip("引擎侧方案读/写口未接线（弱符号 nullptr；target/release 要用本树重编）");
        return;
    }
    auto schemeNow = []() { return opi_fcitx5_scheme(); };
    // state 行末格 `;mode=<token>`（2026-10-14 追加）。本段的判据：动作返回的 token
    // 与 state 那一格在循环面上同源 —— 两处由 opi_ctl_mode.h 的同一张表给出，这条
    // 断言就是「别写第二套派生」在 C++ 侧的门禁（谁在 stateLine_ 里另写一套就红）。
    // 键序/整行归 §8 与 CLI 钉；这里只取末格。
    auto stateMode = [&engine]() -> std::string {
        const std::string s = engine.ctlState_();
        const std::string tag = ";mode=";
        const auto p = s.rfind(tag);
        return p == std::string::npos ? std::string("<无 mode 格>")
                                      : s.substr(p + tag.size());
    };

    std::printf("-- 10a 拼(全拼) → en26：token 与引擎读回逐条一致 --\n");
    opi_fcitx5_switch_mode(kModePinyin); // 前置不靠上游段落的残留
    opi_fcitx5_set_scheme(0);
    check(opi_fcitx5_mode() == kModePinyin && schemeNow() == 0,
          "前置：拼音 + 全拼（读侧回 " + std::to_string(schemeNow()) + "）");
    const std::string r1 = engine.handleCtlAction_("mode");
    check(r1 == "en26", "拼 → 应答 token en26（实际 " + q(r1) + "）");
    check(r1 != "ok", "负对照：本动作的成功应答**不是** \"ok\"（协议是 token）");
    check(opi_fcitx5_mode() == kModeEnglish, "拼 → 引擎模式真的切到 English");
    check(engine.ctlState_().rfind("lang=en;", 0) == 0,
          "拼 → state 的 lang 段跟着变（桌面状态栏读的那一格）");
    check(stateMode() == r1,
          "拼 → state 的 mode 格与应答 token 逐字同源（同一份派生）：" + q(stateMode()));
    fcitx::Action *zhEn = findItem("中", "英");
    check(zhEn != nullptr && zhEn->shortText(&ic) == "英",
          "拼 → 「中/英」状态项文案跟着变（§7c 那个「漏刷新」坑的同款检查）");

    std::printf("-- 10b en26 → 拼音(全拼)：scheme 读回 = 全拼，循环闭合 --\n");
    const std::string r2 = engine.handleCtlAction_("mode");
    check(r2 == "pinyin", "en26 → 应答 token pinyin（实际 " + q(r2) + "）");
    check(opi_fcitx5_mode() == kModePinyin && schemeNow() == 0,
          "en26 → 引擎读回 = 拼音 + 全拼（实际 mode=" + std::to_string(opi_fcitx5_mode())
              + " scheme=" + std::to_string(schemeNow()) + "）");
    check(zhEn != nullptr && zhEn->shortText(&ic) == "中", "「中/英」文案切回「中」");
    check(stateMode() == r2,
          "en26 → state 的 mode 格跟着回到 token（两向都同源）：" + q(stateMode()));
    // 来回两轮：循环是**交替**的（不是单向进英文后卡住）；每步 token 与读回都一致。
    check(engine.handleCtlAction_("mode") == "en26" && opi_fcitx5_mode() == kModeEnglish,
          "再来一轮：拼 → en26（第二次，不是单向下坡）");
    check(engine.handleCtlAction_("mode") == "pinyin" && opi_fcitx5_mode() == kModePinyin,
          "再来一轮：en26 → pinyin（循环闭合两次）");

    std::printf("-- 10c 双拼档不在循环面上：state 报 pinyin，点一下进 en26、再点落回**全拼** --\n");
    // 「进拼音档把 scheme 摆到全拼」的可见后果就在这里（也是那条副作用的取证点：
    // 引擎运行值被摆到全拼，配置文件没动）。双拼的**行为**面（键序解码）在 Rust
    // 侧测；这里只看模式循环对它的处理。
    opi_fcitx5_set_scheme(4); // 4 = 双拼（Scheme::code）
    check(schemeNow() == 4, "前置：引擎在双拼（读侧回 " + std::to_string(schemeNow()) + "）");
    check(stateMode() == "pinyin",
          "双拼态的 state mode 格 = pinyin（面外态收成「拼」：读法仍是拼音；state 面"
          "永远给档位，不像动作路径那样会回空串 —— 状态栏初值读的就是这一格）："
              + q(stateMode()));
    check(engine.handleCtlAction_("mode") == "en26" && opi_fcitx5_mode() == kModeEnglish,
          "双拼 + 点 mode → en26（面外态不进 token 表，但点一下就是正常的一腿）");
    check(engine.handleCtlAction_("mode") == "pinyin" && schemeNow() == 0,
          "再点 → 落回全拼（scheme 读回 0 —— 「pinyin」token 的定义就是全拼）");

    std::printf("-- 10d 其它面外态（繁体 / 符号）：state 报 pinyin，点一下才进 en26 --\n");
    opi_fcitx5_switch_mode(kModeTraditional);
    check(stateMode() == "pinyin",
          "繁体态的 state mode 格 = pinyin（**不是** en26 —— 中文输入时条上不许出现"
          "英文档位这个假标签；简繁由 trad 格自述。2026-10-14 改判）：" + q(stateMode()));
    check(engine.handleCtlAction_("mode") == "en26" && opi_fcitx5_mode() == kModeEnglish,
          "繁体 + 点 mode → en26（点之前显示「拼」，这一跳在显示面上真的变档 —— "
          "初版那个「en26 → en26」的收编跳没有了）");
    fcitx::Action *fanjian = findItem("简", "繁");
    check(fanjian != nullptr && fanjian->shortText(&ic) == "简",
          "「简/繁」文案也从「繁」刷回「简」（跨状态项刷新）");
    opi_fcitx5_switch_mode(kModeSymbol);
    check(stateMode() == "pinyin",
          "符号态同理：state mode 格 = pinyin（" + q(stateMode()) + "）");
    check(engine.handleCtlAction_("mode") == "en26" && opi_fcitx5_mode() == kModeEnglish,
          "符号 + 点 mode → en26");

    std::printf("-- 10e 待定拼音：切模式清缓冲（提交那半归 e2e，见文件头）--\n");
    opi_fcitx5_switch_mode(kModePinyin);
    opi_fcitx5_set_scheme(0);
    typeSeq("nihao"); // 驱动主文件的 type lambda：经引擎进缓冲
    check(bufferOf() == "nihao", "前置：缓冲里有待定拼音（实际 " + q(bufferOf()) + "）");
    check(engine.handleCtlAction_("mode") == "en26", "有待定拼音时也能切（不卡住）");
    check(bufferOf().empty(), "切模式清了缓冲（提交那一半需要真 IC —— e2e 变体 ctl）");

    // 收尾：回到 §8 的基线状态（拼音 + 全拼），别给后续段落留模式残留。
    opi_fcitx5_switch_mode(kModePinyin);
    opi_fcitx5_set_scheme(0);
    check(stateMode() == "pinyin",
          "收尾：回拼音全拼后 state 的 mode 格 = pinyin（跟得住）");
}
