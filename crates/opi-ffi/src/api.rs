//! frb API 薄壳：类型转换与边界校验，内部持 engine_core::Engine。

use engine_core::Engine;

/// 引擎句柄（frb opaque，Dart 侧为 Api 类实例）。
#[allow(dead_code)]
pub struct Api {
    engine: Engine,
}
