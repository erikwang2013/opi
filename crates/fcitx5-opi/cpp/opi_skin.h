// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
//
// 皮肤：把用户选定的 OPI 主题写进 **classicui 的用户配置**并热重载。
//
// 事实基线（2026-10-09 在本机核实，非推导）：
//   · 候选面板**不是本插件画的** —— 它由 UI addon（本机是 classicui）按主题绘制；
//     引擎侧一个颜色都改不了。所以「换皮肤」唯一可行的落点 = 换 fcitx5 主题。
//   · 主题目录的查找位置 = `<前缀>/share/fcitx5/themes/<名字>/`
//     （本机实测 /usr/share/fcitx5/themes/{default,default-dark,deepin-light,deepin-dark}）。
//   · 主题的选择写在用户配置 `$XDG_CONFIG_HOME/fcitx5/conf/classicui.conf`
//     （缺省 $HOME/.config/...），键是 `Theme=` 与 `DarkTheme=`
//     （本机该文件就是 Theme=deepin-light / DarkTheme=deepin-dark / UseDarkTheme=True）。
//   · ⚠️ 这是**框架级**设置：面板对所有输入法只有一份主题，换了 OPI 皮肤，
//     搜狗等其它输入法的候选面板也跟着变。用户选单里的「不改动」即不介入。
//
// 两段式（为可测性拆开）：
//   opiPatchClassicuiTheme(theme)  纯文件操作，返回写没写成功 —— 面板驱动直接测它；
//   applyOpiSkin(...)              判定「值变了才动」+ 只在 classicui 在用才热重载。
// 顺序也是判据：**构造引擎时不 apply**，只把读到的值记为「已应用」——
// 否则每次 fcitx5 启动都会拿配置里的旧值把 classicui.conf 重写一遍，
// 把用户后来在别处（配置工具 / 附加组件→经典界面）改的主题顶回去。

#pragma once

#include <cstdio>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <sstream>
#include <string>
#include <unistd.h> // getpid：原子写的临时名

#include <fcitx/instance.h>

#include "opi_config.h"

// 皮肤 → 主题目录名（必须与 data/themes/ 下的目录名、CMakeLists 的安装规则、
// scripts/gen_fcitx5_themes.py 的产物目录**逐一相同**；改一处等于让皮肤静默失效）。
// Keep 返回 nullptr = OPI 不介入主题。
inline const char *opiSkinThemeName(OpiSkin skin) {
    switch (skin) {
    case OpiSkin::Light:
        return "opi-light";
    case OpiSkin::Dark:
        return "opi-dark";
    case OpiSkin::Paper:
        return "opi-paper";
    case OpiSkin::Cinnabar:
        return "opi-cinnabar";
    case OpiSkin::Fresh:
        return "opi-fresh";
    case OpiSkin::Azure:
        return "opi-azure";
    case OpiSkin::Lemon:
        return "opi-lemon";
    case OpiSkin::Sunset:
        return "opi-sunset";
    case OpiSkin::Starlight:
        return "opi-starlight";
    case OpiSkin::Keep:
        break;
    }
    return nullptr;
}

// classicui.conf 的用户级路径。与桌面设置窗（desktop/.../Settings.kt 的
// settingsPathFor）同一条 XDG 规则：XDG_CONFIG_HOME 空串等同未设置，回落 $HOME/.config。
// 不走 StandardPath 是**有意**的：只用这两条规则，免掉 5.1.21 起的弃用迁移
// （StandardPath→StandardPaths）与它那套 pragma 约定（见 CMakeLists.txt 的说明）。
inline std::string opiClassicuiConfPath() {
    const char *xdg = std::getenv("XDG_CONFIG_HOME");
    if (xdg != nullptr && *xdg != '\0') {
        return std::string(xdg) + "/fcitx5/conf/classicui.conf";
    }
    const char *home = std::getenv("HOME");
    return std::string(home != nullptr ? home : "") + "/.config/fcitx5/conf/classicui.conf";
}

// 把 Theme=/DarkTheme= 两个键改成 themeName，**其余行原样保留**（未知键、注释、
// 用户手加的东西都不动 —— 与桌面设置窗「保留未知键」同一条约定）。
// 原子写：临时名带 pid（固定名在并发下会先丢写、再写坏目标 —— 本仓裁决过的形态）。
// 返回 false 时调用方不回滚「已应用」记录（写失败重试也无收益，用户重选一次即可）。
inline bool opiPatchClassicuiTheme(const char *themeName) {
    const std::string path = opiClassicuiConfPath();
    const std::string themeLine = std::string("Theme=") + themeName;
    const std::string darkLine = std::string("DarkTheme=") + themeName;

    std::string existing;
    {
        std::ifstream in(path);
        if (in) {
            std::ostringstream ss;
            ss << in.rdbuf();
            existing = ss.str();
        }
    }

    std::string out;
    bool sawTheme = false;
    bool sawDark = false;
    std::istringstream iss(existing);
    std::string line;
    while (std::getline(iss, line)) {
        const bool isTheme = line.rfind("Theme=", 0) == 0;
        const bool isDark = line.rfind("DarkTheme=", 0) == 0;
        if (isTheme) {
            out += themeLine;
            sawTheme = true;
        } else if (isDark) {
            out += darkLine;
            sawDark = true;
        } else {
            out += line;
        }
        out += '\n';
    }
    if (!sawTheme) {
        out += themeLine + '\n';
    }
    if (!sawDark) {
        out += darkLine + '\n';
    }

    try {
        std::filesystem::create_directories(
            std::filesystem::path(path).parent_path());
        const std::string tmp = path + ".tmp." + std::to_string(::getpid());
        {
            std::ofstream f(tmp, std::ios::trunc);
            if (!f) {
                std::fprintf(stderr, "fcitx5-opi: 皮肤写入失败（打不开 %s）\n", tmp.c_str());
                return false;
            }
            f << out;
        }
        // 保留原文件权限（fcitx5 自己保存时给的是 rw-rw-r-- 一档；不给的话
        // rename 会把 umask 后的 0644 盖上去）。
        if (std::filesystem::exists(path)) {
            std::filesystem::permissions(
                tmp, std::filesystem::status(path).permissions(),
                std::filesystem::perm_options::replace);
        }
        std::filesystem::rename(tmp, path);
        return true;
    } catch (const std::exception &e) {
        std::fprintf(stderr, "fcitx5-opi: 皮肤写入异常: %s\n", e.what());
        return false;
    }
}

// 值有变化才动；写成功后让 classicui 热重载。
// ⚠️ reloadAddonConfig 用的是**非加载**取法（instance.cpp：
// `addonManager().addon(name)`，不传 true）—— classicui 没加载时这一步就是 no-op，
// **不会**被这里拉出一份多余的面板；别的 UI（如 fcitx5-qt）在用也只是「文件写了、
// 肉眼不变」（那套 UI 不吃 classicui 主题），docs 如实记。
inline void applyOpiSkin(OpiSkin skin, OpiSkin &lastApplied,
                         fcitx::Instance *instance) {
    if (skin == lastApplied) {
        return;
    }
    lastApplied = skin;
    const char *theme = opiSkinThemeName(skin);
    if (theme == nullptr) {
        return; // Keep：不介入（也不「恢复默认」—— 我们不知道用户原来的主题是什么）
    }
    if (!opiPatchClassicuiTheme(theme)) {
        return;
    }
    if (instance != nullptr) {
        instance->reloadAddonConfig("classicui");
    }
}
