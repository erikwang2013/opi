// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// libfcitx5_opi.so（crates/fcitx5-opi/src/lib.rs 的 #[no_mangle] 出口）的 C 面：
// 结构体布局 + 函数声明 + 取字符串的 take()。
//
// 从 opi_fcitx5.cpp 拆出来（那份文件贴着 500 行硬规矩；与 opi_json.h 同一次
// 拆分思路，见那里的注释）。**这里只是声明**，与 lib.rs 的 #[repr(C)] 手工对齐，
// 是 iOS/macOS 那两份 OpiFFI.h 之外的第三份 C 面 —— 改 lib.rs 的导出签名时三处
// 都要跟。
#ifndef OPI_RUST_FFI_H
#define OPI_RUST_FFI_H

#include <cstddef> // size_t
#include <cstdint>
#include <string>

// Rust 侧 OpString { ptr, len }（UTF-8，非 NUL 结尾；ptr==nullptr 视为空串）。
struct OpiString {
    const uint8_t *ptr;
    size_t len;
};

// Rust 侧 KeyEventResult { action, text }：action 0=直通 1=已处理 2=提交。
struct OpiKeyEventResult {
    int32_t action;
    OpiString text;
};

extern "C" {
bool opi_fcitx5_load(const uint8_t *ptr, size_t len);
// 简繁双装（2026-10-09）：两段都是**路径字符串**（同 opi_fcitx5_load 的约定）。
// 返回值只反映第一段（luna，坏 → false → 调用方回退内置）；第二段（trad）坏是
// 静默降级（Traditional 模式回退简体词典），不返回失败。
bool opi_fcitx5_load_with_trad(const uint8_t *ptr, size_t len,
                               const uint8_t *trad_ptr, size_t trad_len);
OpiString opi_fcitx5_input_key(const uint8_t *ptr, size_t len);
void opi_fcitx5_backspace();
void opi_fcitx5_clear();
OpiString opi_fcitx5_select(size_t index);
void opi_fcitx5_switch_mode(int32_t mode); // 0=Pinyin 1=English 2=Number 3=Symbol
void opi_fcitx5_set_learner(bool on);       // 学习开关（addon 配置 → 引擎，字段见 opi_config.h）
void opi_fcitx5_set_chinese_punct(bool on); // 中文标点开关（同上；与全角相互独立）
void opi_fcitx5_set_fuzzy(bool on);   // 模糊音开关（addon 配置 → 引擎，见 opi_config.h）
// 模糊音逐对（2026-10-13）：组名串（UTF-8，逗号分隔，如 "zh/z,n/l"）。同样是弱符号。
extern "C" void opi_fcitx5_set_fuzzy_pairs(const uint8_t *ptr,
                                            size_t len) __attribute__((weak));
void opi_fcitx5_set_jianpin(bool on);
void opi_fcitx5_set_correction(bool on);  // 拼音纠错（2026-10-13，同上） // 简拼开关（同上）
bool opi_fcitx5_toggle_fullwidth();        // 返回切换后的状态（true = 全角）
void opi_fcitx5_set_shift(bool on);
OpiString opi_fcitx5_input_space();
OpiString opi_fcitx5_candidates(size_t limit); // JSON 数组（UTF-8）
OpiString opi_fcitx5_buffer();
int32_t opi_fcitx5_mode();
OpiKeyEventResult opi_fcitx5_key_event(uint32_t keyval, uint32_t states);
void opi_ffi_free_string_utf8(OpiString s);
}

// ---- 符号浏览（22 类目）的进出入口 + 类目数据装载 ----
//
// 一个入口、三个动作：0 = 退出浏览（幂等：非浏览态调用是 no-op）、1 = 进入、
// 2 = 切换（引擎看自己的状态决定进/出；C++ 侧看不到「是否在浏览态」，而热键要
// 来回切 —— 「只能进不能出等于换个地方卡住」是本仓库热键的既有原则）。
//
// 语义契约（engine-track 2026-10-10 定稿，C++ 侧全靠它）：
//   · 退出**必须恢复进入浏览前的真缓冲**（用户进浏览前打了一半的拼音）；
//   · **同步**：调用返回时 `opi_fcitx5_buffer()` 已经是真缓冲。切走/切英文的顺序
//     是「先退浏览 → 再按 W1 提交（拼音/繁体缓冲原文、符号模式不上关键字 ——
//     2026-10-10 深夜裁决 + 同夜模式分叉，opiDirectConfirm 的注释里有时间线）」，
//     异步实现会把浏览态的标题串/首个分类
//     当用户输入提交出去 —— 正是这条契约要防的东西。
//   · **返回值 = 执行后的浏览态**（1=在浏览 / 0=不在）。无数据时 1/2 是静默
//     no-op（引擎侧「无数据 = 不动」，入口禁用），故 0 分不清「刚退出来」与
//     「进不去」；需要区分的调用方（opi-ctl 的 symbols，见 opi_engine.h）另按
//     自己知道的装载态判。
//
// ⚠️ **弱符号**声明（`__attribute__((weak))`，GCC/Clang 都支持）：入口落地前，
// 这里的引用在链接/加载期解析为 nullptr —— **弱引用不会让链接或 dlopen 失败**，
// 插件照常加载；入口落地后**同一个二进制**自动接上真入口（动态符号解析发生在
// 加载期），不需要回头改本文件。调用点必须判空（opi_mode.h 的 opiSymbolBrowseCall
// 是唯一调用点，缺符号时打一行 stderr 诊断，不静默）。
// 名字是**跨 agent 契约**（已发给 engine-track）：改名要两侧同时动，且本仓
// 无 CI 能看见它 —— 唯一看得见的是那句一次性诊断。
extern "C" int32_t opi_fcitx5_symbol_browse(int32_t action) __attribute__((weak));

// 装载符号类目数据（engine-track 2026-10-10 定稿）：**与两份词库同一约定** ——
// C++ 只 locate 路径，Rust 读文件。返回值：-1 = 文件缺失/读不出；-2 = 读到但零
// 合法类目（两者都让入口保持禁用；Rust 侧自己打一行 stderr，不静默）；
// ≥0 = 跳过的坏行数（0 = 全好；Rust 同时打「N 类目，跳 M 行」一行）。
// 调用方判据就一条：`>= 0` 即数据可用（opi_engine.h 的 symbolDataOk_）。
// 同样是弱符号（引擎侧落地前照常编译/加载）；路径为空时调用方**直接跳过调用**。
extern "C" int32_t opi_fcitx5_load_symbols(const uint8_t *ptr,
                                           size_t len) __attribute__((weak));

// 关键字搜索（2026-10-13，桌面符号/表情页的搜索框）：关键字（**不是路径**，
// 也不是命令）→ 命中条目的 **JSON 文本数组**（`["…"]`，serde_json 输出）。
//   · 空关键字 = 全部条目（引擎既有语义，同 Android searchSymbols("")）；
//   · 无命中 = `[]`；引擎未装载 = `[]` —— 这一层**没有错误码**：分不清「没搜到」
//     与「引擎没装载」，也不该在这一层分（调用方不需要区分它们）。
//   · 「空关键字 = 用法错误」的裁决在**调用方**（opi_engine.h 的
//     handleCtlSearch_ 先拒，opi-ctl 本地再拒一次）—— 那一层才是协议边界。
//   · 与类目数据装载（opi_fcitx5_load_symbols）**无关**：搜索表是编译内嵌的
//     （SymbolEngine::builtin），没装 symbol_categories.tsv 也能搜 —— 调用方
//     别拿 symbolDataOk_ 卡这条出口。
// 同样是弱符号（引擎侧落地前照常编译/加载）；调用点必须判空（opi_engine.h）。
extern "C" OpiString opi_fcitx5_search_symbols(const uint8_t *ptr,
                                               size_t len) __attribute__((weak));

// 取走并释放 Rust 侧字符串。ptr==nullptr 视为空串（Rust 侧 OpString::empty 的
// 表示）；仅守卫构造，free 契约不变：每个返回的 OpString 恰好 free 一次。
//
// `static` 而非 `inline`：本头只有 opi_fcitx5.cpp 一个 TU 包含（opi_json.h 是
// 两个），出处唯一，不需要 inline 的合并语义。
static std::string take(OpiString s) {
    std::string out;
    if (s.ptr != nullptr) {
        out.assign(reinterpret_cast<const char *>(s.ptr), s.len);
    }
    opi_ffi_free_string_utf8(s);
    return out;
}

#endif // OPI_RUST_FFI_H
