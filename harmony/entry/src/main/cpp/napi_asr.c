// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-identifier: MIT

// opi-asr 的 N-API 薄壳（与 napi_bridge.c 同一个 libopiime.so）。
// C ABI 真源 crates/opi-asr/src/ffi.rs；本层只搬参数，不碰识别状态。
//
// 线程：Asr 句柄本身 Send+Sync，但 opi_asr_destroy 必须是最后一次调用。
// load_model 会阻塞数秒（239MB 模型 + ONNX 初始化），放 NAPI async work
// 的线程池跑，完成回调只带 int 错误码回 JS —— UI 线程不冻。
// 句柄以 double 往返（arm64 用户指针 < 2^53，与跨设备剪贴板同约定）。

#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

#include "napi/native_api.h"
#include "napi_asr.h"

// crates/opi-asr/src/ffi.rs 的 C ABI（该 crate 不发布头文件）。
typedef void AsrHandle;
extern AsrHandle *opi_asr_create(const char *config_json);
extern void opi_asr_destroy(AsrHandle *handle);
extern int32_t opi_asr_load_model(AsrHandle *handle);
extern int32_t opi_asr_start(AsrHandle *handle);
extern int32_t opi_asr_feed_pcm(AsrHandle *handle, const uint8_t *data, int32_t len);
extern int32_t opi_asr_finish(AsrHandle *handle);
extern int32_t opi_asr_cancel(AsrHandle *handle);
extern int32_t opi_asr_poll_event(AsrHandle *handle, char *buf, int32_t cap);
extern int32_t opi_asr_last_error(AsrHandle *handle, char *buf, int32_t cap);
extern int32_t opi_asr_max_event_bytes(void);

// ------------------------------------------------------------------ 参数搬运

static size_t get_args(napi_env env, napi_callback_info info, napi_value *argv, size_t max) {
    size_t argc = max;
    if (napi_get_cb_info(env, info, &argc, argv, NULL, NULL) != napi_ok) {
        return 0;
    }
    return argc;
}

/// 句柄参数：0 / 非数字 → NULL（C ABI 对 NULL 一律返回参数错误）。
static AsrHandle *handle_arg(napi_env env, napi_value v) {
    double raw = 0;
    if (napi_get_value_double(env, v, &raw) != napi_ok || raw <= 0) {
        return NULL;
    }
    return (AsrHandle *)(uintptr_t)raw;
}

/// JS 字符串 → malloc 的 UTF-8（含 NUL）。opi 的 C ABI 只收 NUL 结尾。
static char *string_arg(napi_env env, napi_value v) {
    size_t len = 0;
    if (napi_get_value_string_utf8(env, v, NULL, 0, &len) != napi_ok) {
        return NULL;
    }
    char *buf = (char *)malloc(len + 1);
    if (buf == NULL) {
        return NULL;
    }
    size_t copied = 0;
    if (napi_get_value_string_utf8(env, v, buf, len + 1, &copied) != napi_ok) {
        free(buf);
        return NULL;
    }
    buf[copied] = 0;
    return buf;
}

/// ArrayBuffer / Uint8Array → 原始字节。其它类型拒绝（JS 侧永远传 Uint8Array）。
static bool bytes_arg(napi_env env, napi_value v, uint8_t **data, size_t *len) {
    *data = NULL;
    *len = 0;
    bool is_buffer = false;
    if (napi_is_arraybuffer(env, v, &is_buffer) == napi_ok && is_buffer) {
        void *p = NULL;
        size_t l = 0;
        if (napi_get_arraybuffer_info(env, v, &p, &l) != napi_ok) {
            return false;
        }
        *data = (uint8_t *)p;
        *len = l;
        return true;
    }
    bool is_typed = false;
    if (napi_is_typedarray(env, v, &is_typed) == napi_ok && is_typed) {
        napi_typedarray_type type = napi_uint8_array;
        size_t l = 0;
        void *p = NULL;
        napi_value arraybuffer = NULL;
        size_t offset = 0;
        if (napi_get_typedarray_info(env, v, &type, &l, &p, &arraybuffer, &offset) != napi_ok
            || type != napi_uint8_array) {
            return false;
        }
        *data = (uint8_t *)p;
        *len = l;
        return true;
    }
    return false;
}

static napi_value ret_int(napi_env env, int32_t code) {
    napi_value out = NULL;
    napi_create_int32(env, code, &out);
    return out;
}

// ------------------------------------------------------------------ 出口

/// asrCreate(configJson: string): number（0 = 失败，原因用 asrLastError(0, buf)）
static napi_value JsAsrCreate(napi_env env, napi_callback_info info) {
    napi_value a[1];
    size_t n = get_args(env, info, a, 1);
    char *cfg = (n >= 1) ? string_arg(env, a[0]) : NULL;
    AsrHandle *h = opi_asr_create(cfg);
    free(cfg);
    napi_value out = NULL;
    napi_create_double(env, (double)(uintptr_t)h, &out);
    return out;
}

typedef struct {
    napi_async_work work;
    napi_ref callback;
    AsrHandle *handle;
    int32_t rc;
} LoadJob;

static void load_execute(napi_env env, void *data) {
    (void)env;
    LoadJob *job = (LoadJob *)data;
    job->rc = opi_asr_load_model(job->handle);
}

static void load_complete(napi_env env, napi_status status, void *data) {
    (void)status;
    LoadJob *job = (LoadJob *)data;
    if (env != NULL) {
        napi_handle_scope scope = NULL;
        if (napi_open_handle_scope(env, &scope) == napi_ok) {
            napi_value cb = NULL;
            napi_value global = NULL;
            napi_value arg = NULL;
            napi_get_reference_value(env, job->callback, &cb);
            napi_get_global(env, &global);
            napi_create_int32(env, job->rc, &arg);
            if (cb != NULL && global != NULL) {
                // 回调是 ArkTS 的 resolve；异常只可能是回调自己抛，清了不当未处理异常。
                if (napi_call_function(env, global, cb, 1, &arg, NULL) != napi_ok) {
                    napi_value ignored = NULL;
                    napi_get_and_clear_last_exception(env, &ignored);
                }
            }
            napi_close_handle_scope(env, scope);
        }
        napi_delete_reference(env, job->callback);
        if (job->work != NULL) {
            napi_delete_async_work(env, job->work);
        }
    }
    free(job);
}

/// asrLoadModel(handle, callback: (code:number)=>void): boolean
/// 返回 false = 没排上（回调不会被调用），调用方按失败处理。
static napi_value JsAsrLoadModel(napi_env env, napi_callback_info info) {
    napi_value a[2];
    size_t n = get_args(env, info, a, 2);
    napi_valuetype type = napi_undefined;
    bool queued = false;
    if (n >= 2 && napi_typeof(env, a[1], &type) == napi_ok && type == napi_function) {
        AsrHandle *h = handle_arg(env, a[0]);
        if (h != NULL) {
            LoadJob *job = (LoadJob *)calloc(1, sizeof(LoadJob));
            if (job != NULL) {
                job->handle = h;
                napi_value name = NULL;
                if (napi_create_string_utf8(env, "opi_asr_load", NAPI_AUTO_LENGTH, &name) == napi_ok &&
                    napi_create_async_work(env, NULL, name, load_execute, load_complete, job,
                                           &job->work) == napi_ok &&
                    napi_create_reference(env, a[1], 1, &job->callback) == napi_ok &&
                    napi_queue_async_work(env, job->work) == napi_ok) {
                    queued = true;
                } else {
                    if (job->callback != NULL) {
                        napi_delete_reference(env, job->callback);
                    }
                    if (job->work != NULL) {
                        napi_delete_async_work(env, job->work);
                    }
                    free(job);
                }
            }
        }
    }
    napi_value out = NULL;
    napi_get_boolean(env, queued, &out);
    return out;
}

/// asrStart(handle): number（0 = 成功）
static napi_value JsAsrStart(napi_env env, napi_callback_info info) {
    napi_value a[1];
    size_t n = get_args(env, info, a, 1);
    int32_t rc = -1;
    if (n >= 1) {
        AsrHandle *h = handle_arg(env, a[0]);
        if (h != NULL) {
            rc = opi_asr_start(h);
        }
    }
    return ret_int(env, rc);
}

/// asrFeedPcm(handle, pcm: Uint8Array): number（0 = 入队；-5 = 队列满，重试）
static napi_value JsAsrFeedPcm(napi_env env, napi_callback_info info) {
    napi_value a[2];
    size_t n = get_args(env, info, a, 2);
    int32_t rc = -1;
    if (n >= 2) {
        AsrHandle *h = handle_arg(env, a[0]);
        uint8_t *data = NULL;
        size_t len = 0;
        if (h != NULL && bytes_arg(env, a[1], &data, &len) && len <= INT32_MAX) {
            rc = opi_asr_feed_pcm(h, data, (int32_t)len);
        }
    }
    return ret_int(env, rc);
}

/// asrFinish(handle): number
static napi_value JsAsrFinish(napi_env env, napi_callback_info info) {
    napi_value a[1];
    size_t n = get_args(env, info, a, 1);
    int32_t rc = -1;
    if (n >= 1) {
        AsrHandle *h = handle_arg(env, a[0]);
        if (h != NULL) {
            rc = opi_asr_finish(h);
        }
    }
    return ret_int(env, rc);
}

/// asrCancel(handle): number（幂等；会话结束/会话切换时调用）
static napi_value JsAsrCancel(napi_env env, napi_callback_info info) {
    napi_value a[1];
    size_t n = get_args(env, info, a, 1);
    int32_t rc = -1;
    if (n >= 1) {
        AsrHandle *h = handle_arg(env, a[0]);
        if (h != NULL) {
            rc = opi_asr_cancel(h);
        }
    }
    return ret_int(env, rc);
}

/// asrPollEvent(handle, out: Uint8Array): number（>0 = JSON 字节数，0 = 暂无）
static napi_value JsAsrPollEvent(napi_env env, napi_callback_info info) {
    napi_value a[2];
    size_t n = get_args(env, info, a, 2);
    int32_t rc = -1;
    if (n >= 2) {
        AsrHandle *h = handle_arg(env, a[0]);
        uint8_t *buf = NULL;
        size_t cap = 0;
        if (h != NULL && bytes_arg(env, a[1], &buf, &cap) && cap > 0 && cap <= INT32_MAX) {
            rc = opi_asr_poll_event(h, (char *)buf, (int32_t)cap);
        }
    }
    return ret_int(env, rc);
}

/// asrLastError(handle, out: Uint8Array): number；handle 传 0 = create 失败原因
static napi_value JsAsrLastError(napi_env env, napi_callback_info info) {
    napi_value a[2];
    size_t n = get_args(env, info, a, 2);
    int32_t rc = -1;
    if (n >= 2) {
        uint8_t *buf = NULL;
        size_t cap = 0;
        if (bytes_arg(env, a[1], &buf, &cap) && cap > 0 && cap <= INT32_MAX) {
            AsrHandle *h = handle_arg(env, a[0]);
            rc = opi_asr_last_error(h, (char *)buf, (int32_t)cap);
        }
    }
    return ret_int(env, rc);
}

/// asrMaxEventBytes(): number（事件缓冲上限，ArkTS 一次性分配）
static napi_value JsAsrMaxEventBytes(napi_env env, napi_callback_info info) {
    (void)info;
    return ret_int(env, opi_asr_max_event_bytes());
}

/// asrDestroy(handle): void（必须在会话结束、load 回调之后；0 安全）
static napi_value JsAsrDestroy(napi_env env, napi_callback_info info) {
    napi_value a[1];
    size_t n = get_args(env, info, a, 1);
    if (n >= 1) {
        opi_asr_destroy(handle_arg(env, a[0]));
    }
    napi_value out = NULL;
    napi_get_undefined(env, &out);
    return out;
}

napi_value opi_asr_napi_init(napi_env env, napi_value exports) {
    napi_property_descriptor desc[] = {
        {"asrCreate", NULL, JsAsrCreate, NULL, NULL, NULL, napi_default, NULL},
        {"asrLoadModel", NULL, JsAsrLoadModel, NULL, NULL, NULL, napi_default, NULL},
        {"asrStart", NULL, JsAsrStart, NULL, NULL, NULL, napi_default, NULL},
        {"asrFeedPcm", NULL, JsAsrFeedPcm, NULL, NULL, NULL, napi_default, NULL},
        {"asrFinish", NULL, JsAsrFinish, NULL, NULL, NULL, napi_default, NULL},
        {"asrCancel", NULL, JsAsrCancel, NULL, NULL, NULL, napi_default, NULL},
        {"asrPollEvent", NULL, JsAsrPollEvent, NULL, NULL, NULL, napi_default, NULL},
        {"asrLastError", NULL, JsAsrLastError, NULL, NULL, NULL, napi_default, NULL},
        {"asrMaxEventBytes", NULL, JsAsrMaxEventBytes, NULL, NULL, NULL, napi_default, NULL},
        {"asrDestroy", NULL, JsAsrDestroy, NULL, NULL, NULL, napi_default, NULL},
    };
    napi_define_properties(env, exports, sizeof(desc) / sizeof(desc[0]), desc);
    return exports;
}
