// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
//
// 「属性设置」：fcitx5 的**原生 addon 配置**（原生选项 + 一个 `Configurable` 入口；
// 条数不在这里写死 —— 看下面 FCITX_CONFIGURATION 块，它才是唯一出处）。
//
// 落点是 fcitx5 给每个 addon 的标准位置（`StandardPath::Type::PkgConfig` +
// `conf/opi_fcitx5.conf`，即 `$XDG_CONFIG_HOME/fcitx5/conf/opi_fcitx5.conf`）。
// **不是自创格式** —— 由 fcitx5 自己的配置机制读写，于是三件事自动成立：
//   · 用户那侧的配置工具能显示并编辑这些选项，前提是 addon conf 里
//     `Configurable=True`（见 data/addon/opi_fcitx5.conf）。**这是「设置入口在
//     配置里」的唯一机制**（2026-10-09 核实）：控制中心/配置工具的
//     「附加组件 → OPI → 齿轮」对话框按 `fcitx://config/addon/opi_fcitx5` 拉
//     本结构的值 + **描述**（含中文说明）逐项渲染；而 ConfigureAddon 那个 D-Bus
//     调用在 5.1.12 里是 no-op（上游源码：`void Instance::configureAddon(...) {}`），
//     **没法**让配置工具拉一个外部程序 —— 所以图形设置窗进不了「配置」，能进的
//     只有这里声明的选项。
//   · 改完 `fcitx5-remote -r`（= ReloadConfig）即时生效 —— 走 `reloadConfig()`。
//   · 面板驱动（opi_panel_driver.cpp 第 9 段）走**同一条读路径**做端到端门禁。
// 本层不做「值之外的」任何事：引擎旋钮推给 Rust 单例，ShiftToggle/Skin
// 由 C++ 侧自己消费（前者见 opi_mode.h，后者见 opi_skin.h）。
// ⚠️ 唯一的例外是 `DisabledDicts`：推名单的同时还会**扫词库目录并逐条导入**
// （零状态的代价与理由见 opi_dicts.h 头注）—— 那不是「顺手多做了点别的」，而是
// 「导入层的启用位」这件事的全部实现就在那里，本层只负责在配置生效时调它。
//
// ⚠️ 字段名（`Learn` / `ShiftToggle` / `Skin` …）**就是配置文件里的键名**：
// 改名 = 用户已有配置读不到。与 fcitx5 其它 addon 的惯例一致（首字母大写、无空格）。
// ⚠️ 本文件是**唯一**声明这些开关的地方；`opi_fcitx5.cpp` 只负责把它们接到
// fcitx5 的生命周期上（那个文件有 500 行硬门禁 —— 新东西往这里放）。
// ⚠️ 桌面设置窗（desktop/.../Settings.kt）只管理其中四个（Learn/ChinesePunct/
// Fuzzy/Jianpin）。它保存时**保留未知键**（2026-10-09 起），所以本文件新增的
// ShiftToggle/Skin 不会被它抹掉；但反过来，键改名仍会让它的读侧静默回默认值。

#pragma once

#include <fcitx-config/configuration.h>
#include <fcitx-config/iniparser.h>
#include <fcitx-config/option.h>
#include <fcitx-utils/i18n.h>
#include <fcitx-utils/standardpath.h>

#include "opi_dicts.h"    // 导入词库（扫目录 + DisabledDicts 名单）
#include "opi_rust_ffi.h"

// 皮肤枚举。取值 → `data/themes/` 下的主题目录名，映射在 opi_skin.h
// （opiSkinThemeName）；多处（枚举/i18n 名/映射/生成脚本/安装清单）改一处即漂，
// 别只改这 —— 跨文件同集同序的回归钉在 desktop 的 SettingsTest
// （`skin names stay in lockstep`），漂了先看它。
// ⚠️ 用 WITH_I18N 变体（与 fcitx5-unikey 的字符集选项同惯例）：写进配置文件的
// **字面值就是这里的字符串**（unmarshallOption 按字面匹配），显示名同源。
// 换言之这些中文字面串是**对外契约**（配置文件里会存「浅色」这种值），别当注释改。
// 顺序 = 枚举序 = 下面那条 i18n 宏的名字序（宏按位置取名字），插入要两处同一位置。
enum class OpiSkin { Keep, Light, Dark, Paper, Cinnabar, Fresh, Azure, Lemon, Sunset, Starlight, Wisteria, Pine };
FCITX_CONFIG_ENUM_NAME_WITH_I18N(OpiSkin, N_("不改动"), N_("浅色"), N_("深色"),
                                 N_("宣纸"), N_("朱砂"), N_("清爽"), N_("湛蓝"),
                                 N_("柠檬"), N_("晚霞"), N_("星光"), N_("紫藤"),
                                 N_("松林"));

// 双拼布局（2026-10-13 用户裁决「搜狗同款全套」）。
// ⚠️ 与皮肤同一条：**字面值就是这里的字符串**（unmarshallOption 按字面匹配），
// 顺序 = 枚举序 = i18n 宏名字序 = 引擎 `shuangpin_tables.rs` 的 `SHUANGPIN_TABLES`
// 表序（applyOpiSettings 传的是 `static_cast<int>` 下标，那个表序就是下标口径）。
// 名字表还有三端镜像（桌面 `SHUANGPIN_LAYOUT_NAMES` / Android
// `SHUANGPIN_LAYOUTS_ANDROID` / TSF 的 `ShuangpinLayout` 键），跨端同集同序的回归钉
// 在 desktop 的 SettingsTest（`shuangpin layout names stay in lockstep`）。
enum class OpiShuangpinLayout { Ziranma, MS, Xiaohe, ABC, Ziguang, PinyinJiaJia };
FCITX_CONFIG_ENUM_NAME_WITH_I18N(OpiShuangpinLayout, N_("自然码"), N_("微软"), N_("小鹤"),
                                 N_("智能ABC"), N_("紫光"), N_("拼音加加"));

// 拼音方式（2026-10-14）：桌面端一直**只有全拼**（两轨的引擎方案从来是默认值 ——
// 没有入口能换）。这张脸把「全拼/双拼」做进配置：与移动端的「输入模式」面板同一个轴
// （`Engine::set_scheme`），只是桌面没有 9/14 键/笔画（物理键盘上那几个方案没有意义）。
// ⚠️ 枚举值**就是方案整数码**（`engine_core::Scheme::code`：0=全拼 4=双拼）——
// applyOpiSettings 直接 `static_cast<int32_t>` 传进引擎，中间**没有**「枚举序 ↔ 方案」
// 的第二张映射表（那种表是两处写必漂）。非连续取值对 fcitx5 的配置框架无碍
// （它按**字面值**存储：`PinyinMethod=双拼`）。
enum class OpiPinyinMethod { Quanpin = 0, Shuangpin = 4 };
FCITX_CONFIG_ENUM_NAME_WITH_I18N(OpiPinyinMethod, N_("全拼"), N_("双拼"));

// 默认值**全是 true / Keep** = 「没配过」等于引擎既有行为（模糊音/简拼见
// `RankOptions::default()`；学习/中文标点见各自 setter 的默认；ShiftToggle 取 true
// 是用户裁决「Shift 就该能切英文」；Skin=Keep = OPI 不碰用户的 fcitx5 主题）。
// 别改成 false：那样「配置文件不存在」这个最常见的情形会变成"悄悄关掉一半功能"。
FCITX_CONFIGURATION(
    OpiSettings,
    fcitx::Option<bool> learn{this, "Learn", "学习（记住选过的词）", true};
    fcitx::Option<bool> chinesePunct{this, "ChinesePunct", "中文标点（，。、与成对引号）", true};
    fcitx::Option<bool> fuzzy{this, "Fuzzy", "模糊音（z/zh、n/l…）", true};
    // 逐对多选（2026-10-13 用户裁决「可多选类似 z=zh」）：逗号分隔组名，名字表见
    // 引擎的 `fuzzy::pair_names`（zh/z,ch/c,sh/s,an/ang,en/eng,in/ing,n/l,f/h）。
    // **空 = 未设置 ⇒ 回落到上面的总开关**（老配置行为一字不变）；非空 ⇒ 以列表为准。
    fcitx::Option<std::string> fuzzyPairs{
        this, "FuzzyPairs", "模糊音逐对（逗号分隔，如 zh/z,n/l；留空按上面的总开关）", ""};
    fcitx::Option<bool> jianpin{this, "Jianpin", "简拼（声母缩写，nh → 你好）", true};
    // 拼音纠错（2026-10-13 用户裁决「拼音纠错（混合参与排序）」）：相邻键错按/多打/少打。
    fcitx::Option<bool> correction{this, "Correction", "拼音纠错（相邻键、多打、少打）", true};
    fcitx::Option<bool> shiftToggle{this, "ShiftToggle", "Shift 轻按切换中英文（组合键与长按不触发）", true};
    fcitx::OptionWithAnnotation<OpiSkin, OpiSkinI18NAnnotation> skin{
        this, "Skin", "候选面板皮肤（fcitx5 全局主题，对所有输入法生效）", OpiSkin::Keep};
    // 双拼布局（2026-10-13）：**下面那项拼音方式选「双拼」时**才参与解码。
    // 默认自然码 = 引擎默认布局（`Engine::shuangpin_layout` 的初值 0）。
    fcitx::OptionWithAnnotation<OpiShuangpinLayout, OpiShuangpinLayoutI18NAnnotation> shuangpinLayout{
        this, "ShuangpinLayout", "双拼布局（拼音方式选双拼时生效）", OpiShuangpinLayout::Ziranma};
    fcitx::OptionWithAnnotation<OpiPinyinMethod, OpiPinyinMethodI18NAnnotation> pinyinMethod{
        this, "PinyinMethod", "拼音方式（全拼 / 双拼）", OpiPinyinMethod::Quanpin};
    // 导入词库的**停用名单**（2026-10-14 词库页，M3）：逗号分隔的**词库名**
    // （= 文件名去 `.tsv`，如 `brand_terms.tsv` → `brand_terms`），与桌面窗写的
    // 那个键同名同义。**空 = 全部启用**。⚠️ 与上面的 FuzzyPairs **反着来**
    // （那里空 = 回落总开关）—— 名单型选项的两种空语义别串。
    // ⚠️ 只管 `~/.local/share/opi/dicts` 下的**导入**词库；系统词库（luna/trad/
    // stroke）不受它管（Rust 侧整条出口都拒系统词典）。目录与名单的字面口径见
    // opi_dicts.h 头注（那份是契约的镜像）。
    fcitx::Option<std::string> disabledDicts{
        this, "DisabledDicts", "停用的导入词库（逗号分隔词库名；留空 = 全部启用）", ""};);

// 把**引擎旋钮**推给 Rust 侧。**冷路径**（构造 / reloadConfig / setConfig
// 三处调用），不需要去抖。ShiftToggle 与 Skin 不在这里 —— 它们是 C++ 侧
// 自己消费的开关（handleModeHotkey / applyOpiSkin），引擎根本不知道它们存在。
//
// 双拼布局**在**这里（引擎要它解码）。传 `static_cast<int>` = 枚举下标：与引擎
// `SHUANGPIN_TABLES` 表序同序（由 SettingsTest 的锁步门禁钉住），C++ 侧因此不必
// 抄一张「枚举名 → 引擎下标」的映射表（抄了就是两处写必漂）。弱符号检查同
// fuzzyPairs：老 .so 上没有这个出口时静默跳过，不是崩。
inline void applyOpiSettings(const OpiSettings &settings) {
    opi_fcitx5_set_learner(*settings.learn);
    opi_fcitx5_set_chinese_punct(*settings.chinesePunct);
    // 逐对列表非空 ⇒ 以它为准；空 ⇒ 回落旧布尔（合同与 TSF/桌面一侧一字不差）。
    if (!settings.fuzzyPairs->empty() && opi_fcitx5_set_fuzzy_pairs != nullptr) {
        const auto &s = *settings.fuzzyPairs;
        opi_fcitx5_set_fuzzy_pairs(reinterpret_cast<const uint8_t *>(s.data()), s.size());
    } else {
        opi_fcitx5_set_fuzzy(*settings.fuzzy);
    }
    opi_fcitx5_set_jianpin(*settings.jianpin);
    opi_fcitx5_set_correction(*settings.correction);
    if (opi_fcitx5_set_shuangpin_layout != nullptr) {
        opi_fcitx5_set_shuangpin_layout(static_cast<int>(*settings.shuangpinLayout));
    }
    // 拼音方式：**先切方案再设布局**（顺序无关紧要 —— 布局不清缓冲、方案清，
    // 但两条都只改运行期状态；分开写只为可读）。
    if (opi_fcitx5_set_scheme != nullptr) {
        opi_fcitx5_set_scheme(static_cast<int32_t>(*settings.pinyinMethod));
    }
    // 导入词库（DisabledDicts）：名单是配置值，推给引擎的口就在本函数 —— 与上面那些
    // 旋钮一字不差。⚠️ 本函数**还会重扫词库目录并逐条导入**（零状态的代价与理由见
    // opi_dicts.h 头注），所以它不能在 loadDictionary 之前调 —— 那个先决条件本来就
    // 写在 opi_engine.h 的构造注释里（「配置推给引擎必须在 loadDictionary 之后」）。
    opiDictsSync(*settings.disabledDicts);
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
