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
# fcitx5-dev 的机器上 find_package **不可用** —— 解包的头就在手边也用不上。
#
# 实测（cmake 3.31.4 + deepin 5.1.12 的 deb 解包树）：configure 与 generate **都过**，
# 红在**编译期**：imported target 的 INTERFACE_INCLUDE_DIRECTORIES 写死成
# /usr/include/Fcitx5/Core，本机没那个目录 ⇒ `fatal error: fcitx-utils/standardpath.h:
# 没有那个文件或目录`。（另：光给 `-DCMAKE_PREFIX_PATH=<解包根>` 连包都找不到 ——
# deb 树是 rootfs，包在 <根>/usr 下，前缀得给 <根>/usr，或直接传 Fcitx5*_DIR。）
# ⚠️ 本文档此前的说法是「在 **generate 期**报 Imported target includes non-existent
# path」，那是**未在本机复现**的旧描述；3.16–3.30 上是否真是 generate 期硬错**未核实**，
# 故不删、只降级。
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
    # ⚠️ SYSTEM 不是可有可无的关键字：理由见下面「为什么两个 .cpp 里有 #pragma …」
    # 段的**末段**（回退路是 -I、find_package 路是 -isystem，不写就不等价）。
    target_include_directories(opi_fcitx5_deps SYSTEM INTERFACE
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

# 繁体词库（2026-10-09 接线）：与 luna 同一套取源/校验规则，同样必装 —— 缺了
# Traditional 模式就只剩简体回退，而那**不报错**（只看得出「繁体模式打出来还是
# 简体」这种静默降级）。入库副本同样在 android 部署目录（README 的树里写着
# 「luna.opid（简体）· trad.opid（繁体）」），两份都在 git 里。
set(OPI_TRAD_OPID "" CACHE FILEPATH
    "trad.opid 源文件。缺省用入库的 android 部署副本")
if(NOT OPI_TRAD_OPID)
    set(_trad_cand "${CMAKE_CURRENT_SOURCE_DIR}/../../../android/app/src/main/assets/trad.opid")
    if(EXISTS "${_trad_cand}")
        set(OPI_TRAD_OPID "${_trad_cand}")
    endif()
endif()
if(NOT EXISTS "${OPI_TRAD_OPID}")
    message(FATAL_ERROR
        "找不到繁体词库源: '${OPI_TRAD_OPID}'\n"
        "生成一份：python3 scripts/gen_trad_dict.py（Unicode Unihan + rime-terra-pinyin）\n"
        "或用 -DOPI_TRAD_OPID=/path/to/trad.opid 指一个已有文件。")
endif()
message(STATUS "OPI 繁体词库源: ${OPI_TRAD_OPID}")

# 符号类目数据（22 类目，2026-10-10 接线）：符号浏览面板的数据源。C++ 侧只 locate
# 路径（opi_engine.h 的 loadDictionary），读文件在 Rust 侧（opi_fcitx5_load_symbols）。
# ⚠️ **入库真源 = scripts/gen_symbol_categories.py 的双输出** ——
# crates/fcitx5-opi/data/ 与 android/app/src/main/assets/ 这两份是同一次生成的、
# 逐字节相同的产物（脚本确定性输出，重跑字节相同）；**android 副本只是兼容旧部署
# 路径**（它先于本接线存在），不是「另一份真源」。两份若漂了：以生成器重跑为准、
# 重新双写，别手工修其中一份。
# 源优先级：-DOPI_SYMBOL_CATEGORIES > data/ 下的入库副本 > android 部署副本。
set(OPI_SYMBOL_CATEGORIES "" CACHE FILEPATH
    "symbol_categories.tsv 源文件。缺省取 crates/fcitx5-opi/data/ 的入库副本")
if(NOT OPI_SYMBOL_CATEGORIES)
    foreach(_sym_cand
            "${CMAKE_CURRENT_SOURCE_DIR}/../data/symbol_categories.tsv"
            "${CMAKE_CURRENT_SOURCE_DIR}/../../../android/app/src/main/assets/symbol_categories.tsv")
        if(NOT OPI_SYMBOL_CATEGORIES AND EXISTS "${_sym_cand}")
            set(OPI_SYMBOL_CATEGORIES "${_sym_cand}")
        endif()
    endforeach()
endif()
# 缺了就是 FATAL（与 luna/trad 同档）：不装的后果是符号浏览入口**静默禁用**
# （热键收到、面板不弹，用户只看得出「按了没反应」）—— 构建期拦下比用户侧发现好。
if(NOT EXISTS "${OPI_SYMBOL_CATEGORIES}")
    message(FATAL_ERROR
        "找不到符号类目数据源: '${OPI_SYMBOL_CATEGORIES}'\n"
        "生成一份：python3 scripts/gen_symbol_categories.py（双写入库路径）\n"
        "或用 -DOPI_SYMBOL_CATEGORIES=/path/to/symbol_categories.tsv 指一个已有文件。")
endif()
message(STATUS "OPI 符号类目源: ${OPI_SYMBOL_CATEGORIES}")

# 康熙字典词库（2026-10-16，「全发」的 Linux 面）：随包安装进 share/opi/，fcitx5 侧
# 首启种子（opi_dicts.h 的 opiSeedBundledDict）把它拷进用户词库目录一次 —— 之后
# 就是普通导入词库。真源 = scripts/gen_kangxi_dict.py 的双输出（同符号类目那条纪律）。
# 缺了同样 FATAL：不装的后果是「全发」在这一端**静默不发生**（用户词库页永远没有它）。
set(OPI_KANGXI_TSV "" CACHE FILEPATH
    "kangxi.tsv 源文件。缺省取 crates/fcitx5-opi/data/ 的入库副本")
if(NOT OPI_KANGXI_TSV)
    foreach(_kx_cand
            "${CMAKE_CURRENT_SOURCE_DIR}/../data/kangxi.tsv"
            "${CMAKE_CURRENT_SOURCE_DIR}/../../../android/app/src/main/assets/kangxi.tsv")
        if(NOT OPI_KANGXI_TSV AND EXISTS "${_kx_cand}")
            set(OPI_KANGXI_TSV "${_kx_cand}")
        endif()
    endforeach()
endif()
if(NOT EXISTS "${OPI_KANGXI_TSV}")
    message(FATAL_ERROR
        "找不到康熙字典词库源: '${OPI_KANGXI_TSV}'\n"
        "生成一份：python3 scripts/gen_kangxi_dict.py（双写入库路径）\n"
        "或用 -DOPI_KANGXI_TSV=/path/to/kangxi.tsv 指一个已有文件。")
endif()
message(STATUS "OPI 康熙字典词库源: ${OPI_KANGXI_TSV}")

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
# 这些 pragma 只盖得住**我们代码里的用法**，盖不住头文件内部的诊断：fcitx5 ≥5.1.21 的
# standardpath.h 自己在 `using StandardPathFileMap = std::map<std::string,
# StandardPathFile>` 那批 using / 声明里就用到 StandardPathFile，这类诊断在**解析头
# 文件时**发出，比 .cpp 里任何 pragma 都早 ⇒ include 目录得按系统头给，光靠 .cpp 里
# 的 pragma 不够。find_package 那条路经 imported target 自带 `-isystem`；解包头/库的
# 回退路是裸的 target_include_directories，故那里写 SYSTEM 让两条路等价（反过来把
# pragma 挪到 include 之前包住它也行，但那是两处源码改动、且与另一条路不同源）。
# 实测（同一个 5.1.21 头）：回退路 EXIT=2 报 standardpath.h 的
# -Werror=deprecated-declarations，同机 find_package 路 EXIT=0；加 SYSTEM 后两条路
# 都 EXIT=0。SYSTEM 在 cmake 3.22.1 起实测生效（3.31.4 / 4.3.4 / 4.4.3 同）；比
# 3.22 更老的没得测，故本文件声明的 3.16 下限对这一条**未覆盖**。
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
# 权限**显式给 0755**（刻意为之，不是缺省）：两族约定本就不同 —— 同一目录实测
# Arch 5.1.23 / Fedora / openSUSE 的 addon 是 0755，Debian 12 / Ubuntu 22.04 /
# Deepin 25 是 0644 ⇒ **没有唯一可对齐的「邻居」**。取 0755 的理由：RPM 系工具挑 0644，
# 没听说哪家挑 0755；要按包格式分支去「逐族匹配」是零功能收益的复杂度。改不改都无功能
# 影响（非 root，uid 1000 下 dlopen 已实测不受影响）。
# ⚠️ 两条规则各写一遍：装的是两个不同文件（CMake target 产物 + Rust cdylib），走
# **两条不同的 install 规则**，只改一条会一好一坏。
# ⚠️ 必须**显式**写，别指望缺省：缺省不是跨工具链的常量（同一份 CMakeLists 实测
# cmake 3.31.4 下两条规则都装成 0644；cmake 4.4.3 下 `install(TARGETS … LIBRARY)`
# 是 0755 而 `install(FILES)` 仍是 0644 ⇒ 同一个包里的两个 .so 会不一致）。
# ⚠️ 验 mode 只能在**包**里验（`dpkg-deb -c` / `rpm -qlp --dump`，mode 是**第 5 个
# 字段**，第一个是路径）：构建树的 mode 由链接器加 umask 决定（同一份源码实测本机账号
# 0775、root 容器 0755）⇒ 拿构建树当判据必假绿。
install(TARGETS opi_fcitx5_glue LIBRARY DESTINATION "${OPI_ADDON_DIR}"
        PERMISSIONS OWNER_READ OWNER_WRITE OWNER_EXECUTE
                    GROUP_READ GROUP_EXECUTE WORLD_READ WORLD_EXECUTE)
install(FILES "${OPI_RUST_LIB}" DESTINATION "${OPI_ADDON_DIR}"
        PERMISSIONS OWNER_READ OWNER_WRITE OWNER_EXECUTE
                    GROUP_READ GROUP_EXECUTE WORLD_READ WORLD_EXECUTE)

# 两个 conf：fcitx5 经 Type::PkgData 找，即 <prefix>/share/fcitx5/。
# 少了 inputmethod/opi.conf 输入法条目注册不出来（可用输入法列表是扫这个目录来的）。
install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/../data/addon/opi_fcitx5.conf"
              DESTINATION "${CMAKE_INSTALL_DATADIR}/fcitx5/addon")
install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/../data/inputmethod/opi.conf"
              DESTINATION "${CMAKE_INSTALL_DATADIR}/fcitx5/inputmethod")

# 项目宠物「小欧」当输入法图标。opi.conf 的 `Icon=opi-pet` 是**图标名**，
# fcitx5 用 IconTheme::findIcon 在 XDG 图标目录里按名解析（hicolor 是 XDG 规定的
# 公共回退主题，任何主题的继承链里都有它；其 index.theme 列出 scalable/apps）——
# 所以装到 share/icons/hicolor/scalable/apps 就会被找到，不需要 fcitx5 专有目录。
# ⚠️ **原样一份，不改名、不重绘**：几何真源是仓库根的 docs/opi-pet.svg，各端
# （Android / Windows / macOS 计划）共用它；这里再画一次或转一次格式就是第三份
# 几何，必然漂。源文件在 cpp/ 之外，是**有意**的 —— 跨目录取真源，好过复制一份。
# ⚠️ 装出来的文件名必须**逐字**等于 opi.conf 的 Icon= 加扩展名：不一致时
# 不报错、只是没图（回退文字标签），所以 opi_locate_check 拿这个值去问真库。
install(FILES "${CMAKE_CURRENT_SOURCE_DIR}/../../../docs/opi-pet.svg"
              DESTINATION "${CMAKE_INSTALL_DATADIR}/icons/hicolor/scalable/apps")

# 皮肤（候选面板主题）：classicui 从 StandardPath::Type::PkgData + "themes/" 找，
# 即 <prefix>/share/fcitx5/themes/<目录名>（本机实装位置 —— /usr/share/fcitx5/
# themes/{default,default-dark,deepin-light,deepin-dark} —— 就是这么装的）。
# 内容是生成产物（scripts/gen_fcitx5_themes.py，--check 对账），别手改产物。
# ⚠️ 目录名是**四处同名契约**：opi_skin.h 的 OpiSkin 映射、这里的清单、
# data/themes/ 下的目录、生成脚本的 SKINS —— 任何一处改名 = 皮肤静默回落默认
# 主题（classicui 找不到不报错）。加新皮肤四处一起加。
install(DIRECTORY
        "${CMAKE_CURRENT_SOURCE_DIR}/../data/themes/opi-light"
        "${CMAKE_CURRENT_SOURCE_DIR}/../data/themes/opi-dark"
        "${CMAKE_CURRENT_SOURCE_DIR}/../data/themes/opi-paper"
        "${CMAKE_CURRENT_SOURCE_DIR}/../data/themes/opi-cinnabar"
        "${CMAKE_CURRENT_SOURCE_DIR}/../data/themes/opi-fresh"
        "${CMAKE_CURRENT_SOURCE_DIR}/../data/themes/opi-azure"
        "${CMAKE_CURRENT_SOURCE_DIR}/../data/themes/opi-lemon"
        "${CMAKE_CURRENT_SOURCE_DIR}/../data/themes/opi-sunset"
        "${CMAKE_CURRENT_SOURCE_DIR}/../data/themes/opi-starlight"
        "${CMAKE_CURRENT_SOURCE_DIR}/../data/themes/opi-wisteria"
        "${CMAKE_CURRENT_SOURCE_DIR}/../data/themes/opi-pine"
        DESTINATION "${CMAKE_INSTALL_DATADIR}/fcitx5/themes")

# 词库：loadDictionary() 找的是 Type::Data + "opi/luna.opid"，
# 即 <prefix>/share/opi/luna.opid。**没有 fcitx5 中间层**，别顺手挪进
# share/fcitx5/ 下面（那种装法 CI 会红，因为 opi_locate_check 问的是真库）。
install(FILES "${OPI_LUNA_OPID}"
              DESTINATION "${CMAKE_INSTALL_DATADIR}/opi"
              RENAME luna.opid)

# 繁体词库（2026-10-09）：同一条查找路（Type::Data + "opi/trad.opid"），
# 由 opi_locate_check 的第 4 个参数守着。可选增强的语义在胶水侧
# （load_with_trad：坏了退回简体，不拖垮拼音）。
install(FILES "${OPI_TRAD_OPID}"
              DESTINATION "${CMAKE_INSTALL_DATADIR}/opi"
              RENAME trad.opid)

# 五笔码表（2026-10-14，M4 第二波）：**两张都装**（运行时按 WubiPlan 选一张装载，
# 见 opi_wubi.h 的裁决 —— 只装选中的，不是两张都进引擎）。查找路与 luna/trad 同一条
# （Type::Data + "opi/wubi86.opid" / "opi/wubi98.opid"），由 opi_locate_check 的
# 第 6/7 个参数守着。
# ⚠️ 缺省源**直接指 data/generated/**（这两张表**没有** android 部署副本 ——
# 桌面/Windows 才是五笔的主场，移动端本轮不发五笔）⇒ 与 trad 的差别是「源从哪来」，
# 别照抄它那句「缺省用入库的 android 部署副本」。
set(OPI_WUBI86_OPID "" CACHE FILEPATH "wubi86.opid 源文件。缺省用 data/generated/")
if(NOT OPI_WUBI86_OPID)
    set(_wubi86_cand "${CMAKE_CURRENT_SOURCE_DIR}/../../../data/generated/wubi86.opid")
    if(EXISTS "${_wubi86_cand}")
        set(OPI_WUBI86_OPID "${_wubi86_cand}")
    endif()
endif()
if(NOT EXISTS "${OPI_WUBI86_OPID}")
    message(FATAL_ERROR
        "找不到五笔 86 码表源: '${OPI_WUBI86_OPID}'\n"
        "生成一份：python3 scripts/gen_wubi_dict.py\n"
        "或用 -DOPI_WUBI86_OPID=/path/to/wubi86.opid 指一个已有文件。")
endif()
message(STATUS "OPI 五笔 86 码表源: ${OPI_WUBI86_OPID}")
install(FILES "${OPI_WUBI86_OPID}"
              DESTINATION "${CMAKE_INSTALL_DATADIR}/opi"
              RENAME wubi86.opid)

set(OPI_WUBI98_OPID "" CACHE FILEPATH "wubi98.opid 源文件。缺省用 data/generated/")
if(NOT OPI_WUBI98_OPID)
    set(_wubi98_cand "${CMAKE_CURRENT_SOURCE_DIR}/../../../data/generated/wubi98.opid")
    if(EXISTS "${_wubi98_cand}")
        set(OPI_WUBI98_OPID "${_wubi98_cand}")
    endif()
endif()
if(NOT EXISTS "${OPI_WUBI98_OPID}")
    message(FATAL_ERROR
        "找不到五笔 98 码表源: '${OPI_WUBI98_OPID}'\n"
        "生成一份：python3 scripts/gen_wubi_dict.py\n"
        "或用 -DOPI_WUBI98_OPID=/path/to/wubi98.opid 指一个已有文件。")
endif()
message(STATUS "OPI 五笔 98 码表源: ${OPI_WUBI98_OPID}")
install(FILES "${OPI_WUBI98_OPID}"
              DESTINATION "${CMAKE_INSTALL_DATADIR}/opi"
              RENAME wubi98.opid)

# 符号类目数据（2026-10-10）：loadDictionary() 找的是 Type::Data +
# "opi/symbol_categories.tsv"，即 <prefix>/share/opi/symbol_categories.tsv ——
# 与两个词库同一层（**没有** fcitx5 中间层，别顺手挪进 share/fcitx5/ 下）。
# 落点由 opi_locate_check 的第 5 个参数与 ci.yml 的「查找常量没漂」grep 守着。
# ⚠️ 源的真源说明见上面取源段：入库真源 = scripts/gen_symbol_categories.py 的
# 双输出，android 那份只是兼容旧部署路径。
install(FILES "${OPI_SYMBOL_CATEGORIES}"
              DESTINATION "${CMAKE_INSTALL_DATADIR}/opi"
              RENAME symbol_categories.tsv)

# 康熙字典词库（2026-10-16）：落点与 lookups 同一条路（Type::Data + opi/kangxi.tsv）
# —— fcitx5 侧首启种子从这儿取源（opi_config.h 的 applyOpiSettings locate 它）。
install(FILES "${OPI_KANGXI_TSV}"
              DESTINATION "${CMAKE_INSTALL_DATADIR}/opi"
              RENAME kangxi.tsv)
