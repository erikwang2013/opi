// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
//
// 五笔码表的**装载**（2026-10-14，M4 第二波 fcitx5 半边）：按 `WubiPlan` 在
// `Type::Data` 的 `opi/` 下定位 `wubi86.opid` / `wubi98.opid`，把选中的那张交给引擎。
//
// ⚠️ 三条裁决（是契约，别当实现细节看）：
//   · **只装选中的那张**（86 xor 98）：两表是互斥替代表（码空间重叠：fpgc 在两表里
//     各是一组不同的词），权重还不可比（98 是顺序秩而非词频）⇒ 同时装等于让两套
//     体系在同一候选层打架。出处：`data/raw/LICENSES.md` 的五笔节 + lead 裁决。
//   · **装载不换方案**：要用五笔还得把 `PinyinMethod` 选到五笔（方案整数码 5）。
//   · **表缺失/装载失败是可选增强**：与 trad/stroke 同一姿态 —— 打日志 + 留空，
//     方案落普通路径（引擎侧不假装有候选）。
//
// 为什么在**这里**而不在 opi_engine.h：那份文件贴着 500 行硬门禁，而且它的
// `loadDictionary()` 跑在**配置应用之前**（ctor 第 51/52 行的顺序注释），根本不知道
// `WubiPlan` 选了哪套 —— 五笔是配置驱动的装载，落点是 `applyOpiSettings`（与
// Learn/Fuzzy 那些旋钮同一个推给引擎的口）。
//
// ⚠️ StandardPath 弃用四件事（为什么不能迁移、边界、何时删）写在 CMakeLists.txt 的
// 那一段 —— 别在这里复制。

#ifndef OPI_WUBI_H
#define OPI_WUBI_H

#include <cstdio>
#include <string>

#include <fcitx-utils/standardpath.h>

#include "opi_rust_ffi.h"

// 按方案（"86" / "98"，认不得按 86）装载五笔码表。**每次配置生效都重装**（与
// opi_dicts.h 的 opiDictsSync 同一条理由：C++ 侧零状态，配置是唯一真源，
// 于是「改方案」在 reloadConfig 后立即生效）。
inline void opiWubiSync(const std::string &plan) {
    if (opi_fcitx5_load_wubi == nullptr) {
        // 弱符号为 nullptr = 这份 .so 里没有这条出口（引擎侧落地前的老 .so）。
        // 一次性诊断（同 opiDictsSync 的姿势），不每次刷。
        static bool warned = false;
        if (!warned) {
            warned = true;
            fprintf(stderr, "fcitx5-opi: 五笔不可用（.so 里没有 opi_fcitx5_load_wubi），"
                            "跳过码表装载\n");
        }
        return;
    }
    // 86 是缺省；认不得的值按 86（与设置面「认不得 ⇒ 回落」同一条容错口径）。
    const char *file = (plan == "98") ? "opi/wubi98.opid" : "opi/wubi86.opid";
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wdeprecated-declarations"
    auto path = fcitx::StandardPath::global().locate(fcitx::StandardPath::Type::Data, file);
#pragma GCC diagnostic pop
    if (path.empty()) {
        // 没装这张表 ⇒ 什么都不做（常态，不是错误；方案落普通路径）。
        return;
    }
    const bool ok = opi_fcitx5_load_wubi(reinterpret_cast<const uint8_t *>(path.data()),
                                         path.size());
    if (!ok) {
        // 「文件在、装不上」（下载不全/拷贝中断）。这行必须有：否则与「本来就没装」
        // 无法区分，用户侧只表现为「打五笔一个字不出」。
        fprintf(stderr, "fcitx5-opi: 五笔码表 %s 装载失败，五笔方案回退普通路径\n",
                path.c_str());
    } else {
        fprintf(stderr, "fcitx5-opi: 五笔码表 %s 已装载\n", path.c_str());
    }
}

#endif // OPI_WUBI_H
