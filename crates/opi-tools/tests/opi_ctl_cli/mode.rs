// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
//
// opi-ctl 的 mode（模式循环，2026-10-14）契约测试 —— 从 `opi_ctl_cli.rs` 拆出来的
// **子模块**（主文件加上这三条会顶破 500 行门禁 `crates/opi-ffi/tests/line_limit.rs`；
// 同 `cabi_test.rs` 带 `cabi/` 子目录的先例）。器具（fake / run_ctl / reply_of /
// REPLY_*）全在主模块里，这里 `use super::*` 取用，不复制一份。
use super::*;

/// mode（模式循环，2026-10-14）：**走 Trigger**（不是新 D-Bus 方法）、成功应答是
/// 状态 token 而不是 "ok"（**唯一**这样的动作）、token 原样进 stdout（桌面
/// cycleMode 读第一行）、rc 0、stderr 静默。命令行逐字如下 —— 与 cpp/opi_ctl.h
/// 的 Trigger 段成对，改任一侧这条就红。
#[test]
fn mode_sends_exact_wire_command_and_prints_token() {
    for token in ["pinyin", "en26"] {
        let dir = fake(REPLY_TO_STDOUT, &reply_of(token));
        let (code, stdout, stderr) = run_ctl(&dir, &["mode"]);
        assert_eq!(code, 0, "{token}: stderr={stderr}");
        assert_eq!(stdout, format!("{token}\n"), "token 要原样进 stdout");
        assert_eq!(stderr, "", "成功不刷 stderr");
        let argv = std::fs::read_to_string(dir.join("argv")).unwrap();
        let got: Vec<&str> = argv.lines().collect();
        assert_eq!(
            got,
            vec![
                "--session",
                "--print-reply",
                "--reply-timeout=5000",
                "--dest=org.fcitx.Fcitx5",
                "/opi",
                "org.fcitx.Fcitx5.Opi1.Trigger",
                "string:mode",
            ]
        );
    }
    // `wubi` 服务端现在不会发（五笔未落地）：**先放行**是前向兼容 —— 引擎侧加档时
    // 客户端不必跟着发版。这条钉的是「放行是有意的」，不是「现在真会拿到」。
    let dir = fake(REPLY_TO_STDOUT, &reply_of("wubi"));
    let (code, stdout, _) = run_ctl(&dir, &["mode"]);
    assert_eq!(code, 0);
    assert_eq!(stdout, "wubi\n");
}

/// mode 载荷形状不对 → 5 + 原始输出带出来，且**不许**往 stdout 写半行。负向形态里
/// `ok` 最要紧：它是「插件把 mode 接到旧路径」的指纹（Trigger 的通用成功应答）——
/// 放过去，桌面就会把 "ok" 当模式名画到条上。大小写/带空白/多词同样判死（取值域
/// 是**枚举**，不是「非空串」）。
#[test]
fn mode_broken_payload_maps_to_5() {
    for bad in [
        "ok",
        "",
        "zh",
        "PINYIN",
        "en26 ",
        "en26 extra",
        "en26;pinyin",
    ] {
        let dir = fake(REPLY_TO_STDOUT, &reply_of(bad));
        let (code, stdout, stderr) = run_ctl(&dir, &["mode"]);
        assert_eq!(code, 5, "{bad:?}: stderr={stderr}");
        assert_eq!(stdout, "", "{bad:?}: 形状不对就不许往 stdout 写东西");
        assert!(stderr.contains("token"), "{bad:?}: stderr={stderr}");
    }
}

/// 服务端分类的失败对 mode 同样生效（与动作/state/search 同一张退出码表）：
/// 老插件没有 mode 分支 → err:5「未知动作」原样透传；通道不通 → 3。
#[test]
fn mode_server_error_and_transport_error_propagate() {
    let dir = fake(REPLY_TO_STDOUT, &reply_of("err:5:未知动作：mode"));
    let (code, stdout, stderr) = run_ctl(&dir, &["mode"]);
    assert_eq!(code, 5, "stderr={stderr}");
    assert_eq!(stdout, "", "服务端报错时 stdout 不许有残留");
    assert!(stderr.contains("未知动作"), "原因要原样带出来：{stderr}");

    let dir = fake(
        REPLY_TO_STDERR,
        "Error org.freedesktop.DBus.Error.ServiceUnknown: The name org.fcitx.Fcitx5 was not provided by any .service files\n",
    );
    let (code, _, stderr) = run_ctl(&dir, &["mode"]);
    assert_eq!(code, 3, "stderr={stderr}");
    assert!(stderr.contains("通道不通"), "stderr={stderr}");
}
