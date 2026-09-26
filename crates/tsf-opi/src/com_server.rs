// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! COM 服务器面（Windows 目标专属）：类工厂 + 四个 `Dll*` 导出 + 注册/注销。
//!
//! 【本机（Linux）无法验证的部分，别当成已验】DLL 链接需要 MSVC/SDK 导入库
//! （本机全盘零命中），注册效果需要 Windows + regsvr32 + 一个收字的宿主
//! （Word/记事本）。本机天花板是 `cargo check --target x86_64-pc-windows-msvc`：
//! 它证明**这些 API 名与签名与 windows crate 0.62 相符**（fcitx5 轨那 7 处 API
//! 误写就是没人编译过才长期存活），不证明注册能成、更不证明能打字。
//!
//! 【注册写什么】见 `register()` 的两段注释：COM 那一段手写注册表（全文件唯一
//! 手写的部分），TSF 那一段调官方 API 让 TSF 自己写。CLSID/GUID 字面量与键路径
//! 拼法都在 `dll.rs`，并有主机单测钉住（`dll_tests.rs`）。

use core::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Mutex;

use windows::Win32::Foundation::{
    CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_FAIL, E_POINTER, ERROR_SUCCESS, HMODULE,
    S_FALSE, S_OK, WIN32_ERROR,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize, IClassFactory, IClassFactory_Impl,
};
use windows::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
    GetModuleFileNameW, GetModuleHandleExW,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CLASSES_ROOT, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
    RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW,
};
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, CLSID_TF_InputProcessorProfiles, GUID_TFCAT_TIP_KEYBOARD, ITfCategoryMgr,
    ITfInputProcessorProfiles,
};
use windows::core::{BOOL, GUID, HRESULT, IUnknown, Interface, PCWSTR, Ref, Result, implement};

use crate::candidate_io::CandidateSink;
use crate::dll::{
    CLSID_TEXT_SERVICE, DISPLAY_NAME, DllLock, GUID_PROFILE, LANGID_ZH_CN, clsid_key,
    dll_can_unload, inproc_server_key,
};
use crate::tsf::{TsfTextService, ensure_panic_hook};

/// 每个 COM 出口（vtable 方法 + `Dll*` 导出）都要经过这里。
///
/// panic 逃出 `extern "system"` = abort 宿主进程 —— 我们是 in-proc，宿主就是
/// 用户的 Word/浏览器。与 `tsf.rs` / `fcitx5-opi` / `jni.rs` / `cabi.rs` 同一策略：
/// 兜住并转成 E_FAIL（调用方看到"失败"，而不是宿主消失）。
fn guard(f: impl FnOnce() -> Result<()>) -> Result<()> {
    ensure_panic_hook();
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|_| Err(E_FAIL.into()))
}

/// `Result` 形态的出口 → `HRESULT` 形态（`Dll*` 导出的返回类型）。
fn com_exit(f: impl FnOnce() -> Result<()>) -> HRESULT {
    guard(f).map_or_else(|e| e.code(), |()| S_OK)
}

/// 把 `dll.rs` 里的 GUID 字面量解析成 `GUID`。
///
/// 只有一个来源（那 36 字符的字面量）：注册表键名由它拼、`GUID` 由它解析，
/// 不存在"两处各写一遍 GUID 然后慢慢漂移"。形状由主机测试
/// `guid_literals_have_parseable_shape` 钉住 —— 解析失败只可能是有人改错了
/// 字面量（编译得过、单测会红），此处返回 E_FAIL 而不是猜一个。
fn parse_guid(s: &str) -> Result<GUID> {
    GUID::try_from(s).map_err(|_| E_FAIL.into())
}

/// `WIN32_ERROR`（注册表 API 的返回）→ `Result`：0 = `ERROR_SUCCESS`。
/// windows 0.62 里 `WIN32_ERROR` 是 newtype 且**没有** `ok()`（那是 `BOOL` /
/// `HRESULT` 才有的），故自己比 0，再用 `HRESULT::from_win32` 换算错误码。
fn win32_ok(err: WIN32_ERROR) -> Result<()> {
    if err == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(windows::core::Error::from_hresult(HRESULT::from_win32(
            err.0,
        )))
    }
}

/// 已打开的注册表键；`Drop` 里 `RegCloseKey`。
/// 与 COM 引用计数同一类问题：漏关就是句柄泄漏。这里的句柄只活在一次 regsvr32
/// 调用内，RAII 保证比"记得在每个出口关一次"可靠。
struct RegKey(HKEY);

impl Drop for RegKey {
    fn drop(&mut self) {
        // SAFETY: self.0 由 RegCreateKeyExW 成功返回，且未被别处关闭。
        // 返回值（WIN32_ERROR）是 must_use 但这里无处置可言：键已经在关，
        // 关不掉也没有"再关一次"以外的补救。
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

/// 创建（或打开已存在的）注册表键。
fn create_key(root: HKEY, subkey: &str) -> Result<RegKey> {
    let wide: Vec<u16> = subkey.encode_utf16().chain([0]).collect();
    let mut hkey = HKEY(core::ptr::null_mut());
    // SAFETY: wide 是 NUL 结尾的 UTF-16，在本次调用期间存活；hkey 是有效的输出
    // 位置；lpclass/lpsecurityattributes/lpdwdisposition 传 None 即 NULL，
    // 是文档允许的。
    let err = unsafe {
        RegCreateKeyExW(
            root,
            PCWSTR(wide.as_ptr()),
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE, // = KEY_CREATE_SUB_KEY | KEY_SET_VALUE，够建子键和写值
            None,
            &mut hkey,
            None,
        )
    };
    win32_ok(err)?;
    Ok(RegKey(hkey))
}

/// 写一个 REG_SZ 值。`name` 为 `None` → 写键的**默认值**。
fn set_string(hkey: HKEY, name: Option<&str>, value: &str) -> Result<()> {
    let name_wide: Vec<u16> = name.unwrap_or("").encode_utf16().chain([0]).collect();
    // REG_SZ 的字节长度**含**结尾 NUL —— 注册表里存的就是带 NUL 的那一份。
    let value_wide: Vec<u16> = value.encode_utf16().chain([0]).collect();
    let name_ptr = if name.is_some() {
        PCWSTR(name_wide.as_ptr())
    } else {
        PCWSTR::null() // NULL = 默认值
    };
    // SAFETY: name_ptr 指向本次活动、NUL 结尾的 UTF-16（或 NULL）；lpdata 的字节
    // 切片与 value_wide 同生命周期，cbdata 按**字节**数传（不是元素数）。
    let err = unsafe {
        RegSetValueExW(
            hkey,
            name_ptr,
            None,
            REG_SZ,
            Some(core::slice::from_raw_parts(
                value_wide.as_ptr().cast::<u8>(),
                value_wide.len() * size_of::<u16>(),
            )),
        )
    };
    win32_ok(err)
}

// ---------- 类工厂 ----------

/// COM 的类工厂：`DllGetClassObject` 返回它，它再按需造服务对象。
#[implement(IClassFactory)]
struct ClassFactory {
    /// 工厂自己占一把模块锁：COM 拿得到工厂，就说明还要用它创建对象。
    /// 有了这一把，即使 `LockServer` 的记账与 COM 的账簿对不上，DLL 也不会在
    /// 工厂还活着时被卸载（`dll.rs` 讲的正是这个崩溃场景）。
    _self_lock: DllLock,
    /// `LockServer(true)` 的锁；`LockServer(false)` 归还。
    server_lock: Mutex<Option<DllLock>>,
}

impl ClassFactory {
    fn new() -> Self {
        Self {
            _self_lock: DllLock::new(),
            server_lock: Mutex::new(None),
        }
    }
}

impl IClassFactory_Impl for ClassFactory_Impl {
    /// `riid`/`ppvobject` 是 **COM 传来的裸指针**，windows-rs 不替我们判空，
    /// 所以这一层自己判；失败路径上 `*ppvobject` 必须先置 NULL —— 宿主会检查它。
    fn CreateInstance(
        &self,
        punkouter: Ref<IUnknown>,
        riid: *const GUID,
        ppvobject: *mut *mut c_void,
    ) -> Result<()> {
        guard(|| {
            // 聚合不支持：本服务不设计成被别的 COM 对象包在里面（TSF 传 NULL）。
            // 必须答 CLASS_E_NOAGGREGATION（不是 E_FAIL）：COM 规定的那一个码。
            if !punkouter.is_null() {
                return Err(CLASS_E_NOAGGREGATION.into());
            }
            if riid.is_null() || ppvobject.is_null() {
                return Err(E_POINTER.into());
            }
            // SAFETY: ppvobject 非空（上面判过）。
            unsafe { *ppvobject = core::ptr::null_mut() };

            // 词库路径传 None → 内置回退词库。**不能**先传一个可能不存在的路径：
            // `TsfLogic::load` 对坏路径返回 Err（既定策略：坏路径不静默回退），
            // 服务就创建不出来 —— 用户看到"输入法整个不在"，比词库小更糟。
            // Windows 词库分发方案定了之后在这里给路径。
            // sink 用 CandidateSink::new_default()（C3 的生产实现）：惰性连
            // named pipe，候选窗进程不在时降级为 no-op，不会让 CreateInstance 失败。
            let service = TsfTextService::new(None, Box::new(CandidateSink::new_default()))?;
            let unknown: IUnknown = service.into();
            // SAFETY: riid 指向 COM 给的合法 GUID（非空已判），ppvobject 是可写
            // 位置；unknown 在本次调用期间存活，query 会为调用方 AddRef 一份。
            let hr = unsafe { unknown.query(riid, ppvobject) };
            hr.ok()
        })
    }

    fn LockServer(&self, flock: BOOL) -> Result<()> {
        guard(|| {
            let mut slot = match self.server_lock.lock() {
                Ok(g) => g,
                Err(_) => return Err(E_FAIL.into()), // 中毒锁：别经 COM vtable 泄漏 panic
            };
            // 幂等赋值而非计数：COM 说要配对调用，重复调用是它的错。就算记错了，
            // `_self_lock` 仍保证工厂活着时 DLL 不会被卸载。
            *slot = if flock.as_bool() {
                Some(DllLock::new())
            } else {
                None
            };
            Ok(())
        })
    }
}

// ---------- 四个导出 ----------

/// `DllGetClassObject`：COM 按注册表里的 CLSID 找到本 DLL 后调它取类工厂。
///
/// `#[unsafe(no_mangle)]` + `extern "system"` 缺一不可：名字被装饰或调用约定
/// 不对，宿主看到的就是"导出不存在"，与没写这个函数没区别。
#[unsafe(no_mangle)]
pub extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    com_exit(|| {
        if rclsid.is_null() || riid.is_null() || ppv.is_null() {
            return Err(E_POINTER.into());
        }
        // SAFETY: rclsid 非空且由 COM 保证指向合法 GUID。
        if unsafe { *rclsid } != parse_guid(CLSID_TEXT_SERVICE)? {
            // **必须**是 CLASS_E_CLASSNOTAVAILABLE（不是 E_FAIL）：这是 COM 规定的
            // "我这里没有这个类"，宿主据此继续往别处找；E_FAIL 会被当成真错误。
            return Err(CLASS_E_CLASSNOTAVAILABLE.into());
        }
        let factory: IClassFactory = ClassFactory::new().into();
        // SAFETY: ppv 非空（上面判过）；factory 在本次调用期间存活。
        unsafe { *ppv = core::ptr::null_mut() };
        let hr = unsafe { factory.query(riid, ppv) };
        hr.ok() // "QueryInterface 失败"当普通错误往上抛，由 com_exit 转成 HRESULT
    })
}

/// `DllCanUnloadNow`：`S_OK` = 可以卸载，`S_FALSE` = 还有活动对象。
///
/// 这里**必须是真判断**。此前是恒 `S_OK` 的占位 —— 在没有真对象时无害，一旦
/// `DllGetClassObject` 开始发对象就是崩溃：COM 会在我们还持有对象时卸载 DLL，
/// 之后任何一次 vtable 调用都跳进已释放的内存（崩在宿主进程里）。
/// 计数与依据见 `dll.rs`。不套 `guard`：读一个原子变量，无分配无锁，panic 不了。
#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    if dll_can_unload() { S_OK } else { S_FALSE }
}

/// `DllRegisterServer`：`regsvr32 opi_tsf.dll` 调它。
///
/// **需要管理员权限**：TSF 侧注册写在 `HKLM\SOFTWARE\Microsoft\CTF` 下。非管理员
/// 跑 regsvr32 时 `HKCR` 的写入会被静默重定向到 HKCU（COM 部分"看似成功"），
/// 而 TSF 那几个 API 会失败 —— 于是整体返回失败、regsvr32 报错。这是有意的：
/// 宁可响亮地失败，也不要"注册成功但输入法不出现"。
#[unsafe(no_mangle)]
pub extern "system" fn DllRegisterServer() -> HRESULT {
    com_exit(register)
}

/// `DllUnregisterServer`：`regsvr32 /u opi_tsf.dll` 调它。与 `register` 逆序。
#[unsafe(no_mangle)]
pub extern "system" fn DllUnregisterServer() -> HRESULT {
    com_exit(unregister)
}

/// 本线程的 COM 初始化守卫。
///
/// `CoCreateInstance` 在未初始化的线程上返回 `CO_E_NOTINITIALIZED`
/// （0x800401F0），而 regsvr32 未必替我们初始化。两种返回要分开看：
/// `S_OK` = 这次是我们初始化的 → 退出时必须 `CoUninitialize`；
/// `S_FALSE` / `RPC_E_CHANGED_MODE` = 本来就初始化过 —— COM 已可用，
/// 而**不能再**调 `CoUninitialize`（那会抵消掉别人的初始化计数）。
struct ComGuard(bool);

impl ComGuard {
    fn new() -> Self {
        // SAFETY: 在未初始化的线程上调 CoInitializeEx 合法；pvReserved 必须为 NULL
        // （传 None），这是文档要求。
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        Self(hr == S_OK)
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: 仅当上面返回 S_OK（本线程的初始化归我们一次）时配对调用。
            unsafe { CoUninitialize() }
        }
    }
}

/// 本 DLL 的全路径 —— 写进 `InprocServer32` 默认值的那一份。
///
/// **不能用 `GetModuleFileNameW(None, …)`**：那返回的是**宿主 EXE** 的路径
/// （regsvr32.exe）。写进注册表等于让 COM 去加载宿主程序 —— regsvr32 报成功，
/// 输入法却永远出现不了。要拿自己的 HMODULE 只有"按地址反查"这一招：传本模块内
/// 一个函数的地址 + `FROM_ADDRESS`。`UNCHANGED_REFCOUNT` = 只查不加模块引用计数。
fn own_module_path() -> Result<String> {
    let mut hmod = HMODULE::default();
    // SAFETY: DllRegisterServer 是本 DLL 内的函数，其地址必然落在本模块内；
    // hmod 是有效的输出位置。函数项先转函数指针再转数据指针。
    unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            PCWSTR(DllRegisterServer as extern "system" fn() -> HRESULT as *const u16),
            &mut hmod,
        )?;
    }
    // 32768 = Win32 的路径长度上限（含结尾 NUL），远超任何实际 DLL 路径。
    let mut buf = vec![0u16; 32768];
    let n = unsafe { GetModuleFileNameW(Some(hmod), &mut buf) };
    // n == 0 是失败；n == buf.len() 是"被截断"（Win32 的返回语义）—— 截断的路径
    // 写进注册表会让 COM 加载不到东西，所以也当失败，不静默接受。
    if n == 0 || n as usize >= buf.len() {
        return Err(E_FAIL.into());
    }
    Ok(String::from_utf16_lossy(&buf[..n as usize]))
}

/// Rust `&str` → NUL 结尾的 UTF-16（Win32 的 W 系 API 要的形态）。
fn wide_z(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

fn register() -> Result<()> {
    let _com = ComGuard::new(); // CoCreateInstance 前必须有
    let clsid = parse_guid(CLSID_TEXT_SERVICE)?;
    let profile = parse_guid(GUID_PROFILE)?;
    let dll_path = own_module_path()?;

    // ① COM 注册（本文件里**唯一**手写的注册表部分）。
    //    依据：进程内 COM 服务器的标准注册 ——
    //      HKCR\CLSID\{CLSID}                 默认值 = 服务显示名
    //      HKCR\CLSID\{CLSID}\InprocServer32  默认值 = DLL 全路径
    //                                         ThreadingModel = "Apartment"
    //    ThreadingModel 必须非空：TSF 从 UI 线程调用我们的 vtable，而本对象不是
    //    自由线程对象（内部是 Mutex + COM 接口，按 STA 使用）。缺这个值时进程内
    //    服务器会被 COM 隐式当成 Apartment（旧行为），而"隐式"正是出问题的来源。
    let hkey = create_key(HKEY_CLASSES_ROOT, &clsid_key(CLSID_TEXT_SERVICE))?;
    set_string(hkey.0, None, DISPLAY_NAME)?;
    let hkey_inproc = create_key(HKEY_CLASSES_ROOT, &inproc_server_key(CLSID_TEXT_SERVICE))?;
    set_string(hkey_inproc.0, None, &dll_path)?;
    set_string(hkey_inproc.0, Some("ThreadingModel"), "Apartment")?;

    // ② TSF 侧的注册：**不手写** `HKLM\SOFTWARE\Microsoft\CTF\...`，改调 TSF 官方
    //    API 让它自己写（ITfInputProcessorProfiles::Register / AddLanguageProfile、
    //    ITfCategoryMgr::RegisterCategory）—— "我把键名/键路径记错了"这一整类失败
    //    就不存在了。两个 CLSID_TF_* 与类别 GUID 都取自 windows crate 的常量
    //    （不是手抄的 GUID 字面量）。
    // SAFETY: 每个调用都在活动 COM 套间内（ComGuard）；栈上值的生命周期覆盖调用。
    unsafe {
        let profiles: ITfInputProcessorProfiles = CoCreateInstance(
            &CLSID_TF_InputProcessorProfiles,
            None::<&IUnknown>,
            CLSCTX_INPROC_SERVER,
        )?;
        profiles.Register(&clsid)?;
        // 描述串按**计数**传（TSF 全线计数串），不要结尾 NUL；空切片 = 无自定义图标。
        let desc: Vec<u16> = DISPLAY_NAME.encode_utf16().collect();
        profiles.AddLanguageProfile(&clsid, LANGID_ZH_CN, &profile, &desc, &[], 0)?;

        let cats: ITfCategoryMgr = CoCreateInstance(
            &CLSID_TF_CategoryMgr,
            None::<&IUnknown>,
            CLSCTX_INPROC_SERVER,
        )?;
        // 类别 = "键盘输入法"。三个参数分别是：本服务 CLSID、类别 GUID、
        // 该类目下的条目 GUID —— 单一服务时条目就是自己。
        cats.RegisterCategory(&clsid, &GUID_TFCAT_TIP_KEYBOARD, &clsid)?;
    }
    Ok(())
}

fn unregister() -> Result<()> {
    let _com = ComGuard::new();
    let clsid = parse_guid(CLSID_TEXT_SERVICE)?;
    let profile = parse_guid(GUID_PROFILE)?;

    // 逆序：先摘 TSF 侧（TSF 找不到 profile 就不会再激活我们），再删 COM 键。
    // 反过来的话，中间态是"COM 键没了但 TSF 还认为有个服务" → 宿主激活时报错。
    // SAFETY: 同 register；两侧都是"尽力而为"的删除，失败即返回，不做清扫。
    unsafe {
        if let Ok(cats) = CoCreateInstance::<_, ITfCategoryMgr>(
            &CLSID_TF_CategoryMgr,
            None::<&IUnknown>,
            CLSCTX_INPROC_SERVER,
        ) {
            cats.UnregisterCategory(&clsid, &GUID_TFCAT_TIP_KEYBOARD, &clsid)?;
        }
        if let Ok(profiles) = CoCreateInstance::<_, ITfInputProcessorProfiles>(
            &CLSID_TF_InputProcessorProfiles,
            None::<&IUnknown>,
            CLSCTX_INPROC_SERVER,
        ) {
            profiles.RemoveLanguageProfile(&clsid, LANGID_ZH_CN, &profile)?;
            profiles.Unregister(&clsid)?;
        }
    }
    // RegDeleteTree 连子键一起删（InprocServer32 是子键，只删父键会失败）。
    // 失败不报错：本来就不存在（没注册过就 /u）不是错误。
    let key = wide_z(&clsid_key(CLSID_TEXT_SERVICE));
    let _ = unsafe { RegDeleteTreeW(HKEY_CLASSES_ROOT, PCWSTR(key.as_ptr())) };
    Ok(())
}
