/* SPDX-FileCopyrightText: 2026 erik.xyz
 * SPDX-License-Identifier: MIT
 *
 * crates/opi-asr 的 C ABI 声明（语义见 crates/opi-asr/src/ffi.rs）。
 * 宿主 App 用 swiftc -import-objc-header 引入；键盘扩展**不**包含本头、**不**链 opi_asr。
 */
#ifndef OPI_ASR_H
#define OPI_ASR_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

void *opi_asr_create(const char *config_json);
void opi_asr_destroy(void *handle);
int32_t opi_asr_load_model(void *handle);
int32_t opi_asr_start(void *handle);
int32_t opi_asr_feed_pcm(void *handle, const uint8_t *data, int32_t len);
int32_t opi_asr_finish(void *handle);
int32_t opi_asr_cancel(void *handle);
int32_t opi_asr_poll_event(void *handle, char *buf, int32_t cap);
int32_t opi_asr_last_error(void *handle, char *buf, int32_t cap);
int32_t opi_asr_max_event_bytes(void);
char *opi_asr_verify_model(const char *dir, const char *manifest_json);
void opi_asr_string_free(char *s);

#ifdef __cplusplus
}
#endif

#endif /* OPI_ASR_H */
