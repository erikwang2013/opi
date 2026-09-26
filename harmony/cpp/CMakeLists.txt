# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT

# ⚠️ 草案 · 从未编译 · 未验证
#
# 本文件在 Linux 上编写，**从未被 CMake 配置过、从未参与任何构建**。
# 本机没有 OHOS SDK / DevEco，`cmake` 跑都没跑过（连语法都没验）。
#
# 这份 CMakeLists 的位置对应 DevEco 工程里的 entry/src/main/cpp/CMakeLists.txt。
# 还要在 entry/build-profile.json5 里登记，否则**CMake 根本不会被调用**，
# 表现为「没生成 .so、ArkTS 侧 import 得到 undefined」：
#
#   "buildOption": {
#     "externalNativeOptions": {
#       "path": "./src/main/cpp/CMakeLists.txt",
#       "abiFilters": ["arm64-v8a"]
#     }
#   }
#
# ---- 名字一致性（四处必须相同，见 napi_bridge.c 顶部）----
#   target 名 opiime  ⇄  nm_modname "opiime"  ⇄  libopiime.so  ⇄  import 'libopiime.so'

cmake_minimum_required(VERSION 3.4.1)
project(opiime)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

# Rust 引擎：静态库，由 scripts/ 里的命令产出后放到 cpp/libs/${OHOS_ARCH}/
# 见 harmony/README.md「构建集成」。三套 ABI 目录名对应关系：
#   arm64-v8a   ← aarch64-unknown-linux-ohos
#   armeabi-v7a ← armv7-unknown-linux-ohos
#   x86_64      ← x86_64-unknown-linux-ohos
set(OPI_RUST_LIB "${CMAKE_CURRENT_SOURCE_DIR}/libs/${OHOS_ARCH}/libopi_ffi.a")

add_library(opiime SHARED napi_bridge.c)

if(NOT EXISTS "${OPI_RUST_LIB}")
    # 显式报错。少了这一步的表现是「链接期一堆 undefined reference to opi_*」，
    # 看着像 C ABI 写错了，其实是 Rust 库没放对地方。
    message(FATAL_ERROR
        "找不到 Rust 静态库: ${OPI_RUST_LIB}\n"
        "先在仓库根跑 harmony/README.md「构建集成」里的 cargo 命令，"
        "把 libopi_ffi.a 拷到 cpp/libs/${OHOS_ARCH}/。")
endif()

target_link_libraries(opiime PUBLIC
    "${OPI_RUST_LIB}"       # Rust 引擎（28 个 C 导出，本桥用到 16 个）
    libace_napi.z.so        # N-API：napi_module_register / napi_create_string_utf16 ...
    libhilog_ndk.z.so       # 日志（OH_LOG_*）；本桥暂未用，留着以便加调试输出
)

# Rust 静态库自带 std/compiler_builtins，与 OHOS 运行时可能重定义同名符号。
# ⚠️ 这两条是**凭记忆的兜底**，未验证：如果链接报 multiple definition，
# 先试放开；如果反而报 --gc-sections 不认，就整条删掉。
target_link_options(opiime PRIVATE
    "-Wl,--gc-sections"
    "-Wl,--allow-multiple-definition"
)

# Rust 静态库 release 带 debug_info，未 strip 时约 9 MB（本机实测）。
# 上架前 strip 一次；调试期可注释掉。
target_link_options(opiime PRIVATE "-Wl,-s")
