// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! Windows 语言栏（ctfutb）按钮：把宠物「小欧」挂上语言栏。
//! 这是 TSF 在 Windows 上**唯一**能新增的图标位（候选窗与发行包图标在别处，
//! 见 README 的宠物清单）。
//!
//! 【验证边界，别读过头】本文件在本机（Linux）只能过
//! `cargo check --target x86_64-pc-windows-msvc`：它证明这些接口名/签名与
//! windows crate 0.62 相符、`implement!` 形态正确。**不证明**语言栏真的调了
//! `GetIcon`、更不证明图标长得对 —— 那要一台 Windows + 开着的语言栏。
//! 两端**本机可验证**的部分都被推到门外：挑条目/校验 RT_ICON 形态在
//! `langbar_icon.rs`（Linux 单测），ico 的产出在 `scripts/gen_pet_icons.py --check`。
//!
//! 【可见性前提，别当缺陷报】语言栏是 Win7 时代的 UI：ctfmon 起着、且在
//! 「设置 → 时间和语言 → 语言栏」里开着才有；Win11 默认没有这个界面
//! （只剩输入指示器，那是系统的，不读第三方图标）。所以"没看见小欧"的
//! **第一判据是语言栏在不在**，不是本文件坏了。
//!
//! 【为什么不用 .rc】`.rc` + 资源编译器能把图标嵌成 `.rsrc`（连带 Explorer 里的
//! DLL 文件图标），但本机与 CI 都**没有**资源编译器 ⇒ 那一步在任何自动门禁里都
//! 不可验证。改成 `include_bytes!` 的 ico + 运行期 `CreateIconFromResourceEx`：
//! 只剩"造 HICON"这一步要真机。代价如实记：**DLL 文件图标仍然没有**。

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;

use windows::Win32::Foundation::{E_INVALIDARG, E_POINTER, POINT, RECT};
use windows::Win32::UI::TextServices::{
    ITfLangBarItem, ITfLangBarItem_Impl, ITfLangBarItemButton, ITfLangBarItemButton_Impl,
    ITfLangBarItemMgr, ITfLangBarItemSink, ITfMenu, ITfSource, ITfSource_Impl, ITfThreadMgr,
    TF_LANGBARITEMINFO, TF_LBI_CLK_LEFT, TF_LBI_DESC_MAXLEN, TF_LBI_STYLE_BTN_BUTTON,
    TF_LBI_STYLE_SHOWNINTRAY, TfLBIClick,
};
use windows::Win32::UI::WindowsAndMessaging::{CreateIconFromResourceEx, HICON, LR_DEFAULTCOLOR};
use windows::core::{BOOL, GUID, IUnknown, Interface, Ref, Result, implement};

use crate::com_server::parse_guid;
use crate::dll::{CLSID_TEXT_SERVICE, DISPLAY_NAME, DllLock, GUID_PROFILE};
use crate::langbar_icon;
use crate::tsf::{E_FAIL, ensure_panic_hook};

/// 语言栏按钮上显示的文字（图标旁/悬停时另有 `GetTooltipString` 用全名）。
const LABEL: &str = "小欧输入法";

/// ITfSource 只认这一个 sink 类型（协议规定的那一个）。
/// 返回的 cookie 是常量：本对象只接受**一个** sink，不需要发号器。
const SINK_COOKIE: u32 = 1;

/// 每个 COM 出口的统一护栏（对照 `com_server.rs` 的 `guard`）：
/// panic 经 vtable 逃出去 = abort 宿主进程（用户的 Word/浏览器）。
/// 这些出口冷的很（点一下、画一次），护栏成本可忽略。
fn outlet<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    ensure_panic_hook();
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|_| Err(E_FAIL.into()))
}

/// 语言栏按钮。**故意不持任何引擎状态** —— 图标与名字都是静态的：
/// 挂上去之后不需要更新，接口面因此最小（3 个接口、11 个方法，全是常量答案）。
#[implement(ITfLangBarItem, ITfLangBarItemButton, ITfSource)]
pub struct LangBarItem {
    /// 语言栏在 `AddItem` 后按协议 QI 本对象的 `ITfSource` 并 `AdviseSink` 一个
    /// `ITfLangBarItemSink` 进来。静态图标**从不**推 `OnUpdate`，但接口必须实现
    /// 且接受这个 sink（拒绝 = QI/Advise 失败 = 按钮挂不上）。
    sink: Mutex<Option<IUnknown>>,
    /// 语言栏持着本对象 ⇒ 模块锁：只要它活着，`DllCanUnloadNow` 必须答 S_FALSE
    /// （理由见 `dll.rs` 的模块级计数）。
    _module_lock: DllLock,
}

impl LangBarItem {
    fn new() -> Self {
        Self {
            sink: Mutex::new(None),
            _module_lock: DllLock::new(),
        }
    }
}

/// `Activate` 时挂上语言栏。返回（管理器, 条目）**一对**：摘除要用管理器，
/// 保活要用条目接口 —— 调用方两样都得存着（见 `tsf.rs` 的字段注释）。
pub fn add(tm: &ITfThreadMgr) -> Result<(ITfLangBarItemMgr, ITfLangBarItem)> {
    outlet(|| {
        // ITfThreadMgr 上 QI 出管理器：这两个接口由同一个对象实现（TSF 的约定）。
        let mgr: ITfLangBarItemMgr = tm.cast()?;
        let item: ITfLangBarItem = LangBarItem::new().into();
        unsafe { mgr.AddItem(&item)? };
        Ok((mgr, item))
    })
}

/// `Deactivate` 时摘掉（与 [`add`] 配对）。失败无处可报（GUI 宿主无 stderr）：
/// 留在语言栏上的条目会持一份指向本 DLL 的 vtable ⇒ 不摘就**不能卸载**模块 ——
/// 这一点由 `DllLock` 兜住（本对象还活着 ⇒ `DllCanUnloadNow` 答 S_FALSE）。
pub fn remove(mgr: &ITfLangBarItemMgr, item: &ITfLangBarItem) {
    let _ = outlet(|| unsafe { mgr.RemoveItem(item) });
}

// ---------- ITfLangBarItem ----------

impl ITfLangBarItem_Impl for LangBarItem_Impl {
    /// 键名/样式都是常量。`guidItem` 复用语言 profile 的 GUID：本服务只有一个条目，
    /// 不需要第二个 GUID 来源（多一个就当多一个会漂的字面量）。
    ///
    /// ⚠️ `pinfo` 是裸指针但本方法**不能**标 `unsafe fn` —— 签名由 windows-rs 生成的
    /// `ITfLangBarItem_Impl` trait 定死。故按方法（**不**按 impl 块或模块）放行
    /// `not_unsafe_ptr_arg_deref`：块级 allow 会把将来新增的方法一起沉默掉。
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn GetInfo(&self, pinfo: *mut TF_LANGBARITEMINFO) -> Result<()> {
        outlet(|| {
            if pinfo.is_null() {
                return Err(E_POINTER.into());
            }
            let mut info = TF_LANGBARITEMINFO {
                clsidService: parse_guid(CLSID_TEXT_SERVICE)?,
                guidItem: parse_guid(GUID_PROFILE)?,
                // 按钮 + 停靠时也显示（语言栏缩进任务栏后只剩 SHOWNINTRAY 的条目；
                // 不给这个标记，浮动状态看得见、停靠状态就"消失"了）。
                dwStyle: TF_LBI_STYLE_BTN_BUTTON | TF_LBI_STYLE_SHOWNINTRAY,
                ulSort: 0,
                szDescription: [0; TF_LBI_DESC_MAXLEN as usize],
            };
            // 宽字符截断到 szDescription 的容量（TF_LBI_DESC_MAXLEN = 32）。
            for (slot, u) in info
                .szDescription
                .iter_mut()
                .zip(DISPLAY_NAME.encode_utf16())
            {
                *slot = u;
            }
            // SAFETY: pinfo 非空（上面判过）；调用方（语言栏）保证它指向可写内存。
            unsafe { pinfo.write(info) };
            Ok(())
        })
    }

    /// 0 = 既不隐藏也不禁用（TF_LBI_STATUS_* 是位标记，没有"正常值"这个枚举）。
    fn GetStatus(&self) -> Result<u32> {
        outlet(|| Ok(0))
    }

    /// 语言栏要求显示/隐藏：本按钮没有状态可切，照单全收（忽略）。
    fn Show(&self, _fshow: BOOL) -> Result<()> {
        outlet(|| Ok(()))
    }

    fn GetTooltipString(&self) -> Result<windows::core::BSTR> {
        outlet(|| Ok(DISPLAY_NAME.into()))
    }
}

// ---------- ITfLangBarItemButton ----------

impl ITfLangBarItemButton_Impl for LangBarItem_Impl {
    /// 左键 = 打开设置窗（与 Linux 状态栏「设」同一条动作，也同 `ITfFnConfigure::Show`）。
    /// 失败不弹 UI（设置 exe 找不到是常驻环境问题，弹框只会更烦）；右键留给语言栏菜单。
    fn OnClick(&self, click: TfLBIClick, _pt: &POINT, _prcarea: *const RECT) -> Result<()> {
        outlet(|| {
            if click == TF_LBI_CLK_LEFT {
                let _ = crate::configure::launch_settings();
            }
            Ok(())
        })
    }

    fn InitMenu(&self, _pmenu: Ref<ITfMenu>) -> Result<()> {
        outlet(|| Ok(()))
    }

    fn OnMenuSelect(&self, _wid: u32) -> Result<()> {
        outlet(|| Ok(()))
    }

    /// 语言栏要图标时**每次新造**一个 HICON（协议规定由**调用方** DestroyIcon，
    /// 缓存一个交出去就是双重释放）。字节来自编译期嵌入的 ico（见 `langbar_icon`）。
    fn GetIcon(&self) -> Result<HICON> {
        outlet(|| {
            // 32 = 库里最大的那条（高 DPI 下比 16 清晰；语言栏画的时候自己缩放）。
            let e = langbar_icon::entry(langbar_icon::ICO, 32)
                .ok_or_else(|| windows::core::Error::from_hresult(E_FAIL))?;
            // SAFETY: e.dib 是 include_bytes 的静态字节（'static）、形态已按
            // RT_ICON 校验过（BITMAPINFOHEADER + XOR + AND，见 langbar_icon 的单测）。
            unsafe {
                CreateIconFromResourceEx(
                    e.dib,
                    true,
                    0x0003_0000, // dwVer：图标资源版本 3.0（RT_ICON 的约定值）
                    e.w as i32,
                    e.h as i32,
                    LR_DEFAULTCOLOR,
                )
            }
        })
    }

    fn GetText(&self) -> Result<windows::core::BSTR> {
        outlet(|| Ok(LABEL.into()))
    }
}

// ---------- ITfSource ----------

impl ITfSource_Impl for LangBarItem_Impl {
    // 同 GetInfo 那条注记：签名由 trait 定死，按方法放行这一个 lint。
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn AdviseSink(&self, riid: *const GUID, punk: Ref<IUnknown>) -> Result<u32> {
        outlet(|| {
            if riid.is_null() {
                return Err(E_POINTER.into());
            }
            // SAFETY: riid 非空（上面判过），语言栏传的是合法 GUID。
            if unsafe { *riid } != ITfLangBarItemSink::IID {
                return Err(E_INVALIDARG.into());
            }
            let Some(punk) = punk.as_ref() else {
                return Err(E_POINTER.into());
            };
            match self.sink.lock() {
                Ok(mut slot) => *slot = Some(punk.clone()),
                Err(_) => return Err(E_FAIL.into()), // 中毒锁：不外泄
            }
            Ok(SINK_COOKIE)
        })
    }

    fn UnadviseSink(&self, dwcookie: u32) -> Result<()> {
        outlet(|| {
            if dwcookie != SINK_COOKIE {
                return Err(E_INVALIDARG.into());
            }
            match self.sink.lock() {
                Ok(mut slot) => *slot = None,
                Err(_) => return Err(E_FAIL.into()),
            }
            Ok(())
        })
    }
}
