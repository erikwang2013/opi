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
#include "opi_mode.h"     // 模式热键（Ctrl+' / Ctrl+\）+ Shift 轻按切换 + 中/英、简/繁状态项
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
          instance_(instance) {
        loadDictionary();
        // 配置推给引擎必须在 loadDictionary 之后：Rust 侧单例在那一刻才建起来。
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
    }

    void keyEvent(const fcitx::InputMethodEntry &entry,
                  fcitx::KeyEvent &keyEvent) override;

    void reset(const fcitx::InputMethodEntry & /*entry*/,
               fcitx::InputContextEvent &event) override {
        // 同 keyEvent：这个入口抛出去也会带走整个守护进程。
        try {
            opi_fcitx5_clear();
            // Shift「轻按」判定态也要清：按住 Shift 时失焦，抬起事件可能到不了
            // 本 IC（或到了别的 IC），不清就是一次「幽灵切换」（opi_mode.h）。
            shiftToggle_.disarm();
            // Rust 侧清了缓冲，但面板是 fcitx5 侧的状态：不一并清，失焦/切换
            // 输入法后预编辑串与候选栏会继续挂在屏幕上（原码只清了引擎，没碰面板）。
            refreshingPanelFor(event.inputContext());
        } catch (const std::exception &e) {
            fprintf(stderr, "fcitx5-opi: reset 异常，已忽略: %s\n", e.what());
        }
    }

private:
    OpiSettings settings_;

    // Shift 轻按切换 + 「中/英」状态项（判定与状态项都在 opi_mode.h）。
    // 构造用构造参数 instance，不读 instance_ 成员 —— 与成员声明顺序无关。
    OpiShiftToggle shiftToggle_;

    // 「简/繁」状态项（opi_mode.h）与「设置」项（opi_status.h）—— 外层入口。
    OpiFanjianToggle fanjianToggle_;
    OpiSettingsAction settingsAction_;

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
    }

    fcitx::Instance *instance_;
};
