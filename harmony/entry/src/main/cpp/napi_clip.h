// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

#ifndef OPI_NAPI_CLIP_H
#define OPI_NAPI_CLIP_H

#include "napi/native_api.h"

// 注册 clip* 出口到同一个 opiime 模块（napi_bridge.c 的 OpiImeInit 调用）。
napi_value opi_clip_napi_init(napi_env env, napi_value exports);

#endif
