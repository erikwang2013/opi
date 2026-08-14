# fcitx5 插件胶水（B2）

`opi_fcitx5.cpp`：fcitx5 AddonInstance + InputMethod 薄层，调用 Rust 逻辑
出口（`libfcitx5_opi.so` 的 `opi_fcitx5_*` C 符号）。键事件路由全部在
Rust 侧（`../src/input_method.rs`，镜像 Android KeyRouter），本文件只做
键事件转发与提交。

## 构建前提

- fcitx5 开发头（`fcitx5-dev`，≥ 5.1，需要 `KeyEvent::isLongPressed`）
- Rust cdylib 已构建：

```bash
cargo build --release -p fcitx5_opi   # → ../../target/release/libfcitx5_opi.so
```

## 编译（fcitx5 addon .so）

```bash
g++ -std=c++17 -shared -fPIC -o libfcitx5_opi_glue.so opi_fcitx5.cpp \
    $(pkg-config --cflags fcitx5) \
    -L../../target/release -lfcitx5_opi -Wl,-rpath,'$ORIGIN' \
    -lfcitx5core
```

## 安装

- `libfcitx5_opi_glue.so` 与 addon 元数据（`opi_fcitx5.conf.in` 等，B3
  接线）放入 fcitx5 addon 目录，例如
  `/usr/lib/fcitx5/`（发行版常见路径，以 `fcitx5-diagnose` 输出为准）。
- 词库路径：B3 的 `opi_fcitx5_init_dict` 把插件分发的 `luna.opid` 经 size
  校验拷贝到 XDG 数据目录（`$XDG_DATA_HOME/opi/luna.opid`，文件名镜像
  Android EngineLoader.FILE_NAME）；初始化时经 `fcitx::StandardPath` 探测
  同一路径；未找到则 Rust 侧使用内置回退词库（打包接线在验收阶段完成）。

## 状态

本机无 fcitx5-dev 头（`pkg-config --exists fcitx5` 失败、
`/usr/include/fcitx5` 不存在），**未在本机编译**——验收阶段在装有
fcitx5-dev 的桌面机上编译验证。
