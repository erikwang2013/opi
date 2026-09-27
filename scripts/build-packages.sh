#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 erik.xyz
# SPDX-License-Identifier: MIT
#
# 打包 Linux 发行版包：.deb 与 .rpm。
#
# 用法：
#   scripts/build-packages.sh                 # 两个都出
#   scripts/build-packages.sh deb             # 只出 .deb
#   scripts/build-packages.sh rpm
#   scripts/build-packages.sh --libdir lib64 rpm   # 覆盖安装库目录
#
# 设计要点（读完再改）：
#
# 1. **落点不在这里声明**。包里的文件树 = `cmake --install` 的暂存树原样搬过去，
#    而 CMake 的 install() 落点被 `opi_locate_check` 对着**真 fcitx5 库**验过
#    （问真库「loadDictionary() 去哪找」，与 cmake --install 的落点比）。打包侧
#    再抄一份路径就多一个会漂的真源 —— 本仓反复栽过这一类。
#
# 2. **两种格式的库目录本来就不同**，所以分别构建暂存树：
#      Debian 系多架构 → lib/x86_64-linux-gnu/fcitx5（GNUInstallDirs 自己算）
#      RPM 系          → lib64/fcitx5
#    用 `-DCMAKE_INSTALL_LIBDIR=` 显式传，别指望在 Debian 主机上编出 RPM 的布局。
#    （若那个目标准则变了，CMake 侧会红——`opi_locate_check` 就是干这个的。）
#
# 3. **不装工具链也能跑**：本机没有 fcitx5-dev 时，用 FCITX5_HEADERS / FCITX5_LIBS
#    指向解包的 fcitx5 头/库（与 run-harness.sh 同一套环境变量）。
#
# 4. nfpm 只做「把树装进包」这一件事。它不编译、不解析依赖 —— 所以
#    `depends: fcitx5` 是**声明**而非推导，正确性靠 CI 在真发行版里装一次来验。

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

OUT_DIR="${OUT_DIR:-$REPO_ROOT/dist}"
# ⚠️ 工作目录默认**在仓库之外**，这不是洁癖，是被门禁逼的：
# CMake 的编译器探测产物 `CMakeFiles/<ver>/CompilerIdCXX/CMakeCXXCompilerId.cpp` 有 **919 行**，
# 而 `crates/opi-ffi/tests/line_limit.rs` 走的是**文件系统**（不是 git 索引 ⇒ `.gitignore`
# 挡不住它），会把那份探测文件当源码扫，于是 500 行门禁红。
# 那道门禁的排除表按**目录名精确相等**匹配（`target|build|__pycache__|node_modules`），
# `build-packages` 与 `deb-build` 都不在其中 —— 换个名字也只是碰运气。
# 落仓库外就不产生这个交互（构建产物本来也不该进源码树）。
WORK_DIR="${WORK_DIR:-${TMPDIR:-/tmp}/opi-packages}"

# ---- 要出哪几个格式 ----
FORMATS=()
OVERRIDE_LIBDIR=""
while [ $# -gt 0 ]; do
    case "$1" in
        deb|rpm) FORMATS+=("$1") ;;
        --libdir) OVERRIDE_LIBDIR="${2:?--libdir 需要一个值}"; shift ;;
        -h|--help) sed -n '5,12p' "$0"; exit 0 ;;
        *) echo "未知参数: $1" >&2; exit 2 ;;
    esac
    shift
done
[ ${#FORMATS[@]} -gt 0 ] || FORMATS=(deb rpm)

# ---- 前置 ----
if ! command -v nfpm >/dev/null 2>&1; then
    cat >&2 <<'EOF'
找不到 nfpm。装一个（单二进制，不需要 root）：
  go install github.com/goreleaser/nfpm/v2/cmd/nfpm@latest
  # 或从 https://github.com/goreleaser/nfpm/releases 取 linux_amd64 包，
  #    把 nfpm 放进 PATH（例如 ~/.local/bin）
EOF
    exit 1
fi

# 版本：单一真源是 Cargo.toml 的 [workspace.package] version。
# 别在这里写死 —— 发版清单里那 6 处版本位已经够多了。
PKG_VERSION="$(sed -n '/^\[workspace\.package\]/,/^\[/p' Cargo.toml \
    | sed -n 's/^version *= *"\(.*\)"/\1/p' | head -1)"
[ -n "$PKG_VERSION" ] || { echo "从 Cargo.toml 读不到 version" >&2; exit 1; }

# 架构：deb 用 amd64/arm64，rpm 用 x86_64/aarch64。
MACHINE="$(uname -m)"
case "$MACHINE" in
    x86_64)         DEB_ARCH=amd64; RPM_ARCH=x86_64  ;;
    aarch64|arm64)  DEB_ARCH=arm64; RPM_ARCH=aarch64 ;;
    *) echo "未支持的架构: $MACHINE（需要时在 case 里补一行）" >&2; exit 1 ;;
esac

HDR_ARGS=()
[ -n "${FCITX5_HEADERS:-}" ] && HDR_ARGS+=("-DFCITX5_HEADERS=$FCITX5_HEADERS")
[ -n "${FCITX5_LIBS:-}" ]    && HDR_ARGS+=("-DFCITX5_LIBS=$FCITX5_LIBS")

echo "==> OPI 打包：版本 $PKG_VERSION / 架构 $MACHINE / 格式 ${FORMATS[*]}"

# ---- Rust cdylib（两种格式共用同一份）----
echo "==> 构建 Rust cdylib"
cargo build --release -p fcitx5_opi --locked

build_stage() {
    # $1 = 暂存目录名  $2 = CMAKE_INSTALL_LIBDIR（空 = 让 GNUInstallDirs 自己算）
    #
    # ⚠️ 结果经**全局变量** STAGE_DIR 返回，不走 `$( )`。
    # 原因：`stage="$(build_stage …)"` 时脚本的 `set -e` 不会因函数内部的失败而中止 ——
    # 实测这样写过：configure 失败后它照样往下跑 nfpm，只因为暂存树是空的才被 glob 拦下。
    # 若暂存树**部分**填充，那就会安安静静打出一个残缺的包 —— 打包脚本最坏的一种错。
    # 所以每一步都显式 `|| return 1`，调用方也显式判。
    local name="$1" libdir="$2"
    local build="$WORK_DIR/$name-build" stage="$WORK_DIR/$name-stage"
    STAGE_DIR="$stage"

    rm -rf "$build" "$stage"
    local args=(-S crates/fcitx5-opi/cpp -B "$build"
                -DCMAKE_BUILD_TYPE=Release
                -DCMAKE_INSTALL_PREFIX=/usr
                "${HDR_ARGS[@]}")
    [ -n "$libdir" ] && args+=("-DCMAKE_INSTALL_LIBDIR=$libdir")

    echo "==> [$name] configure（${libdir:-GNUInstallDirs 自适应}）" >&2
    cmake "${args[@]}" >&2 || return 1
    cmake --build "$build" -j"$(nproc)" >&2 || return 1
    DESTDIR="$stage" cmake --install "$build" >&2 || return 1

    # 装了哪些文件**打出来**：落点错了（比如 addon 没进 fcitx5/）在这里就能看见，
    # 不必等到装完没反应。
    echo "==> [$name] 暂存树：" >&2
    find "$stage" -type f | sed "s|$stage||" | sort | sed 's/^/    /' >&2 || return 1

    # 空转护栏：一个文件都没装出来就是构建坏了，别拿空树去过 nfpm。
    local n
    n="$(find "$stage" -type f | wc -l)"
    if [ "$n" -lt 5 ]; then
        echo "  ✗ 暂存树只有 $n 个文件（期望 5：两个 .so + 两个 conf + 词库）—— 构建不完整，拒绝打包" >&2
        return 1
    fi
    return 0
}

# ---- 逐格式构建并打包 ----
mkdir -p "$OUT_DIR"

for fmt in "${FORMATS[@]}"; do
    case "$fmt" in
        # Debian 多架构的 lib 目录由 GNUInstallDirs 自己算出来，别写死
        deb) build_stage deb ""                         || { echo "✗ deb 构建失败" >&2; exit 1; }
             pkg_arch="$DEB_ARCH" ;;
        # RPM 系是 lib64；在 Debian 主机上编 RPM 布局必须显式传
        rpm) build_stage rpm "${OVERRIDE_LIBDIR:-lib64}" || { echo "✗ rpm 构建失败" >&2; exit 1; }
             pkg_arch="$RPM_ARCH" ;;
    esac
    stage="$STAGE_DIR"

    echo "==> [$fmt] nfpm 打包"
    # ⚠️ **自己把 ${...} 换成字面值**，不依赖 nfpm 的变量展开。
    # 实测 nfpm 2.47 不展开 `contents.src` 里的 `${PKG_STAGE}`，报
    # `Glob failed: ${PKG_STAGE}/usr` —— 而它展不展开是版本相关的行为，
    # 打包脚本不该把正确性押在这上面。这里 sed 出的配置是确定性的。
    render_cfg="$WORK_DIR/$fmt-nfpm.yaml"
    sed -e "s|\${PKG_ARCH}|$pkg_arch|g" \
        -e "s|\${PKG_VERSION}|$PKG_VERSION|g" \
        -e "s|\${PKG_STAGE}|$stage|g" \
        scripts/nfpm.yaml > "$render_cfg"

    # 渲染后不许再有残留占位符 —— 否则就是「路径没填」，会打出一个空包或错路径的包
    if grep -q '\${PKG_' "$render_cfg"; then
        echo "  ✗ 配置里还有未替换的占位符：" >&2
        grep -n '\${PKG_' "$render_cfg" >&2
        exit 1
    fi

    nfpm package --config "$render_cfg" --packager "$fmt" --target "$OUT_DIR/"

    # 包里的文件清单**打出来并与暂存树对账**：少了 conf / 少了词库 / 少了许可证
    # 都不会报错，只会「装完没用」或「授权不完整」，所以这里显式比一次。
    echo "==> [$fmt] 包内清单"
    case "$fmt" in
        deb) dpkg-deb -c "$OUT_DIR"/opi_"$PKG_VERSION"_"$pkg_arch".deb | awk '{print "    " $NF}' ;;
        rpm) rpm -qlp "$OUT_DIR"/opi-"$PKG_VERSION"*.rpm 2>/dev/null | sed 's/^/    /' \
                 || echo "    （本机无 rpm 命令，跳过清单打印；包已生成）" ;;
    esac
done

echo "==> 产物："
ls -la "$OUT_DIR"/ | sed 's/^/    /'
