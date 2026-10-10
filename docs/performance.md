# 性能预算与实测

> 用户裁决（2026-10-11）：**全端**输入法——按键反馈 **10–50ms**、候选词 **50–150ms**、
> 字符上屏 **100–200ms**。本文件记录预算的**分层口径**、各端**实测证据**与复量方法；
> 改引擎热路径（candidates / scheme / JNI 出口）前后都要回来对表。

## 分层口径（每个数字量的是哪一段）

| 预算 | 量什么 | 谁负责 |
|---|---|---|
| 按键反馈 10–50ms | 触摸/按键 → 按键视觉响应（ripple/highlight）出现的帧 | 宿主 UI 层 + **引擎不得阻塞主线程**（引擎耗时是这段的下界之一） |
| 候选词 50–150ms | 按键 → 候选栏内容更新的帧 | 同上（引擎 candidates 耗时 + 重组重绘） |
| 字符上屏 100–200ms | 提交动作 → 目标输入框出现字符 | 我方 = 引擎 select + `commitText`/`dropComposing` 调用；余下是**目标应用**的绘制（不归我们） |

「引擎侧耗时」是三条预算的公共下界——本文件先钉住它（可测、可复量），宿主帧时间由
各端真机验收。

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
   （本仓 release 用 debug 签名，配置已在 `android/app/build.gradle.kts`）。
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
- **TSF / Apple / 鸿蒙**：本机不可运行（无 Windows / 无 macOS / 鸿蒙缺脚手架）——
  引擎层与上述同源可推断，**宿主层未验证**，如实记。
