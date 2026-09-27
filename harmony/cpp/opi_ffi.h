// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 「从未编译」指的是**鸿蒙侧**：本机没有 OHOS SDK、没有 DevEco，本文件既没被
// 鸿蒙 NDK 的 clang 看过，更没被 ArkTS 编译器看过 —— 这是本目录的定位（见 README）。
//
// ---- 本机已经做过的两件事（原始输出见 harmony/README.md「实测记录」）----
// 1. `clang -std=c11 -Wall -Wextra -Wpedantic -fsyntax-only` **零诊断通过**，
//    并且是**对着 node 自带的真 N-API 头**（`node_api.h` + `js_native_api.h`，
//    node v22）跑的，不是手写 stub。做法：在 /tmp 造一个只有一行
//    `#include <node_api.h>` 的 `napi/native_api.h` 垫片（**故意不放进仓库** ——
//    放进 harmony/cpp/ 会 shadow 掉真 NDK 头，那比没有更危险）。
//    另以 C++17 过了一遍，验 `extern "C"` 卫哨可用。
//    **牙齿是验过的**：故意写错 `napi_module` 字段名 / `napi_get_value_string_utf16`
//    的参数个数 / `bufsize` 传指针，clang 三条全报错 —— 说明这次全绿不是空跑。
// 2. 本文件的**每一条声明**都与 `crates/opi-ffi/src/cabi.rs` 的**逐条签名**（不只是名字）
//    核过：参数类型、返回值、`#[repr(C)]` 结构体字段顺序全部一致，见 README 实测记录第 5 条。
//    **这里不写条数** —— 它随本文件增删而漂，与下面「use-site 声明面」那段是同一条教训。
//
// ---- 仍然未验证的（别把上面两条读成「能用」）----
//   * 鸿蒙 NDK 自己的 `napi/native_api.h` 是否提供同名同签名的符号 —— 本机只能
//     拿 node 的头当**规范权威**（N-API 是稳定 ABI 规范），拿不到 OHOS 的实现。
//   * 链接：**从未把 libopi_ffi.a 链进任何东西**，符号能否解析未验。
//   * 运行期语义：`napi_create_string_utf16` 是否真拷贝、`bufsize` 是否含 NUL
//     —— 这两条是**规范说拷贝 / 说含 NUL**，鸿蒙实现未验（README 的 U8/U9）。
//
// ---- 这是什么 ----
// 这是 **use-site 声明面**：只声明 N-API 桥（cpp/napi_bridge.c）真正调用到的出口。
// **不是**完整 ABI 的又一份拷贝。
//   * 完整导出的真源：crates/opi-ffi/src/cabi.rs（**数量以
//     crates/opi-ffi/tests/c_abi_contract.rs 的门禁为准，这里不写数字**
//     —— 写死的数字随每次扩容再假一遍，cabi.rs 已走过 19→20→22→28→31）
//   * 完整声明的另一份（Apple 侧）：macos/OpiFFI.h
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

// ---- 全角 / 符号开关（三条一套，读侧 + 两个写侧）----
//
// ⚠️ 这三条在引擎里早就有（`Engine::toggle_fullwidth` / `fullwidth` / `toggle_symbol`），
// 但直到 2026-09-27 才进 C ABI —— 在此之前**任何客户端都调不到**（`grep -rn
// 'fullwidth\|toggle_symbol' crates/opi-ffi/src/` 当时为空）。所以老客户端里若有人
// 自己抄了一份「按模式给全角默认值」的表，那是**对着一份到不了的语义**抄的，删掉。

/// 全角 ↔ 半角：**切换并返回切换后的新状态**（无参，UI 直接拿去刷新高亮）。
/// ⚠️ 全角**随模式默认、跨模式不粘**：`switch_mode` 会**无条件**按模式默认值重置它
/// （真源 `Mode::default_fullwidth`：Pinyin|Traditional → 全角，其余 → 半角）。
/// 用户手动开的全角会被任何一次模式切换抹掉 —— 这是**设计**（英文/数字必须半角直通），
/// 不是缺陷；别在客户端加一个全局 sticky 标志去「修」它，那会让英文模式出全角标点。
bool opi_toggle_fullwidth(void);

/// 全角开关的**读侧** —— 给状态栏显示用。
///
/// ⚠️ **只有两个出口会在调用后改变它：`opi_switch_mode` 与 `opi_toggle_symbol`。**
/// 后者最容易漏：它内部走了一次 `switch_mode`，而 Symbol 的默认是**半角**
/// ⇒ 按符号键时全角指示会**悄悄灭掉**（不报错、不崩溃，指示就是错的）。
/// **这两处之后必须重读本出口。**
///
/// ⚠️ **不许拿它去预测按键结果。** 全角映射**不是 `(mode, fullwidth)` 的纯函数**：
/// `'` 在缓冲非空时是**音节分隔符**（`xi'an`）而不是引号，同一个键在同一个模式下
/// 结果不同。本出口只喂状态栏，键一律整颗交给 `opi_key_event`。
bool opi_fullwidth_state(void);

/// 拼音 ⇄ 符号模式切换。
///
/// ⚠️ 返回值**不是**「刚切出来的那个符号」，而是**切模式前那截缓冲的待提交文本**：
/// 有候选 → 首候选；无候选的乱码缓冲（如 `zzz`）→ 清掉且**不上屏**
/// （把拼音原文塞进文档比丢掉更糟）。空串 = 无提交。
/// 与 `opi_input_space` 的收尾是**同一份逻辑**（对应 Android `ImeState.commitPendingBuffer`）。
///
/// ⚠️ 它**有副作用，且不止一个**：切了模式、清了缓冲、换了候选、并重置了全角
/// ⇒ 调用后要重读 **mode / buffer / candidates / fullwidth 四样**。
///
/// 拿不到「把文本插入文档」通道的端**不要调它** —— 否则用户按了符号键、模式切了、
/// 待提交的那截也没了，表现成「按了没反应且丢了字」。
OpiString opi_toggle_symbol(void);

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
