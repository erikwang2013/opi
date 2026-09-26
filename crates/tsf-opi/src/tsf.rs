// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

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

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, Once};

use engine_core::composer::Mode;
use windows::core::{
    implement, ComObjectInterface, Interface, InterfaceRef, Ref, Result, BOOL, GUID, HRESULT,
    IUnknown,
};
use windows::Win32::Foundation::{LPARAM, S_OK, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, GetKeyboardLayout, GetKeyboardState, ToUnicodeEx, VK_CAPITAL, VK_CONTROL,
    VK_MENU, VK_SHIFT,
};
use windows::Win32::UI::TextServices::{
    ITfContext, ITfKeyEventSink, ITfKeyEventSink_Impl, ITfKeystrokeMgr, ITfTextInputProcessor,
    ITfTextInputProcessor_Impl, ITfThreadMgr,
};

use crate::logic::{
    KeyOutcome, TsfLogic, KEY_STATE_ALT, KEY_STATE_CAPS_LOCK, KEY_STATE_CTRL, KEY_STATE_RELEASED,
    KEY_STATE_REPEAT, KEY_STATE_SHIFT,
};
use crate::vk::vk_to_engine_keycode;

/// CLASS_E_CLASSNOTAVAILABLE（0x80040111）：骨架阶段不提供类工厂。
const CLASS_E_CLASSNOTAVAILABLE: HRESULT = HRESULT(0x80040111_u32 as i32);

/// E_FAIL：引擎内部错误的统一返回码（panic 被兜住 / 锁中毒）。
const E_FAIL: HRESULT = HRESULT(0x80004005_u32 as i32);

/// 一次性 panic hook（对照 fcitx5-opi/src/lib.rs 的 `ensure_panic_hook`）。
/// 每个 COM 出口都要调：换掉默认 hook 后，宿主进程的事件日志里能看到是引擎炸了。
fn ensure_panic_hook() {
    static PANIC_HOOK: Once = Once::new();
    PANIC_HOOK.call_once(|| {
        std::panic::set_hook(Box::new(|info| {
            eprintln!("tsf-opi: engine panic caught: {info}");
        }));
    });
}

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
    ///
    /// 这是个 `extern "system"` 的 COM 出口（经 vtable 被宿主输入法进程调用）：
    /// panic 逃出去 = 宿主进程 abort（in-proc，用户的 Word/浏览器）。
    /// 故与另三端（jni.rs / cabi.rs / fcitx5-opi）一致，出口一律兜住；
    /// 兜住后返回 BOOL(1)（吞键）——与中毒锁同策略：宁可丢一次键，不炸宿主。
    fn handle_key(&self, wparam: WPARAM, lparam: LPARAM) -> BOOL {
        ensure_panic_hook();
        catch_unwind(AssertUnwindSafe(|| self.handle_key_inner(wparam, lparam))).unwrap_or(BOOL(1))
    }

    fn handle_key_inner(&self, wparam: WPARAM, lparam: LPARAM) -> BOOL {
        let mut logic = match self.logic.lock() {
            Ok(g) => g,
            Err(_) => return BOOL(1), // 中毒锁：吞键，避免键流入应用造成死循环
        };
        let outcome = logic.input_key(to_engine_keycode(wparam.0 as u32), map_key_state(lparam));
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
        ensure_panic_hook();
        // COM 出口：panic 逃出去 = abort 宿主进程（见 handle_key）。兜住 → E_FAIL。
        catch_unwind(AssertUnwindSafe(|| {
            // 中毒锁：跳过状态保存，避免经 COM vtable 泄漏 panic
            let mut tm = match self.thread_mgr.lock() {
                Ok(g) => g,
                Err(_) => return Err(E_FAIL.into()),
            };
            *tm = ptim.cloned();
            // 注册按键监听：0.62 API 为 AdviseKeyEventSink（旧式 SetKeypressSink 已移除）。
            // fforeground=true：前台键盘事件也交本服务（输入法语义）。
            // 本对象同时实现 ITfKeyEventSink，as_interface_ref 取其 IUnknown 指针，
            // TSF 侧会 QueryInterface 到 ITfKeyEventSink。
            let km: ITfKeystrokeMgr = (*ptim).as_ref().ok_or_else(|| windows::core::Error::from_hresult(HRESULT(0x80070057_u32 as i32)))?.cast()?; // E_INVALIDARG：ptim 为空
            let sink: InterfaceRef<'_, IUnknown> = self.as_interface_ref();
            // cast = QueryInterface：对象支持 ITfKeyEventSink，取具体接口指针。
            let key_sink: ITfKeyEventSink = sink.cast()?;
            unsafe { km.AdviseKeyEventSink(tid, &key_sink, true) }
        }))
        .unwrap_or_else(|_| Err(E_FAIL.into()))
    }

    fn Deactivate(&self) -> Result<()> {
        ensure_panic_hook();
        catch_unwind(AssertUnwindSafe(|| {
            // 骨架：仅清状态；验收补全点：UnadviseKeyEventSink + 释放 composition/候选窗。
            let mut tm = match self.thread_mgr.lock() {
                Ok(g) => g,
                Err(_) => return Ok(()), // 中毒锁：吞掉，避免经 COM vtable 泄漏 panic
            };
            *tm = None;
            Ok(())
        }))
        .unwrap_or_else(|_| Err(E_FAIL.into()))
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
/// `ToUnicodeEx` 的 wFlags bit 2：本次调用不改变键盘状态。
/// 不加这个标志时，连续调用会在按下死键（重音等）时吞掉/推进死键状态，
/// 而那些键我们多半要放行给应用 —— 应用侧的组合键输入就会被吃掉。
const TO_UNICODE_NO_STATE_CHANGE: u32 = 0x4;

/// TSF 的 `wParam` 是**虚拟键码**，而 `TsfLogic::input_key` 的约定是
/// 「可打印字符 = Unicode 码点，特殊键 = VK 码」（见 logic.rs 头注释）。
/// 此前直接把 `wparam` 当码点透传，导致：
///   - 英文模式敲 'a' 得到 VK_A=0x41，被当成大写 'A' 提交 → "hello" 变 "HELLO"；
///   - 撇号 `'`（拼音分隔符）的 VK_OEM_7=0xDE 非 ASCII，永远 Unhandled，打不出来。
///
/// 拼音模式只是被 `Composer` 的「字母转小写」兜住才看起来正常，故一直未被发现。
///
/// 用 `ToUnicodeEx` 而非 `MapVirtualKeyW`：前者按**当前键盘布局**与修饰键状态
/// 换算，非 US 布局（AZERTY/Dvorak）下依然正确；后者只给未加修饰的字符，
/// 自行补 Shift/CapsLock 逻辑等于重造键盘布局。
fn to_engine_keycode(vk: u32) -> u32 {
    let mut key_state = [0u8; 256];
    let mut buf = [0u16; 8];
    // SAFETY: key_state 为 256 字节（Win32 要求的键盘状态数组大小）；
    // buf 长度经 cchbuff 如实传入，ToUnicodeEx 不会写出界。
    let n = unsafe {
        if GetKeyboardState(&mut key_state).is_err() {
            -1 // 取不到键盘状态：当"无映射"处理（判定与回退策略全在 vk_to_engine_keycode）
        } else {
            ToUnicodeEx(
                vk,
                0,
                &key_state,
                &mut buf,
                TO_UNICODE_NO_STATE_CHANGE,
                Some(GetKeyboardLayout(0)),
            )
        }
    };
    // 映射判定全在 crate::vk::vk_to_engine_keycode（主机可单测）：特殊键换成
    // 编码键码（SPECIAL_BASE|VK，查一次表即检测+转发）、有映射取码点、
    // 无映射/死键（n <= 0）返回放行哨兵而非 VK。
    vk_to_engine_keycode(vk, n, buf[0])
}

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
