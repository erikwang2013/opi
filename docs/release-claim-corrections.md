# 发布声明订正

> **为什么有这个文件**：**tag 注记与提交信息推出去就改不了** —— 而 v1.3.0 的那两份里有
> 7 条**说重了**、4 个数**不成立**、1 条**只在工作区面出现过（提交态无迹）**。tag 是别人会读的东西
> （`git tag -l -n99`），所以把**实测到的正确说法**落在这里，而不是让下一个人从提交正文里继承错误。
>
> 每条都给命令与实测输出。**凡是本轮没跑过的，一律标「未验证」**，不写成结论。
>
> ⚠️ **补记（2026-09-28 同日第二轮）**：tag 改不了，但**仓库会变** —— 上一轮写下的若干条
> 在当天后续改动里**已被修掉 / 已被改口径**。所以每条另给一行**现状**（见下面的总表）。
> 判据一律落在**文件名 + 符号**上、**不抄行号**：本文件先前抄的两处行号
> （`ci.yml` 的 `42/43`、`run-harness.sh` 的 `74/88/90`）**今天都已经漂了**，
> 这两处就是这条纪律的实证。
>
> ⚠️⚠️ **第三条纪律，第二轮新加的：核对前先定「面」，结论必须带面词。**
> 并发会话下同一个断言**有两个都真的真值面**：
> **提交态**（`git show <commit>:<path>` —— 不动，可复核）与
> **工作区态**（磁盘上未提交的文件 —— **随时在动**）。同一件事在两个面上可以给出
> **两个不同的真值**，**少一个面词，真话就变成假话**。
> 本文件 §二.5 的「560」就是被我这么写错的：我拿 `git log` 判「查无实据」，
> 而 560 **真的出现过**，只是长在工作区那一面上 —— **方法本身够不着目标**。
> 同族的还有 §二.3：`1447` / `1472` / `1362` **不是三个答案，是两个判据 × 两个面**。
> ⇒ 写数字前先问：**这是哪个面的值？这个判据在哪个面上成立？**

---

## v1.3.0（2026-09-28）

### 〇、现状总表 —— 每条的今天是哪一格

四格口径：**已修**（后来改对了）/ **仍不成立**（今天照旧是错的）/ **已作废**（不用改了，
换个说法重写）/ **未接线**（机制在、没有调用方）。判据只给**文件 + 符号**，行号留给
`grep`（见文首那条纪律）。详细取证在各条目里。

| # | tag / 提交里的声明 | 现状 | 判据（文件 + 符号） |
|---|---|---|---|
| 1 | Windows / Linux 装到真词库 | **Windows 仍不成立**；Linux 成立 | `crates/tsf-opi/src/dict_path.rs` 的三个候选至今全空；`crates/fcitx5-opi/cpp/CMakeLists.txt` 的 `install(... RENAME luna.opid)` |
| 2 | fcitx5「只有一个手写的 `g++` 行」 | **仍不成立**（是多条，见 §一.2） | `grep -n 'g++' crates/fcitx5-opi/cpp/run-harness.sh` |
| 3 | 「26 处 `catch_unwind` 收成一个 `guard`」 | **仍说重了**（见 §一.3） | `git show b3f6318^:crates/opi-ffi/src/jni.rs \| grep -c 'catch_unwind('` |
| 4 | 「cap8 覆盖 73/100 双音节词」 | **仍复现不了**（见 §一.4） | `crates/engine-core/src/jianpin.rs` 的 `//!` 模块文档（只有注释，无探针） |
| 5 | 「候选总数上限拆掉 —— 可达率变成 100%」 | **引擎层已修、两条端侧未跟上**（见 §一.5） | `crates/engine-core/src/router.rs` 的 `FETCH_LIMIT` 对 `EngineController.kt` 的 `fetchLimit` / `OpiEngine.swift` 的 `candidates(limit:)` |
| 6 | 「学习落盘机制移到 engine-data」 | **未接线**（见 §一.6） | `crates/engine-data/src/user_words.rs` 的 `atomic_write` / `read_document` 在 `lib.rs` 之外零调用方 |
| 7 | `jni.rs` `499→466` | **已作废**（两个数都不成立，见 §二.1） | `git show b3f6318^:… \| wc -l` vs `git show b3f6318:… \| wc -l` |
| 8 | 「1259 行 CSV」 | **已作废**（见 §二.2） | `scripts/gen_en_dict.py` 的 `EXPECT_ROWS` fail-closed 断言 |
| 9 | 「Unicode 18.0.0 共 1447 条、覆盖 1429」 | **已作废**（判据换过，见 §二.3） | `scripts/gen_symbols.py` 头注释 + `crates/engine-core/tests/symbol_coverage.rs` 的 `UTS51_EMOJI` |
| 10 | 「C 33 / JNI 26」 | **仍不成立**（见 §二.4） | `crates/opi-ffi/tests/c_abi/run.sh`（C 导出覆盖）+ `crates/opi-ffi/tests/jni_contract.rs`（注册表 1:1） |
| 11 | 「`scripts/gen_symbols.py` 560 行」 | **工作区面出现过、提交态从未到过 500**（见 §二.5） | 500 门禁 `crates/opi-ffi/tests/line_limit.rs` |
| 12 | 「中文标点开关出口」（新增出口，tag 正文有） | 出口与宿主声明都在、**六端无入口**（唯一调用点是两个测试夹具，见 §一.7） | `android/app/src/main/kotlin/io/opi/input/jni/OpiEngine.kt` 的 `toggleChinesePunct` 只有声明；C 侧只有 `crates/opi-ffi/tests/c_abi/consumer.c` |
| 13 | 「英文联想词源入库」 | 数据在、**无消费方** | `grep -rn 'en_words' --include=*.rs .` 只命中生成器脚本 |
| 14 | 「Emoji 80 → 1472 条（条目 583 → 3332）」 | **tag 时点成立**（其后判据被换，见 §二.3） | `data/raw/LICENSES.md` 的 symbols.tsv 行已改成「别写死数字」 |
| 15 | 「云同步不做」 | 成立（裁决，未变） | 不在本文件范围（README + 图） |

### 一、说重了的 7 条 —— 功能在，程度不到

#### 1. 「Windows / Linux 装到真词库」

- **tag 原文**：「Windows / Linux 装到真词库 —— TSF 此前恒跑 35 词兜底；fcitx5 此前连构建
  系统都没有（git log --all 零命中，只有一个手写的 g++ 行）」
- **实测**：**Linux 这半成立，Windows 这半不成立。**
  - Linux 成立：`crates/fcitx5-opi/cpp/CMakeLists.txt` 里有
    `install(FILES "${OPI_LUNA_OPID}" … RENAME luna.opid)`（原语句跨三行，此处省略号是省略，
    要原文就 `grep -n 'RENAME luna.opid' crates/fcitx5-opi/cpp/CMakeLists.txt`）。
  - Windows 不成立：**装载通路写好了，但仓库里没有任何打包步骤会把 `luna.opid` 放到它要找的位置**。
    `crates/tsf-opi/src/dict_path.rs` 的候选顺序是「环境变量 `OPI_DICT_PATH` → DLL 同目录 →
    `%LOCALAPPDATA%\opi` → 内建回退」——三个路径今天**全是空的**。
- **命令与输出**：

  ```
  $ ls .github/workflows/                 # 只有一个 job 文件，且 Windows 只有 cargo check
  ci.yml
  $ grep -n 'tsf_opi' .github/workflows/ci.yml
  82:      - name: cargo check -p tsf_opi (x86_64-pc-windows-msvc)
  83:        run: cargo check -p tsf_opi --target x86_64-pc-windows-msvc
  # ⚠️ 行号**只作本次快照**，别抄 —— 上一轮写的是 42/43，同一个文件里插进 fcitx5 job
  # 之后就漂到 82/83 了，而**步骤名一个字母没变**。要复核就 `grep -n 'tsf_opi'`。
  # 结论未变：`runs-on` 三处**全是 ubuntu-latest**（含新增的 fcitx5 job），
  # Windows 目标在这台 Linux runner 上只过 `cargo check`（不链接），**没有产物**。

  $ find . -name '*.ps1' -not -path './target/*'
  ./android/rust_builder/cargokit/cmake/resolve_symlinks.ps1   # 第三方 cargokit，非本仓交付
  ./.agents/skills/ruflo/v3/helpers/claude-flow-v3.ps1         # 工具链，非本仓交付
  # ⚠️ 不是空的（我先写成「（空）」，是错的）—— 但两条都不是 OPI 的打包步骤

  $ grep -rn 'luna.opid' scripts/ .github/ desktop/
  # 11 处命中，逐条看过，全是下面三类，**没有一处**把它送到 TSF 的三个候选路径：
  #   ① scripts/gen_luna_dict.py 的注释/说明文字（5 处）
  #   ② CI 里指向 Android assets 那份 android/app/src/main/assets/luna.opid（5 处）
  #   ③ CI 核对 fcitx5 conf 的那句 grep 'Type::Data, "opi/luna.opid"'（1 处）
  ```

  > ⚠️ **这一条差点以假输出交付**：我第一版把 `find '*.ps1'` 和上面那条 `grep` 的输出都写成
  > 「（空）」，实际分别是 2 条与 11 条。**结论（Windows 没有发货步骤）不变**，但**判据必须
  > 是逐条看过命中**，不是「我看到空输出」—— 写成「（空）」就等于把没人跑过的命令当证据。

  **更强的证据是源码自己写的**（比我上面那两条静态推断硬）：`crates/tsf-opi/src/dict_path.rs`
  的模块文档写着「本仓库目前**没有** Windows 打包脚本，故没有任何构建期拷贝动作 ——
  这两个候选就是全部通路」，并同处记着另一个缺口「`trad.opid` 不在候选里 …… 繁体模式
  无从装载」。

- **正确说法**：Windows 侧是「**通路已通、没有发货步骤**」—— 装到 Windows 上开箱
  **仍然是 35 词兜底**，除非用户自己设 `OPI_DICT_PATH`。这与该功能自己的提交
  （`de39cb0`）正文一致：那句写的是「今天没有任何打包步骤会把 luna.opid 放到 DLL 旁 ——
  该候选通了但空着」。
- **未验证**：本机无 Windows，**没有真的在 Windows 上装过**。上面是静态取证（文件与 CI 配置），
  不是端到端实测。
- **现状**：**Windows 那半仍不成立**（通路口子开着、`luna.opid` 仍没人送过去），
  Linux 那半成立且**已加强** —— 本轮补了 `.github/workflows/ci.yml` 的 fcitx5 job
  （CMake 配置/构建/装到暂存树 + 断言落点 == 查找位置）。⚠️ 但那是**装到 CI 的暂存树**，
  不是装到真机的 fcitx5 数据目录；「真守护进程端到端验过」那句仍未由本文件复核。

#### 2. 「一个手写的 `g++` 行」

- **tag 原文**：「fcitx5 此前连构建系统都没有（…，只有一个手写的 `g++` 行）」
- **实测**：**3 条**。`crates/fcitx5-opi/cpp/run-harness.sh` 里三条 `g++`：
  胶水 `.so`、面板探针、JSON 检查（按 `-shared` / `-o` 的形状认，**不给行号** ——
  理由见下，行号已经漂过一次）。
- **命令与输出**：

  ```
  $ grep -c 'g++' crates/fcitx5-opi/cpp/run-harness.sh
  3                                    # 数这条才是判据
  $ grep -n 'g++' crates/fcitx5-opi/cpp/run-harness.sh
  78:g++ -std=c++17 -Wall -Wextra -Werror -shared -fPIC \
  92:g++ -std=c++17 -Wall -Wextra -Werror -o "$work/opi_panel_driver" \
  94:g++ -std=c++17 -Wall -Wextra -Werror -o "$work/opi_json_check" \
  # ⚠️ 行号又漂了：上一轮记的是 74/88/90。**数没变（3），行号全变了** ——
  # 这正是「引用给文件+符号、不给行号」的理由。
  ```

- **正确说法**：此前是**手写的 `g++` 命令**（今天 3 条：胶水 `.so` / 面板探针 / JSON 检查，
  都按 `-shared`、`-o` 这类**形状**认，别按行号认），而不是「一行」。
- **现状**：**仍不成立**。CMake 通路建起来之后，这三条 `g++` 并没有被删 —— `run-harness.sh`
  仍是手工脚本，`crates/fcitx5-opi/cpp/CMakeLists.txt` 是并存的第二条构建通路。

#### 3. 「26 处 `catch_unwind` 收成一个 `guard`」

- **提交原文**（`b3f6318`）：「jni.rs 499→466：26 处 catch_unwind 收成一个 guard<T: Default>」
- **实测**：**改前 22 处、改后 26 处** —— 「26」是**改后**的数，不是被收掉的数量。
  数量**升了**，因为同一轮还新增了出口（`pub unsafe extern` 从 22 涨到 28）。
- **命令与输出**：

  ```
  $ git show b3f6318^:crates/opi-ffi/src/jni.rs | grep -c 'catch_unwind('
  22                                   # 改前：22 个调用点
  $ git show b3f6318:crates/opi-ffi/src/jni.rs | grep -c 'guard('
  26                                   # 改后：26 个 guard(...) 调用点
  $ git show b3f6318^:crates/opi-ffi/src/jni.rs | grep -c 'pub unsafe extern'
  22
  $ git show b3f6318:crates/opi-ffi/src/jni.rs | grep -c 'pub unsafe extern'
  28
  ```

- **正确说法**：**22** 处 `catch_unwind(…).unwrap_or(…)` 收进 **1** 个 `guard<T: Default>`；
  改后共 26 个调用点（`opijni_import_user_words` 的哨兵是 -1 而非 `Default`，故意不套）。
- **现状**：**仍说重了**（tag 与提交正文都改不了，本条的正确说法继续有效）。
  ⚠️ 上面 22/26/28 是 **`b3f6318` 这个提交态**的数，**不是今天的值** ——
  今天要自己数：
  `git show b3f6318:crates/opi-ffi/src/jni.rs | grep -c 'guard('`（换 `HEAD` 或删 `git show`
  就是另外两个态）。工作区在并发改动下**每天都在变**，抄下来的数活不过一天。

#### 4. 「cap8 覆盖 73/100 双音节词」

- **tag 原文**：「简拼上限 512 由覆盖率拐点定（cap8 覆盖 73/100 双音节词），不是拍的」
- **实测**：**这个数复现不了。** 它只以**注释**形式存在于 `crates/engine-core/src/jianpin.rs`
  的模块文档里；仓库里**没有那条探针**，也**没有 pin 住那 100 个双音节词的词表**。
  换言之「73/100」是当时的一次性观察，**不是可重跑的门禁**。
- **命令与输出**：

  ```
  $ grep -rn '73/100' crates/engine-core/
  crates/engine-core/src/jianpin.rs://!   cap4 = 41/100、cap8 = 73/100、cap16 = 99/100。…
  （全仓只此一处，且是模块文档注释 `//!`，不是断言）
  ```

  > 行号**故意不写**（上一轮写的是 `:33`；那个文件当时正被并发改动，行号随时会漂）。
  > 找它一律用 `grep -rn '73/100' crates/engine-core/`。

- **可复核的是另一件事** —— 线上 2 音节缩写的 cap 是 **22**（`22^2 = 484 ≤ 512`），
  这一条**有门禁钉着**：

  ```
  $ cargo test -p engine-core --lib jianpin
  test jianpin::tests::per_position_cap_never_breaks_the_ceiling ... ok
  test result: ok. 5 passed; 0 failed
  # 断言本体：crates/engine-core/src/jianpin_tests.rs 的
  #   fn per_position_cap_never_breaks_the_ceiling
  # 函数体第一段（rustfmt 拆成了四行，此处不是逐字）：
  #   assert_eq!(
  #       per_position_cap(2),
  #       22,
  #       "22^2 = 484 ≤ 512（23^2 = 529 越界）"
  #   );
  # 「5 passed」= 该文件里 #[test] 的条数（grep -c '#\[test\]' 同文件），
  # 但它**只覆盖 `--lib jianpin`**：5 不是全仓用例数。
  ```

- **正确说法**：上限 512 是真的，且「512 与各位置 cap」由门禁钉住；
  **「73/100 覆盖率拐点」没有可重跑的证据**，不要当已核实的事实引用。
- **现状**：**仍复现不了**，且后来更弱了一步 —— `crates/engine-core/src/jianpin.rs` 的模块文档
  自己写了「这些数字来自临时探针（**已删**），不是门禁」，却仍把 cap4/cap8/cap16 三个分数
  留在正文里。**探针删了 + 那 100 词的基准词表从未 pin** ⇒ 三个分数一个都验不了。
  处置不变：**当注释读，别当数据引**。

#### 5. 「候选总数上限拆掉 —— 长前缀的可达率从 0.8%~23% 变成 100%」

- **tag 原文**：「候选总数上限拆掉 —— 长前缀的可达率从 0.8%~23% 变成 100%」
- **实测**：**引擎层成立，端侧只跟了一半。** 上限拆在**引擎**里，但「用户能不能翻到第 9 页」
  取决于**端侧一次抓多少条**——两者不是一个东西：

  | 端 | 抓取点 | 现状 |
  |---|---|---|
  | engine-core / TSF / fcitx5 | `router.rs` / `logic.rs` / `candidate.rs` 的 `FETCH_LIMIT = usize::MAX` | **不受限**（已跟上） |
  | iOS / 鸿蒙 | `OpiEngine.swift` 的 `candidatesPage()` / `OpiEngine.ets` 的 `candidatesPage()` → `opi_candidates_page()` | **不受限**（走引擎分页，从不由前端给 limit） |
  | **Android** | `EngineController.kt` 的 `const val fetchLimit = 64` | **仍是 64** |
  | **macOS** | `OpiEngine.swift` 的 `candidates(limit: Int = 64)`，调用点 `InputController.swift` 的 `engine.candidates(limit: 64)` | **仍是 64** |

- **为什么这也算「说重了」**：tag 那句话是**用户可见**的承诺。分页数据源是
  `router.rs` 的 `fetched()`（`engine.candidates(FETCH_LIMIT)`），页数 `page_count()` =
  `fetched().len().div_ceil(PAGE_SIZE)`。端侧若只抓 64 条，能翻到的页数就**封在
  「64 / `PAGE_SIZE`」**（两轨的 `PAGE_SIZE` 都是 8）—— 长前缀下**用户翻不到**，
  与引擎侧上限拆没拆无关。**页数这个数别抄，读两个符号即可**：
  `crates/engine-core/src/router.rs` 的 `PAGE_SIZE` 与端侧传进去的 limit。
- **⚠️ 一处注释已经变成假话**（本轮新发现）：`macos/InputController.swift` 那行调用点写着
  `// 与 Rust 侧 FETCH_LIMIT 一致` —— Rust 侧早已是 `usize::MAX`，**这行注释从「一致」那天起
  就不再成立**。它属于代码文件，不在本文件可改范围内，故在此登记；
  改 macOS 的人请连带删掉/改写它。
- **未验证**：Android / macOS **都没有真机跑过**「长前缀能不能翻到末页」。上面是**静态取证**
  （常量与调用点），结论是「端侧传了 limit 就会截在 limit」，**不是**「我在机器上看见只出几页」。
- **正确说法**：说**引擎**上限拆掉是成立的；说**可达率 100%** 只能对
  「TSF / fcitx5 / iOS / 鸿蒙」四端成立，**Android 与 macOS 仍受端侧 limit 约束**。
- **现状**：**未全部修完** —— 两条端侧（Android `fetchLimit`、macOS `candidates(limit:)`）
  今天是 64；引擎与另外两轨已跟上。要复核就分别看上面表格里那四个符号，
  **别抄「64」这个数当结论**（改完它就不对了，判据是「端侧有没有自己传 limit」）。

#### 6. 「学习落盘机制移到 engine-data」

- **tag 原文**：「学习落盘机制移到 engine-data，engine-core 回到零 IO（无 IO 是已勾选的验收项）」
- **实测**：**前半句字面成立、后半句才是真话。**
  - 「`engine-core` 零 IO」**成立**：`crates/engine-core/src/` 下 `grep -rn 'std::fs\|File::\|OpenOptions'`
    **零命中**。
  - 「机制移到 engine-data」**成立**：`crates/engine-data/src/user_words.rs` 有
    `atomic_write` / `read_document`，`lib.rs` 也 `pub use` 出去了。
  - **但机制没有任何调用方**：全仓 `atomic_write` / `read_document` 的命中**只在
    `engine-data` 自己的文件里**（`lib.rs` 的 `pub use` + `user_words.rs` 的内联测试）。
    `engine-data` 确实被 `opi-ffi` / `tsf-opi` / `fcitx5-opi` / `opi-tools` 依赖，
    但那些调用的是词库侧（`load_or_fallback` / `load_mmap` / `fallback_dict`），
    **不是** `user_words` 这一支。
- **为什么这也算「说重了」**：tag 的「未验证（如实）」段落其实已经写了
  「学习落盘……的候选逻辑尚未接线」——**但那是 tag 的最后一段，正文里那句读起来像已完成**。
  两句都在同一份注记里、结论相反，读的人只会记住正文那句。
- **正确说法**：**机制已就位、落盘未接线**（六端都没有）。Android 设置页那个「导入/导出词库」
  是**另一条路**：Kotlin 侧自己写文件（`SettingsScreen.kt` 的注释明写「导入必须自己落盘」），
  走的是 `OpiEngine.importUserWords` / JNI，**不经过 `engine-data` 的 `atomic_write`**。
  「有导入 UI」≠「学习会落盘」。
- **现状**：**未接线**（截至本轮）。复核命令：`grep -rn 'atomic_write\|read_document' --include=*.rs .`
  —— 命中若仍全在 `crates/engine-data/` 内，就还是没接线。

#### 7. 「新增中文标点开关出口」（出口在、但**六端没有入口**）

- **tag 原文**：「ABI / 端侧：C 33 / JNI 26 —— 新增中文标点开关出口（返回值非 void：
  语义不同，客户端记不住）」
- **实测**：这句**字面全对**（出口确实新增了、确实非 void），**但读起来像「客户端用上了」**。
  真实情况是：**没有任何一个端侧产品调用它。** 全仓唯一的调用点是两个**测试夹具**：

  ```text
  # 不是逐字输出，是命中清单（行号按本文纪律略去）：
  $ grep -rnE 'opi_toggle_chinese_punct|opi_set_chinese_punct' \
      --include=*.c --include=*.cpp --include=*.h --include=*.swift --include=*.ets .
    crates/opi-ffi/tests/c_abi/consumer.c   ← 唯一真调用点（C ABI 门禁自带的消费者）
    macos/OpiFFI.h                          ← 只有声明与注释引用，无调用
  # Android 侧同理：调用点只有 android/jni_smoke/Main.java（JNI smoke 夹具），
  # 生产代码 android/app/src/main/** 里一处都没有（OpiEngine.kt 只有 external 声明）。
  ```

  - 路由层**是通的**：`crates/tsf-opi/src/logic_switches_tests.rs` 有真值表用例
    （`chinese_punct_and_fullwidth_are_independent`），说明引擎侧开关确实生效。
  - ⇒ 所以这是**「有实现、有测试、没调用方」**那一类，不是「没实现」。
- **⚠️ 一道门禁为什么会放过它**：`crates/opi-ffi/tests/c_abi/run.sh` 要求「**库里每一条导出
  都被 C 消费者调用过**」—— 这个「消费者」是**门禁自带的 `consumer.c`**，不是一个真宿主。
  ⇒ **门禁全绿只证明「导出被调过」，不证明「有产品在调」**。加出口时别把这道绿当接线证据。
- **正确说法**：出口与宿主**声明面**齐备（C 侧 `macos/OpiFFI.h`、JNI 侧 `OpiEngine.kt`
  都写了），**六端无入口** —— 没有设置项勾选框、没有触发键绑定。
- **现状**：**未接线**（截至本轮）。复核命令：
  `grep -rn 'toggleChinesePunct' android/app/src/` —— 只有 `OpiEngine.kt` 自己那几行声明，
  就是没接线。

### 二、不成立的 4 个数 + 1 条「只在工作区面成立」—— **删掉，不要换成新数**

前三条只出现在提交正文里（tag 与提交都改不了），第四条在 tag 正文，
第五条**只在未提交的工作区面出现过**（提交态无迹），所以把实测结果落在这里。
处置一律是**丢掉原数**：把 499 改成 415 只是把下一个错误留给下一个人 ——
**留下的是取数的方法，不是替抄一个新数**。

#### 1. 「jni.rs `499→466`」

- **实测**：**415 → 484**（父提交 415，`b3f6318` 之后 484）。

  ```
  $ git show b3f6318^:crates/opi-ffi/src/jni.rs | wc -l
  415
  $ git show b3f6318:crates/opi-ffi/src/jni.rs | wc -l
  484
  ```

- **「对不上任何提交态」是本轮补的硬证据** —— 历次提交里 `jni.rs` 的行数是
  `270 218 229 239 266 273 415 484`（按 `git log --format=%h -- crates/opi-ffi/src/jni.rs`
  逐个 `git show | wc -l`），**没有一个是 499，也没有一个是 466**：

  ```
  $ git log --format=%h -- crates/opi-ffi/src/jni.rs | while read c; do
      printf '%s %s\n' "$c" "$(git show "$c:crates/opi-ffi/src/jni.rs" | wc -l)"; done
  # … 415 484（末两条）—— 全序列无 499 / 无 466
  ```

- **处置**：`499` 与 `466` **两个数都不成立**（不是「起点错、终点对」）。
- **现状**：**已作废**（这一格不用再改）。⚠️ 但**别把 484 当今天的值抄** ——
  本轮工作区的 `jni.rs` 已经因为并发改动长到另一行数了。**提交态才是不动的**，
  要引就引 `git show <commit>:<path> | wc -l`，别引裸 `wc -l`。

#### 2. 「1259 行 CSV」

- **提交原文**（`628a5ad`）：「pin commit + sha256（1259 行 CSV，225089 字节），脚本 fail-closed」
- **实测**：**与脚本自己的 fail-closed 断言直接冲突**：

  ```
  $ grep -n 'EXPECT_ROWS' scripts/gen_en_dict.py
  102:EXPECT_ROWS = 10000
  163:    if len(rows) != EXPECT_ROWS:
  164:        print(f"FATAL: 上游 {len(rows)} 行，预期 {EXPECT_ROWS}", file=sys.stderr)
  $ wc -l < data/raw/en_words.tsv
  10000
  ```

  若上游真是 1259 行，这个脚本会**直接 FATAL 退出**、产不出入库的那份
  `data/raw/en_words.tsv`（10000 行）。
- **处置**：删掉 `1259`。要说行数就说 `EXPECT_ROWS` 断言的那个数，或者干脆说
  「行数由脚本的 `EXPECT_ROWS` fail-closed 断言守住」—— **别在散文里再抄一遍**。
- **现状**：**已作废**（提交正文改不了，本条只作存档）。补一句本轮核到的：
  `data/raw/en_words.tsv` 是**已入库且干净**的（`git status --porcelain` 对该路径无输出），
  行数与 `EXPECT_ROWS` 一致；它**只有生成器脚本引用**，没有任何运行期消费方（见总表 #13）。

#### 3. 「Unicode 18.0.0 共 1447 条、覆盖 1429」

- **提交原文**（`df576cf`）：「改按『emoji 区块内全部已分配码位』定范围（Unicode 18.0.0 共 1447 条，覆盖 1429）」
- **实测**：产物里 emoji 标记为 1 的行数，**在本文写作期间自己就变了** —— 这是这条最重要的发现：

  ```
  $ git show HEAD:data/raw/symbols.tsv | awk -F'\t' '$5==1{c++} END{print c+0}'
  1472                                  # HEAD：旧判据（emoji = 非 BMP 的代理指标）
  $ awk -F'\t' '$5==1{c++} END{print c+0}' data/raw/symbols.tsv
  1362                                  # 工作区（并发改动中）：新判据（UTS#51 Emoji 属性）
  ```

  变的**不只是数，还有定义**：`emoji 列` 的判据从「非 BMP」这个代理指标，改成了
  **UTS#51 `Emoji` 属性**（`emoji-data.txt`，同一份 UCD 版本目录；见 `scripts/gen_symbols.py`
  头注释）。⇒ 1447/1429 与 1472 与 1362 **不是同一个问题的三个答案**，
  比数字之前得先问「按哪个判据数」。
- **处置**：`1447` / `1429` 丢掉，**且不要换成一个新数字**。要真值就用生成器每次打印的那个数
  （`scripts/gen_symbols.py` 跑完会打印），或看门禁
  `crates/engine-core/tests/symbol_coverage.rs` 的 `UTS51_EMOJI` 快照 ——
  `data/raw/LICENSES.md` 的 symbols.tsv 行现在也正是这么写的（「条数与 emoji 条数以生成器
  每次打印的为准，别写死数字」）。
- **现状**：**已作废**（`1447`/`1429` 永久作废）。⚠️ 但要**连 tag 的 `1472` / `3332` 一起警惕** ——
  那两个数在 **tag 那个提交上确实成立**（`git show HEAD:data/raw/symbols.tsv` 上按旧判据数
  就是那两个值），**所以它们不是错的，是「有过保质期的对」**。工作区当天已经又变了
  （同一份文件、同一列，两个判据下都不同）。⇒ **无论 1472/3332 还是别的，处境一样：
  别把「某一刻数对了」当成「可以写进文档」**。判据在 `scripts/gen_symbols.py` 头注释
  与 `crates/engine-core/tests/symbol_coverage.rs` 的 `UTS51_EMOJI` 快照里，
  数字只有生成器自己打印。
- ⚠️ **一条方法论校准**：这三条**不能用裸 `grep <数字>` 判「仓库里有没有」**。
  我实测 `grep -rI 1447` 会命中 5 个文件 —— 全是**巧合子串**（`data/raw/en_words.tsv` 的
  词频 `1447748`、`Cargo.lock` 的 sha256 `…1447…`）。判据要落在**声明**上，
  不是落在数字字符上；「零命中」这个说法本身就得先定义清楚搜的是什么。

#### 4. 「C 33 / JNI 26」

- **tag 原文**：「ABI / 端侧：**C 33 / JNI 26** —— 新增中文标点开关出口（返回值非 void：
  语义不同，客户端记不住）」
- **实测**：**两个数都对不上本轮的任何提交态** —— 从 `0092109` 一直到 `3a39293`（= tag 那个提交），
  C 出口**一直是 34**、JNI 注册**一直是 27**。不是「期间涨上去的」，是**从加中文标点出口那天
  起就是**这两个数，tag 少写了一个。

  ```
  $ git show 3a39293:crates/opi-ffi/src/cabi.rs | grep -oE 'pub unsafe extern "C" fn opi_[a-z_0-9]+' | wc -l
  34
  $ git log --format=%h -1 3a39293   # tag v1.3.0 就钉在这个提交上
  3a39293
  # 本轮每个提交（0092109 … 3a39293）都是 C=34 / JNI=27，无一例外

  # JNI 侧按「注册表条目」数（不是按 Java_ 符号 —— 本仓走 RegisterNatives，
  # 所以 grep 'Java_' 恒为 0，那是**另一回事**，别当成「JNI 是空的」）：
  $ git show 3a39293:crates/opi-ffi/src/jni.rs | grep -cE '^\s+opijni_[a-z_]+ as \*mut c_void,'
  27
  ```

- **处置**：`33` 与 `26` **都不要**。要真值**别自己数、更别抄** —— 两个出口面各有一道门禁：
  - C 侧：`crates/opi-ffi/tests/c_abi/run.sh` 会 `nm -D` 打出导出条数，并断言**库里每一条
    导出都被 C 消费者真调用过**（未覆盖的会被列出来）。**以它的 [覆盖] 行为准。**
  - JNI 侧：`crates/opi-ffi/tests/jni_contract.rs` 断言「`#[no_mangle]` 的条数 == 注册表条目
    + `JNI_OnLoad`」1:1，并把宿主类声明面与注册表逐条对齐（改动只落一边就红）。
    **以它的 1:1 断言为准。**
  - ⚠️ 因此本条的正确说法不是「C 是 N 条」，而是「**C 与 JNI 的出口面各有门禁守着，
    数字去门禁打印里看**」。
- **现状**：**仍不成立**（tag 改不了）。但**加出口已经不会再让这个数悄悄漂** —— 两道门禁
  就是为这件事存在的。

#### 5. 「`scripts/gen_symbols.py` 560 行」（500 行门禁）

- **来源**：不是 tag、不是提交信息，是**工作区面的一次测量**（拆分之前量的，未提交）。
- **⚠️ 这一条我第一版写错过 —— 只查了一个面。** 我写的是「**查无实据**」，判据是
  「`git log --all` 里没有 560」。**那只证明「提交态没有」，不证明「560 没出现过」。**
  team-lead 在 02:47 前后 `wc -l scripts/gen_symbols.py` **实测到 560**
  （当时 `git status` 里它是 ` M`，未提交）⇒ **560 是真的，只是活在另一个面上。**

  **两个面分别是什么，这轮核清了：**

  | 面 | 值 | 判据 |
  |---|---|---|
  | **提交态** | **从未到过 500**（`447 → 450 → 450 → 496`） | `git log --format=%h -- scripts/gen_symbols.py` 逐个 `git show \| wc -l` |
  | **工作区态** | **出现过 560**（拆分前，未提交） | team-lead 02:47 前后 `wc -l`；`git` 里查不到是**必然**，不是「没发生」 |

  ```
  $ git show HEAD:scripts/gen_symbols.py | wc -l
  496                            # 提交面：从未到过 500
  $ git status --porcelain scripts/
   M scripts/gen_symbols.py      # 工作区面：560 就长在这一面上，git log 看不见
  ```

  - 附带的假阴性来源：`git grep -n '560'`（**只扫已跟踪文件**）当然扫不到一个
    **只存在于未提交工作区**的数字 —— 我当时拿它当「全仓零命中」用，**方法本身就够不着目标**。
    （它确实只命中 `fuzzy_ranking.rs` 的词频 `3_986_560_156` 这种巧合子串，
    和 §二.3 那条「别用裸 `grep <数字>` 判有没有」是同一个坑。）
- **⚠️ 而且工作区面自己也在动**：同一动作、不同时刻量到不同值 ——
  team-lead 量到 **359**，我随后量到 **361**（两个都真，两个都不是结论）。
  ⇒ **工作区面的任何数字都不许当结论引用**，这正是本文件反复说的那条：
  **引用给「文件 + 符号 + 面」，不给数字。**
- **结论（带面词）**：**提交态从未到过 500；工作区曾出现 560（未提交、其后被拆）。**
  两句都对，**说的时候必须带面词** —— 少一个面词，真话就变成假话。
- **可复核的是拆分结果**（**工作区面**，未提交；下面的数字**只是当次快照**）：

  ```
  $ wc -l scripts/gen_symbols.py scripts/symbol_keywords.py
   361 scripts/gen_symbols.py       # ⚠️ 别人量到过 359 —— 这一面会动，别抄
   225 scripts/symbol_keywords.py
  $ git status --porcelain scripts/
   M scripts/gen_symbols.py          # 改未提交
  ?? scripts/symbol_keywords.py      # 新文件、未跟踪
  ```
  - 拆的理由就写在 `scripts/gen_symbols.py` 头注释里：「在 `scripts/symbol_keywords.py` ——
    数据侧，不是脚本。拆出去的原因：本文件贴着上限，加注释就超。」
  - 人工表（`KANA_EXTRA` / `KANA_ROMAJI` / `MANUAL`）搬去了
    `scripts/symbol_keywords.py`，生成器 `from symbol_keywords import ...`。
  - ⚠️ 拆分**提交了吗**本身也是个面问题：`git cat-file -e HEAD:scripts/symbol_keywords.py`
    会报「在磁盘上，但不在 HEAD 中」⇒ **提交面尚未拆分**，只有工作区面拆了。
- **处置**：**不是「丢掉 560」**（那会删掉一件真事），而是**给它补面词** ——
  要说就写成「**工作区曾出现 560（未提交）**」。要说**合规**则看门禁：
  **以 `crates/opi-ffi/tests/line_limit.rs` 为准**（红线是文件里写死的常量 `LIMIT`，
  口径见其文件头：按文件系统遍历、末行无换行符也算一行、`*.md` 与 `data/raw/` 等排除）。
  ⚠️ 拆出来的 `symbol_keywords.py` 是**未跟踪**文件，而该门禁**刻意不用 `git ls-files`**
  （就为了看住这批未跟踪的拆分产物）—— 所以它**算得进去**。
- **现状**：**工作区面已修**（两份都在红线内、尚未提交）；**提交面未动** ——
  HEAD 上仍是 496 行那一版、`symbol_keywords.py` 不在 HEAD 里。**说「已修」要带面词。**

### 三、本轮**没有**验证的

- **Windows 开箱行为**：本机无 Windows，只做了静态取证（无打包步骤 + CI 只有 `cargo check`），
  **没有真的装一次看是不是 35 词**。
- **`225089 字节`**：那是上游 CSV 的字节数，需要真去下载才能核；本轮**没有下载**
  （`gen_en_dict.py` 是 fail-closed 的，真要核就跑它）。**此数未被本轮否定，也未被本轮证实。**
- **1472 的口径**：上一轮我只核到「`git show HEAD:data/raw/symbols.tsv` 里 emoji 标记 = 1
  的有 1472 行」。这个数**是否等于「Unicode 18.0.0 emoji 区块内全部已分配码位」**
  没有独立复核 —— 要核得重跑生成器并比对上游客体。
  （补记：判据在当天晚些时候被换成了 UTS#51 `Emoji` 属性，见 §二.3 ——
  **「按哪个判据数」比「数出几」更该问**。）

### 三之二、**第二轮**新增的未验证项

- **端侧可达率**（§一.5）：**Android / macOS 都没有真机跑过**。我只静态取到
  「端侧调用点自己传了 limit」这一层，**没有在机器上翻到末页之外去看它是否真的翻不过去**。
  结论「会被端侧 limit 截住」是从常量与调用点推的，**不是看出来的**。
- **`560`**（§二.5）：**已由 team-lead 证实**（他在 02:47 前后 `wc -l` 到过 560，未提交）
  —— 所以它**不再是未验证项**，而是一条**带面词的事实**：提交态从未到过 500，
  工作区曾出现 560。**我第一版写成「查无实据」，错在只查了提交面。**
- **工作区面的一切数字**（`gen_symbols.py` 的 361 vs 359、`jni.rs` 的行数、
  `symbols.tsv` 的 3323 …）：**本文件里凡是来自工作区面的数，都只当「当次快照」**，
  引用时要么带面词、要么干脆不给数。**未验证的是「这些数明天还是不是这样」，
  不是「它们当时是不是真的」。**
- **gen_symbols 的拆分是未提交的工作区状态**：`git status --porcelain scripts/` 是
  ` M gen_symbols.py` + `?? symbol_keywords.py`。**HEAD 上仍是拆之前的状态**
  （`df576cf` 那一版是 496 行），所以「已拆」这句话**只对工作区成立**。
- **`fetchLimit = 64` 是不是「刻意的」**：没有核。它可能是待改的遗漏，也可能是有意的
  性能上限（一次抓 64 条 vs `usize::MAX`）。**本轮只证明「它是 64」，没有证明「它该是别的」。**
- **macOS 那行注释**（§一.5 里那条）：我核的是**文本与 Rust 常量的不一致**，
  没有核 macOS 编不编得过、也没有核它运行期行为（本机无 Xcode）。
- **本轮没有跑任何测试**：文里引的 `cargo test` 输出是**上一轮**那次运行的记录；
  第二轮只做了静态取证（`grep` / `git show` / `wc -l`），**没有重跑步进**。
  凡引用处请自行重跑 —— 尤其 §一.4 那条「5 passed」。
- **§二.5 的「已修」只到「两份文件都在 500 以内」这一步**：我**没有真跑**
  `crates/opi-ffi/tests/line_limit.rs` 去让它判一次。那是**手算 + 静态核对**，
  不是门禁跑绿的记录。
