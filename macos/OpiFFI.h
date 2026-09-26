// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写。**Swift 侧一行都没编译过**；本头文件自身过了
// `clang -fsyntax-only`（C 与 C++ 两种模式），但**从未被链接、从未进入任何真实构建**。
// 声明是**逐条对照** crates/opi-ffi/src/cabi.rs 抄的
// （27 个 C 函数 + opi_ffi_free_string，#[no_mangle] 共 28 个），
// 但「抄对了」与「能链接上」是两件事 —— 见 macos/README.md。
//
// OPI C ABI 声明面（macOS 端唯一一份，共 28 个导出）。
// ⚠️ ios/OpiFFI.h 是本文件的**转发头** —— 改这里两边都变，别在那边另抄一份。
// Swift 侧经 bridging header（Xcode）或 module map（SwiftPM）看到这些符号：
//   Xcode  : SWIFT_OBJC_BRIDGING_HEADER = macos/OpiFFI.h
//   SwiftPM: 与 module.modulemap 一起放进一个 C target
// 声明与 crates/opi-ffi/src/cabi.rs 一一对应；改动请同时改两边，不要在这里
// 另起一份「顺手加个包装」的实现（fcitx5 轨的教训：同一语义抄三份必漂移）。
//
// 声明的完整性已于本机核对：`cargo rustc --crate-type staticlib` 产出的
// aarch64-apple-darwin 归档里能数出 28 个 `_opi_*` 符号，与本文件的 28 条声明
// **逐个 diff 一致**（本机 GNU nm/objdump 不认 Mach-O，用
// `grep -a -o` 读归档的符号名，方法见 README「已验证」）。
//
// 字符串约定：UTF-16（ptr + len，非 NUL 结尾），由 Rust 侧分配，
// 调用方**必须**用 opi_ffi_free_string 释放恰好一次。

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/// 对应 Rust `cabi::OpiString`（#[repr(C)] { *const u16, usize }）。
/// ptr == NULL 视为空串（Rust 侧 empty 哨兵），不是错误。
typedef struct {
    const uint16_t *ptr;
    size_t len;
} OpiString;

/// 对应 `cabi::OpiKeyEventResult`（本轮新增，接口已冻结）。
/// action: 0 = 未处理（键交应用）1 = 已处理（消费）2 = 提交 text。
typedef struct {
    int32_t action;
    OpiString text;
} OpiKeyEventResult;

/// 释放 opi_* 返回的 OpiString。每个句柄恰好释放一次，重复释放是 UB。
void opi_ffi_free_string(OpiString s);

/// 装载词库。空串/NULL → 内置回退词库；坏路径 → false（单例保持为空！）。
/// **返回值必须接**：false 时所有 opi_* 出口都会退化成空操作。
bool opi_load(const uint16_t *path, size_t len);

/// 繁体词库。空/坏路径/引擎未装载 → false（繁体模式回退简体库）。
bool opi_load_trad(const uint16_t *path, size_t len);

/// 单字符入引擎，返回需要提交的文本（多数时候是空串）。
OpiString opi_input_key(const uint16_t *ch, size_t len);
void opi_backspace(void);
void opi_clear(void);

/// 按 index 选词（引擎级全局索引，从 0 起 —— 与 opi_candidates 的数组同序）。
OpiString opi_select(size_t index);

/// 提交**当前页**第 k 个候选（**页内**索引，0 起）。越界/无候选 → 空串（不改状态）；
/// 引擎未装载 → 空串。
///
/// 点击候选请用本出口，**不要**自己算 `opi_page() * 8 + k` 再调 `opi_select()` ——
/// 那个 8 是 PAGE_SIZE 的第三份拷贝，引擎改一次 UI 便**静默选错候选**。
/// 本出口与按数字键（`opi_key_event` 的数字选词）、回车提交**同源**，
/// 三者都走 `KeyRouter::select` 那一份页内换算。
/// ⚠️ 本目录**尚未调用**它（`candidateSelected` 现在走 `opi_select` 的全局索引，
/// 见 README G3）。
OpiString opi_select_page(uint32_t index);

/// 0=Pinyin 1=English 2=Number 3=Symbol 4=Traditional（`api::mode_from_int`）。
/// ⚠️ 越界值是**静默不动作**：没有返回值，调用方无从知道被忽略。
void opi_switch_mode(int32_t mode);
void opi_set_shift(bool on);
OpiString opi_input_space(void);

/// JSON 文本数组，最多 limit 条，从全局第 0 条起（**不分页**）。
/// 显示当前页请用下面的 `opi_candidates_page()`。
OpiString opi_candidates(size_t limit);

/// JSON 文本数组：**当前页**的候选（已按引擎的 PAGE_SIZE 分页）。
/// 引擎未装载 → `[]`。
///
/// **显示当前页必须用本出口，不要拿 `opi_candidates()` 自己按 8 切** ——
/// 页大小是引擎侧常量（路由与候选栏共用），UI 再抄一份，改一次就会静默错位
/// （高亮的页 ≠ 实际选词所在的页）。本出口与 `opi_page()` / `opi_page_count()`
/// 同源，三者永远一致。
OpiString opi_candidates_page(void);

/// 当前拼音缓冲（= preedit 的来源）。
OpiString opi_buffer(void);

/// 当前模式（**0..=4**，4=繁体）。⚠️ 取值必须**全部**列进 Swift 枚举：
/// 漏一个 case 会让 `OpiMode(rawValue:)` 判 nil、落到 `?? .pinyin` 兜底，
/// 表现成「UI 显示拼音、引擎在跑繁体」（显示与行为不一致，比不认识更难查）。
int32_t opi_mode(void);

/// JSON 文本数组。
OpiString opi_search_symbols(const uint16_t *keyword, size_t len);

/// JSON：`[{id,start,end,name,common}]`。
OpiString opi_symbol_blocks(void);

/// JSON 文本数组。负 id 按越界处理（空数组）。
OpiString opi_symbols_in_block(int16_t id);

bool opi_learner_enabled(void);
void opi_set_learner(bool enabled);
void opi_clear_user_words(void);
void opi_remove_user_word(const uint16_t *text, size_t len);

/// 返回导入条数；失败返回 -1（非法 JSON / 版本不符 / 词表过大 / 未装载 / NULL）。
int32_t opi_import_user_words(const uint16_t *json, size_t len);

/// 导出用户词（JSON）。
OpiString opi_export_user_words(void);

/// 键事件路由（本轮新增，接口已冻结）。
///
/// keyval：可打印字符 = Unicode 码点（空格 = 0x20）；
///         特殊键 = 0x1_0000 | 低 16 位码；
///         不可打印且非特殊键 → u32::MAX 哨兵（NO_KEYVAL）。
/// states：位标志（SHIFT/CTRL/ALT/META/RELEASED/REPEAT/LONG_PRESSED）。
///
/// 两套取值的真源是 **crates/engine-core/src/keys.rs**（`KEY_*` 与 `KEY_STATE_*`
/// 常量表）；路由的行为与状态（`KeyRouter`/`KeyAction`/页码）在
/// crates/engine-core/src/router.rs。**不是** crates/tsf-opi/src/logic.rs ——
/// 后者的 ⇧/翻页/Delete/空格四个键编码与这边不同。
OpiKeyEventResult opi_key_event(uint32_t keyval, uint32_t states);

/// 候选总页数（UI 的「共 N 页」）。**无候选 → 0**，引擎未装载 → 0。
/// ⚠️ 本目录**尚未调用**它（同 G3，等 Mac 上编译通过再接）。
uint32_t opi_page_count(void);

/// 当前候选页（0 起，末页由路由钳制）。引擎未装载 → 0。
/// Rust 侧注释的原话：**候选栏的页码必须读这里、不要自己数** ——
/// PageDown 到末页时路由会把页码钳到最后一页，本地计数超过末页就会与引擎漂移
/// （高亮的页 ≠ 实际选词所在的页）。
/// ⚠️ 本目录**尚未调用**它（`InputController.swift` 现在仍把全局列表整份塞给
/// 候选窗；改成按页显示是 G3，等 Mac 上编译通过再接）。
uint32_t opi_page(void);

/// 前端 ⇧ 三态：0=OFF 1=SINGLE（下个字母大写后自动复位）2=LOCK（持续大写）。
/// 引擎未装载 → 0。
/// 与 `opi_set_shift` 不是一回事：那个打的是**引擎侧** shift 位，三态是前端状态，
/// 英文直传路径的大小写由它决定，引擎位看不出来 —— ⇧ 的高亮（尤其 Lock）只能读这里。
/// ⚠️ 本目录**尚未调用**它（macOS 端目前没有 ⇧ 高亮 UI）。
int32_t opi_shift_state(void);

#ifdef __cplusplus
}
#endif
