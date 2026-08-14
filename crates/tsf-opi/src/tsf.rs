//! C2：TSF（Text Services Framework）COM 胶水层（Windows 目标专属，
//! lib.rs 以 `#[cfg(target_os = "windows")]` 引入本模块；Linux 主机不编译）。
//!
//! 结构镜像 fcitx5 轨（crates/fcitx5-opi）：纯 Rust 逻辑层 + 平台胶水分离；
//! 本文件是 Windows 侧的胶水，完成 C1 契约的对接：
//!   - TSF 键事件 wParam/lParam → `TsfLogic::input_key` 的 keyval/KEY_STATE_*；
//!   - 按 `KeyOutcome` 分派：Commit → 提交接缝；CompositionChanged → 刷新接缝；
//!     Consumed → 吞键（BOOL TRUE）；Unhandled → 不拦截（BOOL FALSE，键流入应用）。
//!
//! 【骨架 vs 功能】本文件按"最小可编译骨架"编写（对照 windows-rs 0.62 TSF 示例）：
//!   [功能] ITfTextInputProcessor 生命周期、ITfKeyEventSink 按键转发与键码映射、
//!          KeyOutcome 分派、AdviseKeyEventSink 注册。
//!   [骨架] 文档操作（Commit 插入 / composition 刷新、候选窗 UI）经 `TsfSink`
//!          接缝暴露，真机验收时补全（ITfInsertAtSelection / ITfContextComposition）；
//!          DllGetClassObject 为占位导出（CLASS_E_CLASSNOTAVAILABLE），正式注册
//!          需在 Windows 上生成 CLSID + .rgs 注册脚本（本仓库尚无注册资料）。

use std::sync::Mutex;

use engine_core::composer::Mode;
use windows::core::{
    implement, ComObjectInterface, Interface, InterfaceRef, Ref, Result, BOOL, GUID, HRESULT,
    IUnknown,
};
use windows::Win32::Foundation::{LPARAM, S_OK, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VK_CAPITAL, VK_CONTROL, VK_MENU, VK_SHIFT,
};
use windows::Win32::UI::TextServices::{
    ITfContext, ITfKeyEventSink, ITfKeyEventSink_Impl, ITfKeystrokeMgr, ITfTextInputProcessor,
    ITfTextInputProcessor_Impl, ITfThreadMgr,
};

use crate::logic::{
    KeyOutcome, TsfLogic, KEY_STATE_ALT, KEY_STATE_CAPS_LOCK, KEY_STATE_CTRL,
    KEY_STATE_RELEASED, KEY_STATE_REPEAT, KEY_STATE_SHIFT,
};

/// CLASS_E_CLASSNOTAVAILABLE（0x80040111）：骨架阶段不提供类工厂。
const CLASS_E_CLASSNOTAVAILABLE: HRESULT = HRESULT(0x80040111_u32 as i32);

/// 文档/候选窗接缝：C2 骨架不实现 TSF 文档操作，真机验收在此补全。
/// 语义（对照 C1 契约）：Commit → ITfInsertAtSelection 插入或 composition 提交；
/// CompositionChanged → ITfContextComposition 刷新 composition + 候选窗 UI。
/// 候选窗（C3 的 CMP 窗口）从候选数据刷新，无状态变化时不回调。
pub trait TsfSink: Send + Sync {
    /// 立即提交文本（英文直传、空缓冲空格、缓冲/候选提交等）。
    fn on_commit(&self, _text: &str) {}
    /// composition/候选窗需刷新。
    fn on_composition_changed(
        &self,
        _buffer: &str,
        _candidates: &[String],
        _page: usize,
        _page_count: usize,
        _mode: Mode,
    ) {}
}

/// TSF 服务对象：单对象实现两个 COM 接口，避免跨接口共享状态。
/// - `ITfTextInputProcessor`：TSF 核心入口（Activate/Deactivate 生命周期）。
/// - `ITfKeyEventSink`：按键转发（Activate 中经 ITfKeystrokeMgr::AdviseKeyEventSink 注册）。
#[implement(ITfTextInputProcessor, ITfKeyEventSink)]
pub struct TsfTextService {
    /// 引擎 + 候选分页 + ⇧ 状态机（C1 逻辑层）。
    pub logic: Mutex<TsfLogic>,
    /// Activate 时保存的线程管理器（骨架：仅持有引用，候选窗/文档操作需要）。
    pub thread_mgr: Mutex<Option<ITfThreadMgr>>,
    /// 文档/候选窗接缝（见 `TsfSink`）。
    pub sink: Box<dyn TsfSink>,
}

impl TsfTextService {
    /// 装载逻辑层（`TsfLogic::load`，失败用内置回退词库语义由逻辑层处理）。
    pub fn new(path: Option<&str>, sink: Box<dyn TsfSink>) -> Result<Self> {
        // E_FAIL：词库装载失败（坏路径）→ 服务不可用。
        let logic = TsfLogic::load(path).map_err(|_| HRESULT(0x80004005_u32 as i32))?;
        Ok(Self {
            logic: Mutex::new(logic),
            thread_mgr: Mutex::new(None),
            sink,
        })
    }

    /// 键事件统一入口（OnKeyDown/OnKeyUp 共用）。
    /// wParam = VK 或 Unicode 码点（见 logic.rs 头注释的键码约定）；
    /// lParam 位映射 KEY_STATE_*；KeyOutcome 分派见模块头注释。
    fn handle_key(&self, wparam: WPARAM, lparam: LPARAM) -> BOOL {
        let mut logic = match self.logic.lock() {
            Ok(g) => g,
            Err(_) => return BOOL(1), // 中毒锁：吞键，避免键流入应用造成死循环
        };
        let outcome = logic.input_key(wparam.0 as u32, map_key_state(lparam));
        match outcome {
            KeyOutcome::Commit(text) => {
                self.sink.on_commit(&text);
                BOOL(1)
            }
            KeyOutcome::CompositionChanged => {
                let candidates = logic.candidates();
                self.sink.on_composition_changed(
                    &logic.buffer(),
                    &candidates,
                    logic.page(),
                    logic.page_count(),
                    logic.mode(),
                );
                BOOL(1)
            }
            KeyOutcome::Consumed => BOOL(1),
            KeyOutcome::Unhandled => BOOL(0), // 不拦截，键自然流入应用
        }
    }
}

// ---------- ITfTextInputProcessor：TSF 生命周期 ----------
// 注：0.62 的 #[implement] 生成 `TsfTextService_Impl` 包装（Deref 到原结构），
// `_Impl` trait 实现在包装类型上；字段经 Deref 访问（self.logic 等）。

impl ITfTextInputProcessor_Impl for TsfTextService_Impl {
    fn Activate(&self, ptim: Ref<ITfThreadMgr>, tid: u32) -> Result<()> {
        *self.thread_mgr.lock().unwrap() = ptim.cloned();
        // 注册按键监听：0.62 API 为 AdviseKeyEventSink（旧式 SetKeypressSink 已移除）。
        // fforeground=true：前台键盘事件也交本服务（输入法语义）。
        // 本对象同时实现 ITfKeyEventSink，as_interface_ref 取其 IUnknown 指针，
        // TSF 侧会 QueryInterface 到 ITfKeyEventSink。
        let km: ITfKeystrokeMgr = (*ptim).as_ref().ok_or_else(|| windows::core::Error::from_hresult(HRESULT(0x80070057_u32 as i32)))?.cast()?; // E_INVALIDARG：ptim 为空
        let sink: InterfaceRef<'_, IUnknown> = self.as_interface_ref();
        // cast = QueryInterface：对象支持 ITfKeyEventSink，取具体接口指针。
        let key_sink: ITfKeyEventSink = sink.cast()?;
        unsafe { km.AdviseKeyEventSink(tid, &key_sink, true) }
    }

    fn Deactivate(&self) -> Result<()> {
        // 骨架：仅清状态；验收补全点：UnadviseKeyEventSink + 释放 composition/候选窗。
        *self.thread_mgr.lock().unwrap() = None;
        Ok(())
    }
}

// ---------- ITfKeyEventSink：按键转发 ----------

impl ITfKeyEventSink_Impl for TsfTextService_Impl {
    fn OnSetFocus(&self, _fforeground: BOOL) -> Result<()> {
        Ok(())
    }

    fn OnTestKeyDown(&self, _pic: Ref<ITfContext>, _wparam: WPARAM, _lparam: LPARAM) -> Result<BOOL> {
        // 测试阶段不认领：让系统走正常 OnKeyDown 路径。
        Ok(BOOL(0))
    }

    fn OnTestKeyUp(&self, _pic: Ref<ITfContext>, _wparam: WPARAM, _lparam: LPARAM) -> Result<BOOL> {
        Ok(BOOL(0))
    }

    fn OnKeyDown(&self, _pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        Ok(self.handle_key(wparam, lparam))
    }

    fn OnKeyUp(&self, _pic: Ref<ITfContext>, wparam: WPARAM, lparam: LPARAM) -> Result<BOOL> {
        // 释放状态由 lParam bit31（转换状态）映射，与按下同路由
        // （logic 对释放事件返回 Consumed/Unhandled，见 input_key）。
        Ok(self.handle_key(wparam, lparam))
    }

    fn OnPreservedKey(&self, _pic: Ref<ITfContext>, _rguid: *const GUID) -> Result<BOOL> {
        Ok(BOOL(0))
    }
}

/// lParam 键状态位 → logic 的 KEY_STATE_*（位约定见 logic.rs 头注释）。
/// TSF lParam：bit30 = 按下前状态（1 = 重复），bit31 = 转换状态（1 = 释放）；
/// 修饰键（⇧/Ctrl/Alt/CapsLock）TSF 键事件不携带，用 GetKeyState 移位查询。
fn map_key_state(lparam: LPARAM) -> u32 {
    let lp = lparam.0 as u32;
    let mut s = (lp >> 3) & KEY_STATE_REPEAT; // bit30 → 1<<27
    s |= (lp >> 5) & KEY_STATE_RELEASED; // bit31 → 1<<26
    // GetKeyState 返回 i16：高位为 1 = 按下（负数）。
    if unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0 {
        s |= KEY_STATE_SHIFT;
    }
    if unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0 {
        s |= KEY_STATE_CTRL;
    }
    if unsafe { GetKeyState(VK_MENU.0 as i32) } < 0 {
        s |= KEY_STATE_ALT;
    }
    if unsafe { GetKeyState(VK_CAPITAL.0 as i32) } & 1 != 0 {
        s |= KEY_STATE_CAPS_LOCK;
    }
    s
}

// ---------- COM 服务器导出（regsvr32 注册用；骨架占位） ----------

/// DllGetClassObject：TSF 经注册表 CLSID 定位本服务 DLL。
/// 骨架：返回 CLASS_E_CLASSNOTAVAILABLE。验收补全点：实现 IClassFactory
/// 返回 `TsfTextService`，并生成 CLSID + .rgs 注册脚本（本仓库尚无注册资料；
/// 与 fcitx5 轨的 C 导出 `#[unsafe(no_mangle)]` 同构，调用约定为 system）。
#[unsafe(no_mangle)]
pub extern "system" fn DllGetClassObject(
    _rclsid: *const GUID,
    _riid: *const GUID,
    _ppv: *mut *mut core::ffi::c_void,
) -> HRESULT {
    CLASS_E_CLASSNOTAVAILABLE
}

/// DllCanUnloadNow：骨架实现 —— 无锁驻留，恒可卸载。
#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    S_OK
}
