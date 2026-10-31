// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `dll.rs` 的主机单测（Linux 可跑，`cargo test --workspace` 覆盖）。
//!
//! 为什么这些断言用**字面量**逐字比对、而不是"再算一遍"：这里每个值的错误
//! 后果都是**静默的**。键名少一个花括号 → regsvr32 报成功、TSF 找不到服务；
//! 模块计数算错 → DLL 在还持有活动对象时被卸载，之后任何一次 vtable 调用
//! 都跳进已释放的内存（崩在用户进程里，不是我们进程里）。

use super::*;

/// CLSID 的注册表键名是**带花括号**的大写十六进制，而 `GUID::try_from(&str)`
/// 只收**不带**括号的 36 字符形式（windows-core/guid.rs：`if from.len() != 36`）。
/// 于是常量存不带括号的那一份、括号在拼键时补 —— 一个来源、一处补括号，
/// 本测试把最终字符串整个钉死。
#[test]
fn clsid_key_is_registry_form_with_braces() {
    assert_eq!(
        clsid_key(CLSID_TEXT_SERVICE),
        "CLSID\\{98C51858-590B-404D-8915-21EB44250062}"
    );
}

/// `InprocServer32` 是 COM 在进程内找到本 DLL 的唯一依据：默认值 = DLL 全路径。
#[test]
fn inproc_server_key_appends_subkey() {
    assert_eq!(
        inproc_server_key(CLSID_TEXT_SERVICE),
        "CLSID\\{98C51858-590B-404D-8915-21EB44250062}\\InprocServer32"
    );
}

/// 两个 GUID 的字面量各自都必须是 `GUID::try_from` 收得下的形状：
/// 36 字符、`8-4-4-4-12`、大写十六进制。错一个字符 → 运行时 `try_from` 返回
/// E_INVALIDARG → 注册/创建服务全失败。放在主机门禁里，让它在 CI 就红，
/// 而不是等用户在 Windows 上 regsvr32。
#[test]
fn guid_literals_have_parseable_shape() {
    for id in [CLSID_TEXT_SERVICE, GUID_PROFILE] {
        assert_eq!(id.len(), 36, "{id} 长度应为 36");
        for (i, b) in id.bytes().enumerate() {
            let ok = match i {
                8 | 13 | 18 | 23 => b == b'-',
                _ => b.is_ascii_digit() || (b'A'..=b'F').contains(&b),
            };
            assert!(ok, "{id} 第 {i} 位 {b:?} 不是合法 GUID 字符");
        }
    }
}

/// profile GUID 与 CLSID 撞了的话，TSF 会把同一个 GUID 当两个身份用
/// （CLSID 查 COM 服务器、ProfileID 查语言配置）。这是 uuidgen 生成后
/// 最容易发生的粘贴错误。
#[test]
fn profile_guid_differs_from_clsid() {
    assert_ne!(CLSID_TEXT_SERVICE, GUID_PROFILE);
}

/// 模块计数：只要还有一个活动对象（服务对象或类工厂），`DllCanUnloadNow`
/// 就必须回答 S_FALSE。用**增量**断言而非绝对值：本文件与将来新增的测试
/// 可能并行跑，断言"全局计数恰好等于 N"会在并行下变得脆弱。
#[test]
fn module_lock_blocks_unload_until_last_release() {
    let before = dll_lock_count();
    let a = DllLock::new();
    assert_eq!(dll_lock_count(), before + 1, "持锁后计数应 +1");
    let b = DllLock::new();
    assert_eq!(dll_lock_count(), before + 2);

    // 契约本身：可卸载 ⟺ 计数为零。写成等值断言而不是"计数>0 时跳过"的挡箭牌 ——
    // 后者在并行下才成立，却恰好把要证的那条（手上有锁 → 不许卸载）放走了。
    assert_eq!(
        dll_can_unload(),
        dll_lock_count() == 0,
        "可卸载当且仅当计数为零"
    );

    drop(a);
    assert_eq!(dll_lock_count(), before + 1, "释放一把锁只 -1");
    drop(b);
    assert_eq!(dll_lock_count(), before);
    assert_eq!(dll_can_unload(), before == 0, "回到起点即恢复起点结论");
}
