// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `regsvr32` 的注册/注销：手写 COM 键 + TSF 官方 API 的 profile 注册。
//!
//! Forked from `crates/tsf-opi/src/registration.rs` for the Win7 client.
//! Kept in one file (< 500 lines): COM keys + TSF profile APIs. The jpackage
//! icon lookup was dropped — Phase 1 does not ship a tray icon.
//!
//! Verification boundary: on Linux only API names/signatures against windows
//! 0.61.3 are proven; actual registration needs a Windows 7/8 machine.

use windows::Win32::Foundation::{ERROR_SUCCESS, S_FALSE, S_OK, WIN32_ERROR};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CLASSES_ROOT, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
    RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::HKL;
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, CLSID_TF_InputProcessorProfiles, GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT, GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
    GUID_TFCAT_TIPCAP_UIELEMENTENABLED, ITfCategoryMgr, ITfInputProcessorProfileMgr,
    ITfInputProcessorProfiles, TF_E_ALREADY_EXISTS, TF_INPUTPROCESSORPROFILE,
    TF_PROFILETYPE_INPUTPROCESSOR,
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
///
/// 枚举本身失败（新类别尚无人注册时系统可能直接报错）按"没有"处理：升级安装
/// 第一次加能力类别时它必然为空，不能让一次查询失败挡住后面的 RegisterCategory
/// —— 真失败会在那里报出来。
fn category_has_service(cats: &ITfCategoryMgr, category: &GUID, clsid: &GUID) -> Result<bool> {
    // SAFETY: 枚举器在活动 COM 套间内创建，仅在本函数内使用。
    let items = match unsafe { cats.EnumItemsInCategory(category) } {
        Ok(items) => items,
        Err(_) => return Ok(false),
    };
    loop {
        let mut item = [GUID::default(); 1];
        let mut fetched = 0u32;
        // SAFETY: item 是 1 元素输出缓冲；fetched 是有效的计数输出位置。
        let hr = unsafe { items.Next(&mut item, Some(&mut fetched)) };
        if hr.is_err() || fetched == 0 {
            return Ok(false);
        }
        if item[0] == *clsid {
            return Ok(true);
        }
    }
}

/// 幂等注册一个类别条目：重复 `regsvr32` 不重复写。`RegisterCategory` 的重复
/// 调用错误码不可依赖（同 `RegisterProfile`），所以先枚举再决定是否写。
fn register_category(cats: &ITfCategoryMgr, category: &GUID, clsid: &GUID) -> Result<()> {
    if category_has_service(cats, category, clsid)? {
        return Ok(());
    }
    // SAFETY: 三个 REFGUID 都指向调用方栈上/常量值，生命周期覆盖调用。
    unsafe { cats.RegisterCategory(clsid, category, clsid) }
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
            // 无自定义图标：Phase 1 不带托盘图标，空切片是 TSF 允许的形态。
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
        // 失败**不阻断**：profile 与类别注册必须写完（否则「装成功、键盘不出现」
        // 连手工添加的机会都没有）；失败写进固定日志，由安装器/诊断脚本读回。
        if let Err(e) =
            profiles.EnableLanguageProfileByDefault(&clsid, LANGID_ZH_CN, &profile, true)
        {
            crate::com_server::log_register_failure("EnableLanguageProfileByDefault", &e);
        }

        // 当前用户的启用位（HKCU\Software\Microsoft\CTF\TIP\...\Enable）。
        //    只写 HKLM 的 profile 不够：Windows 10 的输入法列表按**用户**启用状态
        //    过滤，旧实现缺这一步 ⇒ 「装完有进程、键盘不显示」。
        //    best-effort：SYSTEM/异常 HKCU 的失败不应把整包安装打回滚，但必须可见。
        if let Err(e) = profiles.EnableLanguageProfile(&clsid, LANGID_ZH_CN, &profile, true) {
            crate::com_server::log_register_failure("EnableLanguageProfile", &e);
        }

        let cats: ITfCategoryMgr = CoCreateInstance(
            &CLSID_TF_CategoryMgr,
            None::<&IUnknown>,
            CLSCTX_INPROC_SERVER,
        )?;
        // 类别注册。三个参数分别是：本服务 CLSID、类别 GUID、该类目下的条目
        // GUID —— 单一服务时条目就是自己。
        //  · GUID_TFCAT_TIP_KEYBOARD：桌面（Win32）宿主靠它把本 TIP 列为键盘输入法。
        //  · GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT：可在 Store/沉浸式宿主（UWP）中运行。
        //  · GUID_TFCAT_TIPCAP_UIELEMENTENABLED：TIP 自带候选 UI，系统不再代画。
        //  · GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT：支持系统托盘语言栏。
        // 只注册 TIP_KEYBOARD 时，桌面应用能用而 UWP/沉浸式宿主认为本 TIP 不支持
        // 它们 —— 这正是「只在桌面可用、其他应用不生效」的注册侧根因。能力标志
        // TF_IPP_CAPS_* 是这些类别的读回值（TF_INPUTPROCESSORPROFILE.dwFlags），
        // 不是 RegisterProfile 的 dwFlags（那里只收 TF_RP_*，见上面的注释）。
        // 依据：TSF 文档「Predefined Category Values」的 TIP Capability 类别。
        // TIP_KEYBOARD 是「能不能打字」本身 —— 失败要报出来（regsvr32 非 0）。
        if let Err(e) = register_category(&cats, &GUID_TFCAT_TIP_KEYBOARD, &clsid) {
            crate::com_server::log_register_failure("RegisterCategory(TIP_KEYBOARD)", &e);
            return Err(e);
        }
        // 三个能力类别只影响 UWP/托盘语言栏；缺了仍能打字，不能让注册整体失败
        // （Windows 各版本对能力类别集的支持并不一致，失败只记录）。
        for category in [
            GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
            GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
            GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
        ] {
            if let Err(e) = register_category(&cats, &category, &clsid) {
                crate::com_server::log_register_failure("RegisterCategory(capability)", &e);
            }
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
            for category in [
                GUID_TFCAT_TIP_KEYBOARD,
                GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
                GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
                GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
            ] {
                let _ = cats.UnregisterCategory(&clsid, &category, &clsid);
            }
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
