// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
//
// `opi-ctl` 的**接收侧**：addon 自己挂一条 D-Bus 服务，供桌面状态栏（desktop-ui
// 用 ProcessBuilder 调 `/usr/bin/opi-ctl <action>`）驱动本插件。
//
// ---- spike 结论与**拒绝理由**（2026-10-10，lead 已批）----
//
// 选路：**fcitx5 自带的 dbus 包装**（`fcitx-utils/dbus`），从 dbus 模块的导出函数
// 拿**同一个**会话总线，再挂我们自己的对象：
//     instance->addonManager().addon("dbus", true)
//       -> callWithSignature<fcitx::dbus::Bus *()>("DBusModule::bus")
//       -> bus->addObjectVTable("/opi", "org.fcitx.Fcitx5.Opi1", obj)
// 三条理由：
//   · **作用于当前 IC**：dbus 模块把总线挂在 instance 的事件循环上 ⇒ 方法回调在
//     **主循环线程**里跑，于是可以直接 `Instance::mostRecentInputContext()` 施加动作
//     并刷面板。这正是任务书要的「命中当前 IC」，不是「reload 之后全局生效」。
//   · **零新构建依赖**：不需要 glib、不需要 Rust 侧新 crate、不需要新的 .so；
//     符号就在已经链接的 libFcitx5Utils 里。
//   · **双版本都成立**（取证不止读头）：5.0.21 与 5.1.12 的头都有
//     <fcitx-utils/dbus/{bus,objectvtable}.h>，且这些头**不 include sd-bus**
//     （没有 libsystemd-dev 这条编译依赖）；另外解包 5.0.21 的运行时包用
//     `nm -D libFcitx5Utils.so.5.0.21 | c++filt` 逐字核过：
//     `fcitx::dbus::Bus::addObjectVTable(std::string const&, std::string const&,
//     fcitx::dbus::ObjectVTableBase&)`、`ObjectVTableBase::addMethod(...)`、
//     `ObjectVTableMethod::ObjectVTableMethod(...)` **都已导出** ⇒ 5.0.21 上链接得过。
//   · `callWithSignature` 的签名格式来自 addoninstance.h 的
//     `AddonFunctionAdaptorErasure<Ret(Args...)>`（两版本一致）。
//
// 被否的两条（别再回头试）：
//   × **GDBus**（cpp 侧自己连总线）：要新加 glib 的编译/链接依赖，还得自己写
//     introspection XML 与参数编解码 —— 为零新依赖这条就让路了。
//   × **zbus**（Rust 侧接管）：重异步栈，而 cdylib 里没有 executor（要么再引
//     runtime，要么自己 pump），且这是宿主接线，属 C++ 域的活。
//   × **写配置文件 + `fcitx5-remote -r`**（设置窗既有的那条路）：那是全局
//     ReloadConfig —— 会把所有 addon 的配置重读一遍（与别的 addon 的 reload 竞争、
//     时序不可控），而且它**不是**对当前 IC 施加动作。任务书要「作用于当前 IC」，
//     这条路从语义上就不满足。
//
// ---- 失败面（如实记）----
// 拿不到 dbus 模块、或它的总线没连上（无会话总线/连接失败）：**没有 opi-ctl 通道**，
// 插件其余功能一律不受影响。此时发一行 stderr，不静默（调用方是状态栏按钮，
// 它拿到的失败是 opi-ctl 的非零退出码 + 原因）。
//
// ---- 生命周期 ----
// 对象由调用方（OpiEngine 成员）持有；`ObjectVTableBase` 的析构**自动**从总线注销
// （头文件原文："The object will automatically unregister itself upon destruction"），
// 所以 addon 被卸载时不会在总线上留一个指向已释放对象的 vtable。
// 总线指针**不**缓存（每次注册时现取）—— 缓存它就得管 dbus 模块先卸载的时序。

#pragma once

#include <cstdio>
#include <functional>
#include <string>
#include <utility>

#include <fcitx/addoninstance.h>
#include <fcitx/addonmanager.h>
#include <fcitx/instance.h>
#include <fcitx-utils/dbus/bus.h>
#include <fcitx-utils/dbus/objectvtable.h>

// D-Bus 落点/接口名。**只有 opi-ctl 需要知道它们**（desktop-ui 只调 opi-ctl，
// 不直接发 dbus），改这里要同步 crates/opi-tools/src/bin/opi-ctl.rs 的同名常量。
inline constexpr const char *kOpiCtlPath = "/opi";
inline constexpr const char *kOpiCtlInterface = "org.fcitx.Fcitx5.Opi1";

// 动作/查询 → 结果串。结果串就是**返回值协议**（唯一的跨进程契约）：
//   · Trigger（动作，入参是动作串）：
//       "ok"                成功
//       "err:<退出码>:<原因>" 失败，退出码给 opi-ctl 原样当进程退出码（见 .rs 侧注释：
//                            4 = 功能不可用/未接线，5 = 动作被拒或内部错误）
//     ⚠️ **唯一例外**：`mode` 动作（2026-10-14 加）成功时的应答是**新状态 token**
//     （`pinyin` / `en26`，未来 `wubi`）而不是 "ok" —— 桌面状态栏的模式按钮要拿它
//     立即显示新档位（见 opi_ctl_mode.h 的派生段与 opi-ctl.rs 的 check_mode_shape）。
//     其余动作一字不动。
//   · State（只读查询，**无入参**：2026-10-10 用户裁决加）：
//       一行冻结格式的状态串（见下），或同样的 "err:<码>:<原因>"
//   · Search（只读查询，**一个入参**：2026-10-13 加，桌面符号/表情页搜索）：
//       命中条目的 **JSON 文本数组**（`["…"]`，`[]` = 没搜到），或 "err:<码>:<原因>"。
//       空关键字在处理器里就被挡成 err:2（用法错误）—— 见 opi_engine.h 的
//       handleCtlSearch_；这一层只管搬运。
// 返回值用字符串而不是元组：`FCITX_OBJECT_VTABLE_METHOD` 的 RET 参数支持多值
// struct，但那要两端都对上 fcitx 的 tuple 映射；这里只有一个短串，字符串够用。
using OpiCtlHandler = std::function<std::string(const std::string &)>;
using OpiCtlStateHandler = std::function<std::string()>;
using OpiCtlSearchHandler = std::function<std::string(const std::string &)>;

// ---- State 的返回串：**冻结格式**（桌面状态栏每秒轮询它渲染图标：月亮盈亏 =
// 全/半角，zh/en = 语言）----
//     lang=zh|en;trad=on|off;punct=on|off;fw=on|off;im=<输入法唯一名，可为空>;mode=<档位 token>
// punct 那一格是**生效值**（2026-10-15 改，取值域与键序一字未动）：中文类模式 =
// 配置面 ChinesePunct，英文/数字/符号恒 off —— 引擎按模式分派（`engine_punct.rs`
// 的 `effective_chinese_punct`）。此前读配置值 ⇒ 切英文后仍报 on，状态栏那个
// 「。，」按钮看着像没联动（用户裁决「切换中英文的时候，标点符号也联动切换」；
// 引擎**行为本来就对**，改的只是显示）。
// 键序**固定**（桌面端按位置解析）；值全部来自**既有 getter**，不新造状态
// （每一格的出处写在 opi_engine.h 的 stateLine_ 上）。改这个格式要三处同改：
// 这里、opi-ctl.rs 的 check_state_shape、desktop 的解析处。
// im（2026-10-10 追加）：最近 IC 的当前输入法 uniqueName —— 桌面拿它判「本输入法
// 在不在前台」（== "opi" 显示状态栏，其他隐藏）。**追加在行尾**是为兼容：旧桌面
// 只解析前四段（按位置），多出来的一段不会让它读错位。取值域是**任意串**（可为空）。
// 取值 = fcitx5 的 Instance::currentInputMethod()（= mostRecentInputContext 的
// IM）：**没有「当前 IC」时不是失败** —— 前四格是进程级单例（Rust 引擎的模式/
// 全角 + addon 的配置面），所有 IC 同值；im 那一格交给 fcitx5 自己回答：
// ShareInputState=All 时它拿 dummy IC 兜底（= 下一扇窗会拿到的 IM），No 才是
// **空串**（前端约定：未知 = 维持现状，不闪）。⇒ 无 IC 时照常返回同一行、rc 0。
// 桌面每秒轮询，且轮询时**常常没有焦点窗口**，把它判成错误会让图标在这类时刻
// 闪成「读不到」。
// mode（2026-10-14 追加）：档位 token，取值域与 `opi-ctl mode` 的成功应答**同域**
// （上面 Trigger 那条的例外段）—— `pinyin` / `en26`，五笔落地后含 `wubi`。桌面
// 状态栏的模式按钮拿它定**初始档**（此前只能记住自己点出来的档，条一重启就与现实
// 不符）。**同样追加在行尾**，理由与 im 同。派生 = 同一张 token 表（opi_ctl_mode.h
// 的 opiCtlStateModeToken = opiCtlModeToken + 「面外态收成 pinyin」，2026-10-14
// 改判：初版收进 en26，会让繁体中文显示成英文档 —— 假标签，见那里的改判记录），
// 两处各写一套必漂。读侧对**缺这一格**必须容错（老插件/老 CLI 的输出）：回落
// 「拼音」，不是解析失败（见 desktop Actions.kt 的 [ImeState.mode]）。
// 与 Trigger 不同：那条路必须有 IC（要刷它的面板），state 这条路不需要。
class OpiCtlObject : public fcitx::dbus::ObjectVTable<OpiCtlObject> {
public:
    OpiCtlObject(OpiCtlHandler handler, OpiCtlStateHandler stateHandler,
                 OpiCtlSearchHandler searchHandler)
        : handler_(std::move(handler)), stateHandler_(std::move(stateHandler)),
          searchHandler_(std::move(searchHandler)) {}

    // 方法实现：把动作串原样交给宿主（OpiEngine）的处理器。**异常必须在处理器里
    // 挡住**（dbus 的分发路径不捕 C++ 异常，逃出去就是守护进程死），见 opi_engine.h。
    std::string trigger(const std::string &action) { return handler_(action); }

    // 注册 D-Bus 方法：Trigger(string) -> string（名字/签名两头对齐 opi-ctl.rs）。
    FCITX_OBJECT_VTABLE_METHOD(trigger, "Trigger", "s", "s");

    // 只读查询：无入参（空签名）→ 上面那行状态串。空签名**两版头都编得过**：
    // `FCITX_STRING_TO_DBUS_TUPLE("")` 展开成 `std::tuple<>`（探针在 5.1.12 与
    // 5.0.21 两套头上 -fsyntax-only/-c 各跑过一次，rc 0），别改成带占位参数的形态。
    std::string state() { return stateHandler_(); }
    FCITX_OBJECT_VTABLE_METHOD(state, "State", "", "s");

    // 关键字搜索（2026-10-13）：入参 = 关键字（拼音/英文前缀），返回命中条目的
    // JSON 文本数组。**名字与签名两头对齐** opi-ctl.rs 的 `send_search`
    // （`org.fcitx.Fcitx5.Opi1.Search` + `string:<关键字>`）。
    std::string search(const std::string &keyword) { return searchHandler_(keyword); }
    FCITX_OBJECT_VTABLE_METHOD(search, "Search", "s", "s");

private:
    OpiCtlHandler handler_;
    OpiCtlStateHandler stateHandler_;
    OpiCtlSearchHandler searchHandler_;
};

// 把 obj 注册到 fcitx5 自己的会话总线上。返回 false = 这条通道没接上（原因已打 stderr）。
// 幂等性由调用方保证（ObjectVTableBase 重复注册会失败，不是静默成功）。
inline bool opiRegisterCtlService(fcitx::Instance *instance, OpiCtlObject &obj) {
    if (instance == nullptr) {
        // 驱动用 nullptr 构造引擎（做 C++ 层的事，不需要真守护进程）⇒ 这里不是
        // 故障。**但仍要打一行**：调用方（OpiEngine::ensureCtlService_）的措辞是
        // 「原因见上一行」，空手返回会让那句指向空气。
        fprintf(stderr, "fcitx5-opi: opiRegisterCtlService 拿到空 instance（驱动/测试"
                        "环境？）—— opi-ctl 通道未接线\n");
        return false;
    }
    auto *dbusAddon = instance->addonManager().addon("dbus", true);
    if (dbusAddon == nullptr) {
        fprintf(stderr,
                "fcitx5-opi: 拿不到 dbus 模块（addon \"dbus\"）—— opi-ctl 通道未接线，"
                "状态栏的符号/简繁/标点/全角/中英按钮会报失败\n");
        return false;
    }
    // ⚠️ 名字是**跨进程/跨语言的字面契约**（DBusModule 的 FCITX_ADDON_EXPORT_FUNCTION
    // 展开成 "#CLASS::#FUNCTION"）。写错了 findCall 返回 nullptr、再解引用就崩 ——
    // 故这里只认这一处，且由 harness 的 e2e 变体真发一次 dbus 调用来守。
    auto *bus = dbusAddon->callWithSignature<fcitx::dbus::Bus *()>("DBusModule::bus");
    if (bus == nullptr) {
        // dbus 模块在，但它的会话总线没连上（无会话/连接失败）——它的 bus() 就是返回空。
        fprintf(stderr, "fcitx5-opi: fcitx5 的会话总线不可用 —— opi-ctl 通道未接线\n");
        return false;
    }
    if (!bus->addObjectVTable(kOpiCtlPath, kOpiCtlInterface, obj)) {
        fprintf(stderr, "fcitx5-opi: 注册 %s 失败（同路径/接口已被占用？）—— opi-ctl 通道未接线\n",
                kOpiCtlPath);
        return false;
    }
    return true;
}
