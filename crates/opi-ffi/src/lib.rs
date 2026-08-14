//! opi-ffi：引擎双 ABI 出口。
//! - api：引擎薄壳 + 共享单例（JNI / C 共用）
//! - jni_util：UTF-16 字符串转换（禁用 modified UTF-8）
//! - jni：JNI_OnLoad + RegisterNatives（宿主类 io/opi/input/jni/OpiEngine）
//! - cabi：C ABI（opi_* 函数，iOS M7 预留）

pub mod api;
pub mod cabi;
pub mod jni;
pub mod jni_util;
