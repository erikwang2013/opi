// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
//
// opi-ctl 的 `englishpro` 动词 + state 的 `enpro=` 位（2026-10-19）：
// 英文 26 键默认普通（字母直出、不查词库），切到专业才走 en_words.tsv 前缀联想。
// 从 opi_engine.h 拆出（该文件贴 500 行门禁；与 opi_ctl_mode.h 同一先例）。

#pragma once

#include "opi_rust_ffi.h"

/// 切一次普通 ⇄ 专业；旧 .so 没有弱符号时返回 false（调用方报 err:4）。
inline bool opiCtlToggleEnglishPro() {
    if (opi_fcitx5_toggle_english_pro == nullptr) {
        return false;
    }
    opi_fcitx5_toggle_english_pro();
    return true;
}

/// state 行末字段 `;enpro=on|off`（旧 .so 缺省 off）。
inline const char *opiCtlEnglishProField() {
    return (opi_fcitx5_english_pro != nullptr && opi_fcitx5_english_pro()) ? ";enpro=on" : ";enpro=off";
}
