// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// opi-clip 的 N-API 薄壳（与 napi_bridge.c 同一个 libopiime.so）。
// C ABI 真源 crates/opi-ffi/src/cabi_clip.rs + cabi_clip/transfer_exports.rs；
// 本层只搬参数，不做信任/配对/限额判断（那些都在 Rust 内核）。
//
// 线程：OpiClip 句柄按创建线程校验；全部调用都在 ArkTS 主线程。句柄以 double
// 往返（arm64 用户指针 < 2^53，与 ASR 同约定）。失败原因走 clipLastError()。
// 文件传输的 JSON 清单只含路径/名字/MIME，不记录文件内容。

#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <uchar.h>

#include "napi/native_api.h"
#include "napi_clip.h"
#include "opi_ffi.h"

// ------------------------------------------------------------------ 参数搬运

static size_t get_args(napi_env env, napi_callback_info info, napi_value *argv, size_t max) {
    size_t argc = max;
    if (napi_get_cb_info(env, info, &argc, argv, NULL, NULL) != napi_ok) {
        return 0;
    }
    return argc;
}

/// JS 字符串 → 调用方拥有的 UTF-16（含 NUL 余量）；空串/非字符串 → NULL + len 0。
static char16_t *js_to_utf16(napi_env env, napi_value v, size_t *out_len) {
    *out_len = 0;
    size_t len = 0;
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
    if (napi_get_value_string_utf16(env, v, buf, len + 1, &copied) != napi_ok) {
        free(buf);
        return NULL;
    }
    *out_len = copied;
    return buf;
}

/// 取走 Rust 的 OpiString，转 JS 字符串并恰好释放一次（NULL 可无条件 free）。
static napi_value take_string(napi_env env, OpiString s) {
    napi_value out = NULL;
    if (s.ptr != NULL && s.len > 0 &&
        napi_create_string_utf16(env, (const char16_t *)s.ptr, s.len, &out) == napi_ok) {
        // out 已就绪
    } else {
        napi_create_string_utf16(env, u"", 0, &out);
    }
    opi_ffi_free_string(s);
    return out;
}

static napi_value ret_undefined(napi_env env) {
    napi_value v = NULL;
    napi_get_undefined(env, &v);
    return v;
}

static napi_value ret_bool(napi_env env, bool value) {
    napi_value out = NULL;
    napi_get_boolean(env, value, &out);
    return out;
}

/// 句柄参数：0 / 负数 / 非数字 → NULL（C ABI 对 NULL 返回参数错误）。
static OpiClip *clip_arg(napi_env env, napi_value v) {
    double raw = 0;
    return (napi_get_value_double(env, v, &raw) != napi_ok || raw <= 0)
        ? NULL : (OpiClip *)(uintptr_t)raw;
}

// ------------------------------------------------------------------ 出口

/// create(configJson: string): number（0 = 失败）
static napi_value JsClipCreate(napi_env env, napi_callback_info info) {
    napi_value a[1];
    size_t n = get_args(env, info, a, 1);
    size_t len = 0;
    char16_t *cfg = (n >= 1) ? js_to_utf16(env, a[0], &len) : NULL;
    OpiClip *h = opi_clip_create((const uint16_t *)cfg, len);
    free(cfg);
    napi_value out = NULL;
    napi_create_double(env, (double)(uintptr_t)h, &out);
    return out;
}

/// destroy(handle: number): void（0 / NULL 安全）
static napi_value JsClipDestroy(napi_env env, napi_callback_info info) {
    napi_value a[1];
    size_t n = get_args(env, info, a, 1);
    if (n >= 1) {
        opi_clip_destroy(clip_arg(env, a[0]));
    }
    return ret_undefined(env);
}

/// start(handle: number): boolean
static napi_value JsClipStart(napi_env env, napi_callback_info info) {
    napi_value a[1];
    size_t n = get_args(env, info, a, 1);
    bool ok = false;
    if (n >= 1) {
        OpiClip *h = clip_arg(env, a[0]);
        if (h != NULL) {
            ok = opi_clip_start(h);
        }
    }
    return ret_bool(env, ok);
}

/// stop(handle: number): void
static napi_value JsClipStop(napi_env env, napi_callback_info info) {
    napi_value a[1];
    size_t n = get_args(env, info, a, 1);
    if (n >= 1) {
        OpiClip *h = clip_arg(env, a[0]);
        if (h != NULL) {
            opi_clip_stop(h);
        }
    }
    return ret_undefined(env);
}

/// localText(handle, text): 0=已发布 1=防回环 -1=失败
static napi_value JsClipLocalText(napi_env env, napi_callback_info info) {
    napi_value a[2];
    size_t n = get_args(env, info, a, 2);
    size_t len = 0;
    char16_t *t = (n >= 2) ? js_to_utf16(env, a[1], &len) : NULL;
    int32_t code = -1;
    if (n >= 1) {
        OpiClip *h = clip_arg(env, a[0]);
        if (h != NULL) {
            code = opi_clip_local_text(h, (const uint16_t *)t, len);
        }
    }
    free(t);
    napi_value out = NULL;
    napi_create_int32(env, code, &out);
    return out;
}

/// setTextRoutes(handle, sendPeer, receivePeer): 空串 = 该方向全部设备
static napi_value JsClipSetTextRoutes(napi_env env, napi_callback_info info) {
    napi_value a[3];
    size_t n = get_args(env, info, a, 3);
    size_t send_len = 0;
    size_t receive_len = 0;
    char16_t *send = (n >= 2) ? js_to_utf16(env, a[1], &send_len) : NULL;
    char16_t *receive = (n >= 3) ? js_to_utf16(env, a[2], &receive_len) : NULL;
    bool ok = false;
    if (n >= 1) {
        OpiClip *h = clip_arg(env, a[0]);
        if (h != NULL) {
            ok = opi_clip_set_text_routes(
                h, (const uint16_t *)send, send_len, (const uint16_t *)receive, receive_len);
        }
    }
    free(send);
    free(receive);
    return ret_bool(env, ok);
}

/// pollEvent(handle, timeoutMs): 事件 JSON；空串 = 这段时间没有事件
static napi_value JsClipPollEvent(napi_env env, napi_callback_info info) {
    napi_value a[2];
    size_t n = get_args(env, info, a, 2);
    uint32_t wait = 0;
    if (n >= 2) {
        (void)napi_get_value_uint32(env, a[1], &wait);
    }
    OpiClip *h = (n >= 1) ? clip_arg(env, a[0]) : NULL;
    return take_string(env, opi_clip_poll_event(h, wait));
}

/// pairStart(handle, peerId): boolean
static napi_value JsClipPairStart(napi_env env, napi_callback_info info) {
    napi_value a[2];
    size_t n = get_args(env, info, a, 2);
    size_t len = 0;
    char16_t *p = (n >= 2) ? js_to_utf16(env, a[1], &len) : NULL;
    bool ok = false;
    if (n >= 1) {
        OpiClip *h = clip_arg(env, a[0]);
        if (h != NULL) {
            ok = opi_clip_pair_start(h, (const uint16_t *)p, len);
        }
    }
    free(p);
    return ret_bool(env, ok);
}

/// pairConfirm(handle, peerId, accept): boolean
static napi_value JsClipPairConfirm(napi_env env, napi_callback_info info) {
    napi_value a[3];
    size_t n = get_args(env, info, a, 3);
    size_t len = 0;
    char16_t *p = (n >= 2) ? js_to_utf16(env, a[1], &len) : NULL;
    bool accept = false;
    if (n >= 3) {
        (void)napi_get_value_bool(env, a[2], &accept);
    }
    bool ok = false;
    if (n >= 1) {
        OpiClip *h = clip_arg(env, a[0]);
        if (h != NULL) {
            ok = opi_clip_pair_confirm(h, (const uint16_t *)p, len, accept);
        }
    }
    free(p);
    return ret_bool(env, ok);
}

/// pairCancel(handle, peerId): boolean（= confirm(false)）
static napi_value JsClipPairCancel(napi_env env, napi_callback_info info) {
    napi_value a[2];
    size_t n = get_args(env, info, a, 2);
    size_t len = 0;
    char16_t *p = (n >= 2) ? js_to_utf16(env, a[1], &len) : NULL;
    bool ok = false;
    if (n >= 1) {
        OpiClip *h = clip_arg(env, a[0]);
        if (h != NULL) {
            ok = opi_clip_pair_cancel(h, (const uint16_t *)p, len);
        }
    }
    free(p);
    return ret_bool(env, ok);
}

/// peers(handle): JSON 数组；未 start → []
static napi_value JsClipPeers(napi_env env, napi_callback_info info) {
    napi_value a[1];
    size_t n = get_args(env, info, a, 1);
    OpiClip *h = (n >= 1) ? clip_arg(env, a[0]) : NULL;
    return take_string(env, opi_clip_peers(h));
}

/// forgetPeer(handle, peerId): boolean
static napi_value JsClipForgetPeer(napi_env env, napi_callback_info info) {
    napi_value a[2];
    size_t n = get_args(env, info, a, 2);
    size_t len = 0;
    char16_t *p = (n >= 2) ? js_to_utf16(env, a[1], &len) : NULL;
    bool ok = false;
    if (n >= 1) {
        OpiClip *h = clip_arg(env, a[0]);
        if (h != NULL) {
            ok = opi_clip_forget_peer(h, (const uint16_t *)p, len);
        }
    }
    free(p);
    return ret_bool(env, ok);
}

/// status(handle): JSON
static napi_value JsClipStatus(napi_env env, napi_callback_info info) {
    napi_value a[1];
    size_t n = get_args(env, info, a, 1);
    OpiClip *h = (n >= 1) ? clip_arg(env, a[0]) : NULL;
    return take_string(env, opi_clip_status(h));
}

/// lastError(): 最近一次失败原因；空串 = 没有错误
static napi_value JsClipLastError(napi_env env, napi_callback_info info) {
    (void)info;
    return take_string(env, opi_clip_last_error());
}

/// sendFiles(handle, peerId, filesJson): 传输 ID 十六进制；失败 → 空串。
/// 目录条目只能给 path（内核递归展开）；name/mime 只对文件有效。
static napi_value JsClipSendFiles(napi_env env, napi_callback_info info) {
    napi_value a[3];
    size_t n = get_args(env, info, a, 3);
    size_t peer_len = 0;
    size_t files_len = 0;
    char16_t *peer = (n >= 2) ? js_to_utf16(env, a[1], &peer_len) : NULL;
    char16_t *files = (n >= 3) ? js_to_utf16(env, a[2], &files_len) : NULL;
    OpiString id = {NULL, 0};
    if (n >= 1) {
        OpiClip *h = clip_arg(env, a[0]);
        if (h != NULL) {
            id = opi_clip_send_files(h, (const uint16_t *)peer, peer_len,
                                     (const uint16_t *)files, files_len);
        }
    }
    free(peer);
    free(files);
    return take_string(env, id);
}

/// respondTransfer(handle, transferId, accept, saveDir): boolean
/// 接受时 saveDir 必须是可写目录；拒绝传空串。
static napi_value JsClipRespondTransfer(napi_env env, napi_callback_info info) {
    napi_value a[4];
    size_t n = get_args(env, info, a, 4);
    size_t id_len = 0;
    size_t dir_len = 0;
    char16_t *id = (n >= 2) ? js_to_utf16(env, a[1], &id_len) : NULL;
    bool accept = false;
    if (n >= 3) {
        (void)napi_get_value_bool(env, a[2], &accept);
    }
    char16_t *dir = (n >= 4) ? js_to_utf16(env, a[3], &dir_len) : NULL;
    bool ok = false;
    if (n >= 1) {
        OpiClip *h = clip_arg(env, a[0]);
        if (h != NULL) {
            ok = opi_clip_respond_transfer(h, (const uint16_t *)id, id_len, accept,
                                           (const uint16_t *)dir, dir_len);
        }
    }
    free(id);
    free(dir);
    return ret_bool(env, ok);
}

/// cancelTransfer(handle, transferId): boolean
static napi_value JsClipCancelTransfer(napi_env env, napi_callback_info info) {
    napi_value a[2];
    size_t n = get_args(env, info, a, 2);
    size_t len = 0;
    char16_t *id = (n >= 2) ? js_to_utf16(env, a[1], &len) : NULL;
    bool ok = false;
    if (n >= 1) {
        OpiClip *h = clip_arg(env, a[0]);
        if (h != NULL) {
            ok = opi_clip_cancel_transfer(h, (const uint16_t *)id, len);
        }
    }
    free(id);
    return ret_bool(env, ok);
}

// ------------------------------------------------------------------ 注册

napi_value opi_clip_napi_init(napi_env env, napi_value exports) {
    napi_property_descriptor desc[] = {
        {"clipCreate", NULL, JsClipCreate, NULL, NULL, NULL, napi_default, NULL},
        {"clipDestroy", NULL, JsClipDestroy, NULL, NULL, NULL, napi_default, NULL},
        {"clipStart", NULL, JsClipStart, NULL, NULL, NULL, napi_default, NULL},
        {"clipStop", NULL, JsClipStop, NULL, NULL, NULL, napi_default, NULL},
        {"clipLocalText", NULL, JsClipLocalText, NULL, NULL, NULL, napi_default, NULL},
        {"clipSetTextRoutes", NULL, JsClipSetTextRoutes, NULL, NULL, NULL, napi_default, NULL},
        {"clipPollEvent", NULL, JsClipPollEvent, NULL, NULL, NULL, napi_default, NULL},
        {"clipPairStart", NULL, JsClipPairStart, NULL, NULL, NULL, napi_default, NULL},
        {"clipPairConfirm", NULL, JsClipPairConfirm, NULL, NULL, NULL, napi_default, NULL},
        {"clipPairCancel", NULL, JsClipPairCancel, NULL, NULL, NULL, napi_default, NULL},
        {"clipPeers", NULL, JsClipPeers, NULL, NULL, NULL, napi_default, NULL},
        {"clipForgetPeer", NULL, JsClipForgetPeer, NULL, NULL, NULL, napi_default, NULL},
        {"clipStatus", NULL, JsClipStatus, NULL, NULL, NULL, napi_default, NULL},
        {"clipLastError", NULL, JsClipLastError, NULL, NULL, NULL, napi_default, NULL},
        {"clipSendFiles", NULL, JsClipSendFiles, NULL, NULL, NULL, napi_default, NULL},
        {"clipRespondTransfer", NULL, JsClipRespondTransfer, NULL, NULL, NULL, napi_default, NULL},
        {"clipCancelTransfer", NULL, JsClipCancelTransfer, NULL, NULL, NULL, napi_default, NULL},
    };
    napi_define_properties(env, exports, sizeof(desc) / sizeof(desc[0]), desc);
    return exports;
}
