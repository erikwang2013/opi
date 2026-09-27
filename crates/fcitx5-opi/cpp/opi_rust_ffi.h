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
OpiString opi_fcitx5_input_key(const uint8_t *ptr, size_t len);
void opi_fcitx5_backspace();
void opi_fcitx5_clear();
OpiString opi_fcitx5_select(size_t index);
void opi_fcitx5_switch_mode(int32_t mode); // 0=Pinyin 1=English 2=Number 3=Symbol
bool opi_fcitx5_toggle_fullwidth();        // 返回切换后的状态（true = 全角）
void opi_fcitx5_set_shift(bool on);
OpiString opi_fcitx5_input_space();
OpiString opi_fcitx5_candidates(size_t limit); // JSON 数组（UTF-8）
OpiString opi_fcitx5_buffer();
int32_t opi_fcitx5_mode();
OpiKeyEventResult opi_fcitx5_key_event(uint32_t keyval, uint32_t states);
void opi_ffi_free_string_utf8(OpiString s);
}

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
