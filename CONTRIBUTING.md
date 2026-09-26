# 贡献指南

OPI 欢迎两类贡献：**代码**与**词库数据**。两者流程不同，其中词库数据涉及第三方许可证，请务必先读第 2 节。

## 1. 代码贡献

### 构建与测试

```bash
cargo test --workspace                   # 单元 + 集成 + 属性测试
cargo clippy --workspace --all-targets -- -D warnings   # 门禁：零警告
cd android && ./gradlew testDebugUnitTest   # Android 单测
cd android && ./gradlew assembleDebug       # 构建 debug APK（cargokit 编译 opi-ffi 三 ABI .so）
cargo build --release -p fcitx5_opi         # Linux 插件（C++ 胶水另需 fcitx5-dev）
cd desktop && ./gradlew package             # Windows 候选窗（Compose Desktop）
./target/debug/opi-tools --version          # 版本号 + 小欧
```

> `cd android && ./gradlew assembleDebug` 需要机器上有 `dart`（cargokit 的构建工具是 Dart 写的），
> 缺了会在 `cargokitCargoBuildOpi_ffiDebug` 上报 `dart: command not found`（退出码 127）。
> 只跑单测不需要它。

### 要求

- **`cargo clippy --workspace --all-targets -- -D warnings` 必须零警告** —— 这是项目的硬门禁，CI 会拦。
- **`cargo test --workspace` 必须全绿**，新增行为请带测试。
- 引擎层（`crates/engine-core`）**禁止引入 IO 与平台依赖**：它是纯逻辑内核，也是唯一能在任何主机上跑测试的一层。
- Windows TSF 代码在 `#[cfg(target_os = "windows")]` 下，在 Linux 上**不参与类型检查** —— 改动 `crates/tsf-opi` 时请确认 CI 的 Windows 目标检查通过。
- 各端（Android / Linux / Windows / iOS）共享同一套模式整数与输入语义，改语义请各端一起改。
- 保持文件在 500 行以内；提交信息用 `type(scope): 描述` 格式。

## 2. 词库数据贡献

**这一节比代码节重要**：词库数据来自第三方，许可证不是 MIT。

### 许可证合规（必读）

- **本仓库代码**：MIT（见 [`LICENSE`](LICENSE)）。
- **词库数据**：按上游许可证**单独声明**，**不是 MIT**。当前上游中 `rime-luna-pinyin` 与 `rime-terra-pinyin` 均为 **LGPL-3.0**，Unicode Unihan 为 Unicode License。逐条来源与许可证见 [`data/raw/LICENSES.md`](data/raw/LICENSES.md)。

因此：

1. **提交词表改动即是对 LGPL-3.0 数据的派生**，编译产物（`.opid`）随之受 LGPL-3.0 约束。请确认你接受这一点。
2. **新增任何数据来源，必须先在 `data/raw/LICENSES.md` 中登记**：文件、来源 URL、许可证、生成方式。**来源不明或许可证不兼容（如 GPL 与非 LGPL 混用）的数据一律不接受。**
3. **不要把上游词表内容粘进 MIT 许可的源码文件**（`.rs` / `.kt` / `.py` 脚本的逻辑本身可以 MIT，词表内容不行）。词表内容只放 `data/raw/*.tsv`。
4. 引用上游数据时保留其版权声明与许可证文本，不要删除。

### 改动流程

```bash
# 1. 改源数据 —— 只改 data/raw/*.tsv（或改 scripts/ 下的生成脚本后重新生成）
#    繁体：python3 scripts/gen_trad_dict.py          （需网络，自行拉取 Unihan + terra 上游）
#    简体：python3 scripts/gen_luna_dict.py <luna_pinyin.dict.yaml> > /tmp/luna_merged.tsv

# 2. 合并并编译成 .opid（compile 一次只吃一个输入，繁体是两份 TSV 拼接后编译）
cat data/raw/trad_hanzi.tsv data/raw/trad_phrases.tsv > /tmp/trad_merged.tsv
cargo run -p opi-tools -- compile /tmp/trad_merged.tsv  data/generated/trad.opid
cargo run -p opi-tools -- compile data/raw/fallback.tsv data/generated/fallback.opid

# 3. 校验（校验和 + 条目顺序两道）
cargo run -p opi-tools -- verify data/generated/trad.opid

# 4. 更新部署副本（android assets 里的才是运行时真正加载的那份）
cp data/generated/trad.opid android/app/src/main/assets/trad.opid

# 5. 跑门禁测试
cargo test --workspace
cargo test -p opi-tools --test trad_coverage    # GB2312 单字全覆盖门禁
```

### 两道门禁和一个手工检查

- **`trad_coverage`（自动，硬门禁）**：GB2312 6763 个单字逐一 `query(pinyin)` 断言有候选。任何删字、改拼音都会撞上它。
- **`cli` / `m2_integration`（自动）**：编译产物可加载、可查询、损坏可回退。
- **排序质量（人工，必做）**：门禁只保证「有候选」，不保证「候选对」。词频改动请**手动实测**常用输入，例如简体 `fa` 应出 `发`、`nihao` 应出 `你好`，繁体模式同理。历史上 luna 的 rime 原始权重曾让 `樊/泛` 压过 `发`，靠脚本按 GB2312 常用度重排才修好 —— 这类回归只有人眼能发现。

`luna.opid` 是本地重编产物、不入库（`data/generated/.gitignore`），部署副本在 `android/app/src/main/assets/`；`fallback.opid` 与 `trad.opid` 入库。

## 3. 提交与 PR

- 一个 PR 只做一件事；不要在同一个 PR 里既改代码又重编词库，除非二者确实相关。
- PR 描述里写清：改了什么、**怎么验证的**（跑了哪条命令、看到什么结果）。无法验证的部分（如真机触摸、目标平台验收）请明确标注为未验证。
- 不要提交构建产物、`target/`、`.env` 或任何密钥。
