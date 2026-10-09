// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
//
// 状态区的「设置」项：外层入口之一（2026-10-09 用户裁决「外层……没有设置入口」）。
//
// 三层入口互不替代（别把这层读成「配置面进不去了」）：
//   1. 本节项 —— 状态栏里点「设」直接拉起设置窗；
//   2. 系统菜单的 opi-settings.desktop（打包注入，见 desktop/build.gradle.kts 尾）；
//   3. fcitx5 配置面（附加组件 → OPI → 齿轮）—— 六个原生选项。
// ⚠️ 为什么不是「让 fcitx5 的配置按钮拉起设置窗」：做不到。`Instance::configureAddon`
// 是 no-op（上游源码），框架没有从配置面拉起外部程序的通道（详见 opi_config.h 与
// docs/install/linux.md 的属性设置一节）。

#pragma once

#include <cstdio>
#include <fcntl.h>   // posix_spawn 的 open flags
#include <functional>
#include <string>
#include <spawn.h>   // posix_spawnp
#include <unistd.h>  // access

#include <fcitx/action.h>
#include <fcitx/inputcontext.h>
#include <fcitx/instance.h>
#include <fcitx/statusarea.h>
#include <fcitx/userinterfacemanager.h>

extern char **environ;

// 设置窗可执行文件的候选，按顺序：合并包落点 → PATH → 配置工具兜底。
// 拆成按序号取的函数是为了驱动能逐个核对查找规则（纯读，无副作用）。
// ⚠️ 头两个是**本插件的设置窗**（带 --settings）；第三个是 fcitx5 配置工具
// （不带参数）—— 兜底的意思诚实地说：至少能落到配置面，而不是点了没反应。
inline const char *opiSettingsAppCandidate(int i) {
    static const char *cands[] = {
        "/opt/opi-candidates/bin/opi-candidates", // 合并包（deb/rpm）的 jpackage 落点
        "opi-candidates",                         // PATH（发行版打包/手动装）
        "fcitx5-configtool",                      // 兜底：至少到得了配置面
    };
    return (i >= 0 && i < 3) ? cands[i] : nullptr;
}

// posix_spawn 一版（不是 fork+exec）：fcitx5 是多线程进程，fork 后只能用
// async-signal-safe 调用，posix_spawn 没有这个约束。
// 标准流接 /dev/null：GUI 的输出去它自己的日志，**不继承守护进程的 fd**
// （继承了既会把两边日志缠在一起，app 活着时那些 fd 也不释放）。
inline bool opiSpawnDetached(const char *prog, bool withSettingsArg) {
    posix_spawn_file_actions_t fa;
    posix_spawn_file_actions_init(&fa);
    posix_spawn_file_actions_addopen(&fa, 0, "/dev/null", O_RDONLY, 0);
    posix_spawn_file_actions_addopen(&fa, 1, "/dev/null", O_WRONLY, 0);
    posix_spawn_file_actions_addopen(&fa, 2, "/dev/null", O_WRONLY, 0);
    char *argv[3];
    int n = 0;
    argv[n++] = const_cast<char *>(prog);
    if (withSettingsArg) {
        argv[n++] = const_cast<char *>("--settings");
    }
    argv[n] = nullptr;
    pid_t pid = 0;
    const int rc = posix_spawnp(&pid, prog, &fa, nullptr, argv, environ);
    posix_spawn_file_actions_destroy(&fa);
    return rc == 0;
}

// 真启动器。返回值只说明「发起成功」——窗口起没起来这里看不到，别在日志里
// 说成「已打开」。
inline bool opiLaunchSettingsApp() {
    for (int i = 0; opiSettingsAppCandidate(i) != nullptr; ++i) {
        const char *prog = opiSettingsAppCandidate(i);
        if (prog[0] == '/' && ::access(prog, X_OK) != 0) {
            continue;
        }
        if (opiSpawnDetached(prog, i < 2)) {
            return true;
        }
    }
    std::fprintf(stderr,
                 "fcitx5-opi: 设置入口没有可执行文件（/opt/opi-candidates、PATH、"
                 "fcitx5-configtool 都没找到）\n");
    return false;
}

// 测试缝：面板驱动把它换成记录型假货 —— 点「设」不该在测试里真拉起 GUI 窗口
// （与 opi_fcitx5.cpp 的 g_pushedBuffer 同一档的「测试可见性」先例）。
inline std::function<bool()> &opiSettingsLauncherHook() {
    static std::function<bool()> f = opiLaunchSettingsApp;
    return f;
}

class OpiSettingsAction {
public:
    explicit OpiSettingsAction(fcitx::Instance *instance) : instance_(instance) {
        action_.setShortText("设");
        action_.setLongText("打开 OPI 属性设置（学习/中文标点/模糊音/简拼/皮肤）");
        if (instance_ != nullptr) {
            // 名字全局唯一（其它 addon 也往 UserInterfaceManager 注册 action）。
            instance_->userInterfaceManager().registerAction("opi-settings-open",
                                                             &action_);
        }
        connection_ = fcitx::ScopedConnection(
            action_.connect<fcitx::SimpleAction::Activated>(
                [](fcitx::InputContext * /*ic*/) { opiSettingsLauncherHook()(); }));
    }

    void attach(fcitx::InputContext *ic) {
        if (ic == nullptr) {
            return;
        }
        ic->statusArea().addAction(fcitx::StatusGroup::InputMethod, &action_);
    }

private:
    fcitx::Instance *instance_;
    fcitx::SimpleAction action_;
    fcitx::ScopedConnection connection_;
};
