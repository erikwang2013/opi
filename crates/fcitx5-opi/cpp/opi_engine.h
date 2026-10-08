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
#include "opi_rust_ffi.h"

void refreshingPanelFor(fcitx::InputContext *ic);

// fcitx::AddonInstance + fcitx::InputMethod 的组合基类在 fcitx5 5.1.x 里叫
// InputMethodEngine（fcitx/inputmethodengine.h:17，已继承 AddonInstance），自带
// `virtual void keyEvent(...) = 0` 与 `virtual void reset(...)`；仓库里没有别的
// `fcitx::InputMethod` 类型（grep Core/ 只有 InputMethodEngine V2/V3/V4），原先
// 多继承的两个基类都不存在。
class OpiEngine : public fcitx::InputMethodEngine {
public:
    OpiEngine(fcitx::Instance *instance) : instance_(instance) {
        loadDictionary();
        // 配置推给引擎必须在 loadDictionary 之后：Rust 侧单例在那一刻才建起来。
        readOpiSettings(settings_);
        applyOpiSettings(settings_);
    }

    // ---- 「属性设置」：fcitx5 的 addon 配置（字段与落点见 opi_config.h）----
    // 改完 `fcitx5-remote -r` 走 reloadConfig；配置工具直接改走 setConfig；
    // 两者都立刻 apply 到引擎（不必重启输入法）。
    const fcitx::Configuration *getConfig() const override { return &settings_; }
    void setConfig(const fcitx::RawConfig &raw) override {
        settings_.load(raw, true);
        applyOpiSettings(settings_);
    }
    void reloadConfig() override {
        readOpiSettings(settings_);
        applyOpiSettings(settings_);
    }
    void save() override { saveOpiSettings(settings_); }

    void keyEvent(const fcitx::InputMethodEntry &entry,
                  fcitx::KeyEvent &keyEvent) override;

    void reset(const fcitx::InputMethodEntry & /*entry*/,
               fcitx::InputContextEvent &event) override {
        // 同 keyEvent：这个入口抛出去也会带走整个守护进程。
        try {
            opi_fcitx5_clear();
            // Rust 侧清了缓冲，但面板是 fcitx5 侧的状态：不一并清，失焦/切换
            // 输入法后预编辑串与候选栏会继续挂在屏幕上（原码只清了引擎，没碰面板）。
            refreshingPanelFor(event.inputContext());
        } catch (const std::exception &e) {
            fprintf(stderr, "fcitx5-opi: reset 异常，已忽略: %s\n", e.what());
        }
    }

private:
    OpiSettings settings_;

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
#pragma GCC diagnostic pop
        const bool ok =
            !path.empty() &&
            opi_fcitx5_load(reinterpret_cast<const uint8_t *>(path.data()), path.size());
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
