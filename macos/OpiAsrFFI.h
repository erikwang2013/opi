// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// opi-asr 的 macOS C ABI 声明（2026-11-02）。
//
// 真源：crates/opi-asr/src/ffi.rs（**不是** opi-ffi）。单独成头，因为
// crates/opi-ffi/tests/c_abi_contract.rs 要求 macos/OpiFFI.h 的声明与
// opi-ffi 导出**集合相等**，opi-asr 的出口不能混进那份头。
//
// 字符串是 UTF-8 C 字符串（与 OpiString 的 UTF-16 不同）。构建需另链
// libopi_asr 并随包 sherpa-onnx 运行库。生命周期：
// create -> load_model -> start -> feed_pcm* -> finish -> poll_event -> destroy。
// destroy 必须是最后一个调用；除 destroy 外可从任意线程调用（feed 来自音频线程）。
#ifndef OPI_ASR_FFI_H
#define OPI_ASR_FFI_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

void *opi_asr_create(const char *config_json);
void opi_asr_destroy(void *handle);
int32_t opi_asr_load_model(void *handle);
int32_t opi_asr_start(void *handle);
/// data 是 16 kHz 单声道 PCM16 LE；单次最多 256 KiB。返回 -5 = 队列满，稍后重试。
int32_t opi_asr_feed_pcm(void *handle, const uint8_t *data, int32_t len);
int32_t opi_asr_finish(void *handle);
int32_t opi_asr_cancel(void *handle);
/// 非阻塞：>0 JSON 长度，0 无事件，-8 缓冲不足。缓冲按 opi_asr_max_event_bytes() 分配。
int32_t opi_asr_poll_event(void *handle, char *buf, int32_t cap);
int32_t opi_asr_last_error(void *handle, char *buf, int32_t cap);
int32_t opi_asr_max_event_bytes(void);

#ifdef __cplusplus
}
#endif

#endif
