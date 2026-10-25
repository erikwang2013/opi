# 性能预算与实测

> 用户裁决（2026-10-11）：**全端**输入法——按键反馈 **10–50ms**、候选词 **50–150ms**、
> 字符上屏 **100–200ms**。本文件记录预算的**分层口径**、各端**实测证据**与复量方法；
> 改引擎热路径（candidates / scheme / JNI 出口）前后都要回来对表。
>
> **订正（2026-10-22）**：用户新裁决「所有端都是单机运行的，所有的响应要低于 30ms」
> —— 上表的 50ms 上限收紧为 **<30ms**（按键反馈与候选更新同口径）；导入层总账
> 50→30ms、初始扫描 40→20ms（`dict_budget.rs`）。次日实测（每键新建缓冲、只测第一次
> `candidates`）：全键盘 `n/a/xin` 引擎侧 p50 7.2–8.3ms（p90 ≤12.5ms）；拼音缩写/
> 成语/歇后语档 0.9–3.6ms；fcitx5 胶水端到端（含测试面板）p50 21–27ms。

## 2026-10-26 补充：首击、9 键分词与移动取回窗口

- **26 键单键首击（1.25.7）**：单键有限前缀在全量池（9216）上按档位直接选首位段，不再先截小池再补档；桌面 release 实测 **3.5–6ms**。契约是 `candidates(k) == candidates(MAX)[..k]`，见 `engine-core/src/candidates_finite.rs` 与 `prefix_contract` 测试。
- **9 键分词（1.25.6）**：分词查询从全表扫描改为区间枚举，桌面实测 **882ms → 1.1ms**；单键不再做无意义回退扫描。
- **移动端取回窗口（1.26.0）**：Android/iOS/鸿蒙平铺候选一次取回 **512** 条（原 64；`yi` 的「壹」第 79 位、9 键 `94` 第 155 位）。这是**可达性窗口**，不是延迟测量；9 键左列读数仍截 64，9/14 键按键类池仍为 2048，26 键全量池 9216。512 条的取回/渲染成本尚未在各端真机上单独测量，别把它读成已验的性能数字。

## 分层口径（每个数字量的是哪一段）

| 预算 | 量什么 | 谁负责 |
|---|---|---|
| 按键反馈 10–50ms | 触摸/按键 → 按键视觉响应（ripple/highlight）出现的帧 | 宿主 UI 层 + **引擎不得阻塞主线程**（引擎耗时是这段的下界之一） |
| 候选词 50–150ms | 按键 → 候选栏内容更新的帧 | 同上（引擎 candidates 耗时 + 重组重绘） |
| 字符上屏 100–200ms | 提交动作 → 目标输入框出现字符 | 我方 = 引擎 select + `commitText`/`dropComposing` 调用；余下是**目标应用**的绘制（不归我们） |

「引擎侧耗时」是三条预算的公共下界——本文件先钉住它（可测、可复量），宿主帧时间由
各端真机验收。

## 导入词库层（2026-10-17 用户裁决）

「只要有词库输入就要同步匹配……要保证响应小于 50ms；是词库就要抢位，如果响应时间
超过 50ms 就优先使用自带词库」——两条都落在 engine-core（`dict_layers.rs`）：

| 条 | 落点 | 判据（机器红/绿） |
|---|---|---|
| **抢位加成** | `boosted_import_freq`：导入层命中的条目（**广覆盖**，重复项取 `max(主库原频, 加成值)`）获得 `min(数据, 顶/128) + (顶 − 顶/128)`（顶 = 主库最大静态词频） | 严格低于顶、压过顶带以下的普通候选、恒不越过用户档（`tests/dict_layers.rs` 三条 + C-ABI 两条；变异自检＝加成改恒等 ⇒ 点名红） |
| **超时兜底（2026-10-18 重构，2026-10-22 收紧）** | `dict_budget`：**每键一份**预算（总账 30ms、初始导入上限 20ms），`Engine::candidates` 入口建立、所有查询家族共享；base 查询之后按「总账 − 20ms 后处理 − 已耗时」**动态收紧**；trie 可停止遍历，扫描逐条可抢占（每 64 条）；9/14 键 `scheme_entries` / `scheme_find` 同样受截止。超时 ⇒ 导入层**整体丢弃**、回纯内置、不粘滞 | 零预算 ⇒ 与纯内置逐条同分同序（`import_scan_over_budget_falls_back_to_builtin_only`、`nine_key_scheme_honours_import_budget`） |
| **命中上限（2026-10-18）** | 导入层 exact / rest 命中**各自**截到 2048（`IMPORT_HIT_CAP`）；单次扫描命中超过 5 万条（`IMPORT_SCAN_HIT_LIMIT`）直接整层丢弃 —— 病态大层在几毫秒内回退，不把预算耗在无底扫描上 | 超限与超时同语义（整层丢弃）；可见窗口 8/64，2048 / 5 万远大于它 |

粒度如实记：扫描已**逐条可抢占**（每 64 条查一次截止），单层不再「层内不可中断」；
base 查询 + 候选装配的耗时通过动态截止计入总账。端到端可达（含「常用字头不受影响」
「方言词字同表可达」）另有 C-ABI 门禁：`opi-ffi/tests/kangxi_reach.rs`、
`dialect_reach.rs`。

## 2026-10-18 全链性能审计与修复

四路只读审计（引擎 / Android / 桌面 / release 实测）的落地项与实测。**机器为 8 核
共享机，loadavg 12–19、I/O wait 明显；绝对 p95 对负载敏感，同进程 BASE/BIG 差值更可信。**

**引擎导入层**（`dict_budget.rs` 每键共享预算 + 动态收紧、可抢占扫描、exact/rest
2048 截断、5 万命中硬上限；门禁见 `tests/dict_layers.rs`）：

| 场景（release，100 轮或注明） | 修复前 p50/p95/max | 修复后 p50/p95/max |
|---|---|---|
| 26 键 · 10 万行导入 · `ni`（2.5 万命中，40 轮） | 44.31/57.04/60.23 | **23.84/28.22/31.29** |
| 26 键 · 10 万行导入 · `qiu`（1.25 万 exact） | 34.86/41.82/49.31 | **23.97/29.36/31.51** |
| 26 键 · 10 万行导入 · `nihao` | 13.96/16.53/19.51 | **5.74/8.18/10.59** |
| 9 键 · 10 万行导入 · `64`（2.5 万命中，100 轮） | 未测（旧实现无截止） | **34.15/43.72/49.00** |
| 9 键 · 10 万行导入 · `64426`（100 轮） | 未测 | **9.95/13.06/19.25** |
| 单层 40 万命中 · `a`（修复前 6 轮 / 修复后 20 轮） | **656/681，且不回退** | **36.02/46.08/46.08，回退纯内置** |

- zero-budget 兜底：26 键 `ni`/`nihao`/`qiu` 与 9 键 `64426` 的候选序列与纯内置
  **逐条同分同序**（断言 PASS）；不粘滞。
- 对照：BASE（无导入）同轮 `ni` 11.79/14.52/17.31；BIG−BASE p50 差约 +12ms（修复前
  +31ms）。高负载轮（load 35+ / I/O wait 40%）BASE 自身 p95 可到 35ms，BIG p95
  会重新越线 —— 那是基础查询的成本，不是导入层回归（同进程差值稳定）。
- **TSF（Windows）**：候选窗未启动时按键线程旧为 4×50ms=200ms 空等，现改为**单次
  尝试、不睡眠**（下一条 show 再试）；`push_state` 每键两次全量排序（`candidates` +
  `page_count`）改为一次 `fetched` 同时出页内候选与页数。**Windows 宿主未实测**
  （本机无 Windows；静态 + 控制流算术证据）。`WriteFile` 无超时仍是后续项。
- **Android（只审计，未改代码）**：每键约 12 次 JNI（其中 6 个标量读取可拆）、
  `candidateReadings` 全量 `segment`、`exportJson` 在主线程、启动词库 IO 在主线程；
  `PetStrip` 不会全屏重组，但每帧新建 Brush/Path，隐藏时很可能仍在跑。见下方剩余项。
- **证据边界**：引擎数为本机 release、真实 luna + 自造导入库；Android/Windows 宿主层
  未真机实测；导入库规模是合成极端（10 万/40 万行），真实用户导入通常更小。

**剩余优化 / 扩展（未落地，按收益/风险）**：
1. Android `refresh` 拆轻/重（去 6 个标量 JNI）；`candidateReadings` 惰性化；折叠态
   只取 8 条（平铺再取 64）。
2. `PetStrip` 隐藏暂停（`LocalLifecycleOwner` + RESUMED 门控）、`drawWithCache` 复用
   Brush/Path、降帧/减粒子（观感需真机确认）。
3. `UserWordStore.exportJson` 移后台执行器；`EngineLoader` / `DictStore` 启动 IO
   异步化 + loading 态（首次显示 ANR 风险）。
4. 单次 JNI 快照（buffer+candidates+readings+display+pending）；排序移出主线程
   （收益最大、复杂度最高，需压力测试）。
5. C ABI / `KeyRouter` 候选缓存：Harmony 每键 `pageCount` + `candidatesPage` 双排；
   TSF / fcitx5 已改为单抓。
6. TSF 异步写线程（彻底消除 `WriteFile` 无超时的按键线程阻塞）。
7. `learner.freq_of` 去重（主查询 / 9 键 scheme 各重复一次，≤18k/键）；`scheme_readings`
   惰性 / 按页；`candidates.rs` 的 `seen` 去 clone。
8. 空闲机复跑绝对 p95；把「基础查询 + 排序」也纳入总账（宽前缀 base 贵的场景）。

## 2026-10-18 繁体现代词 + 康熙并入后的复量

数据变化（`scripts/gen_trad_modern.py` / `gen_kangxi_extra.py`）：

| 产物 | 之前 | 之后 | 装载（verify） |
|---|---|---|---|
| `trad.opid` | 96,429 条 / 2.58MB | **139,355 条 / 3.7MB**（+现代词 42,304 + 康熙差集 622） | 19.5ms |
| `luna.opid`（部署副本） | 936,501 条 / 32MB | **937,780 条 / 32MB**（+康熙差集 1,279） | 139.4ms（一次性） |

**同期优化**：`candidates.rs` 的前缀扩展上限改为自适应 —— **没有学习词**时取
`min(PREFIX_MATCH_CAP, max(256, limit×8))`（最终只留 `limit` 条，池外低静态词不会被
boost 提上来）；**有学习词或 `limit == usize::MAX`** 时仍留满 9216，保持「学过的低频词
可反超」的既有语义。门禁：`learner.has_user_words()` 的分支 + 全量 `engine-core` 测试。

**本机 release 实测**（8 核共享机，loadavg ≈28；同进程、每个 key 200 轮、`candidates(64)`；
数字是引擎侧，不含 JNI/UI；p50/p95 为轮次分布，min 近似无争用成本）：

| 库 / 模式 | 输入 | min | p50 | p95 |
|---|---|---|---|---|
| trad / Traditional | `ni` | 0.98 | 1.20 | 1.62 |
| trad | `xin` | 2.34 | 2.70 | 3.83 |
| trad | `qiu` | 1.33 | 1.65 | 2.47 |
| trad | `jisuanji`（现代词） | 0.07 | 0.11 | 0.12 |
| trad | `liulanqi`（现代词） | 0.06 | 0.06 | 0.12 |
| trad | `rengongzhineng` | 0.10 | 0.12 | 0.14 |
| luna / Pinyin | `a`（最宽前缀） | 19.35 | 23.57 | **30.55** |
| luna | `ni` | 5.56 | 6.54 | 8.45 |
| luna | `xin` | 16.37 | 20.53 | **29.64** |
| luna | `nihao` | 0.11 | 0.12 | 0.14 |
| luna | `jisuanji` | 0.14 | 0.15 | 0.23 |

结论：最坏档（单字母 `a`、单音节 `xin`）p95 30.55ms < 50ms；现代词/康熙的**精确键**
查询在 0.1ms 量级。更高负载下墙钟会被调度放大（同一轮 300 次循环曾见 p95 60–70ms，
而单次分段计时仍在 2–10ms），报告数字时按本表的同进程分布读，别把调度延迟算成引擎成本。

> **订正（2026-10-22，30ms 目标）**：上表是 10-11 的 50ms 口径。同日把纠错档从
> 前缀查询（`query_all`，单键 15–57ms）改成精确键（`query_prefix_top(v,0)`），
> 并修掉分词回退的两处全量物化。本机 release 单进程、**每键重新组字后只取第一次**
> `candidates(MAX)`（loadavg ≈29）的墙钟 p50：`n` 8.0 / `a` 7.2 / `xin` 8.3 /
> `ni` 7.1ms（p90 8.0–12.5）；缩写/短语档 `yffs` 0.9 / `hstz` 3.6 / `zmkhjjg` 2.2 /
> `nh` 2.0ms；回退档 `jisuanjj` 2.9ms（修前 28ms）。fcitx5 胶水端到端（含探针的
> TestIC 面板刷新）p50：`n` 21.5 / `a` 24.2 / `xin` 27.3 / PageDown 20.9ms ——
> 仍 <30ms。端到端与引擎的差额（约 13ms）落在胶水层：候选 JSON 解码、候选表/选择键
> 构建与 `updateUserInterface`；**本轮没有把这 13ms 再拆细**（真守护进程的 UI 刷新
> 是合并异步的，探针里的 TestIC 不等同）。导入层总账同步收紧到 30ms（初始扫描
> 20ms），超时整层丢弃。

## 引擎侧实测（2026-10-11）

**桌面 release（本机，936k 条新 luna.opid，新旧交错取最小）**：

| 路径 | 修复前 | 现在 |
|---|---|---|
| 26 键单字母（`z`） | 3.5ms（96k 库） | **3.9–6.3ms**（936k 库，前缀层 top-9216） |
| 9 键单键（`6` / `94`） | **268ms / 555ms**（全表逐条扫） | **1.9ms / 4.5ms**（按键类区间枚举 + top-2048） |
| 14 键（`b` / 回退 `bugaoml`） | **358ms / 193ms** | **2.1ms / 0.9ms** |
| 词库装载（verify，堆路径） | 15.6ms（96k 库） | 96–104ms（936k 库；一次性、随文件线性） |

**Android 设备侧（本机 x86_64 模拟器，release Rust，逐键插桩 `SystemClock`，
20 轮按键取分布；模拟器 = 软渲染 + VM，**对 CPU 亦偏悲观**，真机应更快）**：

| 段 | 实测 | 说明 |
|---|---|---|
| 9 键 `input('6')`（JNI inputKey + candidates + readings） | **2.4–47ms，中位 ≈8ms** | 含 JNI 与 Kotlin 侧全链 |
| 26 键 `input('b')` | 23–83ms，中位 ≈35ms | 同链；26 键候选池比 9 键大 ⇒ 桌面同路径也慢约 2.7× |
| 退格 | 0.4–7ms | 缓冲空 ⇒ 候选零成本 |
| **提交（`dropComposing` + `commitText`）** | **17–130µs** | 「字符上屏」的我方段；余下是目标应用绘制 |
| 候选栏/键盘帧时间 | 模拟器不可用（swiftshader 下**英文直通模式**同条件也 113–133ms 中位 —— 环境成本，非引擎） | 帧预算以**真机**为准 |

**换算出真机预期**（模拟器 ÷3–8）：9 键 ≈2–6ms、26 键 ≈5–15ms —— 三条预算的引擎下界
均有余量。

## ⚠️ 两个已修的坑（复量前先读，别重踩）

1. **`assembleDebug` 的 Rust 是 debug 编译**（cargokit `builder.dart`：
   `if (!configuration.isDebug) '--release'`）——它比 release 慢 **3–20×**（实测同一
   按键 debug 46–239ms vs release 23–83ms）。**分发的 APK 必须是 `assembleRelease`**
   （本仓 release 用仓库内的社区签名密钥库，见 `android/keystore/README.md`）。
2. **9/14 键曾全表逐条 `matches` 扫 936k 行**（单键 268ms 起）——现为按键类**区间枚举**
   （`Dictionary::scheme_entries`，排序表上每个字母前缀 = 连续区间）+ `SCHEME_MATCH_CAP
   = 2048` 截断。等价性门禁 `crates/opi-tools/tests/scheme_equivalence.rs` 看着语义。

## 复量方法

- **引擎（桌面）**：`cargo test --release -p opi-tools --test <scratch probe>`（照
  `scheme_equivalence.rs` 的夹具写一次性探针；**新旧同进程交错取最小**，跨进程在并发
  机上不可信）。
- **设备（Android）**：临时插桩 `EngineController.input/backspace` 与
  `OpiImeService.commitWithRetry`（`SystemClock.elapsedRealtimeNanos` + `Log.i`），
  `adb shell input tap` 连续注入 + `logcat -s` 收割，**测完还原**；帧时间用
  `dumpsys gfxinfo`，但**必须跑英文直通基线**对照（见上表：模拟器帧数据无分辨力）。
- **desktop fcitx5**：本机 harness（`fcitx5-local-verification-harness` 记忆）里的
  `opi_panel_driver` 时序；面板绘制归 fcitx5 classicui（非本仓）。
- **TSF / Apple / 鸿蒙**：本机不可运行（无 Windows / 无 macOS / 鸿蒙无设备或模拟器；
  鸿蒙工程与 release 构建 2026-10-20 已通，见 `docs/install/harmony.md` §8）——
  引擎层与上述同源可推断，**宿主层未验证**，如实记。
