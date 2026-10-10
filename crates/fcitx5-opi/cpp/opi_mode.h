// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
//
// 模式切换的两条入口 + 中/英、简/繁两个状态项：
//   · 模式热键（Ctrl+' / Ctrl+\）—— 2026-10-09 从 opi_fcitx5.cpp **原样搬来**
//     （那个文件有 500 行硬门禁；搬移是机械的，判据与实测记录随注释一起走）；
//   · Shift 轻按切换中英文（新）—— 走引擎**已有**的 English 模式（与 Ctrl+' 同一条
//     Rust 通路），本层只负责「轻按」判定与状态指示。
//
// 这里**不做**任何按键消费：两条入口都不 filterAndAccept 之外的动作，Shift 那条
// 干脆从不消费 —— 与 core 对 modifier-only 热键「放行到客户端」的处理同语义
// （instance.cpp 实测：「don't forward to input method, but make it pass through
// to client」）。

#pragma once

#include <cstdint>
#include <cstdio>
#include <ctime>
#include <functional>
#include <type_traits>
#include <utility>

#include <fcitx/action.h>
#include <fcitx/event.h>
#include <fcitx/globalconfig.h>
#include <fcitx/inputcontext.h>
#include <fcitx/instance.h>
#include <fcitx/statusarea.h>
#include <fcitx/userinterfacemanager.h> // registerAction（不完整类型会编不过）
// fcitx::now(CLOCK_MONOTONIC) 的声明在两版之间搬过家：5.0.21 在
// <fcitx-utils/event.h>，5.1.12 在 <fcitx-utils/eventloopinterface.h>
// （5.0 根本没有后一个文件，实测）。按**存在性**选头，别按版本号猜 ——
// 与下面探测 checkModifierOnlyKeyTimeout 同一个理由。
#if __has_include(<fcitx-utils/eventloopinterface.h>)
#include <fcitx-utils/eventloopinterface.h>
#else
#include <fcitx-utils/event.h>
#endif
#include <fcitx-utils/keysym.h>
#include <fcitx-utils/signals.h>

#include "opi_rust_ffi.h"

// ---- 面板推送函数（refreshingPanelFor）的定义在 opi_fcitx5.cpp、主声明在
// opi_engine.h。这里再声明一次：本头文件要独立于 opi_engine.h 可用
// （后者 include 了本文件，反向引用会成环）。同签名重复声明是合法的。
void refreshingPanelFor(fcitx::InputContext *ic);

// W1「直接确认」的实现体也在 opi_fcitx5.cpp（同一个理由：它要用 take()，
// 而 take() 是**每个 TU 一份**的 static 函数，放进本头的 inline 体里会踩 ODR）。
void opiDirectConfirm(fcitx::InputContext *ic);

// 模式编码与 Rust 侧 mode_from_int / mode_to_int **逐值对应**（../src/lib.rs）。
// 抄错一个值就是切到另一个模式，且两边都不报错。
// （原在 opi_fcitx5.cpp，2026-10-09 随模式热键一起搬来。）
enum OpiMode : int32_t {
    kModePinyin = 0,
    kModeEnglish = 1,
    kModeSymbol = 3,
    kModeTraditional = 4,
};

// 这是按住不放补发的**重复**事件吗。**只能从 rawKey() 取** ——
// key() 已过 Key::normalize()，Repeat 在那一步被滤掉（opi_fcitx5.cpp 送 Rust 的
// states 处有实测），拿 key().states() 判重复恒为 false。要排：热键是**切换**
// 语义，按住时系统持续补发重复 ⇒ 不排就是模式疯狂来回切 / 全角位疯狂翻。
// 同判见 tsf-opi/src/vk.rs 的 `hotkey_should_act`（两轨同构，改一处改两处）。
inline bool isRepeatEvent(const fcitx::KeyEvent &keyEvent) {
    return keyEvent.rawKey().states().test(fcitx::KeyState::Repeat);
}

// ---- 「轻按超时」的版本探测 ----
//
// `GlobalConfig::checkModifierOnlyKeyTimeout`（及其 `modifierOnlyKeyTimeout()`）是
// **5.1.12 新增**（本机 5.1.12 头里 @since 5.1.12），而本 addon 声明的门槛是
// core:5.0.0、CI 还要拿 debian:12 的 5.0.21 头 `-Werror` 真编一次 ⇒ 直接调用
// 在 5.0 上是**编译错误**，不是运行期问题。仓库里没有版本宏可用（fcitx5 头不发
// FCITX_VERSION_*），照既有先例办：**探测 API 是否存在**，不猜版本号。
//   · 在（≥5.1.12）：用框架的 helper —— 时限取用户配置的 ModifierOnlyKeyTimeout
//     （本机 250ms），语义与 core 自己的 modifier-only 热键完全一致；
//   · 不在（<5.1.12）：不判超时（恒可触发）。降级面 = 「按住 Shift 很久后直接
//     抬起」在那些系统上也会切一次；Shift+别的键 仍有撤销判定兜底（主要缺陷
//     形态不受影响）。老端上的这个差别记在 docs/install/linux.md 的属性设置一节。
template <typename T, typename = void>
struct OpiHasModifierKeyTimeout : std::false_type {};
template <typename T>
struct OpiHasModifierKeyTimeout<
    T, std::void_t<decltype(std::declval<const T &>().checkModifierOnlyKeyTimeout(
           uint64_t{0}))>> : std::true_type {};

// 模板包一层是必须的：`if constexpr` 的弃置分支在**非模板**函数里也会被完整检查
// （C++ 标准：Outside a template, a discarded statement is fully checked）——
// 直接写在 handle() 里，5.0 头照样编不过。模板里弃置分支不求值，才真的能双版本编译。
template <typename Config>
inline bool opiModifierTimeoutOk(const Config &config, uint64_t pressTime) {
    if constexpr (OpiHasModifierKeyTimeout<Config>::value) {
        return config.checkModifierOnlyKeyTimeout(pressTime);
    } else {
        (void)config;
        (void)pressTime;
        return true;
    }
}

// ---- 符号浏览（22 类目）的进出入口 + W1「直接确认」（2026-10-10）----
//
// 两件事合在一处，因为**顺序**是它们之间的契约：
//   切走/切英文 → ① 先退浏览（恢复真缓冲）→ ② 再按 W1 提交（**按模式分叉**：拼音/
//   繁体上缓冲原文、符号模式不上关键字 —— 2026-10-10 深夜用户裁决「切英文 = 默认是
//   拼音，不要是候选词」+ 同夜追加的模式分叉，白天那版「有候选上首候选」作废，
//   时间线见 opiDirectConfirm）。顺序反了就把浏览态的标题串（或首个分类）当用户
//   输入提交出去 —— 顺序这条是 lead 2026-10-10 的裁决。
//
// 引擎侧入口 `opi_fcitx5_symbol_browse` 是**弱符号**（契约段在 opi_rust_ffi.h）：
// 落地前为 nullptr。缺符号时只打**一次** stderr 诊断（含期望符号名 —— 名字若与
// engine-track 的导出漂移，那是唯一看得见的地方，所以把名字写进去，别只写「未接线」）。
inline constexpr int32_t kOpiSymbolBrowseExit = 0;   // 幂等：非浏览态是 no-op
inline constexpr int32_t kOpiSymbolBrowseEnter = 1;
inline constexpr int32_t kOpiSymbolBrowseToggle = 2; // 引擎看自己的状态决定进/出

// 唯一调用点（判空 + 诊断都只在这里）。返回值三态：
//   -1 = 引擎侧入口没接线（弱符号仍是 nullptr，诊断已打）；
//   ≥0 = 引擎给的**执行后浏览态**（1=在浏览 / 0=不在，engine-track 的返回契约）。
// 注意 0 是二义的（「刚退出来」与「无数据进不去」都是 0）—— 要看真值的调用方
// 得自己判装载态（opi_engine.h 的 symbols 动作就是这么做的）。
inline int32_t opiSymbolBrowseCall(int32_t action) {
    if (opi_fcitx5_symbol_browse == nullptr) {
        static bool warned = false;
        if (!warned) {
            warned = true;
            fprintf(stderr,
                    "fcitx5-opi: 引擎侧符号浏览入口没接线 —— 期望的导出符号是"
                    " `opi_fcitx5_symbol_browse`（弱符号，当前解析为 nullptr）。"
                    "本次动作 %d 无效；插件其余功能不受影响。\n",
                    static_cast<int>(action));
        }
        return -1;
    }
    return opi_fcitx5_symbol_browse(action);
}

// 测试缝（与 opi_status.h 的 opiSettingsLauncherHook 同一先例）：面板驱动把这里
// 换成假货，才能验「先退浏览、后提交」的**顺序** —— 真入口是弱符号，落到本分支
// 之前驱动无法通过它观察任何东西（而顺序断言本来就该独立于引擎侧实现）。
inline std::function<int32_t(int32_t)> &opiSymbolBrowseHook() {
    static std::function<int32_t(int32_t)> f = opiSymbolBrowseCall;
    return f;
}

// ---- Shift 轻按切换中英文 + 中/英状态项（2026-10-09）----
//
// 用户裁决（原话「你这不能用shift键切换英文」）：**轻按**左/右 Shift 在
// 中文 ⇄ 英文间切换，与搜狗一致。实现走引擎既有的 English 模式
// （input_method.rs 的 `Mode::English`：空缓冲下字母直传上屏）。
//
// 「轻按」判定 = 按下后、抬起前**没有别的键按下**，且按住时长不超过
// `GlobalConfig::modifierOnlyKeyTimeout()`（用户配置里的 250ms；<0 = 不判超时）。
// 语义与 core 自己的 modifier-only 热键**共用同一个 helper**
// （`checkModifierOnlyKeyTimeout`，5.1.12 的新 API）—— 不另发明一套时限规则。
//
// 为什么不用 core 的全局触发键（TriggerKeys=Shift_L）：
//   · 那会把**所有**输入法一起改了（全局热键），且组里多过两个输入法时
//     按 TriggerKey 是**轮换**语义不是切换（instance.cpp 的 trigger()/enumerate）；
//   · 本项是 OPI 自己的配置面（opi_config.h 的 ShiftToggle），显示在
//     配置工具的「附加组件 → OPI」里。
// ⚠️ 副作用（如实记）：本机制不消费 Shift，未拦截时 Shift 的事件照旧直通客户端
//   （与 core 的 modifier-only 热键一致）。Shift+点击 这类**没有中间按键**的组合
//   仍会被判成轻按 —— 与搜狗的行为相同，用户的习惯预期如此。
class OpiShiftToggle {
public:
    explicit OpiShiftToggle(fcitx::Instance *instance) : instance_(instance) {
        action_.setShortText("中");
        action_.setLongText("Shift 键切换中英文（点击此项同样切换）");
        if (instance_ != nullptr) {
            // 名字全局唯一（其它 addon 也往 UserInterfaceManager 注册 action）。
            instance_->userInterfaceManager().registerAction("opi-shift-toggle",
                                                             &action_);
        }
        // harness 的 opi_panel_driver 用 nullptr 构造引擎（历史：instance_ 只存不用），
        // 故这里与 handle/attach 一样容忍 nullptr —— 驱动测的是判定与状态项本身。
        connection_ = fcitx::ScopedConnection(
            action_.connect<fcitx::SimpleAction::Activated>(
                [this](fcitx::InputContext *ic) { toggle(ic); }));
    }

    // 每个键事件都先经过这里。**从不消费按键**（void 返回），只做三件事：
    //   · 非 Shift 的**按下** → 撤销「轻按」判定（Shift+字母/方向/空格… 都不是轻按）；
    //   · Shift 按下 → 记录 CLOCK_MONOTONIC 时刻；
    //   · Shift 抬起且仍处于判定态 → 查超时 helper，过了就切换。
    void handle(const fcitx::KeyEvent &ev, bool enabled) {
        const auto sym = ev.key().sym();
        const bool isShift = sym == FcitxKey_Shift_L || sym == FcitxKey_Shift_R;
        if (!enabled) {
            // 关掉开关时清掉残态：否则「按住 Shift 时改配置」会让抬起那一下
            // 用旧状态切一次模式。
            armed_ = false;
            return;
        }
        if (!isShift) {
            if (!ev.isRelease()) {
                armed_ = false;
            }
            return;
        }
        if (!ev.isRelease()) {
            armed_ = true;
            pressTime_ = fcitx::now(CLOCK_MONOTONIC);
            return;
        }
        if (!armed_) {
            return;
        }
        armed_ = false;
        // 超时/时刻两点都在本函数内取，都用 CLOCK_MONOTONIC（helper 的文档要求）。
        // instance_ 为空（面板驱动）时无从查时限 ⇒ 按「不判超时」处理，
        // 超时那半由 e2e 在真守护进程里验（见 opiModifierTimeoutOk）。
        if (instance_ != nullptr
            && !opiModifierTimeoutOk(instance_->globalConfig(), pressTime_)) {
            return;
        }
        toggle(ev.inputContext());
    }

    // 失焦等 reset 时机调用：按住 Shift 时切走窗口，抬起事件可能到不了本 IC
    // （或到了别的 IC），不清就是一次「幽灵切换」。
    void disarm() { armed_ = false; }

    // activate 时挂进本 IC 的状态区（StatusGroup::InputMethod 组会在
    // 下次 activate 前由 core 自动清空，故这里不用管去重）。
    void attach(fcitx::InputContext *ic) {
        if (ic == nullptr) {
            return;
        }
        ic->statusArea().addAction(fcitx::StatusGroup::InputMethod, &action_);
        update(ic);
    }

    // 把「中/英」刷成引擎当前模式。模式是进程级单例（Rust 侧），所有 IC 同值。
    void update(fcitx::InputContext *ic) {
        action_.setShortText(opi_fcitx5_mode() == kModeEnglish ? "英" : "中");
        if (ic != nullptr) {
            action_.update(ic);
        }
    }

    // 模式变了之后的补充刷新（「简/繁」项）。由 OpiEngine 在构造时接上 ——
    // 别在这里直接持有对面那个类：两个状态项互相刷，指针会成环。
    void setOnModeChanged(std::function<void(fcitx::InputContext *)> f) {
        onModeChanged_ = std::move(f);
    }

    // 切换 + 刷面板 + 刷状态项。面板必须刷：切模式会清缓冲换候选
    // （与 handleModeHotkey 的处理同一条理由）。
    void toggle(fcitx::InputContext *ic) {
        const int32_t cur = opi_fcitx5_mode();
        const int32_t next = (cur == kModeEnglish) ? kModePinyin : kModeEnglish;
        if (next == kModeEnglish) {
            // 「中→英」＝ 用户裁决里的「切英文」：先把待定拼音提交掉（W1：先退浏览、
            // 后提交**缓冲原文**；符号模式那半是「不上关键字」—— 2026-10-10 深夜
            // 裁决 + 同夜模式分叉，见 opiDirectConfirm）。
            // **必须在任何 switch_mode 之前** —— switch_mode 会清缓冲
            // （Rust 侧 composer.switch_mode），之后再提交就只剩空串。
            opiDirectConfirm(ic);
            // 进英文时 Rust 的「单次 Shift」状态**按设计**保留 ——
            // `CandidateState::switch_mode` 只在**离开** English 时清它
            // （candidate.rs，判据与 Android `EngineController.switchMode` 逐字一致）：
            // 那是键盘 Shift 的镜像，单击 Shift 该让下一个字母大写。
            // 但这里 Shift 单击的语义是**切模式**：不清的话，用户按一下 Shift、
            // 模式是切过去了，打出的第一个字母却莫名大写（面板驱动第 10 段第一版
            // 实测 'n' 提交了 "N"，就是这条状态漏过来的）。
            // 清法：先绕一步拼音 —— 那一步命中「离开 English 才清」，把 shift_state
            // 归 Off；两步之间不刷面板（refresh 只在最后一次），中间态从不渲染。
            opi_fcitx5_switch_mode(kModePinyin);
        }
        opi_fcitx5_switch_mode(next);
        refreshingPanelFor(ic);
        update(ic);
        if (onModeChanged_) {
            onModeChanged_(ic);
        }
    }

private:
    fcitx::Instance *instance_;
    fcitx::SimpleAction action_;
    fcitx::ScopedConnection connection_;
    std::function<void(fcitx::InputContext *)> onModeChanged_;
    bool armed_ = false;
    uint64_t pressTime_ = 0;
};

// ---- 「简/繁」状态项 + 简繁切换的**唯一实现**（2026-10-09，外层入口补齐）----
//
// 与「中/英」项同档：挂在 StatusGroup::InputMethod 状态区，点击即切。用户在外层
// （状态栏）看到一个「中」、一个「设」、一个「简」，与配置面的六选项互不替代。
// 热键（Ctrl+Shift+F）与状态项**共用下面这一个函数** —— 两份实现会在
// 「英文/符号模式里按下时报切到哪」这类角落上漂，别抄第二份。
inline void opiToggleTraditional(fcitx::InputContext *ic) {
    const int32_t cur = opi_fcitx5_mode();
    opi_fcitx5_switch_mode(cur == kModeTraditional ? kModePinyin : kModeTraditional);
    refreshingPanelFor(ic);
}

class OpiFanjianToggle {
public:
    explicit OpiFanjianToggle(fcitx::Instance *instance) : instance_(instance) {
        action_.setShortText("简");
        action_.setLongText("点击切换简体/繁体（等同 Ctrl+Shift+F）");
        if (instance_ != nullptr) {
            instance_->userInterfaceManager().registerAction("opi-fanjian-toggle",
                                                             &action_);
        }
        connection_ = fcitx::ScopedConnection(
            action_.connect<fcitx::SimpleAction::Activated>(
                [this](fcitx::InputContext *ic) { toggle(ic); }));
    }

    // 见 OpiShiftToggle::setOnModeChanged（反向那一半，由 OpiEngine 接上）：
    // 从英文态点「简」会切进繁体 ⇒「中/英」项也要跟着刷回「中」。
    void setOnModeChanged(std::function<void(fcitx::InputContext *)> f) {
        onModeChanged_ = std::move(f);
    }

    void attach(fcitx::InputContext *ic) {
        if (ic == nullptr) {
            return;
        }
        ic->statusArea().addAction(fcitx::StatusGroup::InputMethod, &action_);
        update(ic);
    }

    // 文案显示**当前**状态（同「中/英」项）：繁体态显示「繁」，其余显示「简」。
    void update(fcitx::InputContext *ic) {
        action_.setShortText(opi_fcitx5_mode() == kModeTraditional ? "繁" : "简");
        if (ic != nullptr) {
            action_.update(ic);
        }
    }

    void toggle(fcitx::InputContext *ic) {
        opiToggleTraditional(ic);
        update(ic); // 刷**自己**的文案（首版漏了这行：模式切了、按钮还写着「简」）
        if (onModeChanged_) {
            onModeChanged_(ic); // 再刷对面那项（中/英）
        }
    }

private:
    fcitx::Instance *instance_;
    fcitx::SimpleAction action_;
    fcitx::ScopedConnection connection_;
    std::function<void(fcitx::InputContext *)> onModeChanged_;
};

// ---- 模式热键（B0/B5）——原样搬自 opi_fcitx5.cpp ----
//
// ⚠️ 这段路由**故意落在 C++ 侧**，与「行为路由全部在 Rust 侧」相悖。
// 原因很具体：Rust 侧拦 Ctrl 的是 handle_key 的首行
// （src/input_method.rs：`states & (CTRL|ALT) → PassThrough`），Ctrl 组合键根本
// 到不了 `opi_fcitx5_key_event` 的动作分发，只能在**送进去之前**拦。
// 长期归属是 input_method.rs —— 那里还能顺手覆盖 TSF 一侧，届时本段整体删除。
//
//   Ctrl+' (FcitxKey_apostrophe) → 英文 ⇄ 拼音（B0）
//   Ctrl+\ (FcitxKey_backslash)  → 符号 ⇄ 拼音（B5）
//   Ctrl+Shift+F (FcitxKey_F)    → 简体 ⇄ 繁体（2026-10-09；键位沿用搜狗惯例）
// 三个键都**来回切**，不是单向进入：只能进不能出等于换个地方卡住。
// 不用 Ctrl+. ：那是 punctuation 模块的系统级占用（addon/punctuation.conf，
// Category=Module）；不用 ` ：拼音生态里那是 quickphrase。
// **不用 Ctrl+;（实测被剪贴板模块抢走）**：libclipboard.so 里就一行
// `Control+semicolon`，而该模块 Category=Module + OnDemand=False → 恒加载、
// 不分输入法。且它是 PreInputMethod 阶段的 watcher，排在输入法之前 ——
// 我们这边 filterAndAccept() 也拦不住它已经弹出来的面板（07-xx 的 harness 里
// e2e 阴性对照步实测到候选栏冒出剪贴板历史条目）。Ctrl+\ 是**逐个枚举**过
// 所有 fcitx5 库的 Control+/Super+/Alt+/Shift+ 组合后确认无占用的键。
//
// **为什么不改用可打印键**（2026-09-27 评估过，别再提，与 tsf-opi/src/vk.rs 同一条）：
// 裸键在中文模式下已被引擎的标点层认领 —— `\`→`、` 明写在 engine-core 的
// CHINESE_PUNCT，`` ` ``→`｀` 走 ascii_fullwidth 兜底，而中文模式**默认全角**；
// 裸 `'` 在缓冲为空时是引号。拿裸键当模式触发＝与用户裁决的标点功能抢同一个键。
//
// 不做键位可配置：一张两个条目的表，配置项等真有第二个人要改再说。
// 返回 true = 本键已被本插件消费。
//
// shift / fanjian：切模式后要同步的两个状态项（Ctrl+' 切换同样要更新它们 ——
// 否则状态项只对 Shift 那条路正确；繁体那条还要刷「中/英」，见 opiToggleTraditional
// 与两边的 setOnModeChanged）。指针允许为空？不 —— 从 .cpp 调用处传两个成员，恒有效。
inline bool handleModeHotkey(fcitx::KeyEvent &keyEvent, OpiShiftToggle &shift,
                             OpiFanjianToggle &fanjian) {
    // ---- Ctrl+Shift+F：简体 ⇄ 繁体（2026-10-09）----
    // 单独判、且**必须看 rawKey()**：normalize() 对我们不利 —— 源实现
    // （fcitx5 的 Key::normalize()）对「任何非空修饰键状态 + 小写字母」一律把小写
    // 折成大写（`states_ != Shift && isLAZ → sym += 'A'-'a'`），于是 key() 里
    // Ctrl+F 与 Ctrl+Shift+F **折出来一模一样**（sym='F'、states=Ctrl）——
    // 拿 key() 判会把应用们的「Ctrl+F 查找」一起吃掉。**实测抓到过**：面板驱动
    // 第 13 段第一版就红在这里（Ctrl+F 把模式从繁体打回了拼音）。
    // rawKey() 保留原样：Ctrl+F 是 'f'、Ctrl+Shift+F 是 'F'（X11 前端对
    // Shift+字母给的是大写 keysym），这才分得开。与 isRepeatEvent 用 rawKey
    // 是同一个理由：normalize 会毁信息（那边毁的是 Repeat 位）。
    // Shift 必须显式在 raw 的 states 里 + 排除 Alt（AltGr = Ctrl+Alt，非美式
    // 布局上的常见组合，别把 AltGr+F 也吞了）。
    {
        const fcitx::Key raw = keyEvent.rawKey();
        if (raw.sym() == FcitxKey_F &&
            raw.states().test(fcitx::KeyState::Ctrl) &&
            raw.states().test(fcitx::KeyState::Shift) &&
            !raw.states().test(fcitx::KeyState::Alt)) {
            if (!keyEvent.isRelease() && !isRepeatEvent(keyEvent)) {
                opiToggleTraditional(keyEvent.inputContext());
                shift.update(keyEvent.inputContext());
                fanjian.update(keyEvent.inputContext());
            }
            keyEvent.filterAndAccept();
            return true;
        }
    }
    // ---- Ctrl+Shift+Z：进入/退出符号浏览（22 类目，2026-10-10）----
    // 判据与上面 Ctrl+Shift+F **逐条同源**（rawKey、Shift 显式在位、排 Alt）：
    // normalize() 会把 Ctrl+z 与 Ctrl+Shift+z 折成同一个 key()（sym='Z'、states=Ctrl），
    // 拿 key() 判会把应用的「撤销」一起吃掉。本块是**唯一**决定这个键位的地方
    // （换键 = 改这一处；备选见下），动作只调 opiSymbolBrowseHook() 一处。
    //
    // 占用表实测（方法同 handleFullwidthHotkey 那段）：`strings` 扫本机全部 fcitx5
    // 库的 `^(Shift|Control|Super|Alt)\+` 默认组合，去重后是 Control+7 / Control+8 /
    // Control+KP_Enter / Control+period / Control+Return / Control+semicolon /
    // Control+Shift+KP_Enter / Control+Shift+Return / Control+Shift+U /
    // Control+Alt+E / Control+Alt+Shift+U / Shift+Tab / Super+grave / Super+semicolon
    // —— **含 Z 的零命中**；/usr/share/fcitx5/addon/*.conf 与用户 conf 里也搜不到。
    // ⚠️ 但它与**生态**冲突：编辑器/IDE 的 Ctrl+Shift+Z = 重做（与 Ctrl+Y 同位），
    // 中文态下会被本插件吃掉，用户按重做没反应。lead 2026-10-10 裁决**维持**这个
    // 键（搜狗 PC 的「符号大全」就是它，属用户预期），冲突如实记在
    // docs/install/linux.md 与交付说明里。备选（真要换时从这里挑）：
    // Ctrl+Shift+J（终端里是换行，冲突面更小）、Alt+Shift+Z、双击 Shift。
    // 用**切换**语义（不是只能进）：只能进不能出等于换个地方卡住，与上面三个热键同一条。
    {
        const fcitx::Key raw = keyEvent.rawKey();
        if (raw.sym() == FcitxKey_Z &&
            raw.states().test(fcitx::KeyState::Ctrl) &&
            raw.states().test(fcitx::KeyState::Shift) &&
            !raw.states().test(fcitx::KeyState::Alt)) {
            if (!keyEvent.isRelease() && !isRepeatEvent(keyEvent)) {
                // ≥0 = 入口已接线（-1 = 弱符号还是 nullptr，诊断已由唯一调用点打过）。
                // 返回值本身（进/出）不用于分支：进和出都要重推面板 —— 浏览态换了
                // 候选面（分类），退出则要把真缓冲的预编辑/候选还原上来。
                if (opiSymbolBrowseHook()(kOpiSymbolBrowseToggle) >= 0) {
                    refreshingPanelFor(keyEvent.inputContext());
                }
            }
            keyEvent.filterAndAccept();
            return true;
        }
    }
    // ---- Ctrl+' / Ctrl+\（通用门：key() 够用，这两个键符不是字母，normalize 不动它们）----
    const fcitx::Key key = keyEvent.key();
    if (!key.states().test(fcitx::KeyState::Ctrl)) {
        return false;
    }
    const int32_t cur = opi_fcitx5_mode();
    int32_t target = cur;
    switch (static_cast<uint32_t>(key.sym())) {
    case FcitxKey_apostrophe:
        target = (cur == kModeEnglish) ? kModePinyin : kModeEnglish;
        break;
    case FcitxKey_backslash:
        target = (cur == kModeSymbol) ? kModePinyin : kModeSymbol;
        break;
    default:
        return false;
    }
    // 抬起一并消费、但不再切。只拦按下的话，客户端会收到**无 keydown 的 keyup**
    // （依赖键状态的游戏/编辑器卡键）—— 与 input_method.rs 里 last_printable
    // 「抬起按按下时记下的结论回复」是同一个理由。不返回来切也是同一个道理：
    // 按下切过去、抬起再切回来，用户看到的是模式纹丝不动。**重复**同理（见上）。
    if (!keyEvent.isRelease() && !isRepeatEvent(keyEvent)) {
        if (target == kModeEnglish) {
            // 「中→英」与 Shift 轻按是同一条裁决（W1 直接确认：先退浏览、后提交
            // 缓冲原文 —— 模式分叉见 opiDirectConfirm）—— 必须赶在 switch_mode
            // 之前，它会清缓冲。Ctrl+\（符号模式）
            // **不**走这条：那不是「切英文」，缓冲本来也会被 switch_mode 清掉。
            opiDirectConfirm(keyEvent.inputContext());
        }
        opi_fcitx5_switch_mode(target);
        // 切模式会动缓冲/候选（Rust 侧 switch_mode → engine.switch_mode），面板
        // 必须跟着走：漏了这步，切到英文后候选栏会继续挂着上一次的拼音候选。
        refreshingPanelFor(keyEvent.inputContext());
        shift.update(keyEvent.inputContext());
        fanjian.update(keyEvent.inputContext());
    }
    keyEvent.filterAndAccept();
    return true;
}
