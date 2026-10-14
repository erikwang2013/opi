// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
//
// 导入词库（2026-10-14，M3 的 2c **fcitx5 半边**）：扫词库目录 → 逐条 import →
// 按 `DisabledDicts` 名单设启用位。**零状态**（没有成员、没有缓存，见下）。
//
// **与另外三端同一份契约**（正式文本在 desktop/src/main/kotlin/xyz/erik/opi/
// candidate/Dicts.kt 的头注释，那边是桌面窗的纯函数实现）：
//   · 目录：Linux = `$XDG_DATA_HOME/opi/dicts`（缺省 `~/.local/share/opi/dicts`）；
//   · 文件 = `*.tsv`；**词库名 = 文件名去 `.tsv`**（= 停用名单里写的就是这个名字）；
//   · `DisabledDicts` = 逗号分隔、去空白、丢空段；**空 = 全部启用**；
//   · 名单由输入法在**下次启动**时读 —— 本轨的「读」就在引擎构造那一趟
//     （opi_engine.h 的 ctor → applyOpiSettings → 这里）。
//
// 本轨**没有「列出词库」的出口**（与 opi-ffi 的 `dictList` 不同，也不是疏漏）：
// 契约里 fcitx5 宿主自己扫这个目录（就是本文件），所以名字与条数的来源是这次扫描
// 加 `opi_fcitx5_import_dict` 的返回值 —— 上面四条没有一条需要 Rust 侧回读。
// 桌面窗扫同一个目录画列表（各扫各的、口径相同）。
//
// ⚠️ 两个反常识处，别照「常识」改：
//   · **每次配置生效都**扫一遍目录（含重导入同名文件）。这不是浪费，是换来下面那条
//     「C++ 侧零状态」：名单是唯一真源，每次都从它重算，于是「用户把名字从名单里
//     去掉」「往目录里丢了个新文件」都在 `fcitx5-remote -r`（reloadConfig）后生效，
//     不需要 C++ 记「上次导了哪些、哪些被我停了」——**记了就是第二份真源，必漂**。
//     同名重导入 = 原位覆盖并置为启用（Rust 侧裁决 2，见 `src/dict_layer.rs` 头注），
//     所以这条重扫不会叠出两层，也天然清掉上一轮的停用位。
//     ponytail: 目录里 N 个文件就重读 N 次（冷路径，用户改配置才来一次）；
//     真到「词库大到重读卡手」再加「只在启动扫一次 + 记名字」的缓存。
//   · **import 的返回值必须接住**：Rust 侧 -1 = 读不到 / 格式全坏 / 零有效条目。
//     本仓出过「坏词库静默失效」的事故（插件照常加载、按键被接受、一个字不出、
//     任何地方都没有报错，用户侧只表现为候选质量骤降）—— 那正是这条出口要防的。
//
// 为什么在**这里**而不在 opi_engine.h：那份文件贴着 500 行硬门禁（本仓那条门禁是
// `crates/opi-ffi/tests/line_limit.rs`）。本文件只是它的一个 include —— 调用点是
// `opi_config.h` 的 `applyOpiSettings`（与 Learn/Fuzzy 那些旋钮同一个推给引擎的口）。

#ifndef OPI_DICTS_H
#define OPI_DICTS_H

#include <algorithm>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <string>
#include <vector>

#include <dirent.h>
#include <sys/stat.h>

#include "opi_rust_ffi.h"

// 词库目录。**与 desktop 的 Dicts.kt（dictsDirFor）、Rust 的 data_dir.rs
// （xdg_data_dir）同口径**：XDG_DATA_HOME **空串等同未设置**；HOME 缺失时退化成
// 相对路径（与 data_dir.rs 的 unwrap_or_default 同姿态 —— 那种环境本来也装不了词库）。
inline std::string opiDictsDir() {
    const char *xdg = std::getenv("XDG_DATA_HOME");
    if (xdg != nullptr && *xdg != '\0') {
        return std::string(xdg) + "/opi/dicts";
    }
    const char *home = std::getenv("HOME");
    return std::string(home != nullptr ? home : "") + "/.local/share/opi/dicts";
}

// DisabledDicts 名单 → 名字表：逗号切、去空白、丢空段。**与 Dicts.kt 的
// parseDisabledDicts 同口径**（那边是契约的正式文本）；空白只按 ASCII 判
// （用户在 conf 里手敲的换行/空格/TAB）—— 桌面窗根本不允许写出首尾带空白的名字
// （withDictEnabled 的 round-trip 判据），故这里不需要 Unicode 空白表。
inline std::vector<std::string> opiParseDisabledDicts(const std::string &raw) {
    static const char *kSpace = " \t\r\n";
    std::vector<std::string> out;
    size_t i = 0;
    while (i <= raw.size()) {
        const size_t comma = raw.find(',', i);
        const size_t end = comma == std::string::npos ? raw.size() : comma;
        const std::string piece = raw.substr(i, end - i);
        const size_t b = piece.find_first_not_of(kSpace);
        if (b != std::string::npos) {
            out.push_back(piece.substr(b, piece.find_last_not_of(kSpace) - b + 1));
        }
        if (comma == std::string::npos) {
            break;
        }
        i = comma + 1;
    }
    return out;
}

// 扫目录并逐条导入，返回导入了的名字（已排序 = 日志顺序确定、失败可复现）。
// 目录不在（用户还没导入过词库）⇒ 空表，**零噪音**（与 loadDictionary 对空路径的
// 「跳过调用」同一条：那是常态，不是错误）。
inline std::vector<std::string> opiDictsImportAll(const std::string &dir) {
    std::vector<std::string> names;
    DIR *d = opendir(dir.c_str());
    if (d == nullptr) {
        return names;
    }
    while (const dirent *e = readdir(d)) {
        const std::string file = e->d_name;
        if (file.size() <= 4 || file.compare(file.size() - 4, 4, ".tsv") != 0) {
            continue;
        }
        // 目录/符号链接指向别处都要排掉：`*.tsv` 的**普通文件**才是词库
        // （与 Dicts.kt 的 listDicts 判据一致）。d_type 在部分文件系统上是
        // DT_UNKNOWN，故不图省事，直接 stat。
        struct stat st {};
        if (stat((dir + "/" + file).c_str(), &st) != 0 || !S_ISREG(st.st_mode)) {
            continue;
        }
        names.push_back(file.substr(0, file.size() - 4));
    }
    closedir(d);
    std::sort(names.begin(), names.end());
    for (const std::string &name : names) {
        const std::string path = dir + "/" + name + ".tsv";
        const int32_t count = opi_fcitx5_import_dict(
            reinterpret_cast<const uint8_t *>(path.data()), path.size());
        if (count < 0) {
            // Rust 侧已打具体原因（读不到 / 格式全坏 / 零有效条目 / 引擎未装载）；
            // 这里补的是「哪个词库、它没进引擎」——返回值被接住这件事在代码里看得见。
            fprintf(stderr, "fcitx5-opi: 词库 %s 未导入（返回 %d），这一层不进引擎\n",
                    path.c_str(), static_cast<int>(count));
        } else {
            // 条数**只有这里有**（本轨没有 dictList 出口）：不记下来就没人知道导了多少。
            // 坏行另由 Rust 侧打一行（「跳过了 N 行坏行」）—— 不静默吞。
            fprintf(stderr, "fcitx5-opi: 词库 %s 已导入 %d 条\n", path.c_str(),
                    static_cast<int>(count));
        }
    }
    return names;
}

// 配置生效时的唯一入口（opi_config.h 的 applyOpiSettings 调它）：重扫目录 +
// 按名单逐条设启用位。空名单 ⇒ 一条都不停（全启用）—— 上面那趟重导入已把每条
// 都置为启用，故「名单里没有 = 启用」自动成立，不需要再走一遍 setEnabled(true)。
inline void opiDictsSync(const std::string &disabledRaw) {
    if (opi_fcitx5_import_dict == nullptr || opi_fcitx5_set_dict_enabled == nullptr) {
        // 弱符号为 nullptr = 这份 .so 里没有这两条出口（引擎侧落地前的老 .so）。
        // 一次性诊断：每次配置生效都刷一行才是噪音（同 opiMode 的缺符号诊断姿势）。
        static bool warned = false;
        if (!warned) {
            warned = true;
            fprintf(stderr, "fcitx5-opi: 导入词库不可用（.so 里没有 opi_fcitx5_import_dict / "
                            "opi_fcitx5_set_dict_enabled），跳过词库目录\n");
        }
        return;
    }
    opiDictsImportAll(opiDictsDir());
    for (const std::string &name : opiParseDisabledDicts(disabledRaw)) {
        const bool ok = opi_fcitx5_set_dict_enabled(
            reinterpret_cast<const uint8_t *>(name.data()), name.size(), false);
        if (!ok) {
            // 名单里有、导入层里没有 —— 多半是目录里已经没有这个词库文件（用户删了，
            // 而名单还留着）。如实说明「已忽略」，不冒充「停用成功」；也不静默。
            fprintf(stderr, "fcitx5-opi: 停用名单里的「%s」不在导入层里（目录里没有这个"
                            "词库文件？），已忽略\n",
                    name.c_str());
        }
    }
}

#endif // OPI_DICTS_H
