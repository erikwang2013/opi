// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// libfcitx5_opi.so（crates/fcitx5-opi/src/lib.rs 的 #[no_mangle] 出口）的 C 面：
// 结构体布局 + 函数声明 + 取字符串的 take()。
//
// 从 opi_fcitx5.cpp 拆出来（那份文件贴着 500 行硬规矩；与 opi_json.h 同一次
// 拆分思路，见那里的注释）。**这里只是声明**，与 lib.rs 的 #[repr(C)] 手工对齐，
// 是 iOS/macOS 那两份 OpiFFI.h 之外的第三份 C 面 —— 改 lib.rs 的导出签名时三处
// 都要跟。
#ifndef OPI_RUST_FFI_H
#define OPI_RUST_FFI_H

#include <cstddef> // size_t
#include <cstdint>
#include <string>

// Rust 侧 OpString { ptr, len }（UTF-8，非 NUL 结尾；ptr==nullptr 视为空串）。
struct OpiString {
    const uint8_t *ptr;
    size_t len;
};

// Rust 侧 KeyEventResult { action, text }：action 0=直通 1=已处理 2=提交。
struct OpiKeyEventResult {
    int32_t action;
    OpiString text;
};

extern "C" {
bool opi_fcitx5_load(const uint8_t *ptr, size_t len);
// 简繁双装（2026-10-09）：两段都是**路径字符串**（同 opi_fcitx5_load 的约定）。
// 返回值只反映第一段（luna，坏 → false → 调用方回退内置）；第二段（trad）坏是
// 静默降级（Traditional 模式回退简体词典），不返回失败。
bool opi_fcitx5_load_with_trad(const uint8_t *ptr, size_t len,
                               const uint8_t *trad_ptr, size_t trad_len);
OpiString opi_fcitx5_input_key(const uint8_t *ptr, size_t len);
void opi_fcitx5_backspace();
void opi_fcitx5_clear();
OpiString opi_fcitx5_select(size_t index);
void opi_fcitx5_switch_mode(int32_t mode); // 0=Pinyin 1=English 2=Number 3=Symbol
void opi_fcitx5_set_learner(bool on);       // 学习开关（addon 配置 → 引擎，字段见 opi_config.h）
void opi_fcitx5_set_chinese_punct(bool on); // 中文标点开关（同上；与全角相互独立）
// 生效中文标点**读侧**（2026-10-15，与 set_chinese_punct 配对但**不是同一个量**）：
// true = 此刻按标点键出中文标点（中文类模式读设置值；英文/数字/符号恒 false）。
// 状态栏 `punct=` 那一格读它 —— 显示要跟着模式联动（用户裁决「切换中英文的时候，
// 标点符号也联动切换」；**行为本来就对**，缺的只是显示）。语义见
// engine-core/src/engine_punct.rs 的 effective_chinese_punct。
// **非**弱符号（与 toggle_fullwidth 同档）：出口与调用点同树同包落地，没有「声明早于
// 落地」的窗口；真错配时链接期就报缺符号，不留加载期静默。
bool opi_fcitx5_effective_chinese_punct();
void opi_fcitx5_set_fuzzy(bool on);   // 模糊音开关（addon 配置 → 引擎，见 opi_config.h）
// 模糊音逐对（2026-10-13）：组名串（UTF-8，逗号分隔，如 "zh/z,n/l"）。同样是弱符号。
extern "C" void opi_fcitx5_set_fuzzy_pairs(const uint8_t *ptr,
                                            size_t len) __attribute__((weak));
void opi_fcitx5_set_jianpin(bool on);
void opi_fcitx5_set_correction(bool on);  // 拼音纠错（2026-10-13，同上） // 简拼开关（同上）
// 双拼布局（2026-10-13）：枚举**下标**（0=自然码…5=拼音加加，序见 opi_config.h 的
// OpiShuangpinLayout = 引擎表序）。同样是弱符号。
extern "C" void opi_fcitx5_set_shuangpin_layout(int32_t index) __attribute__((weak));
// 拼音方式 / 输入方案（2026-10-14）：方案整数码（0=全拼 4=双拼，= `Scheme::code`，
// C++ 的 OpiPinyinMethod 枚举值就取这两个码）。同样是弱符号。
extern "C" void opi_fcitx5_set_scheme(int32_t code) __attribute__((weak));
// 五笔（2026-10-14，M4）：码表路径（UTF-8）+ 五个旋钮（整数码 = engine-core 的
// `WubiStyle/WubiMixedEntry/WubiReturnStyle::code`，C++ 枚举值就取那几个码；个数 1..=9
// 由引擎钳）。全部弱符号：老 .so 上静默跳过（一次诊断，见 opi_wubi.h）。
extern "C" bool opi_fcitx5_load_wubi(const uint8_t *ptr, size_t len)
    __attribute__((weak));
extern "C" void opi_fcitx5_set_wubi_style(int32_t style) __attribute__((weak));
extern "C" void opi_fcitx5_set_wubi_mixed_entry(int32_t entry) __attribute__((weak));
extern "C" void opi_fcitx5_set_wubi_candidate_limit(int32_t n) __attribute__((weak));
extern "C" void opi_fcitx5_set_wubi_return_style(int32_t style) __attribute__((weak));
extern "C" void opi_fcitx5_set_wubi_auto_commit(bool on) __attribute__((weak));
// 自定义短语（2026-10-24）：从 `<数据目录>/opi/custom_phrases.json` 与
// `custom_phrases_en.json` 重读中英两套整表并推给引擎（设置窗保存 →
// ReloadAddonConfig → applyOpiSettings 调到这里）。读文件与
// 校验都在 Rust 侧（坏文件归一为空表）。弱符号：老 .so 上静默跳过 —— 那份 .so
// 本来就没有短语能力，不能因此让插件整个加载不了。
extern "C" void opi_fcitx5_reload_custom_phrases() __attribute__((weak));
// 方案**读侧**（2026-10-14，与 set_scheme 配对）：当前方案码（`Scheme::code`）。
// opi-ctl 的 `mode` 动作派生态 token（pinyin/en26）靠它 —— 调用点见 opi_ctl_mode.h。
// 同样是弱符号（落地前按全拼 0 处理，读侧缺省与 `opi_fcitx5_mode()` 未装载给 0 同姿态）。
extern "C" int32_t opi_fcitx5_scheme() __attribute__((weak));
// 英文专业模式（2026-10-19）：读/切普通⇄专业；弱符号，老 .so 上静默跳过。
extern "C" bool opi_fcitx5_english_pro() __attribute__((weak));
extern "C" bool opi_fcitx5_toggle_english_pro() __attribute__((weak));
// 清空用户词（学习记忆）；弱符号，老 .so 上按「入口未接线」报 4。
extern "C" bool opi_fcitx5_clear_user_words() __attribute__((weak));
bool opi_fcitx5_fullwidth();               // 只读：当前是否全角（状态栏 State 读它）
bool opi_fcitx5_toggle_fullwidth();        // 返回切换后的状态（true = 全角）
void opi_fcitx5_set_shift(bool on);
OpiString opi_fcitx5_input_space();
OpiString opi_fcitx5_candidates(size_t limit); // JSON 数组（UTF-8）
OpiString opi_fcitx5_buffer();                 // 显示串：全键盘带 `'` 分词（预编辑读它）
// 原文（不分词）：W1 直接确认/切输入法提交进文档用。弱符号，老 .so 上没有 ——
// 那时 `opi_fcitx5_buffer` 本来就返回原文，退回它即可（见 opi_fcitx5.cpp 的 opiDirectConfirm）。
extern "C" OpiString opi_fcitx5_raw_buffer() __attribute__((weak));
int32_t opi_fcitx5_mode();
OpiKeyEventResult opi_fcitx5_key_event(uint32_t keyval, uint32_t states);
void opi_ffi_free_string_utf8(OpiString s);
}

// ---- 符号浏览（22 类目）的进出入口 + 类目数据装载 ----
//
// 一个入口、三个动作：0 = 退出浏览（幂等：非浏览态调用是 no-op）、1 = 进入、
// 2 = 切换（引擎看自己的状态决定进/出；C++ 侧看不到「是否在浏览态」，而热键要
// 来回切 —— 「只能进不能出等于换个地方卡住」是本仓库热键的既有原则）。
//
// 语义契约（engine-track 2026-10-10 定稿，C++ 侧全靠它）：
//   · 退出**必须恢复进入浏览前的真缓冲**（用户进浏览前打了一半的拼音）；
//   · **同步**：调用返回时 `opi_fcitx5_buffer()` 已经是真缓冲。切走/切英文的顺序
//     是「先退浏览 → 再按 W1 提交（拼音/繁体缓冲原文、符号模式不上关键字 ——
//     2026-10-10 深夜裁决 + 同夜模式分叉，opiDirectConfirm 的注释里有时间线）」，
//     异步实现会把浏览态的标题串/首个分类
//     当用户输入提交出去 —— 正是这条契约要防的东西。
//   · **返回值 = 执行后的浏览态**（1=在浏览 / 0=不在）。无数据时 1/2 是静默
//     no-op（引擎侧「无数据 = 不动」，入口禁用），故 0 分不清「刚退出来」与
//     「进不去」；需要区分的调用方（opi-ctl 的 symbols，见 opi_engine.h）另按
//     自己知道的装载态判。
//
// ⚠️ **弱符号**声明（`__attribute__((weak))`，GCC/Clang 都支持）：入口落地前，
// 这里的引用在链接/加载期解析为 nullptr —— **弱引用不会让链接或 dlopen 失败**，
// 插件照常加载；入口落地后**同一个二进制**自动接上真入口（动态符号解析发生在
// 加载期），不需要回头改本文件。调用点必须判空（opi_mode.h 的 opiSymbolBrowseCall
// 是唯一调用点，缺符号时打一行 stderr 诊断，不静默）。
// 名字是**跨 agent 契约**（已发给 engine-track）：改名要两侧同时动，且本仓
// 无 CI 能看见它 —— 唯一看得见的是那句一次性诊断。
extern "C" int32_t opi_fcitx5_symbol_browse(int32_t action) __attribute__((weak));

// 装载符号类目数据（engine-track 2026-10-10 定稿）：**与两份词库同一约定** ——
// C++ 只 locate 路径，Rust 读文件。返回值：-1 = 文件缺失/读不出；-2 = 读到但零
// 合法类目（两者都让入口保持禁用；Rust 侧自己打一行 stderr，不静默）；
// ≥0 = 跳过的坏行数（0 = 全好；Rust 同时打「N 类目，跳 M 行」一行）。
// 调用方判据就一条：`>= 0` 即数据可用（opi_engine.h 的 symbolDataOk_）。
// 同样是弱符号（引擎侧落地前照常编译/加载）；路径为空时调用方**直接跳过调用**。
extern "C" int32_t opi_fcitx5_load_symbols(const uint8_t *ptr,
                                           size_t len) __attribute__((weak));

// 关键字搜索（2026-10-13，桌面符号/表情页的搜索框）：关键字（**不是路径**，
// 也不是命令）→ 命中条目的 **JSON 文本数组**（`["…"]`，serde_json 输出）。
//   · 空关键字 = 全部条目（引擎既有语义，同 Android searchSymbols("")）；
//   · 无命中 = `[]`；引擎未装载 = `[]` —— 这一层**没有错误码**：分不清「没搜到」
//     与「引擎没装载」，也不该在这一层分（调用方不需要区分它们）。
//   · 「空关键字 = 用法错误」的裁决在**调用方**（opi_engine.h 的
//     handleCtlSearch_ 先拒，opi-ctl 本地再拒一次）—— 那一层才是协议边界。
//   · 与类目数据装载（opi_fcitx5_load_symbols）**无关**：搜索表是编译内嵌的
//     （SymbolEngine::builtin），没装 symbol_categories.tsv 也能搜 —— 调用方
//     别拿 symbolDataOk_ 卡这条出口。
// 同样是弱符号（引擎侧落地前照常编译/加载）；调用点必须判空（opi_engine.h）。
extern "C" OpiString opi_fcitx5_search_symbols(const uint8_t *ptr,
                                               size_t len) __attribute__((weak));

// ---- 导入词库（2026-10-14，M3）----
//
// ⚠️ **本轨没有「列出词库」的出口**：契约里 fcitx5 宿主自己扫
// `~/.local/share/opi/dicts/*.tsv`（`opi_dicts.h` 的 OpiDicts），名字与条数的来源
// 是那次扫描 + 下面 import 的返回值。别去别处找 `opi_fcitx5_dict_list`——不存在。
//
// 逐条语义与三条裁决（数据留 Rust 内存 / 同名重导入 = 原位覆盖 / 系统词典不归本层）
// 与 opi-ffi 的 `api/dict.rs` 同源，全文见 `fcitx5-opi/src/dict_layer.rs` 头注。
// 两条都是**弱符号**（引擎侧落地前照常编译/加载），调用点必须判空 ——
// 调用点在 `opi_dicts.h`，缺符号时打一行 stderr 诊断并跳过整段（不静默）。

// importDict(path) -> 条目数。路径是**路径**（同 opi_fcitx5_load 的约定，UTF-8 裸
// 字节，不是文件内容）。失败（路径空/读不到/格式全坏/没有有效条目/引擎未装载）
// → **-1**，既有导入层不动；跳过坏行时 Rust 侧打一行 stderr 如实报数
// （「坏词库静默失效」是本仓出过的事故，这条出口不许静默）。
extern "C" int32_t opi_fcitx5_import_dict(const uint8_t *ptr,
                                          size_t len) __attribute__((weak));

// setDictEnabled(name, on) -> 是否执行。`true` = 这条导入层的启用位已经是 `on`
// （含本来就在目标态）；`false` = 名字认不得（含系统词典 luna/trad/stroke——
// 它们不归本层管，两方向一律 false）、或引擎未装载。
// 名字 = **文件名词干**（`foo.tsv` → `foo`，与 DisabledDicts 名单同口径）。
extern "C" bool opi_fcitx5_set_dict_enabled(const uint8_t *ptr, size_t len,
                                            bool on) __attribute__((weak));

// 取走并释放 Rust 侧字符串。ptr==nullptr 视为空串（Rust 侧 OpString::empty 的
// 表示）；仅守卫构造，free 契约不变：每个返回的 OpString 恰好 free 一次。
//
// `static` 而非 `inline`：本头只有 opi_fcitx5.cpp 一个 TU 包含（opi_json.h 是
// 两个），出处唯一，不需要 inline 的合并语义。
static std::string take(OpiString s) {
    std::string out;
    if (s.ptr != nullptr) {
        out.assign(reinterpret_cast<const char *>(s.ptr), s.len);
    }
    opi_ffi_free_string_utf8(s);
    return out;
}

#endif // OPI_RUST_FFI_H
