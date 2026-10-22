// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
//
// addon 类（OpiEngine）的**声明**。拆出来只有一个理由：`opi_fcitx5.cpp` 有 500 行
// 硬门禁，加了「属性设置」之后越线（511）—— 与 `opi_json.h` / `opi_config.h`
// 同一个拆分先例。
//
// 实现留在 .cpp（keyEvent / keyEventImpl / loadDictionary 都要用文件内的静态量
// `g_pushedBuffer`）。为此 `refreshingPanelFor` 从文件级 `static` 改成**外部链接**：
// 声明在这里（reset() 的内联体要用它），定义仍在 .cpp。
//
// ⚠️ 本头文件不引入新的配置字段 —— 开关只在 `opi_config.h` 里声明一处。

#pragma once

#include <exception>
#include <cstdio>

#include <fcitx/addoninstance.h>
#include <fcitx/event.h>
#include <fcitx/inputcontext.h>
#include <fcitx/inputmethodengine.h>
#include <fcitx/instance.h>
#include <fcitx-utils/standardpath.h>

#include "opi_config.h"   // 「属性设置」：OpiSettings + read/apply/save
#include "opi_ctl.h"      // opi-ctl 接收侧（D-Bus 对象 + 注册；spike 结论在那边）
#include "opi_ctl_englishpro.h" // opi-ctl `englishpro` 动词/状态位
#include "opi_ctl_mode.h" // opi-ctl `mode` 动词（模式循环；500 行门禁拆出）
#include "opi_mode.h"     // 模式热键（Ctrl+' / Ctrl+\ / Ctrl+Shift+Z）+ Shift 轻按切换 + 状态项
#include "opi_rust_ffi.h"
#include "opi_skin.h"     // 皮肤：写 classicui.conf + 热重载
#include "opi_status.h"   // 状态区「设置」项（拉起设置窗）

void refreshingPanelFor(fcitx::InputContext *ic);

// fcitx::AddonInstance + fcitx::InputMethod 的组合基类在 fcitx5 5.1.x 里叫
// InputMethodEngine（fcitx/inputmethodengine.h:17，已继承 AddonInstance），自带
// `virtual void keyEvent(...) = 0` 与 `virtual void reset(...)`；仓库里没有别的
// `fcitx::InputMethod` 类型（grep Core/ 只有 InputMethodEngine V2/V3/V4），原先
// 多继承的两个基类都不存在。
class OpiEngine : public fcitx::InputMethodEngine {
public:
    // 初始化列表按**声明顺序**写（instance_ 声明在类尾，shiftToggle_ 在前）——
    // 都是拿构造参数初始化，纯粹为了过 -Werror=reorder。
    OpiEngine(fcitx::Instance *instance)
        : shiftToggle_(instance), fanjianToggle_(instance), settingsAction_(instance),
          ctlObject_([this](const std::string &action) { return ctlAction_(action); },
                     [this]() { return ctlState_(); },
                     [this](const std::string &keyword) { return ctlSearch_(keyword); }),
          instance_(instance) {
        loadDictionary();
        // 配置推给引擎必须在 loadDictionary 之后：Rust 侧单例在那一刻才建起来。
        // 导入词库的启动扫描也走这条路（applyOpiSettings → opi_dicts.h 的 opiDictsSync）。
        readOpiSettings(settings_);
        applyOpiSettings(settings_);
        // 皮肤**不在构造时应用**（理由在 opi_skin.h 头部）：只把读到的值记为
        // 「已应用」，免得每次 fcitx5 启动都拿配置里的旧值改写 classicui.conf、
        // 把用户后来在别处改的主题顶回去。
        lastSkin_ = *settings_.skin;
        // 两个状态项互相刷：模式一变，另一项的文案跟着变（「中/英」↔「简/繁」；
        // 从英文点「简」会切进繁体 ⇒「中/英」要回「中」，反之亦然）。
        shiftToggle_.setOnModeChanged(
            [this](fcitx::InputContext *ic) { fanjianToggle_.update(ic); });
        fanjianToggle_.setOnModeChanged(
            [this](fcitx::InputContext *ic) { shiftToggle_.update(ic); });
        // opi-ctl 通道（接收侧）：构造期先试一次。**失败不报错** —— 构造期拿不到
        // 多半只是加载顺序（dbus 模块是 Category=Module + OnDemand=False，正常在
        // 进程启动时就加载好了，此时 addon("dbus", true) 直接命中缓存），留给
        // activate() 再试一次并在那里作诊断（见 ensureCtlService_）。
        ensureCtlService_(false);
    }
    // ---- 「属性设置」：fcitx5 的 addon 配置（字段与落点见 opi_config.h）----
    // 改完 `fcitx5-remote -r` 走 reloadConfig；配置工具直接改走 setConfig；
    // 两者都立刻 apply 到引擎（不必重启输入法）。
    const fcitx::Configuration *getConfig() const override { return &settings_; }
    void setConfig(const fcitx::RawConfig &raw) override {
        settings_.load(raw, true);
        applyOpiSettings(settings_);
        applyOpiSkin(*settings_.skin, lastSkin_, instance_); // 值变了才动
    }
    void reloadConfig() override {
        readOpiSettings(settings_);
        applyOpiSettings(settings_);
        applyOpiSkin(*settings_.skin, lastSkin_, instance_);
    }
    void save() override { saveOpiSettings(settings_); }
    // 状态区三项（中/英、简/繁、设）随激活挂上去（见 opi_mode.h 的 attach；
    // 该组在下次 activate 前由 core 自动清空，不必自己去重）。
    void activate(const fcitx::InputMethodEntry & /*entry*/,
                  fcitx::InputContextEvent &event) override {
        shiftToggle_.attach(event.inputContext());
        fanjianToggle_.attach(event.inputContext());
        settingsAction_.attach(event.inputContext());
        // opi-ctl 的 D-Bus 服务在这里才是可靠的注册时机：守护进程起完了、事件循环
        // 在跑、dbus 模块在。构造期已成功的话这里是一条空操作。
        // 无 IC 时状态栏按钮也不存在，所以「服务恰好在这时才有」不构成缺口。
        ensureCtlService_(true);
    }
    void keyEvent(const fcitx::InputMethodEntry &entry,
                  fcitx::KeyEvent &keyEvent) override;
    // ---- 切走 / 切英文：先退浏览，再按 W1 提交（按模式分叉：拼音/繁体缓冲原文、
    //      符号模式不上关键字）；焦点移出 / 重置：静默清空 ----
    //
    // 落点为什么是 deactivate() 而不是 reset()（**取证过的，不是猜**）：
    //   · fcitx5 5.0.21 的 src/lib/fcitx/instance.cpp 里，FocusOut watcher 与
    //     SwitchInputMethod watcher **都**调 deactivateInputMethod() → engine->deactivate()；
    //     只有 InputContextReset 走 engine->reset()。即 reset() **收不到**「切输入法」
    //     「焦点移出」—— 「切走时把拼音交出去」写在 reset() 里永远不触发。
    //   · 类型判别两版都有（fcitx/event.h）：EventType::InputContextFocusOut
    //     ( = InputContextEventFlag|0x4 ) 与 EventType::InputContextSwitchInputMethod
    //     ( = |0xA )，5.0.21 与 5.1.12 **逐值相同**，`Event::type()` 两版也都有。
    //     ⇒ 直接按事件类型分支，不需要任何版本探测。
    //   · 不 override 的话基类的 deactivate 只是转发给 reset()（inputmethodengine.h
    //     两版都写 `deactivate(...) { reset(...); }`），那「提交」这一半就没地方落。
    //
    // 覆盖面（lead 2026-10-10 裁决）：
    //   · SwitchInputMethod → 先退符号浏览、再按 W1 提交（opiDirectConfirm：拼音/
    //     繁体**缓冲原文**、符号模式**不上关键字**，2026-10-10 深夜裁决 + 同夜模式
    //     分叉；顺序是契约，见那个函数的注释）。能力位
    //     变化（进密码框之类）发的也是这个事件，
    //     所以「打了半截拼音点了密码框」会把拼音交出去 —— 与「切走不丢输入」同一意图。
    //   · FocusOut / InputContextReset → **静默清空**，绝不提交（用户裁决：焦点移出
    //     不能提交）。两者都落到 clearState_。
    void deactivate(const fcitx::InputMethodEntry & /*entry*/,
                    fcitx::InputContextEvent &event) override {
        // 同 keyEvent：这个入口抛出去也会带走整个守护进程。
        try {
            if (event.type() == fcitx::EventType::InputContextSwitchInputMethod) {
                opiDirectConfirm(event.inputContext());
            }
            clearState_(event.inputContext());
        } catch (const std::exception &e) {
            fprintf(stderr, "fcitx5-opi: deactivate 异常，已忽略: %s\n", e.what());
        } catch (...) { fprintf(stderr, "fcitx5-opi: deactivate 未知异常，已忽略\n"); }
    }

    void reset(const fcitx::InputMethodEntry & /*entry*/,
               fcitx::InputContextEvent &event) override {
        // 同 keyEvent：这个入口抛出去也会带走整个守护进程。
        try {
            clearState_(event.inputContext());
        } catch (const std::exception &e) {
            fprintf(stderr, "fcitx5-opi: reset 异常，已忽略: %s\n", e.what());
        } catch (...) { fprintf(stderr, "fcitx5-opi: reset 未知异常，已忽略\n"); }
    }

    // opi-ctl 的动作分发（接收侧）。动作串是**冻结的契约**（任务书）：
    //   symbols|trad|punct|fullwidth|lang —— 新增动作要先找 lead 定，别自己加。
    //   mode（2026-10-14 用户裁决加）：模式循环 拼→英→（未来五笔）；是**唯一**
    //   成功应答不是 "ok" 而是状态 token（pinyin/en26）的动作（协议见 opi_ctl.h）。
    //   insert:<文本>（2026-10-12 用户裁决加）：桌面大页面（符号/表情）的上屏通道 ——
    //   自绘窗自己没有提交通道（那在插件进程里），文本经既有 Trigger(string) 下沉到这里。
    //   clearuserwords（2026-10-23 加）：清空引擎学习记忆；桌面设置窗的确认按钮调用。
    // 这里跑在**主循环线程**（dbus 模块把总线挂在自己的事件循环上），所以可以安全地
    // 碰引擎单例与 IC，也就能满足「作用于当前 IC」。
    // 只读查询 state **不在**这张表里：它走另一个 D-Bus 方法（vtable 的 State →
    // ctlState_），动作与查询在协议上分开（见 opi_ctl.h 的 State 段）。
    std::string handleCtlAction_(const std::string &action) {
        // 「当前 IC」= 最近获得焦点的那个；没有 IC 时动作仍会改引擎状态（单例），
        // 只是没有面板可刷 —— 不是错误。
        fcitx::InputContext *ic =
            instance_ != nullptr ? instance_->mostRecentInputContext() : nullptr;
        if (action.rfind("insert:", 0) == 0) {
            // 大页面上屏（见动作表注释）：提交到**当前焦点 IC**。
            const std::string text = action.substr(7);
            if (text.empty()) {
                return "err:5:insert 的文本为空";
            }
            if (ic == nullptr) {
                // 没有焦点窗口 = 现在没有地方可上屏。给**可见原因**（非零退出码），
                // 别静默成功 —— 与 symbols 那条「入口禁用报 4」同一条纪律。
                return "err:4:没有焦点输入框（先点到要输入的地方再试）";
            }
            ic->commitString(text);
        } else if (action == "symbols") {
            // 三态（-1 未接线 / 0 不在浏览 / 1 在浏览），0 是二义的（见 opi_mode.h）：
            // 「数据没装上、入口禁用」与「刚从浏览里退出来」都返回 0。用本类记的
            // 装载态 symbolDataOk_ 把它们分开 —— 数据没装上时报 4 而不是哑成功，
            // 否则状态栏按钮点了没反应还报成功（lead 明确要的「非零=可见反馈」）。
            const int32_t st = opiSymbolBrowseHook()(kOpiSymbolBrowseToggle);
            if (st < 0) {
                // 引擎侧入口还没接线 —— 用 4（功能不可用）而不是 5，
                // opi-ctl 会原样把它当进程退出码（desktop-ui 按非零=可见反馈处理）。
                return "err:4:符号浏览入口未接线（引擎侧未落地），本次动作无效";
            }
            if (st == 0 && !symbolDataOk_) {
                // 措辞覆盖装载态为假的**三条路**（locate 空串 / 弱符号没接线 /
                // Rust 侧读不出或零合法类目 —— 见 symbolDataOk_ 的注释）：原先只写
                // 「$XDG_DATA_DIRS 下没有 …」，在「文件在但读了是零合法类目」时是错的
                // （harness 的 data-nosym 现在就用零字节文件表达这一支）。
                return "err:4:符号类目数据缺失或读不出（opi/symbol_categories.tsv），"
                       "本次动作无效";
            }
            refreshingPanelFor(ic);
        } else if (action == "trad") {
            // 与 Ctrl+Shift+F、状态栏「简/繁」项**共用** opiToggleTraditional
            // （两份实现会在「英文/符号模式里按下时报切到哪」这类角落上漂）。
            opiToggleTraditional(ic);
            fanjianToggle_.update(ic);
            shiftToggle_.update(ic);
        } else if (action == "punct") {
            // 中文标点开关：改的是**配置面**（opi_config.h 的 ChinesePunct）——
            // 与配置工具/设置窗读的是同一个值，改完落盘 ⇒ 下次 reloadConfig 不会
            // 把它顶回去、设置窗显示的也是新值（内存里翻一个 bool 会两头都错）。
            // 落盘**失败要可见**（2026-10-10 裁决）：回滚内存+引擎后报 err:5 ——
            // 静默吞返回值 =「按钮报成功、重启后值又自己变回去」的幽灵。
            // trad/lang/fw **不落盘**：模式类开关（会话内偏好），不是默认类值。
            const bool want = !settings_.chinesePunct.value();
            settings_.chinesePunct.setValue(want);
            applyOpiSettings(settings_);
            if (!saveOpiSettings(settings_)) {
                settings_.chinesePunct.setValue(!want);
                applyOpiSettings(settings_);
                return "err:5:中文标点配置保存失败（conf/opi_fcitx5.conf 写不进去？），"
                       "本次切换已回滚";
            }
        } else if (action == "fullwidth") {
            opi_fcitx5_toggle_fullwidth();
        } else if (action == "lang") {
            // 中/英切换 = 引擎 English 模式（**不是** `fcitx5-remote -t`：那是整个
            // IM 的激活开关，会让状态栏文案不同步、且之后 OPI 热键全部失效 ——
            // 2026-10-10 裁决）。与 Shift 轻按共用 toggle（含 W1 直接确认与状态项刷新）。
            shiftToggle_.toggle(ic);
        } else if (action == "englishpro") {
            if (!opiCtlToggleEnglishPro()) return "err:4:英文专业入口未接线（插件版本过旧）";
            refreshingPanelFor(ic);
        } else if (action == "clearuserwords") {
            // 清空用户词（学习记忆）。弱符号：老 .so 上按未接线报 4，别哑成功。
            if (opi_fcitx5_clear_user_words == nullptr || !opi_fcitx5_clear_user_words()) {
                return "err:4:清空用户词不可用（引擎未装载或插件版本过旧）";
            }
            refreshingPanelFor(ic);
        } else if (action == "mode") {
            // 模式循环（用户裁决 2026-10-14；实现、派生与五笔插入点都在
            // opi_ctl_mode.h 的 opiCtlCycleMode —— 本文件 500 行门禁）。与 lang
            // 今天同效、**语义不同轴**：lang 是二值切换，本动词是 N 档循环，
            // 五笔落地后两者分家；返回的是新状态 token，不是 "ok"。
            return opiCtlCycleMode(ic, shiftToggle_, fanjianToggle_);
        } else {
            return "err:5:未知动作：" + action;
        }
        return "ok";
    }

    // opi-ctl search（只读查询，2026-10-13）的入口：关键字 → 命中条目的 JSON 文本
    // 数组，或 "err:<码>:<原因>"（协议见 opi_ctl.h 的 Search 段）。
    // public 与 handleCtlAction_ 同理由：C++ 驱动直接调它（vtable 的 searchHandler_
    // 就是这个函数，测到的就是 D-Bus 回复出去的那一串）。
    //
    // · 空关键字 = **用法错误**（err:2）—— opi-ctl 的 2 就是「用法错误」，本侧不另
    //   造码。引擎侧 `search("")` 的语义是**全部条目**（Android searchSymbols("")
    //   同义），把它当搜索结果回出去是答非所问（桌面搜索框清空时该回浏览态，不发
    //   这一问）。opi-ctl 本地也拒一次（双保险：手发 dbus-send 的人不会绕过）。
    // · 弱符号为 nullptr（引擎侧未落地）⇒ err:4（功能不可用），同符号浏览那条 ——
    //   别静默回空数组：桌面会把它显示成「没搜到」，那是**假成功**。
    // · 类目数据装载态（symbolDataOk_）**不参与**判据：搜索表是编译内嵌的
    //   （SymbolEngine::builtin），没装 symbol_categories.tsv 也照样能搜。
    // · **不取 IC**：搜索是进程级单例里的数据，与「当前焦点窗口」无关 ⇒ 没有 IC
    //   也照常回（与 state 同一条裁决），失败面比 insert/symbols 小一格。
    std::string handleCtlSearch_(const std::string &keyword) {
        if (keyword.empty()) {
            return "err:2:搜索关键字为空（用法错误：opi-ctl search 需要一个关键词）";
        }
        if (opi_fcitx5_search_symbols == nullptr) {
            return "err:4:符号搜索入口未接线（引擎侧未落地），本次查询无效";
        }
        return take(opi_fcitx5_search_symbols(
            reinterpret_cast<const uint8_t *>(keyword.data()), keyword.size()));
    }

    // opi-ctl state（只读查询）的入口。格式与「没有 IC 不是失败」的裁决写在
    // opi_ctl.h 的 State 注释；四个值各自的出处写在 stateLine_ 上。
    // public 与前一个同理由：C++ 驱动直接调它，验的就是 D-Bus 方法回复出去的那一行
    // （vtable 的 stateHandler_ 就是这个函数）。
    std::string ctlState_() noexcept {
        try {
            return stateLine_();
        } catch (const std::exception &e) {
            // 与 ctlAction_ 同一个外壳：dbus 的分发路径不捕 C++ 异常。
            return std::string("err:5:内部异常：") + e.what();
        } catch (...) {
            return "err:5:内部异常（非 std::exception）";
        }
    }

private:
    // 「回到无 composing」的唯一实现：deactivate 与 reset 共用。
    void clearState_(fcitx::InputContext *ic) {
        opi_fcitx5_clear();
        // Shift「轻按」判定态也要清：按住 Shift 时失焦，抬起事件可能到不了
        // 本 IC（或到了别的 IC），不清就是一次「幽灵切换」（opi_mode.h）。
        shiftToggle_.disarm();
        // Rust 侧清了缓冲，但面板是 fcitx5 侧的状态：不一并清，失焦/切换
        // 输入法后预编辑串与候选栏会继续挂在屏幕上（原码只清了引擎，没碰面板）。
        refreshingPanelFor(ic);
    }

    // opi-ctl 的 D-Bus 处理器外壳：**异常在这里挡住**。dbus 的分发路径不捕 C++
    // 异常，逃出去就是守护进程死（与 keyEvent 那条同一类风险）。
    // 返回串是跨进程协议："ok" / "err:<退出码>:<原因>"（见 opi_ctl.h）。
    std::string ctlAction_(const std::string &action) noexcept {
        try {
            return handleCtlAction_(action);
        } catch (const std::exception &e) {
            return std::string("err:5:内部异常：") + e.what();
        } catch (...) {
            return "err:5:内部异常（非 std::exception）";
        }
    }

    // search 的外壳：与 ctlAction_ 逐条同构（同一个 D-Bus 分发路径，同样不捕 C++
    // 异常）。转发本身要走 std::function 与 take()（拷贝 JSON），都可能 bad_alloc。
    std::string ctlSearch_(const std::string &keyword) noexcept {
        try {
            return handleCtlSearch_(keyword);
        } catch (const std::exception &e) {
            return std::string("err:5:内部异常：") + e.what();
        } catch (...) {
            return "err:5:内部异常（非 std::exception）";
        }
    }

    // ---- opi-ctl state 的读侧（冻结格式见 opi_ctl.h 的 State 注释）----
    //
    // 六个值全部来自**既有 getter**，不新造状态：
    //   · lang  = 引擎模式 == English —— 与「中/英」状态项同一条判据（opi_mode.h 的
    //     update()）；zh 覆盖拼音/繁体/符号/数字四个「中文系」模式。
    //   · trad  = 引擎模式 == Traditional —— 与「简/繁」状态项同一条判据。
    //   · punct = **生效值**（引擎直读口，2026-10-15 改）：中文类模式 = 配置面 ChinesePunct，
    //     英文/数字/符号恒 off —— 配置值不随模式变，当显示值就是「切英文后还画 。，」。
    //   · fw    = opiFullwidthState_()（Rust 只读出口，见那里）。
    //   · im    = 最近 IC 的当前输入法唯一名（Instance::currentInputMethod()，
    //     2026-10-10 追加）—— **唯一与 IC 有关的一格**，给桌面判「状态栏该不该
    //     显示」（== "opi" 显示，其他隐藏；见 opi_ctl.h 的 State 注释）。
    //   · mode  = 档位 token（`pinyin` / `en26`；2026-10-14 追加，**追加在行尾**）——
    //     与 `opi-ctl mode` 的应答**同一张 token 表**（opi_ctl_mode.h 的
    //     opiCtlStateModeToken：面外态收成 pinyin，见那里的改判记录）；状态栏的
    //     初始档读的就是这一格。
    // 除 im 外都不取 IC：都是进程级的（Rust 单例 + 本 addon 的配置），所有 IC 同值 ⇒
    // 「没有 IC」不是失败。im 那一格查不到时给**空串**（前端约定：未知=维持现状），
    // 同样不是失败 —— 桌面每秒轮询时常常没有焦点窗口（理由细写在 opi_ctl.h）。
    // 动作路径（handleCtlAction_）另取 IC —— 那是为了刷面板，与这里理由不同。
    std::string stateLine_() {
        const int32_t mode = opi_fcitx5_mode();
        std::string s = "lang=";
        s += (mode == kModeEnglish) ? "en" : "zh";
        s += ";trad=";
        s += (mode == kModeTraditional) ? "on" : "off";
        s += ";punct="; // 生效值、不是配置值（2026-10-15 改；理由见上面 punct 那一条）
        s += opi_fcitx5_effective_chinese_punct() ? "on" : "off";
        s += ";fw=";
        s += opiFullwidthState_() ? "on" : "off";
        s += ";im=";
        // currentInputMethod() 就是「最近 IC 的输入法唯一名，没有 IC 给空」——
        // 空 ic 的保护在 fcitx5 里面（它的 inputMethodEntry 先取 inputMethod()，
        // 空名直接 nullptr；两版都导出该符号，nm -D 核过）。本层只需守 instance_：
        // 驱动用 nullptr 构造引擎。密码框/Disable 的 IC 按 fcitx5 语义返回
        // keyboard-<布局> ⇒ 那种 IC 里状态栏本就该收起，与语义一致。
        if (instance_ != nullptr) {
            s += instance_->currentInputMethod();
        }
        s += ";mode=";
        // 档位 token（2026-10-14 追加，**行尾**：旧前端按位置读前面几段，不看末格）。
        // 派生与 `opi-ctl mode` 同一份 —— opi_ctl_mode.h 的 opiCtlStateModeToken。
        s += opiCtlStateModeToken(mode);
        s += opiCtlEnglishProField();
        return s;
    }

    // 全角读侧：走 Rust 的只读出口 `opi_fcitx5_fullwidth()`（opi_rust_ffi.h）。
    // **别再用 toggle 两次借读** —— 状态栏每秒调 State，读路径不该改引擎状态，
    // 哪怕净效应为零。**不落本地镜像**：平台侧记副本必然漂（engine_punct.rs 对
    // chinese_punct 写过同一句）。
    bool opiFullwidthState_() { return opi_fcitx5_fullwidth(); }

    // 注册 opi-ctl 的 D-Bus 服务（构造期试一次、activate 再试一次）。见 opi_ctl.h 的
    // spike 段：注册对象是**本类的成员** ctlObject_，它析构时自动从总线注销。
    bool ensureCtlService_(bool warnOnFailure) {
        if (ctlRegistered_) {
            return true;
        }
        if (!opiRegisterCtlService(instance_, ctlObject_)) {
            // 只在**后一次**（activate）报错，且只报一次：构造期那次失败多半只是
            // 加载顺序，报出来是假警报；每个 IC 激活都报会刷屏。
            if (warnOnFailure && !ctlWarned_) {
                ctlWarned_ = true;
                fprintf(stderr,
                        "fcitx5-opi: opi-ctl 通道没接上（原因见上一行）—— 状态栏的"
                        "符号/简繁/标点/全角/中英按钮会报失败；插件其余功能不受影响\n");
            }
            return false;
        }
        ctlRegistered_ = true;
        return true;
    }

    OpiSettings settings_;

    // Shift 轻按切换 + 「中/英」状态项（判定与状态项都在 opi_mode.h）。
    // 构造用构造参数 instance，不读 instance_ 成员 —— 与成员声明顺序无关。
    OpiShiftToggle shiftToggle_;

    // 「简/繁」状态项（opi_mode.h）与「设置」项（opi_status.h）—— 外层入口。
    OpiFanjianToggle fanjianToggle_;
    OpiSettingsAction settingsAction_;

    // opi-ctl 的 D-Bus 对象（opi_ctl.h）。**是成员**：它析构时自动从总线注销，
    // 所以 addon 被卸载时不会在总线上留下指向已释放对象的 vtable。
    // 声明位置要在 instance_ 之前 —— 初始化列表按声明顺序走（-Werror=reorder）。
    OpiCtlObject ctlObject_;
    bool ctlRegistered_ = false;
    bool ctlWarned_ = false;

    // 符号类目数据装载态（loadDictionary 里定，只写一次；handleCtlAction_ 的
    // symbols 动作要读）。false 的两条路：文件不在（locate 空串）/弱符号没接线/
    // Rust 侧读不出（返回 -1）—— 对调用方是同一种「入口禁用」。
    bool symbolDataOk_ = false;

    // 皮肤「已应用」的最后值（opi_skin.h）：构造时只记住、不应用。
    OpiSkin lastSkin_ = OpiSkin::Keep;

    // keyEvent 的实现体。拆出来只为了在唯一的虚函数入口上套 try/catch ——
    // fcitx5 core **不捕获**引擎回调抛出的异常（实测：裸抛 → std::terminate →
    // 整个守护进程死掉）。Rust 侧每个导出入口都套了 catch_unwind，两侧对齐：
    // 把「整个输入法挂掉」降级成「这一次按键无效」。
    void keyEventImpl(const fcitx::InputMethodEntry &entry,
                      fcitx::KeyEvent &keyEvent);

    // XDG 数据目录下找 OPI 词库（B3：Rust 侧 opi_fcitx5_init_dict 把插件分发的
    // luna.opid 拷到 $XDG_DATA_HOME/opi/luna.opid，镜像 Android EngineLoader.
    // FILE_NAME；此处探测同一路径）。B3 接线在验收阶段完成。
    //
    // **必须接返回值**：Rust 侧 `install()` 是 `CandidateState::load(path)?` ——
    // 坏词库在 `?` 处提前返回，单例保持 None，`with_state` 恒为 None，所有导出
    // 函数退化成空操作。而 `install(坏路径) -> Err` 是 Rust 侧**有意**的语义
    // （lib.rs 的 install_singleton_fallback_and_path 断言「坏路径 → Err，不回
    // 退」），所以回退必须由本层做 —— 这正是原码丢返回值造成的洞：词库**存在但
    // 损坏**（下载不全/拷贝中断）时，插件进入「已加载、已注册、按键被接受、但一个
    // 字都不出，且任何地方都没有错误信息」的状态。实测（XDG_DATA_HOME 指向 6 字节
    // 垃圾 luna.opid）：四个键全 action=0；接上返回值后同场景回退内置词库，四键
    // action=1/1/1/2 并提交 '好'。
    //
    // 注意 `opi_fcitx5_load` 收的是**路径**不是词库内容（Rust 侧 read_utf8 →
    // install），故传 `path.data()` 是对的，不要改成传内容。
    void loadDictionary() {
        // StandardPath 自 5.1.21 起弃用，但**不能迁移**（老版本没有 StandardPaths）。
        // 为什么 / 边界在哪 / 何时删这三件事只写在一处：cpp/CMakeLists.txt 的
        // 「为什么两个 .cpp 里有 -Wdeprecated-declarations」段 —— 别在这里复制。
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wdeprecated-declarations"
        auto path = fcitx::StandardPath::global().locate(
            fcitx::StandardPath::Type::Data, "opi/luna.opid");
        // 繁体词典是**可选增强**（2026-10-09 接线）：装了就一起传给引擎
        // （Traditional 模式查它），没装/坏了都退回简体，不影响拼音可用性 ——
        // 语义分别钉在 opi_rust_ffi.h 的声明注释与 candidate.rs 的 load_with_trad。
        // 落点常量与 luna 同一条路（Type::Data + opi/），由 opi_locate_check 的
        // 第 4 个参数与 ci.yml 的「查找常量没漂」grep 一并守着。
        auto trad = fcitx::StandardPath::global().locate(
            fcitx::StandardPath::Type::Data, "opi/trad.opid");
        // 符号类目数据（22 类目，engine-track 2026-10-10 定稿）：**同一约定** ——
        // 这里只 locate 路径，Rust 侧读文件。路径为空就跳过调用（零噪音）。
        // 落点常量与 luna/trad 同一条路（Type::Data + opi/），由 opi_locate_check 的
        // 第 5 个参数与 ci.yml 的「查找常量没漂」grep 一并守着。
        auto sym = fcitx::StandardPath::global().locate(
            fcitx::StandardPath::Type::Data, "opi/symbol_categories.tsv");
#pragma GCC diagnostic pop
        const bool ok =
            !path.empty() &&
            opi_fcitx5_load_with_trad(
                reinterpret_cast<const uint8_t *>(path.data()), path.size(),
                reinterpret_cast<const uint8_t *>(trad.data()), trad.size());
        if (!ok) {
            if (!path.empty()) {
                // 「文件在、装不上」（下载不全/拷贝中断）。这行日志是必要的：否则
                // 词库损坏与「本来就没装词库」在用户侧无法区分，只表现为候选质量骤降。
                fprintf(stderr, "fcitx5-opi: 词库 %s 装载失败，回退内置词库\n", path.c_str());
            }
            opi_fcitx5_load(nullptr, 0); // 不存在或装载失败 → 内置回退词库
        }
        // 符号类目数据与拼音可用性**解耦**：装不上只让符号浏览入口保持禁用
        // （引擎侧「无数据 = 不动」；opi-ctl/热键会报错或被诊断），其余功能不受影响。
        // 装载态记下来 —— symbols 动作靠它把「数据缺失」与「刚退出浏览」分开
        // （两者都返回 0，见 opi_mode.h 的三态说明）。
        // 弱符号为 nullptr（引擎侧未落地）时静默跳过：那种情况由浏览入口的
        // 一次性诊断负责报出来，这里再报就是重复噪音。
        if (!sym.empty() && opi_fcitx5_load_symbols != nullptr) {
            // 返回值：-1 = 读不出 / -2 = 零合法类目（都禁用入口）；≥0 = 跳过的坏行数
            // （0 = 全好，可用）。判据就一条 `>= 0`。日志由 Rust 侧打。
            symbolDataOk_ = opi_fcitx5_load_symbols(
                                reinterpret_cast<const uint8_t *>(sym.data()),
                                sym.size()) >= 0;
        }
    }

    fcitx::Instance *instance_;
};
