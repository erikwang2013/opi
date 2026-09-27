// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! JNI 出口：`JNI_OnLoad` + `RegisterNatives` 注册（不用 Java_ 命名导出，防签名脆断）。
//! 宿主类：`io/opi/input/jni/OpiEngine`。每个函数 `catch_unwind` 包裹，
//! panic / 错误返回哨兵（boolean false、int 0、String/数组 null）。
//! 语义与 C ABI（cabi.rs）完全一致，共享 api::SINGLETON 与内部实现。
//!
//! # Safety（本模块所有 `opijni_*` 与 `JNI_OnLoad` 的统一契约）
//! `env` 必须为当前线程有效且非空的 JNIEnv（JVM 调用约定保证）；jstring 参数须为
//! 有效本地引用（可 null）。每个函数内部均以 catch_unwind 包裹，panic 不跨 FFI 边界。
#![allow(clippy::missing_safety_doc)]

use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

use jni::jni_str;
use jni::sys::{self, jboolean, jint, jshort, jstring};
use jni::{JavaVM, NativeMethod, ScopeToken};

use crate::api;
use crate::jni_util;

type JEnv = *mut sys::JNIEnv;

/// 出口统一包装：`catch_unwind` + 未装载哨兵。替代本文件里逐字重复的
/// `catch_unwind(AssertUnwindSafe(|| …)).unwrap_or(…)`（26 份）。**语义逐条等价** ——
/// `T::default()` 正是各出口原本的哨兵（false / 0 / 空串 / 空数组），不是新约定。
/// 完整契约（panic 不跨 FFI 边界、未装载语义）见模块头 `# Safety`。
/// ⚠️ 唯一不适用的是 `import_user_words`：它的哨兵是 **-1 而非 `Default`**，故仍用裸
/// `catch_unwind` —— 别把它「顺手统一」掉，那会把「失败」变成「导入了 0 条」。
fn guard<T: Default>(f: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_default()
}

// ---------- native 方法（条数以 jni_contract 门禁为准） ----------

/// load(path: String?) -> bool。null/空串 → 内置回退词库；坏路径 → false。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_load(
    env: JEnv,
    _class: sys::jclass,
    path: jstring,
) -> jboolean {
    guard(|| {
        let path = unsafe { jni_util::jstring_to_rust(env, path) };
        api::install(path.as_deref()).is_ok()
    })
}

/// loadTrad(path: String) -> bool。空/坏路径/引擎未加载 → false（繁体模式回退简体库）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_load_trad(
    env: JEnv,
    _class: sys::jclass,
    path: jstring,
) -> jboolean {
    guard(|| {
        let path = unsafe { jni_util::jstring_to_rust(env, path) }.unwrap_or_default();
        api::install_trad(&path).is_ok()
    })
}

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

// 标点/全角/符号这组开关出口：**完整契约在 `cabi.rs` 对应导出的 doc 上**（两个 ABI 面
// 语义必须一致），这里只记 JNI 侧独有的一条：读侧哨兵与其余出口同构（未装载 → false /
// 空串，不 panic，不报错码）。⚠️ `toggleFullwidth`/`fullwidthState` 与
// `chinesePunct`/`toggleChinesePunct` **不是同构的两组** ——
// 全角随模式默认（`switchMode` 后必须重读），中文标点是用户偏好
// （`switchMode` / `toggleSymbol` **都不重置**）。当成同构照抄 = 造 bug。

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

/// chinesePunct() -> boolean：中文标点表读侧。未装载 → false。
/// 关掉本档 ≠ 半角：表未命中的字符仍由全角那一档管。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_chinese_punct(_env: JEnv, _class: sys::jclass) -> jboolean {
    guard(|| api::with_engine(|e| e.chinese_punct()).unwrap_or_default())
}

/// setChinesePunct(on: boolean)：设置中文标点表是否生效。**无返回值**（设置项）；
/// 要拿结果读 `chinesePunct()`。**写入口有两个**：本出口是设置项，触发键走
/// `toggleChinesePunct`（那个有返回值）。未装载时是空操作。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_set_chinese_punct(
    _env: JEnv,
    _class: sys::jclass,
    on: jboolean,
) {
    guard(|| api::with_engine(|e| e.set_chinese_punct(on)));
}

/// toggleChinesePunct() -> boolean：中文标点表开关的**触发键入口**，返回**切换后的
/// 新状态**。⚠️ 与 `toggleFullwidth` 的重读规则不通用（本档 `switchMode` 不重置）。
/// 完整契约在 `cabi.rs` 的 `opi_toggle_chinese_punct` 上。未装载 → false。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_toggle_chinese_punct(
    _env: JEnv,
    _class: sys::jclass,
) -> jboolean {
    guard(|| api::with_engine(|e| e.toggle_chinese_punct()).unwrap_or_default())
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_input_space(env: JEnv, _class: sys::jclass) -> jstring {
    let out = guard(|| api::with_engine(|e| e.input_space()).unwrap_or_default());
    unsafe { jni_util::rust_to_jstring(env, &out) }
}

/// candidates(limit: Int) -> String[]。仅文本数组（kind/score UI 不用）。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_candidates(
    env: JEnv,
    _class: sys::jclass,
    limit: jint,
) -> sys::jobjectArray {
    let texts = guard(|| {
        api::with_engine(|e| api::candidate_texts(e, limit.max(0) as usize)).unwrap_or_default()
    });
    unsafe { jni_util::string_array(env, texts) }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_buffer(env: JEnv, _class: sys::jclass) -> jstring {
    let out = guard(|| api::with_engine(|e| e.buffer()).unwrap_or_default());
    unsafe { jni_util::rust_to_jstring(env, &out) }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_mode(_env: JEnv, _class: sys::jclass) -> jint {
    guard(|| api::with_engine(|e| api::mode_to_int(e.mode().into())).unwrap_or_default())
}

/// searchSymbols(keyword: String) -> String[]。仅文本数组。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_search_symbols(
    env: JEnv,
    _class: sys::jclass,
    keyword: jstring,
) -> sys::jobjectArray {
    let texts = guard(|| {
        let kw = unsafe { jni_util::jstring_to_rust(env, keyword) }.unwrap_or_default();
        api::with_engine(|e| api::search_symbol_texts(e, &kw)).unwrap_or_default()
    });
    unsafe { jni_util::string_array(env, texts) }
}

/// symbolBlocks() -> String。JSON：`[{id,start,end,name,common}]`。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_symbol_blocks(env: JEnv, _class: sys::jclass) -> jstring {
    let json = guard(|| api::with_engine(|e| api::symbol_blocks_json(e)).unwrap_or_default());
    unsafe { jni_util::rust_to_jstring(env, &json) }
}

/// symbolsInBlock(id: Short) -> String[]。仅文本数组。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_symbols_in_block(
    env: JEnv,
    _class: sys::jclass,
    id: jshort,
) -> sys::jobjectArray {
    let texts = guard(|| {
        // 负 id 同理按越界处理（空数组），不钳成块 0。jshort 转 u16 前必须先判负，
        // 否则 -1 会变成 65535（一个合法但错误的块号）。
        api::with_engine(|e| {
            if id < 0 {
                Vec::new()
            } else {
                api::symbol_texts(e, id as u16)
            }
        })
        .unwrap_or_default()
    });
    unsafe { jni_util::string_array(env, texts) }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_learner_enabled(_env: JEnv, _class: sys::jclass) -> jboolean {
    guard(|| api::with_engine(|e| e.learner_enabled()).unwrap_or_default())
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_set_learner(
    _env: JEnv,
    _class: sys::jclass,
    enabled: jboolean,
) {
    guard(|| api::with_engine(|e| e.set_learner(enabled)));
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_clear_user_words(_env: JEnv, _class: sys::jclass) {
    guard(|| api::with_engine(|e| e.clear_user_words()));
}

/// removeUserWord(text: String)。长按删词（B4）。词不存在 / null / 空串 → 无操作。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_remove_user_word(
    env: JEnv,
    _class: sys::jclass,
    text: jstring,
) {
    guard(|| {
        let text = unsafe { jni_util::jstring_to_rust(env, text) }.unwrap_or_default();
        api::with_engine(|e| e.remove_user_word(text));
    });
}

/// importUserWords(json: String) -> Int。返回导入条数；
/// 负数表示失败（非法 JSON / 版本不符 / 引擎未装载 / null）且不改动既有用户词。
#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_import_user_words(
    env: JEnv,
    _class: sys::jclass,
    json: jstring,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| -> jint {
        let Some(json) = (unsafe { jni_util::jstring_to_rust(env, json) }) else {
            return -1;
        };
        match api::with_engine(|e| e.import_user_words(json)) {
            Some(Ok(n)) => i32::try_from(n).unwrap_or(-1),
            Some(Err(_)) | None => -1,
        }
    }))
    .unwrap_or(-1)
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn opijni_export_user_words(env: JEnv, _class: sys::jclass) -> jstring {
    let out = guard(|| api::with_engine(|e| e.export_user_words()).unwrap_or_default());
    unsafe { jni_util::rust_to_jstring(env, &out) }
}

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
            .find_class(jni_str!("io/opi/input/jni/OpiEngine"))
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
                    jni_str!("buffer"),
                    jni_str!("()Ljava/lang/String;"),
                    opijni_buffer as *mut c_void,
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
                    jni_str!("symbolBlocks"),
                    jni_str!("()Ljava/lang/String;"),
                    opijni_symbol_blocks as *mut c_void,
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
            ]
        };
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
