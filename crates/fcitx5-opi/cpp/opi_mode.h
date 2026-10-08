// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
//
// 模式切换的两条入口 + 中/英状态项：
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

// 面板推送函数（refreshingPanelFor）的定义在 opi_fcitx5.cpp、主声明在
// opi_engine.h。这里再声明一次：本头文件要独立于 opi_engine.h 可用
// （后者 include 了本文件，反向引用会成环）。同签名重复声明是合法的。
void refreshingPanelFor(fcitx::InputContext *ic);

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

    // 切换 + 刷面板 + 刷状态项。面板必须刷：切模式会清缓冲换候选
    // （与 handleModeHotkey 的处理同一条理由）。
    void toggle(fcitx::InputContext *ic) {
        const int32_t cur = opi_fcitx5_mode();
        const int32_t next = (cur == kModeEnglish) ? kModePinyin : kModeEnglish;
        if (next == kModeEnglish) {
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
    }

private:
    fcitx::Instance *instance_;
    fcitx::SimpleAction action_;
    fcitx::ScopedConnection connection_;
    bool armed_ = false;
    uint64_t pressTime_ = 0;
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
// indicator：切模式后同步中/英状态项（Ctrl+' 切换同样要更新它 —— 否则状态项只对
// Shift 那条路正确）。指针允许为空？不 —— 从 .cpp 调用处传 shiftToggle_，恒有效。
inline bool handleModeHotkey(fcitx::KeyEvent &keyEvent, OpiShiftToggle &indicator) {
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
                const int32_t cur = opi_fcitx5_mode();
                opi_fcitx5_switch_mode(cur == kModeTraditional ? kModePinyin
                                                               : kModeTraditional);
                refreshingPanelFor(keyEvent.inputContext());
                indicator.update(keyEvent.inputContext());
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
        opi_fcitx5_switch_mode(target);
        // 切模式会动缓冲/候选（Rust 侧 switch_mode → engine.switch_mode），面板
        // 必须跟着走：漏了这步，切到英文后候选栏会继续挂着上一次的拼音候选。
        refreshingPanelFor(keyEvent.inputContext());
        indicator.update(keyEvent.inputContext());
    }
    keyEvent.filterAndAccept();
    return true;
}
