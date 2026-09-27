// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写，**未经过任何 ArkTS 编译器**，也**没有经过鸿蒙 NDK 的
// clang** —— 本机没有 OHOS SDK，连 `napi/native_api.h` 都拿不到，
// 所以 `clang -fsyntax-only` 都跑不起来。这里每一个 N-API 调用都是**凭记忆写的**，
// 「最不确定的 API」清单见 harmony/README.md —— **在 DevEco 上第一件事就是让
// 这个文件编译过**，编译错误即待办清单。
//
// 唯一被验证过的部分是它调用的 opi_* 出口：那 16 个函数在
// `cargo rustc -p opi_ffi --target aarch64-unknown-linux-ohos --release
//  --crate-type staticlib` 的产物里 nm 得到（实测记录见 README）。
//
// ---- 为什么要这一层 ----
// 鸿蒙原生模块走 **Node-API（N-API）**，不是 JNI 也不是裸 C ABI。
// 但**底下的 Rust 引擎不必改**：本层是薄壳，只做三件事
//   1. JS 字符串 ⇄ UTF-16 缓冲的搬运；
//   2. 把手写 N-API 声明挂到模块导出上；
//   3. **释放** Rust 返回的 OpiString。
// 所有输入法逻辑仍在 crates/engine-core，与 Android/iOS/桌面**同一份引擎**。
//
// ---- 为什么不引 napi-rs ----
// napi-rs 要在 crates/opi-ffi/Cargo.toml 加依赖 + 加 #[napi] 宏，那会改到
// crates/**（本轮边界之外），并把「同一份引擎、多端薄壳」变成「Rust 侧长出
// 平台分支」。手写 N-API 声明约 200 行、零新依赖、且 Rust 侧一个字符不用动。
// 代价：这 200 行没有编译器的保护 —— 所以它被明确标成草案。
//
// ---- 模块名一致性（**注册不上时先查这里**）----
// 四个名字必须完全一致（注意大小写，全小写最稳）：
//   CMakeLists.txt 的 target           : opiime
//   nm_modname（下方 opi_ime_module）  : "opiime"
//   产物文件名                         : libopiime.so
//   ArkTS 侧 import                    : 'libopiime.so'
// 任一处不匹配的表现是**静默注册失败**：ArkTS 侧 import 得到 undefined，
// 而不是报错。

#include <stdlib.h>  // malloc / free
#include <uchar.h>   // char16_t（C11）—— N-API 的字符串参数用这个类型

#include "napi/native_api.h"

#include "opi_ffi.h"

// ============================================================================
// 搬运工具
// ============================================================================

/// 「取参数」：把回调参数抓进 argv，返回实际个数。
/// 全部出口都对缺参做容错（缺参 = 空串/false/0），**不抛异常** ——
/// 输入法路径上一个 JS 异常会让整个键盘失效，静默降级比抛错安全。
static size_t get_args(napi_env env, napi_callback_info info, napi_value *argv, size_t max) {
    size_t argc = max;
    if (napi_get_cb_info(env, info, &argc, argv, NULL, NULL) != napi_ok) {
        return 0;
    }
    return argc;
}

/// JS 字符串 → **调用方拥有**的 UTF-16 缓冲（含 NUL 余量）。
/// 空串/非字符串/失败 → NULL（*out_len = 0），对 opi_* 等同于「没传」。
/// 调用方用 free() 释放。
static char16_t *js_to_utf16(napi_env env, napi_value v, size_t *out_len) {
    *out_len = 0;
    size_t len = 0;
    // 第一次只问长度（buf 传 NULL）
    if (napi_get_value_string_utf16(env, v, NULL, 0, &len) != napi_ok) {
        return NULL;
    }
    if (len == 0) {
        return NULL;
    }
    char16_t *buf = (char16_t *)malloc((len + 1) * sizeof(char16_t));
    if (buf == NULL) {
        return NULL;
    }
    size_t copied = 0;
    // bufsize 是**字符数**（含结尾 NUL 的位置），所以 len + 1
    if (napi_get_value_string_utf16(env, v, buf, len + 1, &copied) != napi_ok) {
        free(buf);
        return NULL;
    }
    *out_len = copied;
    return buf;
}

/// 取走 Rust 返回的 OpiString，转成 JS 字符串，并**恰好释放一次**。
///
/// 所有权契约（改这里前先读 opi_ffi.h 顶部）：
///   * `napi_create_string_utf16` 会把数据**拷贝**进 JS 引擎，所以
///     「先 create、再 free」是安全的；free 之后 ptr 立即失效、不得再用。
///   * **空句柄（ptr == NULL）可以无条件送去 free** —— opi_ffi_free_string
///     对 NULL 是 no-op。所以这里不需要分支判断，直接出口统一释放。
///   * 空句柄 → **空串 `""`**（不是 undefined）：输入法侧「无缓冲」「无候选」
///     都是正常的空态，用空串表达可以让 ArkTS 少一层判空。
static napi_value take_string(napi_env env, OpiString s) {
    napi_value out = NULL;
    // opi_* 的指针类型是 uint16_t*，N-API 用 char16_t* —— 同为 16 位无符号，
    // 这里是指针类型的桥接转换，不改变任何位。
    if (s.ptr != NULL && s.len > 0 &&
        napi_create_string_utf16(env, (const char16_t *)s.ptr, s.len, &out) == napi_ok) {
        // 成功，out 已就绪
    } else {
        napi_create_string_utf16(env, u"", 0, &out);
    }
    opi_ffi_free_string(s);  // 恰好一次
    return out;
}

static napi_value ret_undefined(napi_env env) {
    napi_value v = NULL;
    napi_get_undefined(env, &v);
    return v;
}

// ============================================================================
// 15 个模块导出（下面 desc[] 表里的 15 条；cpp/types/libopiime/index.d.ts
// 必须与它逐条对齐）。
// ⚠️ 别和「桥调用 16 个 opi_* C 函数」混了 —— 那是 C 侧的被调方，
// 含 opi_ffi_free_string，不是 JS 侧的方法数。两个数不相等是正常的。
// ============================================================================

/// load(path?: string): boolean
/// 空/缺参 → 内置回退词库。**返回值必须接**：false 时后面所有出口都退化成空操作。
static napi_value JsLoad(napi_env env, napi_callback_info info) {
    napi_value argv[1];
    size_t argc = get_args(env, info, argv, 1);
    size_t len = 0;
    char16_t *path = (argc >= 1) ? js_to_utf16(env, argv[0], &len) : NULL;
    bool ok = opi_load((const uint16_t *)path, len);
    free(path);
    napi_value out = NULL;
    napi_get_boolean(env, ok, &out);
    return out;
}

/// inputKey(ch: string): string —— 提交给应用的文本（多数时候是空串）。
static napi_value JsInputKey(napi_env env, napi_callback_info info) {
    napi_value argv[1];
    size_t argc = get_args(env, info, argv, 1);
    size_t len = 0;
    char16_t *ch = (argc >= 1) ? js_to_utf16(env, argv[0], &len) : NULL;
    OpiString s = opi_input_key((const uint16_t *)ch, len);
    free(ch);  // Rust 侧已拷成 String，不留引用
    return take_string(env, s);
}

/// keyEvent(keyval: number, states: number): { action: number, text: string }
/// **特殊的键（空格/回车/退格/翻页/⇧）都走这里** —— action 决定消费还是交还应用。
/// action: 0 = 未处理（**键要交回应用**，如缓冲为空时的退格）1 = 已消费 2 = 提交 text。
static napi_value JsKeyEvent(napi_env env, napi_callback_info info) {
    napi_value argv[2];
    size_t argc = get_args(env, info, argv, 2);

    // 缺参/类型不对 → 发 keys.rs 的 NO_KEYVAL 哨兵（u32::MAX），
    // 路由对它一律 PassThrough —— 最安全的降级。
    uint32_t keyval = 0xFFFFFFFFu;
    if (argc >= 1 && napi_get_value_uint32(env, argv[0], &keyval) != napi_ok) {
        keyval = 0xFFFFFFFFu;
    }
    uint32_t states = 0;
    if (argc >= 2) {
        (void)napi_get_value_uint32(env, argv[1], &states);
    }

    OpiKeyEventResult r = opi_key_event(keyval, states);

    napi_value obj = NULL;
    napi_create_object(env, &obj);
    napi_value action = NULL;
    napi_create_int32(env, r.action, &action);
    napi_set_named_property(env, obj, "action", action);
    // take_string 内部负责释放 r.text —— 这里**不要**再 free 一次
    napi_set_named_property(env, obj, "text", take_string(env, r.text));
    return obj;
}

/// selectPage(k: number): string —— 提交**当前页**第 k 个候选（页内索引，0 起）。
/// 越界/无候选 → 空串（不改状态）。**不要**自己算 page*8+k 去调全局 select。
static napi_value JsSelectPage(napi_env env, napi_callback_info info) {
    napi_value argv[1];
    size_t argc = get_args(env, info, argv, 1);
    uint32_t index = 0;
    if (argc >= 1) {
        (void)napi_get_value_uint32(env, argv[0], &index);
    }
    return take_string(env, opi_select_page(index));
}

/// candidatesPage(): string —— JSON 文本数组（**当前页**）。未装载 → `[]`。
static napi_value JsCandidatesPage(napi_env env, napi_callback_info info) {
    (void)info;
    return take_string(env, opi_candidates_page());
}

/// buffer(): string —— 当前拼音缓冲（preedit 的来源）。
static napi_value JsBuffer(napi_env env, napi_callback_info info) {
    (void)info;
    return take_string(env, opi_buffer());
}

/// page(): number —— 当前候选页。**页码必须读这里、不要自己数。**
static napi_value JsPage(napi_env env, napi_callback_info info) {
    (void)info;
    napi_value out = NULL;
    napi_create_uint32(env, opi_page(), &out);
    return out;
}

/// pageCount(): number —— 候选总页数，无候选 → 0。
static napi_value JsPageCount(napi_env env, napi_callback_info info) {
    (void)info;
    napi_value out = NULL;
    napi_create_uint32(env, opi_page_count(), &out);
    return out;
}

/// clear(): void —— 清空缓冲。
static napi_value JsClear(napi_env env, napi_callback_info info) {
    (void)info;
    opi_clear();
    return ret_undefined(env);
}

/// mode(): number —— 0=Pinyin 1=English 2=Number 3=Symbol 4=Traditional。
/// ⚠️ 五个取值都要进 ArkTS 枚举，漏一个会让 UI 显示与引擎行为不一致。
static napi_value JsMode(napi_env env, napi_callback_info info) {
    (void)info;
    napi_value out = NULL;
    napi_create_int32(env, opi_mode(), &out);
    return out;
}

/// switchMode(mode: number): void
/// ⚠️ 越界值是**静默不动作**（ABI 没有返回值），传之前自己先夹紧。
static napi_value JsSwitchMode(napi_env env, napi_callback_info info) {
    napi_value argv[1];
    size_t argc = get_args(env, info, argv, 1);
    int32_t mode = 0;
    if (argc >= 1) {
        (void)napi_get_value_int32(env, argv[0], &mode);
    }
    opi_switch_mode(mode);
    return ret_undefined(env);
}

/// shiftState(): number —— **前端** ⇧ 三态 0=OFF 1=SINGLE 2=LOCK。
/// ⇧ 的高亮（尤其 Lock）只能读这里，引擎侧 shift 位看不出三态。
static napi_value JsShiftState(napi_env env, napi_callback_info info) {
    (void)info;
    napi_value out = NULL;
    napi_create_int32(env, opi_shift_state(), &out);
    return out;
}

/// setShift(on: boolean): void —— 打**引擎侧** shift 位（与三态不是一回事）。
static napi_value JsSetShift(napi_env env, napi_callback_info info) {
    napi_value argv[1];
    size_t argc = get_args(env, info, argv, 1);
    bool on = false;
    if (argc >= 1) {
        (void)napi_get_value_bool(env, argv[0], &on);
    }
    opi_set_shift(on);
    return ret_undefined(env);
}

/// learnerEnabled(): boolean —— 关掉 = 小欧睡着（见 OpiPet.ets 的 petMood）。
static napi_value JsLearnerEnabled(napi_env env, napi_callback_info info) {
    (void)info;
    napi_value out = NULL;
    napi_get_boolean(env, opi_learner_enabled(), &out);
    return out;
}

/// setLearner(on: boolean): void
static napi_value JsSetLearner(napi_env env, napi_callback_info info) {
    napi_value argv[1];
    size_t argc = get_args(env, info, argv, 1);
    bool on = false;
    if (argc >= 1) {
        (void)napi_get_value_bool(env, argv[0], &on);
    }
    opi_set_learner(on);
    return ret_undefined(env);
}

// ============================================================================
// 模块注册
// ============================================================================

static napi_value OpiImeInit(napi_env env, napi_value exports) {
    napi_property_descriptor desc[] = {
        {"load", NULL, JsLoad, NULL, NULL, NULL, napi_default, NULL},
        {"inputKey", NULL, JsInputKey, NULL, NULL, NULL, napi_default, NULL},
        {"keyEvent", NULL, JsKeyEvent, NULL, NULL, NULL, napi_default, NULL},
        {"selectPage", NULL, JsSelectPage, NULL, NULL, NULL, napi_default, NULL},
        {"candidatesPage", NULL, JsCandidatesPage, NULL, NULL, NULL, napi_default, NULL},
        {"buffer", NULL, JsBuffer, NULL, NULL, NULL, napi_default, NULL},
        {"page", NULL, JsPage, NULL, NULL, NULL, napi_default, NULL},
        {"pageCount", NULL, JsPageCount, NULL, NULL, NULL, napi_default, NULL},
        {"clear", NULL, JsClear, NULL, NULL, NULL, napi_default, NULL},
        {"mode", NULL, JsMode, NULL, NULL, NULL, napi_default, NULL},
        {"switchMode", NULL, JsSwitchMode, NULL, NULL, NULL, napi_default, NULL},
        {"shiftState", NULL, JsShiftState, NULL, NULL, NULL, napi_default, NULL},
        {"setShift", NULL, JsSetShift, NULL, NULL, NULL, napi_default, NULL},
        {"learnerEnabled", NULL, JsLearnerEnabled, NULL, NULL, NULL, napi_default, NULL},
        {"setLearner", NULL, JsSetLearner, NULL, NULL, NULL, napi_default, NULL},
    };
    napi_define_properties(env, exports, sizeof(desc) / sizeof(desc[0]), desc);
    return exports;
}

// nm_modname 必须与 CMake target / libopiime.so / import 串**四处一致**。
static napi_module opi_ime_module = {
    .nm_version = 1,
    .nm_flags = 0,
    .nm_filename = NULL,
    .nm_register_func = OpiImeInit,
    .nm_modname = "opiime",
    .nm_priv = NULL,
    .reserved = {0},
};

/// 动态库被加载时由构造器做注册。**没有它 = import 得到 undefined。**
__attribute__((constructor)) static void RegisterOpiImeModule(void) {
    napi_module_register(&opi_ime_module);
}
