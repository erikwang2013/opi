// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
//
// opi-ctl 的 `mode` 动词：**模式循环**（用户裁决 2026-10-14）——
//     拼音(全拼+Pinyin) → 英文26键(全拼+English) →（未来五笔）→ …
// 从 opi_engine.h 拆出来只有一个理由：那个文件贴着 500 行硬门禁（与 opi_mode.h /
// opi_status.h 同一个拆分先例）。`handleCtlAction_` 的 "mode" 分支只调本文件。
//
// 与「中/英」按钮（`lang`）**今天同效、语义不同轴**（lead 2026-10-14 裁决）：
//   · lang 是二值切换，永远只有中/英两档（既有语义一字不动，状态栏「中英」按钮
//     也不许改指到本动词）；
//   · 本动词是 N 档循环 —— 五笔落地后两者分家（lang 仍是二值，本动词走 拼→英→五）。
// 所以**不复用** OpiShiftToggle::toggle，但共用它的两个收尾（见下）。
//
// 状态**不落本地镜像**（镜像必漂，先例见 opi_engine.h 的 opiFullwidthState_）：
// 档位与 token 全部由引擎既有读口现算（opi_fcitx5_mode / opi_fcitx5_scheme）。

#pragma once

#include <string>

#include "opi_config.h" // OpiPinyinMethod（枚举值 = 方案整数码，见 scheme.rs）
#include "opi_mode.h"   // kMode*、OpiShiftToggle/OpiFanjianToggle、opiDirectConfirm

// 状态 token（**跨进程协议，冻结**，desktop 的 cycleMode 与 opi-ctl 的
// check_mode_shape 都按这三个字面量判）：pinyin / en26 /（未来 wubi）。
//
// 派生规则（无额外状态位，lead 2026-10-14 定）：
//   mode == English                                ⇒ en26
//   mode == Pinyin 且 scheme == 全拼                 ⇒ pinyin
// 落不进上面两条的态（繁体/符号/数字，以及**双拼**——它不是本循环的档）回**空串**；
// 动作路径不会停在那里（见下），所以 `opi-ctl mode` 的调用点只见上面两支。空串 =
// 「没拿到 token」，桌面按失败处理（与退出码非 0 同一口径）—— 但 `opi-ctl state`
// 那一格见不得空串，它把面外态收成 pinyin（见 opiCtlStateModeToken 的改判记录）。
inline std::string opiCtlModeToken(int32_t mode, int32_t scheme) {
    if (mode == kModeEnglish) {
        return "en26";
    }
    if (mode == kModePinyin
        && scheme == static_cast<int32_t>(OpiPinyinMethod::Quanpin)) {
        return "pinyin";
    }
    // 五笔（2026-10-14 落地，M4 第二波）：与全拼同为 Pinyin 模式，**只有 scheme 分得开**
    // ⇒ 必须判在下面那条之前。码 5 = `Scheme::code`（engine-core 的协议表）。
    if (mode == kModePinyin && scheme == 5 /* Scheme::Wubi */) {
        return "wubi";
    }
    return "";
}

// 方案读口的判空包装：弱符号缺失（比本树旧的 .so）时按**全拼** —— 与读侧缺省
// （Rust 侧未装载给 0）同一个数，不在这里另造语义。
inline int32_t opiCtlSchemeRead() {
    return opi_fcitx5_scheme != nullptr
               ? opi_fcitx5_scheme()
               : static_cast<int32_t>(OpiPinyinMethod::Quanpin);
}

// `opi-ctl state` 行尾 `mode=` 那一格的取值（2026-10-14 追加，**同日改判**）。
// **与动作路径同一张 token 表**（opiCtlModeToken —— 两处各写一套必漂，这一格的由来
// 就是让状态栏初值有个真源）：
//   · 循环面上的档（全拼+Pinyin / English / **五笔**）⇒ 原样（五笔已落地，
//     自动带出来，本函数一字未改）；
//   · 面外的态（繁体/数字/符号/**双拼** —— 点击不停在它们上）⇒ **pinyin**：它们的
//     **读法仍是拼音**，繁体/数字/符号只是拼音的变体或瞬态面板，简繁另有 trad 位
//     表达 ⇒ 档位如实报「拼」。
// 改判记录（2026-10-14，lead 复查并推翻初版）：初版把面外态收进 **en26**，后果是
// **繁体中文输入时状态栏画「en26」**—— 用户明明在打中文，条上却写英文档，正是
// 「别给用户看假标签」（wubi 那行）要防的同一类错，只是换了个来源。
// 动作路径的**行为一字未改**：从繁体点一下仍是进英文（下一次 state 就已经是
// en26）—— 改的只是「点之前」显示什么。「拼 → en26」因此成了正常的一腿，初版那个
// 「en26 → en26」的收编跳在显示面上不复存在（双拼/符号同理）。
// 为什么不像动作路径那样回空串：空串在那里 = 失败（桌面不显示），在这一格 =
// 「读不到」—— 状态栏的初始档读的正是这一格（旧版无本字段才回落「拼音」，见
// desktop 的 Actions.kt 的 [ImeState.mode]），不能让它常年空着。
// 可见形态如实记（改判后）：**繁体** 行 = `lang=zh;trad=on;…;mode=pinyin`（中文
// 读法；简繁由 trad 格自述，两格各管各），**英文** 行 = `lang=en;…;mode=en26`
// （lang 与 mode 在英文态同向变）—— 驱动 §8a 把这两行的原文钉住了。
inline std::string opiCtlStateModeToken(int32_t mode) {
    const std::string t = opiCtlModeToken(mode, opiCtlSchemeRead());
    // 面外态 ⇒ pinyin。字面量只此一处：面内的三档（含将来 wubi）全来自上面那张表。
    return t.empty() ? std::string("pinyin") : t;
}

// `opi-ctl mode` 的实现体（handleCtlAction_ 的 "mode" 分支调它；C++ 驱动直接调
// handleCtlAction_，测到的就是 D-Bus 回复出去的这一串）。返回值 = **动作后**的
// 新状态 token（见上）。
//
// 当前档 → 下一档（**拼 → 五 → en26 → 拼**，用户 2026-10-14 定的三档循环）：
//   全拼+Pinyin（拼）→ 五笔：只换 scheme（码 5），模式留在 Pinyin。
//   五笔             → en26（走下面那条 else —— 五笔不在 en26 档里，天然落进去）。
//   en26             → 拼音（全拼）：scheme 先归全拼，再切 Pinyin。
//   其余一切（繁体/符号/数字/双拼）→ **五笔**：它们在**显示面上是「拼」**
//   （opiCtlStateModeToken 的改判：面外态报 pinyin），所以按显示档往前走一格就是
//   五笔 —— 与「拼 → 五」同一条腿。
//
// 「其余一切 → en26」= 切英文，与 lang/Shift 轻按**同一条 W1 裁决**（2026-10-10
// 深夜）：先把待定拼音交出去（先退浏览、后提交缓冲原文；符号模式不上关键字 ——
// opiDirectConfirm），再清 shift 残留。
inline std::string opiCtlCycleMode(fcitx::InputContext *ic, OpiShiftToggle &shift,
                                   OpiFanjianToggle &fanjian) {
    if (opi_fcitx5_mode() == kModeEnglish) {
        // en26 → 拼音。scheme 归全拼是**契约的一部分**：对外 token「pinyin」的定义
        // 就是 mode==Pinyin && scheme==全拼，本动作不记额外状态位 ⇒ 进拼音档时把
        // scheme 摆到全拼。副作用如实记（2026-10-14）：正在用双拼的用户点这个按钮
        // 会落到全拼（引擎侧运行值；配置文件没动，下次 reloadConfig 由配置面顶回）。
        // 双拼不在本循环面上 —— 循环只认「全拼+Pinyin」与「全拼+English」这两档。
        if (opi_fcitx5_set_scheme != nullptr) {
            opi_fcitx5_set_scheme(static_cast<int32_t>(OpiPinyinMethod::Quanpin));
        }
        // en26 → 拼音：scheme 归全拼（见上「契约的一部分」）。
        opi_fcitx5_switch_mode(kModePinyin);
    } else if (opiCtlStateModeToken(opi_fcitx5_mode()) == "pinyin"
               && opi_fcitx5_set_scheme != nullptr) {
        // **显示档是「拼」**（全拼 / 面外态：繁体/符号/数字/双拼）→ 五笔。
        // 判据用**显示 token**而不是 scheme：面外态下 scheme 可能是 4/5/0 任意值
        // （用户上次留下的运行值），「码已经是 5」不等于「档位在五笔」—— 用 scheme==5
        // 判会漏掉「符号态 + scheme 已是 5」这种组合（驱动 §10d 抓到过：它应当进五笔，
        // 却被判进了 en26）。显示 token 是「用户看到的那一格」，按它往前走一格永远对。
        //
        // ⚠️ **模式也要归一为 Pinyin**：面外态的 mode 不是 Pinyin，只换 scheme 的话
        // token 派生会回空串（`opiCtlModeToken` 对非 Pinyin 态返回 ""）—— 那是「动作
        // 失败」的表示，可这动作明明成功了。
        // ⚠️ **待定拼音先交出去**（与 en26 那条同一条 W1 裁决）：留着会被新 scheme
        // 当五笔码重解（打了一半的拼音秒变乱码候选）。顺序：先确认 → 归模式 → 摆 scheme。
        opiDirectConfirm(ic);
        opi_fcitx5_switch_mode(kModePinyin);
        opi_fcitx5_set_scheme(5 /* Scheme::Wubi */);
        refreshingPanelFor(ic);
        shift.update(ic);
        fanjian.update(ic);
        return opiCtlModeToken(opi_fcitx5_mode(), opiCtlSchemeRead());
    } else {
        // 其余一切 → en26。W1 直接确认**必须赶在 switch_mode 之前**（switch_mode
        // 清缓冲，之后只剩空串 —— OpiShiftToggle::toggle 的同一条顺序）。ic 为空
        // （驱动用 nullptr 构造引擎）时 opiDirectConfirm 自己守卫。
        opiDirectConfirm(ic);
        // 「先绕一步拼音再切英文」：进英文时引擎的单次 Shift 状态按设计保留
        // （`CandidateState::switch_mode` 只在**离开** English 时清），不清就是
        // 「切过去后第一个字母莫名大写」—— 与 OpiShiftToggle::toggle 里那个坑
        // 同一个，修法也照抄（两步之间不刷面板，中间态从不渲染）。
        opi_fcitx5_switch_mode(kModePinyin);
        opi_fcitx5_switch_mode(kModeEnglish);
    }
    // 收尾与 lang 那条一致：切模式会清缓冲换候选，面板必须跟着走；两个状态项
    // 的文案也要刷（「中/英」跟着模式变，「简/繁」从英文点过来时同理 —— 两边
    // 互相刷的接线在 OpiEngine 构造里）。
    refreshingPanelFor(ic);
    shift.update(ic);
    fanjian.update(ic);
    // 读回派生（不假设「设进去的一定生效」：弱符号缺失 / 写侧被静默忽略时，
    // token 如实反映引擎里**实际**落成的状态，而不是我们打算设的）。
    return opiCtlModeToken(opi_fcitx5_mode(), opiCtlSchemeRead());
}
