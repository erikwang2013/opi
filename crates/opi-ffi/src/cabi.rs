// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! C ABI 出口（iOS M7 预留）：`opi_*` 函数，UTF-16 字符串（ptr+len），
//! Rust 侧分配，调用方用 `opi_ffi_free_string` 释放。语义与 JNI 出口（jni.rs）
//! 完全一致，共享 api::SINGLETON 与内部实现，无重复逻辑。
//! 多字符串返回值（candidates/searchSymbols/symbolsInBlock）编码为 JSON 文本数组。
//!
//! # Safety（本模块**无外部内存参数**那一档导出的统一契约）
//! 共享单例由内部 `Mutex` 保护，跨线程调用安全；除 `opi_ffi_free_string` 外每个函数
//! 都以 `catch_unwind` 包裹，panic 不跨 FFI 边界。这一档的 `# Safety` 段落**统一在此处**，
//! 函数上不再逐条重复（照 `jni.rs` 的做法，原先 26 条逐字相同的句子占了 52 行）。
//! ⚠️ **带 `ptr` / `len` 入参的导出不在此列** —— 它们各自写明前置条件（指针必须
//! 有效或为 null 等），别从本段推断。
//! ⚠️ `opi_ffi_free_string` 是上面那句 `catch_unwind` 的**唯一例外**，且是有意的：它只
//! `drop` 调用方给的句柄、不碰共享单例，而包装本身要再分配/加锁 —— 释放路径上再加一层
//! 反而更危险。「唯一」由 `tests/cabi_safety_doc.rs` 钉住（别只改这句话）。
//!
//! 下面这条 `allow` 关掉的是 clippy 的 `missing_safety_doc`（作用范围：本模块**全部**
//! 导出，含将来新增的），替代执行点见 `tests/cabi_safety_doc.rs`（**动导出面后跑它**）。
#![allow(clippy::missing_safety_doc)]

use std::panic::{AssertUnwindSafe, catch_unwind};

use engine_core::router::KeyAction;

use crate::api;

mod utf16;
use utf16::{read_utf16, texts_to_json};

/// UTF-16 字符串句柄（Rust 侧分配，调用方负责 opi_ffi_free_string）。
/// `impl` 在 [`utf16`]（**非导出**辅助，拆出去给本文件腾行数；结构与导出留在这里，
/// 因为 `c_abi_contract.rs` 只解析本文件）。
#[repr(C)]
pub struct OpiString {
    pub ptr: *const u16,
    pub len: usize,
}

/// 释放 `opi_*` 返回的 OpiString。
/// # Safety
/// `s` 必须是 `opi_*` 返回且尚未释放过的句柄（Rust 侧分配）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_ffi_free_string(s: OpiString) {
    if !s.ptr.is_null() && s.len > 0 {
        let v = unsafe { Vec::from_raw_parts(s.ptr as *mut u16, s.len, s.len) };
        drop(v);
    }
}

// ---------- C 函数导出（另有 opi_ffi_free_string；条数以 c_abi_contract 门禁为准） ----------

/// load(path: const uint16_t*, len) -> bool。① null / 空串 → 内置回退词库并返回
/// **true**（既有契约，有意为之）；② 非法 UTF-16 → **false 且不替换**已有词库
/// （折成一个 `None` 就会把乱码路径静默装成内置 35 词库）；③ 合法但加载失败 → false。
/// # Safety
/// `ptr` 必须指向至少 `len` 个有效 `u16`（或为 null，视为空串）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_load(path: *const u16, len: usize) -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        unsafe { read_utf16(path, len) }.is_some_and(|p| api::install(Some(&p)).is_ok())
    }))
    .unwrap_or(false)
}

/// loadTrad(path: const uint16_t*, len) -> bool。空/坏路径/引擎未加载 → false
/// （繁体模式回退简体库，见 spec 错误处理）。
/// # Safety
/// `ptr` 必须指向至少 `len` 个有效 `u16`（或为 null，视为空串）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_load_trad(path: *const u16, len: usize) -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        let path = unsafe { read_utf16(path, len) }.unwrap_or_default();
        api::install_trad(&path).is_ok()
    }))
    .unwrap_or(false)
}

/// inputKey(ch) -> OpiString。永不 panic。单字符外返回空串。
/// # Safety
/// `ptr` 必须指向至少 `len` 个有效 `u16`（或为 null，视为空串）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_input_key(ptr: *const u16, len: usize) -> OpiString {
    let out = catch_unwind(AssertUnwindSafe(|| {
        let ch = unsafe { read_utf16(ptr, len) }.unwrap_or_default();
        api::with_engine(|e| e.input_key(ch)).unwrap_or_default()
    }))
    .unwrap_or_default();
    OpiString::from_utf16(&out)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_backspace() {
    let _ = catch_unwind(AssertUnwindSafe(|| api::with_engine(|e| e.backspace())));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_clear() {
    let _ = catch_unwind(AssertUnwindSafe(|| api::with_engine(|e| e.clear())));
}

/// select(index) -> OpiString。越界返回空串（旧语义）。
///
/// index 是**全局**索引（JNI 与既有调用方用）。点击候选请用 `opi_select_page(k)`
/// （页内索引）—— 自己算 `opi_page() * 8 + k` 会把 PAGE_SIZE 抄进 UI，见该导出的注释。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_select(index: usize) -> OpiString {
    let out = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.select(index)).unwrap_or_default()
    }))
    .unwrap_or_default();
    OpiString::from_utf16(&out)
}

/// switchMode(mode: i32)。**0=Pinyin 1=English 2=Number 3=Symbol 4=Traditional**，越界忽略。
///
/// 注意：这个编码**不等于 `Mode` 枚举的声明序**（声明序是 Pinyin, Traditional, English,
/// Number, Symbol —— 照声明序推会得到 Traditional=1）。跨语言侧一律照 `mode_to_int` 的
/// 编码写，别照枚举声明序写：错了不会编译失败，只会静默显示成拼音。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_switch_mode(mode: i32) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if let Some(m) = api::mode_from_int(mode) {
            api::with_engine(|e| e.switch_mode(m.into()));
        }
    }));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_set_shift(on: bool) {
    let _ = catch_unwind(AssertUnwindSafe(|| api::with_engine(|e| e.set_shift(on))));
}

// ---------- 标点/全角/符号开关（B3 / B5 的语义出口；键位仍不在引擎层） ----------
// 五个出口的完整契约（含未装载哨兵、「切模式后必须重读全角」的硬规则、
// 以及「中文标点这一档**不**随模式重置」的对照）在 `tests/cabi_fullwidth.rs`
// 的模块注释里 —— 那里同时是这些条款的门禁。

/// toggleFullwidth() -> bool：全角 ↔ 半角，返回**切换后的新状态**（状态栏直接拿去
/// 刷新）。只影响标点（中文模式出中文标点，其余机械全角），字母/数字不动；
/// 未装载 → false（那时按键全走 action=0，宿主拿到的就是半角，不是错误码）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_toggle_fullwidth() -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.toggle_fullwidth()).unwrap_or(false)
    }))
    .unwrap_or(false)
}

/// fullwidthState() -> bool：全角开关读侧。**每一次 `opi_switch_mode` 之后都必须重读**
/// —— 切模式把它重置为该模式的默认值（拼音/繁体全角，英文/数字/符号半角）；平台侧
/// 自记一份五模式默认值表就是第五份拷贝，必漂。未装载 → false。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_fullwidth_state() -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.fullwidth()).unwrap_or(false)
    }))
    .unwrap_or(false)
}

/// chinesePunct() -> bool：中文标点表开关读侧（`.。`、引号交替那一档）。未装载 → false。
///
/// ⚠️ **与 `opi_fullwidth_state` 不是一档**：本档是**用户偏好**，`opi_switch_mode`
/// **不重置它** —— 硬规则「切模式后必须重读全角」**不适用于本出口**。仍必须有读侧：
/// 设置页与下次启动的持久化都会改动它。关掉本档**不等于半角** —— 表未命中的字符
/// 仍走全角那一档（两闸各管一段，真值表见 `tests/cabi_fullwidth.rs`）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_chinese_punct() -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.chinese_punct()).unwrap_or(false)
    }))
    .unwrap_or(false)
}

/// setChinesePunct(on: bool)：设置中文标点表是否生效。**无返回值** —— 它是设置项
/// （勾选框知道要设成什么值），要拿结果请读 `opi_chinese_punct()`（未装载时是空操作）。
/// **写入口有两个**：设置项走本出口，触发键走 `opi_toggle_chinese_punct()`（那个有返回值，
/// 且客户端本来就不知道该设成什么值）。只翻一个 bool，不改 buffer/候选/页码。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_set_chinese_punct(on: bool) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.set_chinese_punct(on))
    }));
}

/// toggleChinesePunct() -> bool：中文标点表开关的**触发键入口**，返回**切换后的新状态**
/// （状态栏/勾选框直接拿去刷新，不必再查一次）。与 `setChinesePunct` 是同一档的两个写入口，
/// 语义在引擎层（键位不在本层，客户端在引擎之前截获）。
///
/// ⚠️ 与 `opi_toggle_fullwidth` 的**重读规则不通用**：本档是用户偏好，`opi_switch_mode`
/// / `opi_toggle_symbol` **都不重置它**。只翻一个 bool，不改 buffer/候选/页码。
/// 未装载 → false：与「已装载且表关」共用同一个 false，**不是**在报错
/// （那时按键全走 action=0 交系统，返回值与可观测行为一致）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_toggle_chinese_punct() -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.toggle_chinese_punct()).unwrap_or(false)
    }))
    .unwrap_or(false)
}

/// toggleSymbol() -> OpiString：符号面板开关，**返回需要上屏的文本（空串 = 无提交）**。
/// ⚠️ 返回的**不是**「刚切出来的那个符号」，而是切进面板前那截缓冲的待上屏文本
/// （有候选提首候选、无候选的乱码缓冲清掉而不上屏）。为何不是 void：这段文本必须
/// 交出来，拿不到插入通道的端不要调它。调用方负责 `opi_ffi_free_string`
/// （空句柄可无条件释放）。未装载 → 空句柄。
/// ⚠️ 调用后须重读 mode / buffer / candidates / fullwidth 四样 —— 内部走了一次
/// `switch_mode`，而它会按模式默认值重置全角（Symbol 默认半角）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_toggle_symbol() -> OpiString {
    let out = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.toggle_symbol()).unwrap_or_default()
    }))
    .unwrap_or_default();
    OpiString::from_utf16(&out)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_input_space() -> OpiString {
    let out = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.input_space()).unwrap_or_default()
    }))
    .unwrap_or_default();
    OpiString::from_utf16(&out)
}

/// 键路由结果（Swift 侧 `struct { int32_t action; OpiString text; }` 同构）。
///
/// **所有权**：`text` 由 Rust 侧分配（`Box<[u16]>`），调用方**恰好**用
/// `opi_ffi_free_string` 释放一次；空句柄（ptr null）也可无条件释放（那是 no-op），
/// 故调用方不必按 action 分支决定要不要释放。
#[repr(C)]
pub struct OpiKeyEventResult {
    /// 0=未处理（**交系统**处理该键）1=已处理（无提交文本，调用方刷新 composition/
    /// 候选栏）2=提交（`text` 有效）。
    pub action: i32,
    /// action=2 时由 Rust 侧分配的 UTF-16 文本（调用方用 `opi_ffi_free_string` 释放）；
    /// 其余情况为空句柄（ptr null）——调用方可以无条件释放。
    pub text: OpiString,
}

/// keyEvent(keyval, states) -> OpiKeyEventResult。平台中立键路由（iOS / macOS）：
/// 可打印字符 = Unicode 码点（**空格 = 0x20**），特殊键 = `0x1_0000 | 低 16 位码`
/// （常量表与取值理由见 `engine_core::keys`），`states` 为修饰位。语义与 fcitx5 /
/// TSF 两轨逐条同构，路由表在 `engine_core::router::KeyRouter`。
///
/// # 键码对照（**映射键位前必读**）
///
/// 两轨的键码空间**不同**，本层是第三套（`engine_core::keys`）。照另一个轨抄的后果
/// 不是「多认了一个键」而是键**静默失效**：
///
/// | 键 | 本层 | fcitx5 轨 | TSF 轨 | 用错编码的后果 |
/// |---|---|---|---|---|
/// | 空格 | `0x20`（可打印段） | `0x20` | `0x1_0020` | 用 TSF 编码 → 落进非 ASCII 直通分支：**打拼音按空格不提交候选**，只出一个空格 |
/// | ⇧ | `0x1_0083`（左右同码） | `0xffe1`/`0xffe2` | `0x1_0010` | 键值对不上 → ⇧ 状态机不触发，英文模式打不出大写 |
/// | PageUp/Down | `0x1_0080`/`0x1_0081` | `0xff55`/`0xff56` | `0x1_0021`/`0x1_0022` | 候选翻页失效 |
/// | Delete | `0x1_0082` | `0xffff` | `0x1_002e` | 向后删除失效（TSF 的 `0x2e` 还与 `.` 同值，见 keys.rs 的撞号注释） |
///
/// 退格/Tab/回车/Esc 四者的低字节三轨一致（BS/HT/CR/ESC），差异只在基址。
/// **空格是本层唯一一处故意不套 `SPECIAL_BASE` 的键**：本 ABI 的冻结约定是
/// 「可打印字符 = Unicode 码点」，空格属于可打印段（与 fcitx5 轨一致，TSF 轨的
/// `SPECIAL_BASE|0x20` 在本层会退化成直通）。
///
/// 引擎未装载或内部 panic → `action=0`（交系统），**绝不静默吞键**。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_key_event(keyval: u32, states: u32) -> OpiKeyEventResult {
    let action = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.key_event(keyval, states)).unwrap_or(KeyAction::PassThrough)
    }))
    .unwrap_or(KeyAction::PassThrough);
    match action {
        KeyAction::Input(text) => OpiKeyEventResult {
            action: 2,
            text: OpiString::from_utf16(&text),
        },
        KeyAction::EngineHandled => OpiKeyEventResult {
            action: 1,
            text: OpiString::empty(),
        },
        KeyAction::PassThrough => OpiKeyEventResult {
            action: 0,
            text: OpiString::empty(),
        },
    }
}

/// selectPage(k) -> OpiString：提交**当前页**第 k 个候选（**页内**索引，0 起）。
/// 越界/无候选 → 空串（不改状态、不 panic）。**引擎未装载 → 空串**。
///
/// 点击候选请用本出口，**不要**自己算 `opi_page() * 8 + k` 再调 `opi_select()`：
/// 那个 8 就是 PAGE_SIZE 的第三份拷贝，引擎改一次 UI 便**静默选错候选**。
/// 本出口与按数字键（`opi_key_event` 的数字选词）、回车提交**同源** ——
/// 三者都走 `KeyRouter::select` 那一份页内换算。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_select_page(index: u32) -> OpiString {
    let out = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.select_page(index as usize)).unwrap_or_default()
    }))
    .unwrap_or_default();
    OpiString::from_utf16(&out)
}

/// candidatesPage() -> JSON 文本数组（**当前页**，已按引擎的 PAGE_SIZE 分页）。
/// **引擎未装载 → 空数组（`[]`）**。
///
/// 前端显示当前页请用本出口、**不要**拿 `opi_candidates()` 自己按 8 切：页大小是
/// 引擎侧常量（`KeyRouter`/候选栏共用），抄一份到 UI，改一次就会静默错位
/// （高亮的页 ≠ 实际选词所在的页）。本出口与 `opi_page()`/`opi_page_count()`
/// 同源，三者永远一致。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_candidates_page() -> OpiString {
    let texts = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.candidate_texts_page()).unwrap_or_default()
    }))
    .unwrap_or_default();
    texts_to_json(texts)
}

/// pageCount() -> uint32：候选总页数（UI 的「共 N 页」；**无候选 → 0**，
/// 引擎未装载 → 0）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_page_count() -> u32 {
    catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.page_count()).unwrap_or(0)
    }))
    .unwrap_or(0)
}

/// page() -> uint32：当前候选页（0 起，末页由路由钳制）。**引擎未装载 → 0**。
///
/// 候选栏的页码必须读这里、不要自己数：PageDown 到末页时路由会把页码钳到最后一页，
/// 本地计数超过末页就会与引擎漂移（高亮的页 ≠ 实际选词所在的页）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_page() -> u32 {
    catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.page()).unwrap_or(0)
    }))
    .unwrap_or(0)
}

/// shiftState() -> int32：**前端** ⇧ 三态 0=OFF 1=SINGLE（下个字母大写后自动复位）
/// 2=LOCK（持续大写）。**引擎未装载 → 0**。
///
/// 与 `opi_set_shift` 不是一回事：那个打的是**引擎侧** shift 位；三态是前端状态，
/// 英文直传路径的大小写由它决定（见 `KeyRouter::key_event`），引擎位看不出来 ——
/// 所以 ⇧ 键的高亮（尤其 Lock）只能读本出口。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_shift_state() -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.shift_state_int()).unwrap_or(0)
    }))
    .unwrap_or(0)
}

/// candidates(limit) -> JSON 文本数组。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_candidates(limit: usize) -> OpiString {
    let texts = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| api::candidate_texts(e, limit)).unwrap_or_default()
    }))
    .unwrap_or_default();
    texts_to_json(texts)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_buffer() -> OpiString {
    let out = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.buffer()).unwrap_or_default()
    }))
    .unwrap_or_default();
    OpiString::from_utf16(&out)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_mode() -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| api::mode_to_int(e.mode().into())).unwrap_or(0)
    }))
    .unwrap_or(0)
}

/// searchSymbols(keyword) -> JSON 文本数组。
/// # Safety
/// `ptr` 必须指向至少 `len` 个有效 `u16`（或为 null，视为空串）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_search_symbols(ptr: *const u16, len: usize) -> OpiString {
    let texts = catch_unwind(AssertUnwindSafe(|| {
        let kw = unsafe { read_utf16(ptr, len) }.unwrap_or_default();
        api::with_engine(|e| api::search_symbol_texts(e, &kw)).unwrap_or_default()
    }))
    .unwrap_or_default();
    texts_to_json(texts)
}

/// symbolBlocks() -> JSON：`[{id,start,end,name,common}]`。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_symbol_blocks() -> OpiString {
    let json = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| api::symbol_blocks_json(e)).unwrap_or_default()
    }))
    .unwrap_or_default();
    OpiString::from_utf16(&json)
}

/// symbolsInBlock(id: i16) -> JSON 文本数组。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_symbols_in_block(id: i16) -> OpiString {
    let texts = catch_unwind(AssertUnwindSafe(|| {
        // 负 id 按越界处理（空数组），不钳成块 0 —— 与 jni.rs 的
        // opijni_symbols_in_block 同语义，两个 ABI 面必须一致。
        api::with_engine(|e| {
            if id < 0 {
                Vec::new()
            } else {
                api::symbol_texts(e, id as u16)
            }
        })
        .unwrap_or_default()
    }))
    .unwrap_or_default();
    texts_to_json(texts)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_learner_enabled() -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.learner_enabled()).unwrap_or(false)
    }))
    .unwrap_or(false)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_set_learner(enabled: bool) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.set_learner(enabled))
    }));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_clear_user_words() {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.clear_user_words())
    }));
}

/// remove_user_word(text: const uint16_t*, len)。长按删词的 C ABI 出口
/// （镜像 jni.rs 的 opijni_remove_user_word，两个 ABI 面语义必须一致）。
/// 词不存在 / 空串 / null → 无操作；导出 JSON 逐字节不变。
/// # Safety
/// `ptr` 必须指向至少 `len` 个有效 `u16`（或为 null，视为空串）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_remove_user_word(ptr: *const u16, len: usize) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let text = unsafe { read_utf16(ptr, len) }.unwrap_or_default();
        api::with_engine(|e| e.remove_user_word(text));
    }));
}

/// import_user_words(json: const uint16_t*, len) -> i32。
/// 返回导入条数；失败返回负值（-1）——非法 JSON / 版本不符 / 词表过大 /
/// 引擎未装载 / null 入参。失败不改动既有用户词（引擎侧「全有或全无」）。
/// # Safety
/// `ptr` 必须指向至少 `len` 个有效 `u16`（或为 null，视为失败）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_import_user_words(ptr: *const u16, len: usize) -> i32 {
    catch_unwind(AssertUnwindSafe(|| -> i32 {
        // null 与空串一律失败：空串不是合法 JSON，无需特判成空词表。
        let Some(json) = (unsafe { read_utf16(ptr, len) }) else {
            return -1;
        };
        match api::with_engine(|e| e.import_user_words(json)) {
            // 条数上限 MAX_IMPORT_WORDS = 100_000，必然装得下 i32；
            // try_from 只是不给理论溢出留 unwrap panic 的口子。
            Some(Ok(n)) => i32::try_from(n).unwrap_or(-1),
            Some(Err(_)) | None => -1,
        }
    }))
    .unwrap_or(-1)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn opi_export_user_words() -> OpiString {
    let out = catch_unwind(AssertUnwindSafe(|| {
        api::with_engine(|e| e.export_user_words()).unwrap_or_default()
    }))
    .unwrap_or_default();
    OpiString::from_utf16(&out)
}
