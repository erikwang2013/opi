// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! JNI 出口：`JNI_OnLoad` + `RegisterNatives` 注册（不用 Java_ 命名导出，防签名脆断）。宿主类：
//! `xyz/erik/opi/jni/OpiEngine`。每个函数 `catch_unwind` 包裹，panic / 错误返回哨兵（false / 0 / null）；
//! 语义与 C ABI（cabi.rs）完全一致，共享 api::SINGLETON 与内部实现。
//!
//! # Safety（本模块所有 `opijni_*` 与 `JNI_OnLoad` 的统一契约）
//! `env` 必须为当前线程有效且非空的 JNIEnv（JVM 调用约定保证）；jstring 参数须为
//! 有效本地引用（可 null）。每个函数内部均以 catch_unwind 包裹，panic 不跨 FFI 边界。
#![allow(clippy::missing_safety_doc)]

use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

use jni::jni_str;
use jni::sys::{self, jboolean, jint, jstring};
use jni::{JavaVM, NativeMethod, ScopeToken};

use crate::api;
use crate::jni_util;

pub(crate) type JEnv = *mut sys::JNIEnv;

/// 出口统一包装：`catch_unwind` + 未装载哨兵。替代本文件里逐字重复的
/// `catch_unwind(AssertUnwindSafe(|| …)).unwrap_or(…)`（26 份）。**语义逐条等价** ——
/// `T::default()` 正是各出口原本的哨兵（false / 0 / 空串 / 空数组），不是新约定。
/// 完整契约（panic 不跨 FFI 边界、未装载语义）见模块头 `# Safety`。
/// ⚠️ 唯一不适用的是 `import_user_words`：它的哨兵是 **-1 而非 `Default`**，故仍用裸
/// `catch_unwind` —— 别把它「顺手统一」掉，那会把「失败」变成「导入了 0 条」。
pub(crate) fn guard<T: Default>(f: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_default()
}

// ---------- native 方法（条数以 jni_contract 门禁为准） ----------

// 拆到 `jni_scheme.rs` 的五个函数（词典装载 + 输入方案）：注册表仍在本文件
// （`JNI_OnLoad`），与 `jni_user_words.rs` 同一次拆分的手续 —— 这两个 use 是注册表
// 能引用到它们的唯一原因，别删。
// 「属性设置」旋钮族（2026-10-13 拆到 `jni_switches.rs`）：注册表仍在本文件，
// 这些 use 是注册表引用的手续 —— 与 `jni_user_words.rs` 同一次拆分的手续。
use crate::jni_scheme::{
    opijni_load, opijni_load_stroke, opijni_load_trad, opijni_scheme, opijni_set_scheme,
    opijni_set_shuangpin_layout, opijni_shuangpin_layout, opijni_switch_language,
};
use crate::jni_switches::{
    opijni_chinese_punct, opijni_correction_enabled, opijni_fuzzy_enabled, opijni_fuzzy_pairs,
    opijni_jianpin_enabled, opijni_set_chinese_punct, opijni_set_correction, opijni_set_fuzzy,
    opijni_set_fuzzy_pairs, opijni_set_jianpin, opijni_toggle_chinese_punct,
};
// 拆到 `jni_candidates.rs` 的四个**读侧**出口（candidates / candidateReadings /
// buffer / displayBuffer）：同一手续（2026-10-09：加 candidateReadings 后本文件
// 越 500 行）。同样只为注册表能引用到它们。
use crate::jni_candidates::{
    opijni_buffer, opijni_buffer_is_carried, opijni_candidate_readings, opijni_candidates,
    opijni_display_buffer, opijni_pending_text, opijni_pin_reading,
};
/// inputKey(ch: String) -> String。永不 panic。单字符外（空/多字符/非 ASCII）返回空串。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_input_key(
    env: JEnv,
    _class: sys::jclass,
    key: jstring,
) -> jstring {
    let out = guard(|| {
        let ch = unsafe { jni_util::jstring_to_rust(env, key) }.unwrap_or_default();
        api::with_engine(|e| e.input_key(ch)).unwrap_or_default()
    });
    unsafe { jni_util::rust_to_jstring(env, &out) }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_backspace(_env: JEnv, _class: sys::jclass) {
    guard(|| api::with_engine(|e| e.backspace()));
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_clear(_env: JEnv, _class: sys::jclass) {
    guard(|| api::with_engine(|e| e.clear()));
}

/// select(index: Int) -> String。越界返回空串（旧语义）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_select(
    env: JEnv,
    _class: sys::jclass,
    index: jint,
) -> jstring {
    let out = guard(|| {
        api::with_engine(|e| {
            // 负索引按越界处理（空串）。不能 `index.max(0)` 钳成 0 —— 那会把负数
            // 变成合法下标并提交第 0 个候选，与本函数的文档约定相反。
            if index < 0 {
                String::new()
            } else {
                e.select(index as usize)
            }
        })
        .unwrap_or_default()
    });
    unsafe { jni_util::rust_to_jstring(env, &out) }
}

/// switchMode(mode: Int)。**0=Pinyin 1=English 2=Number 3=Symbol 4=Traditional**，越界忽略。
///
/// 注意：这个编码**不等于 `Mode` 枚举的声明序**（声明序是 Pinyin, Traditional, English,
/// Number, Symbol —— 照声明序推会得到 Traditional=1）。跨语言侧一律照 `mode_to_int` 的
/// 编码写，别照枚举声明序写：错了不会编译失败，只会静默显示成拼音。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_switch_mode(_env: JEnv, _class: sys::jclass, mode: jint) {
    guard(|| {
        if let Some(m) = api::mode_from_int(mode) {
            api::with_engine(|e| e.switch_mode(m.into()));
        }
    });
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_set_shift(_env: JEnv, _class: sys::jclass, on: jboolean) {
    guard(|| api::with_engine(|e| e.set_shift(on)));
}

// 标点/全角/符号这组开关出口：**完整契约在 `cabi.rs` 对应导出的 doc 上**（两个 ABI 面语义必须一致），这里只记 JNI 侧独有的一条：
// 读侧哨兵与其余出口同构（未装载 → false / 空串，不 panic，不报错码）。⚠️ `toggleFullwidth`/`fullwidthState` 与
// `chinesePunct`/`toggleChinesePunct` **不是同构的两组** —— 全角随模式默认（`switchMode` 后必须重读）；中文标点是用户偏好（`switchMode` / `toggleSymbol` **都不重置**）。当成同构照抄 = 造 bug。
/// toggleFullwidth() -> boolean：全角 ↔ 半角，返回**切换后的新状态**。未装载 → false。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_toggle_fullwidth(_env: JEnv, _class: sys::jclass) -> jboolean {
    guard(|| api::with_engine(|e| e.toggle_fullwidth()).unwrap_or_default())
}

/// fullwidthState() -> boolean：全角开关读侧。**每一次 `switchMode` 之后都必须重读**。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_fullwidth_state(_env: JEnv, _class: sys::jclass) -> jboolean {
    guard(|| api::with_engine(|e| e.fullwidth()).unwrap_or_default())
}

/// toggleSymbol() -> String：符号面板开关，**返回需要上屏的文本（空串 = 无提交）**。
/// **拿不到插入通道的宿主不要调它**；调用后须重读 mode/buffer/candidates/fullwidth 四样
/// （与 `cabi.rs` 的 `opi_toggle_symbol` 同契约，细节见那里）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_toggle_symbol(env: JEnv, _class: sys::jclass) -> jstring {
    let out = guard(|| api::with_engine(|e| e.toggle_symbol()).unwrap_or_default());
    unsafe { jni_util::rust_to_jstring(env, &out) }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_input_space(env: JEnv, _class: sys::jclass) -> jstring {
    let out = guard(|| api::with_engine(|e| e.input_space()).unwrap_or_default());
    unsafe { jni_util::rust_to_jstring(env, &out) }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_mode(_env: JEnv, _class: sys::jclass) -> jint {
    guard(|| api::with_engine(|e| api::mode_to_int(e.mode().into())).unwrap_or_default())
}

// 拆到 `jni_symbols.rs` 的五个函数（符号 / 表情一族）：注册表仍在本文件
// （`JNI_OnLoad`），名字要从那边 use 进来 —— 少了这一行是**编译错误**（不是静默），
// 正好。
use crate::jni_symbols::{
    opijni_emoji_groups, opijni_emoji_symbols, opijni_search_symbols, opijni_symbol_blocks,
    opijni_symbols_in_block,
};

// 拆到 `jni_user_words.rs` 的六个函数：注册表仍在本文件（`JNI_OnLoad`），
// 名字要从那边 use 进来 —— 少了这一行是**编译错误**（不是静默），正好。
use crate::jni_user_words::{
    opijni_clear_user_words, opijni_export_user_words, opijni_import_user_words,
    opijni_learner_enabled, opijni_remove_user_word, opijni_set_learner,
};

// 拆到 `jni_dict.rs` 的四个函数（词库导入一族，2026-10-14）：同一手续 ——
// 注册表能引用到它们全靠这一行，别删。
use crate::jni_dict::{
    opijni_dict_list, opijni_import_dict, opijni_remove_dict, opijni_set_dict_enabled,
};

// ---------- JNI_OnLoad ----------
// ---------- JNI_OnLoad ----------

#[unsafe(no_mangle)]
pub unsafe extern "system" fn JNI_OnLoad(vm: *mut sys::JavaVM, _reserved: *mut c_void) -> jint {
    let result = catch_unwind(AssertUnwindSafe(|| -> Result<jint, String> {
        // JNI_OnLoad 必然运行在已 attach 的 JVM 线程上（System.loadLibrary 的调用线程），
        // get_env_attachment 即 GetEnv，不会触发 attach。
        let mut scope = ScopeToken::default();
        let mut guard = unsafe {
            JavaVM::from_raw(vm)
                .get_env_attachment(&mut scope)
                .map_err(|e| format!("get_env_attachment 失败: {e}"))?
        };
        let env = guard.borrow_env_mut();
        let class = env
            .find_class(jni_str!("xyz/erik/opi/jni/OpiEngine"))
            .map_err(|e| format!("find_class 失败: {e}"))?;
        let methods = unsafe {
            [
                NativeMethod::from_raw_parts(
                    jni_str!("load"),
                    jni_str!("(Ljava/lang/String;)Z"),
                    opijni_load as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("loadTrad"),
                    jni_str!("(Ljava/lang/String;)Z"),
                    opijni_load_trad as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("inputKey"),
                    jni_str!("(Ljava/lang/String;)Ljava/lang/String;"),
                    opijni_input_key as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("backspace"),
                    jni_str!("()V"),
                    opijni_backspace as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("clear"),
                    jni_str!("()V"),
                    opijni_clear as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("select"),
                    jni_str!("(I)Ljava/lang/String;"),
                    opijni_select as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("switchMode"),
                    jni_str!("(I)V"),
                    opijni_switch_mode as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("setShift"),
                    jni_str!("(Z)V"),
                    opijni_set_shift as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("toggleFullwidth"),
                    jni_str!("()Z"),
                    opijni_toggle_fullwidth as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("fullwidthState"),
                    jni_str!("()Z"),
                    opijni_fullwidth_state as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("toggleSymbol"),
                    jni_str!("()Ljava/lang/String;"),
                    opijni_toggle_symbol as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("chinesePunct"),
                    jni_str!("()Z"),
                    opijni_chinese_punct as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("setChinesePunct"),
                    jni_str!("(Z)V"),
                    opijni_set_chinese_punct as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("fuzzyEnabled"),
                    jni_str!("()Z"),
                    opijni_fuzzy_enabled as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("setFuzzy"),
                    jni_str!("(Z)V"),
                    opijni_set_fuzzy as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("fuzzyPairs"),
                    jni_str!("()I"),
                    opijni_fuzzy_pairs as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("setFuzzyPairs"),
                    jni_str!("(I)V"),
                    opijni_set_fuzzy_pairs as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("jianpinEnabled"),
                    jni_str!("()Z"),
                    opijni_jianpin_enabled as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("setJianpin"),
                    jni_str!("(Z)V"),
                    opijni_set_jianpin as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("correctionEnabled"),
                    jni_str!("()Z"),
                    opijni_correction_enabled as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("setCorrection"),
                    jni_str!("(Z)V"),
                    opijni_set_correction as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("setScheme"),
                    jni_str!("(I)V"),
                    opijni_set_scheme as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("switchLanguage"),
                    jni_str!("(II)V"),
                    opijni_switch_language as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("scheme"),
                    jni_str!("()I"),
                    opijni_scheme as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("setShuangpinLayout"),
                    jni_str!("(I)V"),
                    opijni_set_shuangpin_layout as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("shuangpinLayout"),
                    jni_str!("()I"),
                    opijni_shuangpin_layout as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("loadStroke"),
                    jni_str!("(Ljava/lang/String;)Z"),
                    opijni_load_stroke as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("toggleChinesePunct"),
                    jni_str!("()Z"),
                    opijni_toggle_chinese_punct as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("inputSpace"),
                    jni_str!("()Ljava/lang/String;"),
                    opijni_input_space as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("candidates"),
                    jni_str!("(I)[Ljava/lang/String;"),
                    opijni_candidates as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("candidateReadings"),
                    jni_str!("()[Ljava/lang/String;"),
                    opijni_candidate_readings as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("pinReading"),
                    jni_str!("(Ljava/lang/String;)V"),
                    opijni_pin_reading as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("buffer"),
                    jni_str!("()Ljava/lang/String;"),
                    opijni_buffer as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("displayBuffer"),
                    jni_str!("()Ljava/lang/String;"),
                    opijni_display_buffer as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("pendingText"),
                    jni_str!("()Ljava/lang/String;"),
                    opijni_pending_text as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("bufferIsCarried"),
                    jni_str!("()Z"),
                    opijni_buffer_is_carried as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("mode"),
                    jni_str!("()I"),
                    opijni_mode as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("searchSymbols"),
                    jni_str!("(Ljava/lang/String;)[Ljava/lang/String;"),
                    opijni_search_symbols as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("emojiSymbols"),
                    jni_str!("()[Ljava/lang/String;"),
                    opijni_emoji_symbols as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("symbolBlocks"),
                    jni_str!("()Ljava/lang/String;"),
                    opijni_symbol_blocks as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("emojiGroups"),
                    jni_str!("()Ljava/lang/String;"),
                    opijni_emoji_groups as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("symbolsInBlock"),
                    jni_str!("(S)[Ljava/lang/String;"),
                    opijni_symbols_in_block as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("learnerEnabled"),
                    jni_str!("()Z"),
                    opijni_learner_enabled as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("setLearner"),
                    jni_str!("(Z)V"),
                    opijni_set_learner as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("clearUserWords"),
                    jni_str!("()V"),
                    opijni_clear_user_words as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("removeUserWord"),
                    jni_str!("(Ljava/lang/String;)V"),
                    opijni_remove_user_word as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("importUserWords"),
                    jni_str!("(Ljava/lang/String;)I"),
                    opijni_import_user_words as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("exportUserWords"),
                    jni_str!("()Ljava/lang/String;"),
                    opijni_export_user_words as *mut c_void,
                ),
                // —— 词库导入一族（2026-10-14，M3）——
                NativeMethod::from_raw_parts(
                    jni_str!("importDict"),
                    jni_str!("(Ljava/lang/String;)I"),
                    opijni_import_dict as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("dictList"),
                    jni_str!("()Ljava/lang/String;"),
                    opijni_dict_list as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("setDictEnabled"),
                    jni_str!("(Ljava/lang/String;Z)Z"),
                    opijni_set_dict_enabled as *mut c_void,
                ),
                NativeMethod::from_raw_parts(
                    jni_str!("removeDict"),
                    jni_str!("(Ljava/lang/String;)Z"),
                    opijni_remove_dict as *mut c_void,
                ),
            ]
        };
        // 五笔旋钮族的 12 条在 `jni_wubi.rs`（本文件 500 行门禁：整块搬走，条目一字未改）。
        // 数组没有 extend ⇒ 先转 Vec 再并。
        let mut methods: Vec<NativeMethod> = methods.to_vec();
        methods.extend(crate::jni_wubi::wubi_native_methods());
        unsafe {
            env.register_native_methods(class, &methods)
                .map_err(|e| format!("register_natives 失败: {e}"))?;
        }
        Ok(sys::JNI_VERSION_1_6)
    }));
    match result {
        Ok(Ok(v)) => v,
        _ => 0, // 注册失败：System.load 将抛出 UnsatisfiedLinkError
    }
}
