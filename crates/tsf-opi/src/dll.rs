// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! DLL 服务器资料：CLSID/GUID 标识、注册表键路径、模块级对象计数。
//!
//! **本模块是平台中立的（无 windows 类型），这是有意的** —— 与 `vk.rs` 同理：
//! 判定若留在 `#[cfg(target_os = "windows")]` 的胶水里，本机（Linux）门禁就
//! 永远覆盖不到，而这里每个值错了都是**静默**失败（见 `dll_tests.rs`）。
//! 故 CLSID/GUID 以**字符串字面量**保存（`GUID::try_from(&str)` 在 Windows 侧
//! 解析同一份字面量），而不是 `windows_core::GUID` —— `windows-core` 是
//! target 作用域依赖，主机根本编译不到它（E0 定的隔离，Cargo.toml 有注释）。
//!
//! 键路径与注册项的依据（官方文档 + API 名，**本机无法验证注册效果**）：
//! - COM 在进程内定位本服务：`HKCR\CLSID\{CLSID}\InprocServer32`
//!   默认值 = DLL 全路径，`ThreadingModel` = `Apartment`（TSF 文本服务是 STA 对象）。
//! - TSF 自己的注册（语言 profile、类别）**不手写注册表**，改调官方 API：
//!   `ITfInputProcessorProfiles::Register` / `AddLanguageProfile`、
//!   `ITfCategoryMgr::RegisterCategory`（调用点见 `com_server.rs`）——
//!   让 TSF 自己写自己的键，就不存在"我记错键名"这一整类失败。

use core::sync::atomic::{AtomicIsize, Ordering};

/// 本服务的 CLSID，**不带花括号**的 36 字符形式（大写十六进制）。
/// 用 `uuidgen`（v4）生成；**一经发布不得更改** —— 改了就是换了个输入法，
/// 用户机器上的旧键仍在，TSF 会同时看到两个服务。
/// 注册表键名要带花括号，由 `clsid_key` 补（`GUID::try_from` 不收带括号的形式）。
pub const CLSID_TEXT_SERVICE: &str = "98C51858-590B-404D-8915-21EB44250062";

/// 语言配置文件（LanguageProfile）GUID。同一个 CLSID 可以有多个 profile
/// （不同语言/输入模式各一个）；本服务只有一个（zh-CN · 拼音）。
pub const GUID_PROFILE: &str = "A31A137C-1404-42F2-B82C-4E63F15DEFD6";

/// 语言 ID：zh-CN（0x0804）。TSF 的 profile 按 LANGID 分桶。
pub const LANGID_ZH_CN: u16 = 0x0804;

/// 服务显示名（注册表默认值 + 语言栏里显示的名字）。
pub const DISPLAY_NAME: &str = "OPI 拼音输入法";

/// `HKCR\CLSID\{...}` 键路径。注册表里的 CLSID 子键名**带**花括号，
/// 而 `CLSID_TEXT_SERVICE` 与 `GUID::try_from` 用的都是不带括号的形式 ——
/// 括号只在这里补一次。
pub fn clsid_key(clsid: &str) -> String {
    format!("CLSID\\{{{clsid}}}")
}

/// `HKCR\CLSID\{...}\InprocServer32` 键路径：COM 唯一的进程内定位依据。
pub fn inproc_server_key(clsid: &str) -> String {
    format!("{}\\InprocServer32", clsid_key(clsid))
}

// ---------- 模块级对象计数 ----------

/// 模块锁计数。**COM 的 `DllCanUnloadNow` 契约要求**：只要还有活动对象
/// （服务实例、类工厂），就必须回答 S_FALSE。
///
/// `windows-rs` 的 `#[implement]` **不给这个** —— 它只管对象自身的引用计数
/// （`Release` 归零时析构对象），不知道"模块里还有几个对象活着"。
/// 少了它，`DllCanUnloadNow` 一律答 S_OK，TSF/COM 会在我们仍持有对象时
/// 卸载 DLL，之后任何一次 vtable 调用都跳进已释放的内存 —— 崩在**用户**的
/// Word 里，不是我们的进程里。
static MODULE_LOCKS: AtomicIsize = AtomicIsize::new(0);

/// 当前模块锁计数（`dll_can_unload` 的原始量；测试用增量断言，见 dll_tests.rs）。
pub fn dll_lock_count() -> isize {
    MODULE_LOCKS.load(Ordering::Relaxed)
}

/// 无活动锁 → 可以安全卸载（对应 `DllCanUnloadNow` 的 S_OK，否则 S_FALSE）。
pub fn dll_can_unload() -> bool {
    dll_lock_count() == 0
}

/// 模块锁的 RAII 持有者：服务对象与类工厂各持一个，`Drop` 时自动归还。
/// 用类型而不是"记得在每个出口调一次 release"来保证配对 —— 漏减是
/// "DLL 永不卸载"（可接受），漏加才是崩宿主（不可接受），而 RAII 两件都不会。
pub struct DllLock(());

impl DllLock {
    pub fn new() -> Self {
        MODULE_LOCKS.fetch_add(1, Ordering::Relaxed);
        DllLock(())
    }
}

/// `new()` 无参 → clippy 要求配对给出 `Default`（`-D warnings` 下是硬门禁）。
/// 语义一致：`default()` 也**取一把锁**，不是"零值"。
impl Default for DllLock {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for DllLock {
    fn drop(&mut self) {
        MODULE_LOCKS.fetch_sub(1, Ordering::Relaxed);
    }
}

// 单测独立成文件（`#[path]` 引入）以保持各文件 <500 行：dll_tests.rs
#[cfg(test)]
#[path = "dll_tests.rs"]
mod tests;
