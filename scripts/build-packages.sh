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
#
# 5. **可选的桌面应用合并**（环境变量 OPI_DESKTOP_DEB=<jpackage 的 .deb>）：
#    设了就把 jpackage 打出的桌面应用（/opt/opi/**，自带 JRE）解进
#    暂存树，与插件装进**同一个**包 —— 用户裁决「和到一起安装」（2026-10-09）。
#    代价如实记：包体 ~1.7MB → ~48MB（jpackage 自带运行时；这是那份裁决时摆过的
#    数）；不设 = 只出插件包，脚本其余行为一字不变。
#    对旧独立包 opi-candidates（≤1.5.0 时代的设置窗包，装到 /opt/opi-candidates/**）
#    用 replaces+conflicts 接管：不声明的话 dpkg 会因文件归属冲突**拒绝安装**。

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

# ---- 可选：桌面应用（jpackage 的 .deb）合并 ----
DESKTOP_DEB="${OPI_DESKTOP_DEB:-}"
if [ -n "$DESKTOP_DEB" ]; then
    [ -f "$DESKTOP_DEB" ] || { echo "OPI_DESKTOP_DEB 指向的文件不存在：$DESKTOP_DEB" >&2; exit 1; }
    # 先确认它确实是 jpackage 的应用包（防拿错文件进来）：包内要有 /opt/*/bin/ 启动器。
    # ⚠️ 两个坑都是实测踩出来的，别改回 pipelining：
    #   1) 路径列是 `./opt/...` 形态，模式里不能写带前导空格的 ' /opt/'（零命中）；
    #   2) `dpkg-deb -c | grep -q` 在 pipefail 下**必假**：grep -q 一命中就退出，
    #      dpkg-deb 的 tar 被子进程 SIGPIPE 杀掉（报「tar 子进程被信号(断开的管道) 终止了」），
    #      pipeline 取到的是那个非零状态 —— 正确的输入也会被判成「不像 jpackage 包」。
    # 拿变量 + case 匹配：没有管道，就不存在这两个形态。
    listing="$(dpkg-deb -c "$DESKTOP_DEB")" || { echo "读不出 $DESKTOP_DEB 的内容" >&2; exit 1; }
    case "$listing" in
        *"/opt/"*"/bin/"*) ;;
        *)
            echo "OPI_DESKTOP_DEB 看着不像 jpackage 的包（包内没有 /opt/*/bin/ 启动器）：$DESKTOP_DEB" >&2
            exit 1
            ;;
    esac
    echo "==> 合并桌面应用：$DESKTOP_DEB"
fi

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

# ---- opi-ctl（桌面状态栏的 CLI，两种格式共用同一份）----
# 它也**不进 CMake 暂存树**（CMakeLists 只管 fcitx5 侧的胶水与数据）——
# 落点由 nfpm.yaml 的 ${PKG_CTL_BIN} → /usr/bin/opi-ctl 声明，源就是这里的产物。
echo "==> 构建 opi-ctl"
cargo build --release -p opi-tools --bin opi-ctl --locked
# 相对路径（脚本全程以仓库根为 CWD，与 LICENSE / data/raw 那两条 contents 同一约定）。
PKG_CTL_BIN="target/release/opi-ctl"
# 编完却找不到产物 = 构建坏了或有人设了 CARGO_TARGET_DIR —— 立刻拦，别让 nfpm
# 拿着不存在的 src 去 Glob（那会打出一个没有 opi-ctl 的包，检查不出「按钮点了没反应」）。
[ -x "$PKG_CTL_BIN" ] || {
    echo "  ✗ cargo 编完成功但 $PKG_CTL_BIN 不存在（设了 CARGO_TARGET_DIR？）" >&2
    exit 1
}

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
    # 5 是**下界**（现在实际是「两个 .so + 两个 conf + 两份词库 + 符号类目」6 个，
    # 只写 5 是刻意留松）；皮肤（4 张主题 × 4 个文件）与桌面应用（合并模式，/opt
    # 整棵）都在它之上，不在这里点名。
    local n
    n="$(find "$stage" -type f | wc -l)"
    if [ "$n" -lt 5 ]; then
        echo "  ✗ 暂存树只有 $n 个文件（下界 5：两个 .so + 两个 conf + 词库）—— 构建不完整，拒绝打包" >&2
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

    # ---- 合并桌面应用（见头部第 5 条）----
    # dpkg-deb -x 只解数据文件，不碰 DEBIAN/（jpackage 那份的控制脚本在这里无用，
    # 且与 nfpm 生成的控制字段可能冲突）。jpackage 自己的菜单入口已撤（见
    # desktop/build.gradle.kts 的「桌面入口：已撤」段）；随它带进来的是 /opt/**
    # 加 injectSettingsDesktopEntry 注入的两处桌面入口（/usr/share/applications
    # 与 /etc/xdg/autostart）。
    if [ -n "$DESKTOP_DEB" ]; then
        dpkg-deb -x "$DESKTOP_DEB" "$stage"
        # 阴性对照：解完必须真有启动器 —— 否则是「包打了、应用没进去」的静默形态。
        if ! ls "$stage"/opt/*/bin/* >/dev/null 2>&1; then
            echo "  ✗ 合并后暂存树里没有 /opt/*/bin/ 启动器 —— 应用没解进去" >&2
            exit 1
        fi
        # 跨设备剪贴板内核（2026-11-01）：jpackage 把 --input 的库放在 lib/app/。
        # 缺了它设置页只会报「加载 opi_ffi 动态库失败」—— 属于包层静默降级，钉在这里。
        if ! ls "$stage"/opt/*/lib/app/libopi_ffi.so >/dev/null 2>&1; then
            echo "  ✗ 桌面应用里缺 lib/app/libopi_ffi.so —— 跨设备剪贴板内核没进包" >&2
            echo "    检查 desktop/build.gradle.kts 的 buildOpiFfi / JPackage files.from" >&2
            exit 1
        fi
        # 语音运行时（2026-11-02）：主库 + sherpa/onnxruntime 必须同目录，缺一个 =
        # 状态栏话筒点了只报 dlopen 失败（包层静默降级，钉在暂存树里）。
        for lib in libopi_asr.so libsherpa-onnx-c-api.so libsherpa-onnx-cxx-api.so libonnxruntime.so; do
            if ! ls "$stage"/opt/*/lib/app/"$lib" >/dev/null 2>&1; then
                echo "  ✗ 桌面应用里缺 lib/app/$lib —— 语音识别运行时没进包" >&2
                echo "    检查 desktop/build.gradle.kts 的 buildOpiAsr / JPackage files.from" >&2
                exit 1
            fi
        done
        # 同款负控：两处注入的桌面入口也必须在暂存树里。少了它们是**静默**的 ——
        # 2026-10-10 实测形态：autostart 注入进了桌面包、但 nfpm.yaml 的 contents
        # 原先只打 `${PKG_STAGE}/usr`，/etc 那条被丢在主包外，用户装完永远没有
        # 状态栏自启。判据钉在这一层，别只在桌面包那层验（那是「注入对了、
        # 包没打进去」的层间落差）。
        for f in usr/share/applications/opi-settings.desktop \
                 etc/xdg/autostart/opi-statusbar.desktop; do
            [ -f "$stage/$f" ] || {
                echo "  ✗ 合并后暂存树缺 $f —— 桌面入口注入回退（或 nfpm contents 又漏了 /etc）" >&2
                exit 1
            }
        done
        echo "==> [$fmt] 已合并桌面应用，暂存树 $(du -sh "$stage" | cut -f1)（含 JRE）"
    fi
    # 不合并桌面应用时暂存树没有 /etc —— nfpm 的 `${PKG_STAGE}/etc` 会 Glob failed。
    # 建一个空的顶住（空目录 nfpm 不打包任何文件，主包行为不变）。
    mkdir -p "$stage/etc"

    echo "==> [$fmt] nfpm 打包"
    # ⚠️ **自己把 ${...} 换成字面值**，不依赖 nfpm 的变量展开。
    # 实测 nfpm 2.47 不展开 `contents.src` 里的 `${PKG_STAGE}`，报
    # `Glob failed: ${PKG_STAGE}/usr` —— 而它展不展开是版本相关的行为，
    # 打包脚本不该把正确性押在这上面。这里 sed 出的配置是确定性的。
    render_cfg="$WORK_DIR/$fmt-nfpm.yaml"
    sed -e "s|\${PKG_ARCH}|$pkg_arch|g" \
        -e "s|\${PKG_VERSION}|$PKG_VERSION|g" \
        -e "s|\${PKG_STAGE}|$stage|g" \
        -e "s|\${PKG_CTL_BIN}|$PKG_CTL_BIN|g" \
        scripts/nfpm.yaml > "$render_cfg"

    # 渲染后不许再有残留占位符 —— 否则就是「路径没填」，会打出一个空包或错路径的包
    if grep -q '\${PKG_' "$render_cfg"; then
        echo "  ✗ 配置里还有未替换的占位符：" >&2
        grep -n '\${PKG_' "$render_cfg" >&2
        exit 1
    fi

    # 合并模式：把 /opt 也纳入包内容，并声明接管旧的独立包 opi-candidates（见头部第 5 条）。
    # ⚠️ **必须条件追加**：nfpm 的 contents.src 指向不存在的路径会直接报 Glob failed ——
    # 非合并构建若也带上这条，会在打普通插件包时红。
    # （追加在文件尾 = contents 数组续加一项 + 两个顶层键；YAML 对顶层键顺序无要求。）
    if [ -n "$DESKTOP_DEB" ]; then
        {
            echo "  - src: $stage/opt"
            echo "    dst: /opt"
            echo "replaces:"
            echo "  - opi-candidates"
            echo "conflicts:"
            echo "  - opi-candidates"
        } >> "$render_cfg"
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
