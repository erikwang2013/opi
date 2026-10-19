// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `regsvr32` 的注册/注销：手写 COM 键 + TSF 官方 API 的 profile 注册。
//!
//! 与 `com_server.rs` 分开只为守住 500 行：这里是"写什么键、调哪些 TSF API"，
//! 那边是类工厂与 `Dll*` 导出。验证边界同 `com_server`：Linux 本机只能证明
//! API 名/签名与 windows crate 0.62 相符，注册效果要 Windows 真机。

use windows::Win32::Foundation::{ERROR_SUCCESS, S_FALSE, S_OK, WIN32_ERROR};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CLASSES_ROOT, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE, REG_DWORD,
    REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegOpenKeyExW,
    RegQueryValueExW, RegSetValueExW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::HKL;
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, CLSID_TF_InputProcessorProfiles, GUID_TFCAT_TIP_KEYBOARD, ITfCategoryMgr,
    ITfInputProcessorProfileMgr, ITfInputProcessorProfiles, TF_E_ALREADY_EXISTS,
    TF_INPUTPROCESSORPROFILE, TF_PROFILETYPE_INPUTPROCESSOR,
};
use windows::core::{GUID, HRESULT, IUnknown, PCWSTR, Result};

use crate::com_server::{own_module_path, parse_guid};
use crate::dll::{
    CLSID_TEXT_SERVICE, DISPLAY_NAME, GUID_PROFILE, LANGID_ZH_CN, clsid_key, inproc_server_key,
};

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

/// 读一个 REG_DWORD；键/值不存在或类型不符一律 `None`。
fn read_dword(root: HKEY, subkey: &str, name: &str) -> Option<u32> {
    let sub = wide_z(subkey);
    let mut hkey = HKEY(core::ptr::null_mut());
    // SAFETY: sub 是 NUL 结尾 UTF-16；hkey 是有效的输出位置。
    let err = unsafe { RegOpenKeyExW(root, PCWSTR(sub.as_ptr()), None, KEY_READ, &mut hkey) };
    if err != ERROR_SUCCESS {
        return None;
    }
    let key = RegKey(hkey);
    let name = wide_z(name);
    let mut data = 0u32;
    let mut size = size_of::<u32>() as u32;
    let mut value_type = REG_DWORD;
    // SAFETY: data/size/type 都是有效的输出位置；保留参数传 None 即 NULL。
    let err = unsafe {
        RegQueryValueExW(
            key.0,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut value_type),
            Some((&mut data as *mut u32).cast::<u8>()),
            Some(&mut size),
        )
    };
    (err == ERROR_SUCCESS && value_type == REG_DWORD && size == size_of::<u32>() as u32)
        .then_some(data)
}

/// HKLM 的机器级默认启用位是否已是 1（`EnableLanguageProfileByDefault` 写的值）。
/// 只在那个 API 报错时用来复核状态，避免把「已经启用」误判成安装失败。
fn hklm_default_enabled() -> bool {
    let subkey = format!(
        "SOFTWARE\\Microsoft\\CTF\\TIP\\{{{CLSID_TEXT_SERVICE}}}\\LanguageProfile\\0x{LANGID_ZH_CN:08X}\\{{{GUID_PROFILE}}}"
    );
    read_dword(HKEY_LOCAL_MACHINE, &subkey, "Enable") == Some(1)
}

/// 本线程的 COM 初始化守卫。
///
/// `CoCreateInstance` 在未初始化的线程上返回 `CO_E_NOTINITIALIZED`
/// （0x800401F0），而 regsvr32 未必替我们初始化。返回值要分开看：
/// `S_OK` = 这次是我们初始化的，`S_FALSE` = 本线程已按**同一**套间模型
/// 初始化过 —— 两者都让本次调用持有一次初始化计数，退出时都必须
/// `CoUninitialize`；`RPC_E_CHANGED_MODE` = 别人的套间模型不同，COM 仍可用，
/// 但**不能**调 `CoUninitialize`（那会抵消掉别人的计数）。
struct ComGuard(bool);

impl ComGuard {
    fn new() -> Self {
        // SAFETY: 在未初始化的线程上调 CoInitializeEx 合法；pvReserved 必须为 NULL
        // （传 None），这是文档要求。
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        Self(hr == S_OK || hr == S_FALSE)
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: S_OK/S_FALSE 都为本线程加了计数，这里配对减一次。
            unsafe { CoUninitialize() }
        }
    }
}

/// Rust `&str` → NUL 结尾的 UTF-16（Win32 的 W 系 API 要的形态）。
fn wide_z(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

/// 只吞 `TF_E_ALREADY_EXISTS`（0x80040506，保留键冲突）这一个码。
///
/// 它**不是**通用的「profile 已存在」：`RegisterProfile` / `RegisterCategory`
/// 在重复调用时返回什么并不可依赖，那两处改用 `GetProfile` / 类别枚举显式
/// 判存在（2026-10-19 复审）。这里只兜旧接口 `ITfInputProcessorProfiles::
/// Register` 的重复调用。
fn ok_if_already_exists(r: Result<()>) -> Result<()> {
    match r {
        Err(e) if e.code() == TF_E_ALREADY_EXISTS => Ok(()),
        other => other,
    }
}

/// profile 是否已注册。`GetProfile` 是 Vista+ 的查询正路；只把 `Ok` 当存在，
/// 查询失败（含真的不存在）交给随后的 `RegisterProfile` 报出真实错误。
fn profile_exists(mgr: &ITfInputProcessorProfileMgr, clsid: &GUID, profile: &GUID) -> bool {
    let mut found = TF_INPUTPROCESSORPROFILE::default();
    // SAFETY: found 是本次调用独占的输出结构；GUID 指针指向调用方栈上的值，
    // 生命周期覆盖调用；hkl 对 INPUTPROCESSOR 类型被忽略，传 NULL。
    unsafe {
        mgr.GetProfile(
            TF_PROFILETYPE_INPUTPROCESSOR,
            LANGID_ZH_CN,
            clsid,
            profile,
            HKL::default(),
            &mut found,
        )
        .is_ok()
    }
}

/// 类别下是否已有本服务。`ITfCategoryMgr` 没有"查一项"的接口，只能枚举；
/// `IEnumGUID::Next` 返回 S_FALSE 且 fetched=0 表示取完。
fn category_has_service(cats: &ITfCategoryMgr, clsid: &GUID) -> Result<bool> {
    // SAFETY: 枚举器在活动 COM 套间内创建，仅在本函数内使用。
    let items = unsafe { cats.EnumItemsInCategory(&GUID_TFCAT_TIP_KEYBOARD)? };
    loop {
        let mut item = [GUID::default(); 1];
        let mut fetched = 0u32;
        // SAFETY: item 是 1 元素输出缓冲；fetched 是有效的计数输出位置。
        let hr = unsafe { items.Next(&mut item, Some(&mut fetched)) };
        if hr.is_err() {
            return Err(hr.into());
        }
        if fetched == 0 {
            return Ok(false);
        }
        if item[0] == *clsid {
            return Ok(true);
        }
    }
}

pub(crate) fn register() -> Result<()> {
    let _com = ComGuard::new(); // CoCreateInstance 前必须有
    let clsid = parse_guid(CLSID_TEXT_SERVICE)?;
    let profile = parse_guid(GUID_PROFILE)?;
    let dll_path = own_module_path()?;

    // ① COM 注册（本模块里**唯一**手写的注册表部分）。
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
    //    API 让它自己写（ITfInputProcessorProfiles::Register /
    //    EnableLanguageProfile / EnableLanguageProfileByDefault、
    //    ITfInputProcessorProfileMgr::RegisterProfile、
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
        // 描述串按**计数**传（TSF 全线计数串），不要结尾 NUL；空切片 = 无自定义图标。
        let desc: Vec<u16> = DISPLAY_NAME.encode_utf16().collect();

        // 新接口：ITfInputProcessorProfileMgr::RegisterProfile（Vista+ 的 profile
        //    注册正路，与 ITfInputProcessorProfiles 是**同一个 coclass** 的两个接口）。
        //    Windows 0.62 的实际签名：
        //      RegisterProfile(&clsid, langid, &profile, desc: &[u16], icon: &[u16],
        //                      icon_index, hkl, preferred_layout, enabled_by_default,
        //                      dwflags) -> Result<()>
        //    · bEnabledByDefault=TRUE：新用户/新语言配置默认启用；
        //    · dwFlags=0：RegisterProfile 的 dwFlags 只收 TF_RP_*（HIDDENINSETTINGUI /
        //      LOCALPROCESS / LOCALTHREAD）；TF_IPPMF_ENABLEPROFILE 是
        //      **ActivateProfile** 的标志，不能混用到 RegisterProfile（SDK 头 + MSDN）。
        //    · hkl=HKL::default()（NULL）：不替换成某个键盘布局。
        let profile_mgr: ITfInputProcessorProfileMgr = CoCreateInstance(
            &CLSID_TF_InputProcessorProfiles,
            None::<&IUnknown>,
            CLSCTX_INPROC_SERVER,
        )?;
        // 先查后注册：升级/修复/重复 regsvr32 都会再跑一遍。`RegisterProfile`
        // 对已存在 profile 的返回值不可依赖（TF_E_ALREADY_EXISTS 是保留键冲突，
        // 不是"已注册"），所以用 GetProfile 判存在；只有真的不存在才写。
        // `profiles.Register` 同样只在首次注册时调用，重复安装不再碰旧接口。
        if !profile_exists(&profile_mgr, &clsid, &profile) {
            ok_if_already_exists(profiles.Register(&clsid))?;
            profile_mgr.RegisterProfile(
                &clsid,
                LANGID_ZH_CN,
                &profile,
                &desc,
                &[],
                0,
                HKL::default(),
                0,
                true,
                0,
            )?;
        }
        // 旧接口 AddLanguageProfile 已删除：RegisterProfile 是 Vista+ 的正式路径，
        // 目标 Windows 10 不需要它；两者叠加会在首次安装时就写重复 profile。

        // HKLM 的「默认启用」位。交互式安装下面还会写当前用户的 HKCU 启用位；
        // SYSTEM / Intune / GPO 部署没有目标用户的 HKCU，这一位才是目标用户
        // 之后能在输入法列表里看到 OPI 的保险。已设置时是 no-op（成功）。
        // 旧文档只列 S_OK/E_FAIL：报错时读回 HKLM 状态，已是 1 就仍算成功。
        if let Err(e) =
            profiles.EnableLanguageProfileByDefault(&clsid, LANGID_ZH_CN, &profile, true)
        {
            if !hklm_default_enabled() {
                return Err(e);
            }
        }

        // 当前用户的启用位（HKCU\Software\Microsoft\CTF\TIP\...\Enable）。
        //    只写 HKLM 的 profile 不够：Windows 10 的输入法列表按**用户**启用状态
        //    过滤，旧实现缺这一步 ⇒ 「装完有进程、键盘不显示」。
        //    best-effort：机器级默认位已经写好（见上），SYSTEM/异常 HKCU 的失败
        //    不应把整包安装打回滚。IsEnabled 复核保留，仅用于状态确认。
        if profiles
            .EnableLanguageProfile(&clsid, LANGID_ZH_CN, &profile, true)
            .is_err()
        {
            let _ = profiles.IsEnabledLanguageProfile(&clsid, LANGID_ZH_CN, &profile);
        }

        let cats: ITfCategoryMgr = CoCreateInstance(
            &CLSID_TF_CategoryMgr,
            None::<&IUnknown>,
            CLSCTX_INPROC_SERVER,
        )?;
        // 类别 = "键盘输入法"。三个参数分别是：本服务 CLSID、类别 GUID、
        // 该类目下的条目 GUID —— 单一服务时条目就是自己。
        // 同样先枚举判存在，避免依赖 RegisterCategory 的重复调用错误码。
        if !category_has_service(&cats, &clsid)? {
            cats.RegisterCategory(&clsid, &GUID_TFCAT_TIP_KEYBOARD, &clsid)?;
        }
    }
    Ok(())
}

pub(crate) fn unregister() -> Result<()> {
    let _com = ComGuard::new();
    let clsid = parse_guid(CLSID_TEXT_SERVICE)?;
    let profile = parse_guid(GUID_PROFILE)?;

    // 逆序：先摘 TSF 侧（TSF 找不到 profile 就不会再激活我们），再删 COM 键。
    // 反过来的话，中间态是"COM 键没了但 TSF 还认为有个服务" → 宿主激活时报错。
    // SAFETY: 同 register。卸载是**尽力而为**的清理：任何一步失败都不能阻止后面
    // 删除 COM 键 —— 否则会留下"COM 键在、TSF profile 半摘"的中间态。
    unsafe {
        if let Ok(cats) = CoCreateInstance::<_, ITfCategoryMgr>(
            &CLSID_TF_CategoryMgr,
            None::<&IUnknown>,
            CLSCTX_INPROC_SERVER,
        ) {
            let _ = cats.UnregisterCategory(&clsid, &GUID_TFCAT_TIP_KEYBOARD, &clsid);
        }
        if let Ok(profile_mgr) = CoCreateInstance::<_, ITfInputProcessorProfileMgr>(
            &CLSID_TF_InputProcessorProfiles,
            None::<&IUnknown>,
            CLSCTX_INPROC_SERVER,
        ) {
            // register() 经 RegisterProfile 写下的 profile 在这里摘掉。
            let _ = profile_mgr.UnregisterProfile(&clsid, LANGID_ZH_CN, &profile, 0);
        }
        if let Ok(profiles) = CoCreateInstance::<_, ITfInputProcessorProfiles>(
            &CLSID_TF_InputProcessorProfiles,
            None::<&IUnknown>,
            CLSCTX_INPROC_SERVER,
        ) {
            // 先关当前用户的 Enable 位，再摘服务（尽力而为）。profile 已由上面的
            // ITfInputProcessorProfileMgr::UnregisterProfile 摘掉；旧接口
            // RemoveLanguageProfile 不再配对使用。
            let _ = profiles.EnableLanguageProfile(&clsid, LANGID_ZH_CN, &profile, false);
            let _ = profiles.Unregister(&clsid);
        }
    }
    // RegDeleteTree 连子键一起删（InprocServer32 是子键，只删父键会失败）。
    // 失败不报错：本来就不存在（没注册过就 /u）不是错误。
    let key = wide_z(&clsid_key(CLSID_TEXT_SERVICE));
    let _ = unsafe { RegDeleteTreeW(HKEY_CLASSES_ROOT, PCWSTR(key.as_ptr())) };
    Ok(())
}
