// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写，**未经过任何 ArkTS 编译器**，也未经过鸿蒙 NDK 的
// clang。它是 C 声明面，本机没有 OHOS SDK，因此连 `clang -fsyntax-only`
// 都没有跑过（对比：macos/OpiFFI.h 至少过了 clang 的语法检查）。
//
// 声明的名字已**机械核对**过：本文件的 16 个 opi_* 声明与
// `cargo rustc -p opi_ffi --target aarch64-unknown-linux-ohos --release
//  --crate-type staticlib` 产出的 libopi_ffi.a 里 nm 出的 28 个符号做过子集比对，
// 16/16 全部命中、0 个落空（16 已桥 + 12 未桥 = 28，实测命令见 harmony/README.md）。
// 但「名字对得上」不等于「签名对得上、能链接上」—— 参数类型与返回值仍需在
// DevEco 上过一遍编译器。
//
// ---- 这是什么 ----
// 这是 **use-site 声明面**：只声明 N-API 桥（cpp/napi_bridge.c）真正调用到的出口。
// **不是**完整 ABI 的又一份拷贝。
//   * 完整 28 个导出的真源：crates/opi-ffi/src/cabi.rs
//   * 完整 28 个声明的另一份（Apple 侧）：macos/OpiFFI.h
// 本项目被「同一语义抄三份必漂移」坑过（fcitx5 轨），所以这里**故意不抄全**。
// 要加一个新调用：在本文件加声明 + 在 bridge 里调用 + 跑下面这条命令核对符号真存在：
//
//   nm -g --defined-only target/aarch64-unknown-linux-ohos/release/libopi_ffi.a \
//     | awk '$2 ~ /^[TtDdBbRr]$/ {print $3}' | grep '^opi_' | sort -u
//
// ---- 字符串所有权（**这是本文件最重要的约定**）----
// `OpiString` 是 UTF-16（ptr + len，**不是** NUL 结尾），由 **Rust 侧分配**。
// 调用方拿到后**必须**用 `opi_ffi_free_string` 释放，**恰好一次**。
//   * ptr == NULL 是**空串哨兵**，不是错误：空句柄可以**无条件**送去 free。
//   * `napi_create_string_utf16` 会把数据**拷贝**进 JS 引擎的字符串 ——
//     所以「先 create、再 free」是安全的，且 free 之后 ptr 立即失效。
//   * 反方向（JS → Rust）的缓冲由**调用方**（N-API 桥）分配与释放；
//     Rust 侧只在调用期间读，**不留引用**（cabi.rs 的 read_utf16 会拷成 String）。

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

/// 对应 `cabi::OpiKeyEventResult`。
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

/// 单字符入引擎，返回需要提交的文本（多数时候是空串）。
OpiString opi_input_key(const uint16_t *ch, size_t len);

void opi_clear(void);

/// 提交**当前页**第 k 个候选（**页内**索引，0 起）。越界/无候选 → 空串（不改状态）。
/// 点击候选请用本出口，**不要**自己算 `opi_page() * 8 + k` 再调全局的 opi_select ——
/// 那个 8 是 PAGE_SIZE 的第三份拷贝，引擎改一次 UI 便**静默选错候选**。
OpiString opi_select_page(uint32_t index);

/// JSON 文本数组：**当前页**的候选（已按引擎的 PAGE_SIZE 分页）。未装载 → `[]`。
/// 显示当前页必须用本出口，不要拿全量候选自己按 8 切（同上，页大小是引擎侧常量）。
OpiString opi_candidates_page(void);

/// 当前拼音缓冲（= preedit 的来源）。
OpiString opi_buffer(void);

/// 候选总页数（UI 的「共 N 页」）。**无候选 → 0**，引擎未装载 → 0。
uint32_t opi_page_count(void);

/// 当前候选页（0 起，末页由路由钳制）。引擎未装载 → 0。
/// 页码必须读这里、不要自己数 —— PageDown 到末页时路由会把页码钳到最后一页。
uint32_t opi_page(void);

/// 当前模式（**0..=4**，4=繁体，见 `api::mode_from_int`）。
/// ⚠️ 取值必须**全部**列进 ArkTS 枚举：漏一个 case 会让 UI 显示拼音、引擎在跑繁体。
int32_t opi_mode(void);

/// 0=Pinyin 1=English 2=Number 3=Symbol 4=Traditional。
/// ⚠️ 越界值是**静默不动作**：没有返回值，调用方无从知道被忽略。
void opi_switch_mode(int32_t mode);

/// **前端** ⇧ 三态：0=OFF 1=SINGLE（下个字母大写后自动复位）2=LOCK（持续大写）。
/// 与 opi_set_shift 不是一回事（那个打的是**引擎侧** shift 位）；
/// ⇧ 键的高亮（尤其 Lock）只能读本出口。
int32_t opi_shift_state(void);

void opi_set_shift(bool on);

bool opi_learner_enabled(void);
void opi_set_learner(bool enabled);

/// 键事件路由。**这是唯一的「这个键归谁」的判据** —— 返回值 action 决定
/// 调用方是消费还是把键交还给应用。
///
/// keyval：可打印字符 = Unicode 码点（**空格 = 0x20**）；
///         特殊键 = 0x1_0000 | 低 16 位码；
///         不可打印且非特殊键 → u32::MAX 哨兵。
/// states：位标志（见下方 KEY_STATE_*）。
///
/// 真源是 crates/engine-core/src/keys.rs（KEY_* / KEY_STATE_* 常量表），
/// 路由行为在 crates/engine-core/src/router.rs。
///
/// ⚠️ 退格/回车**不要**走「直接删一个字符」的旁路 —— 引擎在缓冲为空时
/// 返回 action=0（PassThrough），此时**应当由应用删掉光标前一个字符**。
/// 自己无条件 deleteForward(1) 会吞掉这个信号，表现成「删不掉已上屏的字」。
OpiKeyEventResult opi_key_event(uint32_t keyval, uint32_t states);

#ifdef __cplusplus
}
#endif

// ---- keys.rs 的键码 / 修饰位（本文件只抄了桥要用的那些，真源在 keys.rs）----
// 特殊键 = 0x1_0000 | 低 16 位
#define OPI_KEY_SPECIAL_BASE 0x10000u
#define OPI_KEY_SPACE 0x20u /* 可打印段，唯一不套 SPECIAL_BASE 的键 */
#define OPI_KEY_BACK_SPACE (OPI_KEY_SPECIAL_BASE | 0x08u)
#define OPI_KEY_TAB (OPI_KEY_SPECIAL_BASE | 0x09u)
#define OPI_KEY_RETURN (OPI_KEY_SPECIAL_BASE | 0x0Du)
#define OPI_KEY_ESCAPE (OPI_KEY_SPECIAL_BASE | 0x1Bu)
#define OPI_KEY_PAGE_UP (OPI_KEY_SPECIAL_BASE | 0x80u)
#define OPI_KEY_PAGE_DOWN (OPI_KEY_SPECIAL_BASE | 0x81u)
#define OPI_KEY_DELETE (OPI_KEY_SPECIAL_BASE | 0x82u)
#define OPI_KEY_SHIFT (OPI_KEY_SPECIAL_BASE | 0x83u)

#define OPI_KEY_STATE_SHIFT (1u << 0)
#define OPI_KEY_STATE_CAPS_LOCK (1u << 1)
#define OPI_KEY_STATE_CTRL (1u << 2)
#define OPI_KEY_STATE_ALT (1u << 3)
#define OPI_KEY_STATE_META (1u << 4)
#define OPI_KEY_STATE_RELEASED (1u << 26)
#define OPI_KEY_STATE_REPEAT (1u << 27)
#define OPI_KEY_STATE_LONG_PRESSED (1u << 28)
