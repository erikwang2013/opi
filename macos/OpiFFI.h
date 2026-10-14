// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

// ⚠️ 草案 · 从未编译 · 未验证
//
// 本文件在 Linux 上编写。**Swift 侧一行都没编译过**；本头文件自身过了
// `clang -fsyntax-only`（C 与 C++ 两种模式），但**从未被链接、从未进入任何真实构建**。
// 声明是**逐条对照** crates/opi-ffi/src/cabi.rs 抄的（**条数以
// crates/opi-ffi/tests/c_abi_contract.rs 的那道门禁为准，这里不写数字** ——
// 写死的数会随下次扩容再假一遍，与 ios/OpiFFI.h 同一条规矩），
// 但「抄对了」与「能链接上」是两件事 —— 见 macos/README.md。
//
// OPI C ABI 声明面（macOS 端唯一一份；条数以 c_abi_contract 门禁为准）。
// ⚠️ ios/OpiFFI.h 是本文件的**转发头** —— 改这里两边都变，别在那边另抄一份。
// Swift 侧经 bridging header（Xcode）或 module map（SwiftPM）看到这些符号：
//   Xcode  : SWIFT_OBJC_BRIDGING_HEADER = macos/OpiFFI.h
//   SwiftPM: 与 module.modulemap 一起放进一个 C target
// 声明与 crates/opi-ffi/src/cabi.rs 一一对应；改动请同时改两边，不要在这里
// 另起一份「顺手加个包装」的实现（fcitx5 轨的教训：同一语义抄三份必漂移）。
//
// 声明的完整性已于本机核对：`cargo rustc --crate-type staticlib` 产出的归档里能数出
// `_opi_*` 符号，与本文件的声明**逐个 diff 一致**（本机 GNU nm/objdump 不认 Mach-O，
// 用 `grep -a -o` 读归档的符号名，方法见 README「已验证」）。
//
// 核对记录（**平台写在数字旁边，别把两者拆开读**）：首次核对用的是
// aarch64-apple-darwin 归档（当时 28）；2026-09-27 扩容到 31 后**重建同一个 target
// 复跑**：31 个符号，与 cabi.rs 的导出名 diff 为空（`grep -a -o` 与 `strings`
// 两种数法都是 31）。**另建**的 aarch64-apple-ios 归档同样是 31 —— 两条独立路径同一结论。
// ⚠️ 这一段是 **2026-09-27 的当时值**，其后 cabi.rs 又扩容过 —— 别把它当当前条数读；
// 当前条数以 c_abi_contract 门禁为准（本文件头已写明「这里不写数字」）。
// ⚠️ 顺带一条会让人挑错平台的坑：`rustup target list --installed` **不列出**
// aarch64-apple-darwin（`rustup target add` 会以 `detected conflict: libaddr2line-*.rlib`
// 失败），**但 `cargo rustc --target aarch64-apple-darwin` 照样能产出归档** ——
// 我就为此先绕到了 iOS 平台。**别拿 `rustup target list` 的空输出当成「目标没装」**，
// 详见 macos/README.md「复验」一节。
//
// 字符串约定：UTF-16（ptr + len，非 NUL 结尾），由 Rust 侧分配，
// 调用方**必须**用 opi_ffi_free_string 释放恰好一次。

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/// 对应 Rust `cabi::OpiString`（#[repr(C)] { *const u16, usize }）。
/// ptr == NULL 视为空串（Rust 侧 empty 哨兵），不是错误。
typedef struct {
    const uint16_t *ptr;
    size_t len;
} OpiString;

/// 对应 `cabi::OpiKeyEventResult`（本轮新增，接口已冻结）。
/// action: 0 = 未处理（键交应用）1 = 已处理（消费）2 = 提交 text。
typedef struct {
    int32_t action;
    OpiString text;
} OpiKeyEventResult;

/// 释放 opi_* 返回的 OpiString。每个句柄恰好释放一次，重复释放是 UB。
void opi_ffi_free_string(OpiString s);

/// 装载词库。空串/NULL → 内置回退词库；坏路径 → false（单例保持为空！）。
/// **返回值必须接**：false 时所有 opi_* 出口都会退化成空操作。
bool opi_load(const uint16_t *path, size_t len);

/// 繁体词库。空/坏路径/引擎未装载 → false（繁体模式回退简体库）。
bool opi_load_trad(const uint16_t *path, size_t len);

/// 笔画词典（stroke.opid，2026-10-09）。空/坏路径/引擎未装载 → false。
bool opi_load_stroke(const uint16_t *path, size_t len);

/// 输入方案（2026-10-09）：0=全键盘 1=拼音9键 2=拼音14键 3=笔画 4=双拼，越界忽略。
/// **换方案清缓冲** —— 调用后按 `opi_switch_mode` 那档重读 buffer/候选/页码。
void opi_set_scheme(int32_t scheme);
/// 方案读侧（未装载 → 0 是哨兵，不是"用户选的就是全键盘"）。
int32_t opi_scheme(void);

/// 双拼布局（2026-10-13）：0=自然码 1=微软 2=小鹤 3=智能ABC 4=紫光 5=拼音加加；
/// 负数钳到 0、过大钳到末位。**不清缓冲**（区别于 setScheme）—— 下次取候选即按新布局解码。
void opi_set_shuangpin_layout(int32_t index);
/// 布局读侧（未装载 → 0 是哨兵，不是"用户选的就是自然码"）。
int32_t opi_shuangpin_layout(void);

/// 单字符入引擎，返回需要提交的文本（多数时候是空串）。
OpiString opi_input_key(const uint16_t *ch, size_t len);
void opi_backspace(void);
void opi_clear(void);

/// 按 index 选词（引擎级全局索引，从 0 起 —— 与 opi_candidates 的数组同序）。
OpiString opi_select(size_t index);

/// 提交**当前页**第 k 个候选（**页内**索引，0 起）。越界/无候选 → 空串（不改状态）；
/// 引擎未装载 → 空串。
///
/// 点击候选请用本出口，**不要**自己算 `opi_page() * 8 + k` 再调 `opi_select()` ——
/// 那个 8 是 PAGE_SIZE 的第三份拷贝，引擎改一次 UI 便**静默选错候选**。
/// 本出口与按数字键（`opi_key_event` 的数字选词）、回车提交**同源**，
/// 三者都走 `KeyRouter::select` 那一份页内换算。
/// ⚠️ 本目录**尚未调用**它（`candidateSelected` 现在走 `opi_select` 的全局索引，
/// 见 README G3）。
OpiString opi_select_page(uint32_t index);

/// 0=Pinyin 1=English 2=Number 3=Symbol 4=Traditional（`api::mode_from_int`）。
/// ⚠️ 越界值是**静默不动作**：没有返回值，调用方无从知道被忽略。
void opi_switch_mode(int32_t mode);
void opi_set_shift(bool on);

// ---------- 标点 / 全角 / 符号开关（B3 / B5 的语义出口；键位仍不在引擎层） ----------
//
// **四条硬约定**，全部经消费者对着 crates/engine-core/src/{engine,composer,router}.rs
// 逐行核过。⚠️ **四条全部只讲「全角」那一档**；中文标点表是**另一档独立开关**
// （`opi_chinese_punct` / `opi_set_chinese_punct`，见其声明下的对照说明）。
//
// 1. `opi_switch_mode` 与 `opi_toggle_symbol` 是**仅有的两个**「调用后必须重读
//    `opi_fullwidth_state()`」的出口。后者最容易漏：`opi_toggle_symbol` 内部调
//    `switch_mode(target)`，而 `switch_mode` **无条件**把 fullwidth 重置为该模式的
//    默认值 —— 按符号键时全角指示灯会**悄悄灭掉**（Symbol 默认半角）。
// 2. 调完 `opi_toggle_symbol` 要重读的不止 fullwidth：`opi_mode` / `opi_buffer` /
//    `opi_candidates*` / `opi_fullwidth_state` —— **四个全要**。
// 3. `opi_toggle_symbol` 返回的是**切模式前那截缓冲**的待提交文本（见其声明下的说明）。
// 4. 全角**随模式默认、跨模式不粘**：Pinyin | Traditional → 全角；
//    English | Number | Symbol → 半角；**任何模式切换都重置**（含 `opi_toggle_symbol`
//    内部那次）。硬理由：全局 sticky 会把拼音的全角带进英文模式，把 `,` 变成 `，`，
//    违背「英文/数字半角直通」。
//
// ⚠️ **客户端不许自己推导映射**：撇号在缓冲非空时是音节分隔符（`engine.rs` 的
// `'` 分支），所以**全角映射不是 `(mode, fullwidth)` 的纯函数**。
// `opi_fullwidth_state()` 是给**状态栏显示**用的，不是给客户端预测按键结果用的 ——
// 键一律整颗交给引擎（`opi_key_event` / `opi_input_key`），客户端别自己算
// 「这个键在全角下该出什么」。

/// 全角 ↔ 半角，返回**切换后的新状态**（状态栏直接拿去刷新，不必再查一次）。
/// 全角开 ⇒ ASCII 可打印（字母/数字/标点）平移 +0xFEE0（用户裁决 2026-10-12；
/// 空格不宽化）；中文模式另有中文标点表那一档（`. → 。` U+3002 与 `．` U+FF0E 是
/// 两个开关，别混）。⚠️ 英文/数字模式下字母数字**直传、不进引擎** ⇒ 宿主须在直传点
/// 调 `opi_*` 侧的宽化语义（目前**没有** text 宽化出口，见 `cabi.rs` 同名出口注释）。
/// 引擎未装载 → false —— 与「已装载且是半角」共用同一个 false，**不是**在报错：
/// 那时按键全部交系统，宿主拿到的就是半角字符，返回值与可观测行为一致。
bool opi_toggle_fullwidth(void);

/// 全角开关的**读侧**（与 `opi_shift_state` / `opi_mode` 同构）。
/// 装载后、以及上面第 1 条点名的两个出口之后，**必须重读**。
/// 引擎未装载 → false（同上，不是错误）。
bool opi_fullwidth_state(void);

/// 中文标点表开关的**读侧**（`.。`、引号交替那一档）。引擎未装载 → false。
///
/// ⚠️ **与 `opi_fullwidth_state` 不是一档，重读规则也不通用**：
/// 本档是**用户偏好**，`opi_switch_mode` / `opi_toggle_symbol` **都不重置它** ——
/// 上面的硬约定 1/4 **只对全角成立，别照抄到本出口**（以为「切模式会重置它」
/// 是错的前提；真重置了才是 bug）。仍然必须有读侧：设置页与下次启动的持久化
/// 都会改动它，客户端自记一份必然漂。
/// 关掉本档**不等于半角**：表里没有的字符仍由全角那一档管（两闸各管一段，
/// 真值表见 `engine_core::Engine::punct_text` 与 `crates/engine-core/tests/punctuation.rs`）。
bool opi_chinese_punct(void);

/// 设置中文标点表是否生效。**无返回值**：它是设置项（勾选框知道要设成什么值）；
/// 要拿结果请读 `opi_chinese_punct()`（引擎未装载时设置是空操作，读侧会如实返回 false）。
/// **写入口有两个**：设置项走本出口，触发键走 `opi_toggle_chinese_punct()`。
/// 只翻一个 bool：不改 buffer / 候选集 / 页码，调用后无需重读任何东西。
void opi_set_chinese_punct(bool on);

/// 中文标点表开关的**触发键入口**：翻转并返回**切换后的新状态**（状态栏 / 勾选框直接
/// 拿去刷新，不必再查一次）。与 `opi_set_chinese_punct` 是同一档的两个写入口 ——
/// 键位不在引擎层：客户端自己截获键，再调本出口（同 `opi_toggle_fullwidth`）。
///
/// ⚠️ 与 `opi_toggle_fullwidth` 的**重读规则不通用**：本档是用户偏好，
/// `opi_switch_mode` / `opi_toggle_symbol` **都不重置它**（见上面 `opi_chinese_punct`
/// 那一段）。只翻一个 bool：不改 buffer / 候选集 / 页码。
/// 引擎未装载 → false —— 与「已装载且表关」共用同一个 false，**不是**在报错。
bool opi_toggle_chinese_punct(void);

/// 符号面板开关：返回**需要上屏的文本（空串 = 无提交）**。
///
/// 为什么不是 void：切进符号面板前要先收尾未提交的拼音缓冲（有候选 → 首候选；
/// 缓冲是 `zzz` 这类无候选的乱码 → 直接清掉、**不上屏**），这段文本必须交出来，
/// 否则就是「只记了学习记录却不上屏」。拿不到插入通道的端**不要调它**。
/// 空串句柄可无条件 `opi_ffi_free_string`。引擎未装载 → 空句柄。
///
/// ⚠️ **返回的不是「刚切出来的那个符号」** —— 是**切换之前那截缓冲**的待提交文本。
/// 名字里的 Symbol 指的是这次切换的**目标**，不是返回值的内容。
OpiString opi_toggle_symbol(void);

OpiString opi_input_space(void);

/// JSON 文本数组，最多 limit 条，从全局第 0 条起（**不分页**）。
/// 显示当前页请用下面的 `opi_candidates_page()`。
OpiString opi_candidates(size_t limit);

/// JSON 文本数组：**当前页**的候选（已按引擎的 PAGE_SIZE 分页）。
/// 引擎未装载 → `[]`。
///
/// **显示当前页必须用本出口，不要拿 `opi_candidates()` 自己按 8 切** ——
/// 页大小是引擎侧常量（路由与候选栏共用），UI 再抄一份，改一次就会静默错位
/// （高亮的页 ≠ 实际选词所在的页）。本出口与 `opi_page()` / `opi_page_count()`
/// 同源，三者永远一致。
OpiString opi_candidates_page(void);

/// 当前拼音缓冲（= preedit 的来源）。
OpiString opi_buffer(void);

/// 当前模式（**0..=4**，4=繁体）。⚠️ 取值必须**全部**列进 Swift 枚举：
/// 漏一个 case 会让 `OpiMode(rawValue:)` 判 nil、落到 `?? .pinyin` 兜底，
/// 表现成「UI 显示拼音、引擎在跑繁体」（显示与行为不一致，比不认识更难查）。
int32_t opi_mode(void);

/// JSON 文本数组。
OpiString opi_search_symbols(const uint16_t *keyword, size_t len);

/// JSON：`[{id,start,end,name,common}]`。
OpiString opi_symbol_blocks(void);

/// JSON 文本数组。负 id 按越界处理（空数组）。
OpiString opi_symbols_in_block(int16_t id);

/// 表情面板**浏览全量**（JSON 文本数组：官方分组序，含旗帜/家庭等多**码位序列**条目）。
/// ⚠️ **不经引擎**：编译期静态表 —— `opi_load` 失败（未装载）时上面几条都退化成
/// 空串 / `[]`，本出口**照样返回全量**（数据不依赖词库，冷启动第一帧就能画）。
/// `opi_emoji_groups()` 的 `start`/`count` 就是本数组的下标。
OpiString opi_emoji_symbols(void);

/// 表情**分类条**（JSON：`[{name,start,count}]`）。`start` 与 `opi_emoji_symbols()`
/// 下标对齐：点分类 = 网格滚到 `start`。同上，未装载也有值。
OpiString opi_emoji_groups(void);

bool opi_fuzzy_enabled(void);
bool opi_jianpin_enabled(void);
void opi_set_fuzzy(bool on);
/// 模糊音逐对启用集（2026-10-13）：位序 zh/z=0 ch/c=1 sh/s=2 an/ang=3 en/eng=4 in/ing=5 n/l=6 f/h=7，
/// 0xFF = 全开（= opi_set_fuzzy(true)）；入参越界位钳掉（只留低 8 位）。未装载 → 读侧 0。
int32_t opi_fuzzy_pairs(void);
void opi_set_fuzzy_pairs(int32_t mask);
/// 组名规范串（逗号分隔；设置界面把它当唯一名源）与按名设置（认不得的名字忽略）。
OpiString opi_fuzzy_pair_names(void);
void opi_set_fuzzy_pairs_names(const uint16_t *names, size_t len);
void opi_set_jianpin(bool on);
/// 拼音纠错（2026-10-13）：缺省开；未装载读侧为 false（哨兵）。
bool opi_correction_enabled(void);
void opi_set_correction(bool on);
bool opi_learner_enabled(void);
void opi_set_learner(bool enabled);
void opi_clear_user_words(void);
void opi_remove_user_word(const uint16_t *text, size_t len);

/// 返回导入条数；失败返回 -1（非法 JSON / 版本不符 / 词表过大 / 未装载 / NULL）。
int32_t opi_import_user_words(const uint16_t *json, size_t len);

/// 导出用户词（JSON）。
OpiString opi_export_user_words(void);

// ---------- 导入词库（2026-10-14，M3）----------
//
// 宿主契约四端同一份（实现与完整裁决见 crates/opi-ffi/src/api/dict.rs 头注）：
// · **名字 = 文件名词干**（`foo.tsv` → `foo`）——与桌面设置页（desktop/…/Dicts.kt）
//   和 `DisabledDicts` 停用名单里写的那个名字同一口径；
// · **数据留在 FFI 层内存里**：停用只把它从引擎的额外层里拿掉，**再启用不依赖原
//   文件还在**（宿主删了/改了文件也照样能再启用）；
// · **系统词典（luna/trad/stroke）不接受 set_dict_enabled** —— 它们不是「导入层」，
//   宿主 UI 按 dictList 的 `system: true` 把开关隐藏；
// · 路径由宿主给**绝对路径**；本层不碰磁盘（**删文件是宿主的事**）。

/// 读一个 UTF-8 的 tsv 词库文件（三列 `词<TAB>拼音<TAB>频次`，**词在前** —— 列序
/// 真源是 `engine-data/src/dict_text.rs` 的 `cols[0]=word`/`cols[1]=pinyin`）当**一个导入层**，
/// 返回条目数。失败（读不到 / 格式全坏 / 没有有效条目 / 引擎未装载 / NULL）→ -1，
/// 且不改动既有导入层。**同名重导入 = 覆盖**并置为启用。
int32_t opi_import_dict(const uint16_t *path, size_t len);

/// 词库列表（JSON 数组）：**系统词典在前**（luna/trad/stroke，`system: true`），
/// 导入层随后（导入顺序）；每项 `{name, count, enabled, system}`。
/// **未装载 → `[]`**（数组哨兵，不是空句柄 —— 宿主 parse 就得到空列表）。
/// ⚠️ JSON 键序是 serde_json 的字典序（count/enabled/name/system）：按**键**取。
OpiString opi_dict_list(void);

/// 装载 / 卸载一条**导入**词库（原子：一次全量替换，引擎看不到半个集合）。
/// 已是目标态 → true（幂等）；名字认不得（含系统词典）/ NULL / 未装载 → false。
bool opi_set_dict_enabled(const uint16_t *name, size_t len, bool on);

/// 把一条**导入**词库从内存里删掉（引擎的额外层跟着重算）；**文件是宿主的事**。
/// 名字认不得 / 已经不在 / NULL / 未装载 → false（删掉的层要重新 importDict）。
bool opi_remove_dict(const uint16_t *name, size_t len);

/// 键事件路由（本轮新增，接口已冻结）。
///
/// keyval：可打印字符 = Unicode 码点（空格 = 0x20）；
///         特殊键 = 0x1_0000 | 低 16 位码；
///         不可打印且非特殊键 → u32::MAX 哨兵（NO_KEYVAL）。
/// states：位标志（SHIFT/CTRL/ALT/META/RELEASED/REPEAT/LONG_PRESSED）。
///
/// 两套取值的真源是 **crates/engine-core/src/keys.rs**（`KEY_*` 与 `KEY_STATE_*`
/// 常量表）；路由的行为与状态（`KeyRouter`/`KeyAction`/页码）在
/// crates/engine-core/src/router.rs。**不是** crates/tsf-opi/src/logic.rs ——
/// 后者的 ⇧/翻页/Delete/空格四个键编码与这边不同。
OpiKeyEventResult opi_key_event(uint32_t keyval, uint32_t states);

/// 候选总页数（UI 的「共 N 页」）。**无候选 → 0**，引擎未装载 → 0。
/// ⚠️ 本目录**尚未调用**它（同 G3，等 Mac 上编译通过再接）。
uint32_t opi_page_count(void);

/// 当前候选页（0 起，末页由路由钳制）。引擎未装载 → 0。
/// Rust 侧注释的原话：**候选栏的页码必须读这里、不要自己数** ——
/// PageDown 到末页时路由会把页码钳到最后一页，本地计数超过末页就会与引擎漂移
/// （高亮的页 ≠ 实际选词所在的页）。
/// ⚠️ 本目录**尚未调用**它（`InputController.swift` 现在仍把全局列表整份塞给
/// 候选窗；改成按页显示是 G3，等 Mac 上编译通过再接）。
uint32_t opi_page(void);

/// 前端 ⇧ 三态：0=OFF 1=SINGLE（下个字母大写后自动复位）2=LOCK（持续大写）。
/// 引擎未装载 → 0。
/// 与 `opi_set_shift` 不是一回事：那个打的是**引擎侧** shift 位，三态是前端状态，
/// 英文直传路径的大小写由它决定，引擎位看不出来 —— ⇧ 的高亮（尤其 Lock）只能读这里。
/// ⚠️ 本目录**尚未调用**它（macOS 端目前没有 ⇧ 高亮 UI）。
int32_t opi_shift_state(void);

#ifdef __cplusplus
}
#endif
