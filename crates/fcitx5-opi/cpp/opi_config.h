// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
//
// 「属性设置」：fcitx5 的**原生 addon 配置**（两个排序开关 + 一个 `Configurable` 入口）。
//
// 落点是 fcitx5 给每个 addon 的标准位置（`StandardPath::Type::PkgConfig` +
// `conf/opi_fcitx5.conf`，即 `$XDG_CONFIG_HOME/fcitx5/conf/opi_fcitx5.conf`）。
// **不是自创格式** —— 由 fcitx5 自己的配置机制读写，于是两件事自动成立：
//   · 用户那侧的配置工具（fcitx5-configtool / Deepin 内置的）能显示并编辑这两项，
//     前提是 addon conf 里 `Configurable=True`（见 data/addon/opi_fcitx5.conf）；
//   · 改完 `fcitx5-remote -r`（= ReloadConfig）即时生效 —— 走 `reloadConfig()`。
// 本层不做「值之外的」任何事：读出来就推给引擎（Rust 单例），不存第二份状态。
//
// ⚠️ 字段名（`Fuzzy` / `Jianpin`）**就是配置文件里的键名**：改名 = 用户已有配置读不到。
// 与 fcitx5 其它 addon 的惯例一致（首字母大写、无空格）。
// ⚠️ 本文件是**唯一**声明这两个开关的地方；`opi_fcitx5.cpp` 只负责把它们接到
// fcitx5 的生命周期上（那个文件有 500 行硬门禁，余量个位数 —— 新东西往这里放）。

#pragma once

#include <fcitx-config/configuration.h>
#include <fcitx-config/iniparser.h>
#include <fcitx-utils/standardpath.h>

#include "opi_rust_ffi.h"

// 默认值**四个都是 true** = 引擎既有行为（模糊音/简拼见 `RankOptions::default()`；
// 学习/中文标点见各自 setter 的默认）。别改成 false：那样「配置文件不存在」这个
// 最常见的情形会变成"悄悄关掉一半功能"。
// ⚠️ 字段名必须与桌面设置窗（desktop/.../Settings.kt）写的**四个键**逐一对应：
// 少一个的后果不是"少个开关"，而是 fcitx5 保存配置时**把它从文件里抹掉**
// （readAsIni 认不出的键不会保留）—— 设置窗刚写下的值会在 fcitx5 退出时丢。
FCITX_CONFIGURATION(
    OpiSettings,
    fcitx::Option<bool> learn{this, "Learn", "学习（记住选过的词）", true};
    fcitx::Option<bool> chinesePunct{this, "ChinesePunct", "中文标点（，。、与成对引号）", true};
    fcitx::Option<bool> fuzzy{this, "Fuzzy", "模糊音（z/zh、n/l…）", true};
    fcitx::Option<bool> jianpin{this, "Jianpin", "简拼（声母缩写，nh → 你好）", true};);

// 把配置推给引擎。**冷路径**（构造 / reloadConfig / setConfig 三处调用），不需要去抖。
inline void applyOpiSettings(const OpiSettings &settings) {
    opi_fcitx5_set_learner(*settings.learn);
    opi_fcitx5_set_chinese_punct(*settings.chinesePunct);
    opi_fcitx5_set_fuzzy(*settings.fuzzy);
    opi_fcitx5_set_jianpin(*settings.jianpin);
}

// 从磁盘读进 settings。**文件不存在不是错误**：readAsIni 会把字段留作默认值 ——
// 这正是「没配过 = 全默认」的表达，别自己先 stat 一次（多一个判据就多一处会漂）。
inline void readOpiSettings(OpiSettings &settings) {
    fcitx::readAsIni(settings, fcitx::StandardPath::Type::PkgConfig,
                     "conf/opi_fcitx5.conf");
}

// 落盘（fcitx5 在退出与配置变更时调）。返回值是"写成了吗"，调用方（save 覆写）
// 无处可报 —— GUI 宿主里 stderr 通常看不见；保留返回值是为了让门禁/单测能断言。
inline bool saveOpiSettings(const OpiSettings &settings) {
    return fcitx::safeSaveAsIni(settings, fcitx::StandardPath::Type::PkgConfig,
                                "conf/opi_fcitx5.conf");
}
