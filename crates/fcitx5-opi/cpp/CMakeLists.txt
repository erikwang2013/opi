# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# OPI 的 fcitx5 胶水：编译 + 安装。
#
# 这份文件是**唯一的安装通路**。它此前不存在 —— 于是源码树里没有任何东西会把
# 词库放到 loadDictionary() 找的位置，只得靠人按 README 手抄一遍（run-harness.sh
# 也是这么干的）。后果不是「装不上」，而是更坏的一种：addon 在、输入法注册出来了、
# 按键也被接受，但候选永远是内置的 35 个词 —— 装了、能用、但用不出东西来。
#
# ---- 安装位置与查找位置的对应（fcitx5 5.1.12 实测，非读文档转述）----
# 这份对应关系是拿真 StandardPath 探针跑出来的，探针在 opi_locate_check.cpp，
# CI 每次都会重跑一遍 —— 改这里的 DESTINATION 会让 CI 红，不是靠注释守。
#
#   opi_fcitx5.cpp loadDictionary()
#     Type::Data    + "opi/luna.opid"   -> <prefix>/share/opi/luna.opid
#   fcitx5 的 addon 加载器
#     Type::PkgData + "addon/…"         -> <prefix>/share/fcitx5/addon/
#     Type::PkgData + "inputmethod/…"   -> <prefix>/share/fcitx5/inputmethod/
#     Type::Addon   + "*.so"            -> fcitx5 编译期定的 addon 目录
#
# 注意词库用的是 Type::Data，**没有** fcitx5 前缀（带 fcitx5 前缀的是 PkgData）。
# 两个 conf 用 PkgData，所以它们确实在 share/fcitx5/ 下面。词库若跟着放进
# share/fcitx5/opi/ 就是**找不到且不报错**，只表现为候选质量骤降 —— 与「压根
# 没装词库」在用户侧无法区分。这正是本文件要钉死的东西。
#
# ---- 用法 ----
#   cmake -S crates/fcitx5-opi/cpp -B build-fcitx5 -DCMAKE_BUILD_TYPE=Release
#   cmake --build build-fcitx5
#   cmake --install build-fcitx5
# 发行版打包：加 DESTDIR= 暂存；fcitx5 的 addon 目录要它自己的前缀，见下面
# OPI_ADDON_DIR 那段。

cmake_minimum_required(VERSION 3.16)
project(opi_fcitx5 LANGUAGES CXX)

include(GNUInstallDirs)

# ---- fcitx5 的头/库 ----
#
# 有 fcitx5-dev 的机器走 find_package（发行版打包、CI 都是这条）。
#
# 没有 fcitx5-dev 的机器（本机就是）走 FCITX5_HEADERS/FCITX5_LIBS：从 deb 解包
# 拿头/库，run-harness.sh 用的就是这两个名字 —— 沿用同一组变量名，好让两条通路
# 指同一个目录树，别各自维护一份。
#   apt-get download libfcitx5{core,utils,config}-dev libfcitx5{core7,utils2,config6}
#   for f in *.deb; do dpkg-deb -x "$f" root/; done
#   cmake -S . -B build \
#     -DFCITX5_HEADERS=$PWD/root/usr/include/Fcitx5 \
#     -DFCITX5_LIBS=$PWD/root/usr/lib/x86_64-linux-gnu
#
# 为什么要这个口子：deb 里的 CMake 包把路径写死成 /usr/include（它按
# FCITX_INSTALL_USE_FCITX_SYS_PATHS=On 构建，不做重定位），于是没装
# fcitx5-dev 的机器上 find_package 会在 generate 期报
# 「Imported target includes non-existent path /usr/include/Fcitx5/Core」——
# 解包的头就在手边也用不上。
set(FCITX5_HEADERS "" CACHE PATH
    "含 Core/ Utils/ Config/ 的目录。设了就替掉 find_package（无 fcitx5-dev 的机器用）")
set(FCITX5_LIBS "" CACHE PATH "FCITX5_HEADERS 模式下的库目录（含 libFcitx5Core.so）")

add_library(opi_fcitx5_deps INTERFACE)
if(FCITX5_HEADERS)
    if(NOT FCITX5_LIBS)
        # 与 run-harness.sh 同样的做法：实地探有没有 libFcitx5Core.so，别数 ../ 层数
        foreach(_cand "${FCITX5_HEADERS}/../../lib/x86_64-linux-gnu"
                      "${FCITX5_HEADERS}/../../lib64"
                      "${FCITX5_HEADERS}/../../lib")
            if(EXISTS "${_cand}/libFcitx5Core.so")
                get_filename_component(FCITX5_LIBS "${_cand}" ABSOLUTE)
                break()
            endif()
        endforeach()
    endif()
    if(NOT EXISTS "${FCITX5_LIBS}/libFcitx5Core.so")
        message(FATAL_ERROR
            "FCITX5_HEADERS 模式下找不到 ${FCITX5_LIBS}/libFcitx5Core.so。\n"
            "注意：只有 -dev 包里的 .so 符号链接还不够 —— 它指到 .so.7，而 .so.7 在\n"
            "运行库包（libfcitx5core7 等）里，两个都要解包到同一个 root/ 下。")
    endif()
    target_include_directories(opi_fcitx5_deps INTERFACE
        "${FCITX5_HEADERS}/Core" "${FCITX5_HEADERS}/Utils" "${FCITX5_HEADERS}/Config")
    target_link_directories(opi_fcitx5_deps INTERFACE "${FCITX5_LIBS}")
    target_link_libraries(opi_fcitx5_deps INTERFACE Fcitx5Core Fcitx5Utils Fcitx5Config)
    message(STATUS "fcitx5 头/库取自解包目录（未用 find_package）: ${FCITX5_HEADERS}")
else()
    # Fcitx5Utils 除了头/库，还导出 FCITX_INSTALL_ADDONDIR —— fcitx5 官方 addon
    # 就是用它定落点的，别自己拼 libdir/fcitx5。
    find_package(Fcitx5Core REQUIRED)
    find_package(Fcitx5Utils REQUIRED)
    find_package(Fcitx5Config REQUIRED)
    target_link_libraries(opi_fcitx5_deps INTERFACE Fcitx5::Core Fcitx5::Utils Fcitx5::Config)
endif()

# ---- Rust 侧 cdylib ----
#
# 和 harmony/cpp/CMakeLists.txt 同一套做法：不在这里驱动 cargo，只要产物，
# 找不到就 FATAL_ERROR 把命令写清楚（否则链接期一堆 undefined reference，
# 看着像 C ABI 写错了，其实是 Rust 库没构建）。
set(OPI_RUST_LIB "" CACHE FILEPATH
    "libfcitx5_opi.so 路径。缺省猜 <repo>/target/release/，即 cargo build --release -p fcitx5_opi 的产物")
if(NOT OPI_RUST_LIB)
    set(_rust_cand "${CMAKE_CURRENT_SOURCE_DIR}/../../../target/release/libfcitx5_opi.so")
    if(EXISTS "${_rust_cand}")
        set(OPI_RUST_LIB "${_rust_cand}")
    endif()
endif()
if(NOT EXISTS "${OPI_RUST_LIB}")
    message(FATAL_ERROR
        "找不到 Rust cdylib: '${OPI_RUST_LIB}'\n"
        "先在仓库根跑：cargo build --release -p fcitx5_opi\n"
        "（或用 -DOPI_RUST_LIB=/path/to/libfcitx5_opi.so 指一个已有产物）")
endif()

# ---- 词库源 ----
#
# luna.opid 是 gitignore 的本地重编产物（data/generated/.gitignore），干净
# checkout 上**不存在**；入库的部署副本在 android/app/src/main/assets/。
# 取入库那份：始终存在、且与 android 端发的是同一份文件（当前两份 md5 相同）。
# 自己想用刚重编的那份就 -DOPI_LUNA_OPID=data/generated/luna.opid。
#
# 这里**不**把词库拷进 crates/fcitx5-opi/data/ 入库 —— 2.3MB 的二进制，仓库里
# 已经有一份了，再存一份只会多一个会漂的副本。
set(OPI_LUNA_OPID "" CACHE FILEPATH
    "luna.opid 源文件。缺省用入库的 android 部署副本")
if(NOT OPI_LUNA_OPID)
    set(_dict_cand "${CMAKE_CURRENT_SOURCE_DIR}/../../../android/app/src/main/assets/luna.opid")
    if(EXISTS "${_dict_cand}")
        set(OPI_LUNA_OPID "${_dict_cand}")
    endif()
endif()
if(NOT EXISTS "${OPI_LUNA_OPID}")
    message(FATAL_ERROR
        "找不到词库源: '${OPI_LUNA_OPID}'\n"
        "生成一份：python3 scripts/gen_luna_dict.py\n"
        "或用 -DOPI_LUNA_OPID=/path/to/luna.opid 指一个已有文件。")
endif()
message(STATUS "OPI 词库源: ${OPI_LUNA_OPID}")

# ---- addon .so 的落点 ----
#
# FCITX_INSTALL_ADDONDIR 是 fcitx5 自己给的变量，但它跟着 CMAKE_INSTALL_PREFIX
# 走：前缀是 /usr（发行版打包的标准做法）时才等于 fcitx5 真的会去搜的目录；
# 前缀是缺省的 /usr/local 时得出 /usr/local/lib/fcitx5，而发行版 fcitx5 编译
# 进去的搜索路径里**没有**这一条（本机实测：/usr/lib/x86_64-linux-gnu/fcitx5）。
# 那种情况下装了也加载不了 —— 又是一个「装完没反应」。故这种时候吱一声：
# 配置期的警告有执行点，README 里的提醒没有。
if(FCITX_INSTALL_ADDONDIR)
    set(_opi_addon_default "${FCITX_INSTALL_ADDONDIR}")
    set(_opi_addon_is_sys "${FCITX_INSTALL_USE_FCITX_SYS_PATHS}")
else()
    # 解包头/库模式（没装 fcitx5-dev）没有 FCITX_INSTALL_ADDONDIR。用相对路径，
    # 好让 cmake --install --prefix <暂存目录> 能把它也挪过去。
    set(_opi_addon_default "${CMAKE_INSTALL_LIBDIR}/fcitx5")
    # 这条分支问不到 fcitx5 自己的变量，只能按前缀判：前缀是 /usr 就是发行版形态
    # （此时 CMAKE_INSTALL_LIBDIR 在多架构发行版上即 lib/<arch>，落点正是
    # fcitx5 编译期定的那个）。判对了才不会为 /usr 白报一条 warning ——
    # 会假报警的 warning 等于没有 warning。
    if(CMAKE_INSTALL_PREFIX STREQUAL "/usr")
        set(_opi_addon_is_sys TRUE)
    else()
        set(_opi_addon_is_sys FALSE)
    endif()
endif()
set(OPI_ADDON_DIR "${_opi_addon_default}" CACHE PATH
    "addon .so（胶水 + Rust cdylib）的安装目录")
if(OPI_ADDON_DIR STREQUAL _opi_addon_default AND NOT _opi_addon_is_sys)
    message(WARNING
        "addon 将装到 ${OPI_ADDON_DIR}，而本机 fcitx5 的 addon 搜索路径通常不含它"
        "（CMAKE_INSTALL_PREFIX=${CMAKE_INSTALL_PREFIX}）。\n"
        "  发行版打包：-DCMAKE_INSTALL_PREFIX=/usr\n"
        "  本机自用：  -DOPI_ADDON_DIR=/usr/lib/${CMAKE_LIBRARY_ARCHITECTURE}/fcitx5"
        "（路径以 fcitx5-diagnose 输出为准）")
endif()

# ---- 胶水本体 ----
#
# 编译选项与 run-harness.sh 的 g++ 行逐字对齐（-Wall -Wextra -Werror：警告即
# 失败）。两者是同一份源码的两条通路，别让它们漂。
#
# ---- C++ 标准：20（两个 target 都写，清单在下面两处 set_target_properties）----
#
# fcitx5 从 5.1.21 起头里用了 std::span / std::ranges / string::starts_with（要
# C++20）。实测：5.1.21、5.1.22、5.1.23 有，5.1.12 与 5.0.21 没有 —— **分界在
# 5.1.12 与 5.1.21 之间**（没逐版二分到具体那一个版本）。钉 17 的后果是 Arch
# （fcitx5 5.1.23 + GCC 16）上两个 target 都编不过，报的是
# 「'std::span' is only available from C++20 onwards」这类**硬错误**：与 -Werror
# 无关，剥掉 -Werror 照样红。改 20 对老版本安全：5.0.21 与 5.1.12 的头在
# -std=gnu++20 -Wall -Wextra -Werror 下实测**零错误零警告**。
#
# ⚠️ 别改成「删掉 CXX_STANDARD 让它继承」：fcitx5 的 Fcitx5CompilerSettings.cmake
# （那里写着 CMAKE_CXX_STANDARD 20）**没有被任何已安装的 Fcitx5*Config.cmake
# include**（实测 grep 零命中），继承不到。Arch 上删了也编得过，只是因为 GCC 16
# 的默认标准恰好是 C++20（实测 __cplusplus=202002L）；默认标准随编译器版本变
# （本机 GCC 12 实测 201703L）⇒ 换个默认 17 的编译器 + 5.1.21 以上的头，红的就是
# 上面那个硬错误（这条等价于把 CXX_STANDARD 钉回 17 —— Arch 上已实测红）。
#
# ---- 为什么两个 .cpp 里有 #pragma … "-Wdeprecated-declarations" ----
#
# fcitx5 自 5.1.21 起把 StandardPath 标为弃用（替代品是 StandardPaths）。**不能迁移**：
# standardpaths.h 在 5.1.12 与 5.0.21 里根本不存在（实测零命中），而这两个版本仍在支持
# 范围内；分界在 5.1.12 与 5.1.21 之间（未逐版二分）。于是每一处 StandardPath 用法各包
# 一层 push / ignored / pop，**只包住那几条语句**（opi_fcitx5.cpp 的 loadDictionary()、
# opi_locate_check.cpp 的 check() 与 main()）。
#
# ⚠️ 别把它挪成这里的全局 -Wno-error=deprecated-declarations —— 那等于对**将来任何**
# 弃用一起关掉这道门禁，而它现在是有效的一票（5.1.23 上剥掉抑制即红）。
# 何时删：不再支持 <5.1.21 时改用 StandardPaths，并连同两个 .cpp 里的 pragma 一起删。
#
# 这段散文**只写这一处**：三个 pragma 点各留一行指针。复制成多份必漂（本仓老毛病），
# 而且 opi_fcitx5.cpp 的行数余量本来就小（500 行门禁）。
add_library(opi_fcitx5_glue SHARED opi_fcitx5.cpp)

# addon 的 Library= 写的是 libfcitx5_opi_glue，而 fcitx5 按 `X` -> `X.so` 找
# （**不自动补 lib 前缀**），所以文件名必须正好是 libfcitx5_opi_glue.so。
# CMake 缺省会自己加 lib 前缀拼成 liblibfcitx5_opi_glue.so，故 PREFIX 置空、
# 名字里自己带 lib。
# （落点目录里那些 .so 是 0644 不带 x —— 系统 fcitx5 addon 就是这样，别去补。）
set_target_properties(opi_fcitx5_glue PROPERTIES
    PREFIX ""
    OUTPUT_NAME "libfcitx5_opi_glue"
    CXX_STANDARD 20 # 理由见上面「C++ 标准：20」那段 —— 别改回 17、也别删
    CXX_STANDARD_REQUIRED ON
    # 胶水靠 dlopen 找同目录的 libfcitx5_opi.so（Rust cdylib），$ORIGIN 是
    # 唯一不写死绝对路径的办法 —— 与 run-harness.sh 的 -Wl,-rpath,'$ORIGIN' 同义。
    INSTALL_RPATH "$ORIGIN"
    BUILD_RPATH "$ORIGIN")

target_compile_options(opi_fcitx5_glue PRIVATE -Wall -Wextra -Werror)
target_link_libraries(opi_fcitx5_glue PRIVATE opi_fcitx5_deps)

# ★ Rust cdylib 必须**按 -l 名字链**，不能按仓库里的完整路径链。
# rustc 产出的 cdylib **没有 SONAME**，而链接器对没有 SONAME 的 .so 会把命令行上
# 给的形式原样记进 DT_NEEDED：
#   给完整路径 /path/to/target/release/libfcitx5_opi.so
#     -> NEEDED [/path/to/target/release/libfcitx5_opi.so]   ← 装到用户机器上必然找不到
#   给 -L<dir> -lfcitx5_opi
#     -> NEEDED [libfcitx5_opi.so]                            ← 由 $ORIGIN rpath 在同目录解析
# 两种写法都能编译通过、都 ldd -r 干净（在构建机上），差别只在**装完之后**。
# run-harness.sh 一直用的是后者，这里与它对齐。
get_filename_component(_opi_rust_dir "${OPI_RUST_LIB}" DIRECTORY)
get_filename_component(_opi_rust_name "${OPI_RUST_LIB}" NAME_WE)
string(REGEX REPLACE "^lib" "" _opi_rust_linkname "${_opi_rust_name}")
target_link_directories(opi_fcitx5_glue PRIVATE "${_opi_rust_dir}")
target_link_libraries(opi_fcitx5_glue PRIVATE "${_opi_rust_linkname}")

# ---- 落点探针 ----
#
# 问真 fcitx5 库「loadDictionary() 会去哪找词库」，与 cmake --install 的落点比。
#
# 编译成 target（而不是在 CI 里手写一行 g++）是有来历的：CI 原来那行 g++
# **漏了 -I**，于是这个探针从写完到 v1.3.0 发布**一次都没被编译器看过** ——
# 发布 CI 就红在它身上，而它后面的「查找常量没漂」grep 被 skip、从未跑过。
# 挂在 opi_fcitx5_deps 上就自动继承它算好的 include 目录（find_package 与
# 解包头/库两种模式都对，胶水走的就是这两条路），不会再漂。
# 放进 ALL（默认）而不是 EXCLUDE_FROM_ALL：只有每次 cmake --build 都编它，
# 「没被编译器看过」才不会再发生一次。
add_executable(opi_locate_check opi_locate_check.cpp)
set_target_properties(opi_locate_check PROPERTIES
    CXX_STANDARD 20 # 与胶水同一档，理由见上面「C++ 标准：20」那段 —— 别改回 17
    CXX_STANDARD_REQUIRED ON)
# 与胶水用同一档告警（run-harness.sh 的 g++ 行也是这一档）：警告即失败。
target_compile_options(opi_locate_check PRIVATE -Wall -Wextra -Werror)
target_link_libraries(opi_locate_check PRIVATE opi_fcitx5_deps)

# ---- 安装 ----
#
# 两个 .so 必须**同一目录**：胶水的 $ORIGIN rpath 找的就是旁边那个 cdylib。
# 探针不装（不在下面的 install(TARGETS) 里）—— 它是构建期门禁，不是交付物。
#
# 权限**用缺省（0644），刻意不加 PERMISSIONS**：曾按「与同目录发行版 addon 一致」的
# 理由想改成 0755，实测该目标本身没有唯一解 —— 邻居的 mode **按发行版族分裂**（同一
# 目录实测：Arch 5.1.23 / Fedora / openSUSE 是 0755；Debian 12 / Ubuntu 22.04 /
# Deepin 25 是 0644），而本构建面向 Debian 系 ⇒ **缺省即与邻居一致**。改变它也无功能
# 影响（非 root，uid 1000 下 dlopen 已实测不受影响）。
# ⚠️ 要验 mode 只能在**包**里验（`dpkg-deb -c` / `rpm -qlp --dump`，mode 是**第 5 个
# 字段**，第一个是路径）：构建树的 mode 由链接器加 umask 决定（同一份源码实测本机账号
# 0775、root 容器 0755）⇒ 拿构建树当判据必假绿。
install(TARGETS opi_fcitx5_glue LIBRARY DESTINATION "${OPI_ADDON_DIR}")
install(FILES "${OPI_RUST_LIB}" DESTINATION "${OPI_ADDON_DIR}")

# 两个 conf：fcitx5 经 Type::PkgData 找，即 <prefix>/share/fcitx5/。
# 少了 inputmethod/opi.conf 输入法条目注册不出来（可用输入法列表是扫这个目录来的）。
install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/../data/addon/opi_fcitx5.conf"
              DESTINATION "${CMAKE_INSTALL_DATADIR}/fcitx5/addon")
install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/../data/inputmethod/opi.conf"
              DESTINATION "${CMAKE_INSTALL_DATADIR}/fcitx5/inputmethod")

# 词库：loadDictionary() 找的是 Type::Data + "opi/luna.opid"，
# 即 <prefix>/share/opi/luna.opid。**没有 fcitx5 中间层**，别顺手挪进
# share/fcitx5/ 下面（那种装法 CI 会红，因为 opi_locate_check 问的是真库）。
install(FILES "${OPI_LUNA_OPID}"
              DESTINATION "${CMAKE_INSTALL_DATADIR}/opi"
              RENAME luna.opid)
