// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
//
// opi-ctl 的进程级契约测试：拿一个**假 dbus-send** 挡在 PATH 前面，
// 断言三件事 ——
//   1. 真发出去的命令行**逐字**（destination/path/interface.Trigger/string:<动作>），
//      这是与 fcitx5-opi 接收侧（cpp/opi_ctl.h）唯一的接口，写错哪一段都是哑失败；
//   2. 服务端应答 → 退出码的映射（"ok" → 0；err:<码>:<原因> → 原码 + 原因上 stderr）；
//   3. dbus-send 失败/缺失/应答异形时的退出码分类（lead 钉的契约：非 0 必须带
//      可区分的 stderr 原因；desktop-ui 按「非零 = 可见反馈」实现）。
// 不依赖机器上真有 fcitx5 与会话总线 —— 那是 e2e 的事，这里是纯客户端逻辑。
//
// 只用 cfg(unix)：假 dbus-send 是 sh 脚本 + 可执行位（本工具的宿主只有 Linux）。
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

/// 假 dbus-send：把收到的 argv 记在脚本旁边的 `argv` 文件里，再把 `reply`
/// 文件原样印到 stdout，退出 0。
const REPLY_TO_STDOUT: &str = r#"#!/bin/sh
d=$(dirname "$0")
printf '%s\n' "$@" > "$d/argv"
cat "$d/reply"
"#;

/// 同上，但把 `reply` 印到 **stderr** 并以 1 退出（模拟 dbus-send 的失败面）。
const REPLY_TO_STDERR: &str = r#"#!/bin/sh
d=$(dirname "$0")
printf '%s\n' "$@" > "$d/argv"
cat "$d/reply" >&2
exit 1
"#;

/// 服务端成功应答（dbus-send --print-reply 的真实形状）。
const OK_REPLY: &str = "method return time=1.0 sender=:1.2 -> destination=:1.3 serial=7 reply_serial=2\n   string \"ok\"\n";

/// 造一个只含假 dbus-send 的目录。每例一个独立目录（测试并行跑）。
fn fake(script: &str, reply: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "opi-ctl-test-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let bin = dir.join("dbus-send");
    std::fs::write(&bin, script).unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    std::fs::write(dir.join("reply"), reply).unwrap();
    dir
}

/// 跑 opi-ctl，返回 (退出码, stdout, stderr)，PATH **原样**给（调用方自己定）。
fn run_ctl_with_path(path: &str, args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_opi-ctl"))
        .args(args)
        .env("PATH", path)
        .output()
        .expect("起不了 opi-ctl");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// 跑 opi-ctl，假 dbus-send 在最前、系统目录在后 —— 假脚本自己要用
/// `dirname`/`cat`（真 dbus-send 被假货遮住，排在后面没有影响）。
fn run_ctl(fake_dir: &Path, args: &[&str]) -> (i32, String, String) {
    run_ctl_with_path(&format!("{}:/usr/bin:/bin", fake_dir.display()), args)
}

/// `string "<s>"` 形状的应答（转义由服务端做，这里只拼裸串 —— 转义用例单列）。
fn reply_of(s: &str) -> String {
    format!(
        "method return time=1.0 sender=:1.2 -> destination=:1.3 serial=7 reply_serial=2\n   string \"{s}\"\n"
    )
}

/// 成功路径：退出 0、静默（成功不刷屏），且发出去的命令行**逐字**如下 ——
/// 这一段与 cpp/opi_ctl.h 的 kOpiCtlPath / kOpiCtlInterface 是一对，改任一处
/// 这条就红。
#[test]
fn ok_path_sends_exact_wire_command() {
    let dir = fake(REPLY_TO_STDOUT, OK_REPLY);
    let (code, stdout, stderr) = run_ctl(&dir, &["symbols"]);
    assert_eq!(code, 0, "stderr={stderr}");
    assert_eq!(stdout, "", "成功不该有输出");
    assert_eq!(stderr, "", "成功不该有输出");
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
            "string:symbols",
        ]
    );
}

/// 五个冻结动作都能原样走到总线上（动作名不许在客户端被改写成别的）。
#[test]
fn all_frozen_actions_pass_through() {
    for action in ["symbols", "trad", "punct", "fullwidth", "lang"] {
        let dir = fake(REPLY_TO_STDOUT, OK_REPLY);
        let (code, _, stderr) = run_ctl(&dir, &[action]);
        assert_eq!(code, 0, "{action}: stderr={stderr}");
        let argv = std::fs::read_to_string(dir.join("argv")).unwrap();
        assert_eq!(
            argv.lines().last(),
            Some(format!("string:{action}").as_str()),
            "{action} 的最后一个参数不对"
        );
    }
}

/// 服务端 err:<码>:<原因> → 原样透传退出码 + 原因上 stderr（desktop-ui 就靠这个
/// 显示可见反馈）。4 = 功能不可用，5 = 动作被拒 —— 两档都要能区分。
#[test]
fn server_error_codes_propagate_with_reason() {
    for (n, msg) in [
        (4, "符号浏览入口未接线（引擎侧未落地），本次动作无效"),
        (5, "未知动作：frobnicate"),
    ] {
        let dir = fake(REPLY_TO_STDOUT, &reply_of(&format!("err:{n}:{msg}")));
        let (code, _, stderr) = run_ctl(&dir, &["symbols"]);
        assert_eq!(code, n, "stderr={stderr}");
        assert!(stderr.contains(msg), "原因要原样带出来：{stderr}");
    }
}

/// 服务端把用户输入回显进 err 消息时带 C 转义 —— 客户端要反转义，不能漏字符。
#[test]
fn escaped_reply_is_unescaped() {
    let dir = fake(REPLY_TO_STDOUT, &reply_of(r#"err:5:未知动作：a\"b"#));
    let (code, _, stderr) = run_ctl(&dir, &["symbols"]);
    assert_eq!(code, 5);
    assert!(stderr.contains("a\"b"), "stderr={stderr}");
}

/// fcitx5 没在跑（服务名无属主）→ 3：这是「通道不通」，不是「功能不可用」。
#[test]
fn service_unknown_maps_to_3() {
    let dir = fake(
        REPLY_TO_STDERR,
        "Error org.freedesktop.DBus.Error.ServiceUnknown: The name org.fcitx.Fcitx5 was not provided by any .service files\n",
    );
    let (code, _, stderr) = run_ctl(&dir, &["trad"]);
    assert_eq!(code, 3, "stderr={stderr}");
    assert!(stderr.contains("通道不通"), "stderr={stderr}");
}

/// 插件没装/没加载（/opi 不在总线上）→ 4，且与 3 的措辞要能区分。
#[test]
fn unknown_object_maps_to_4() {
    let dir = fake(
        REPLY_TO_STDERR,
        "Error org.freedesktop.DBus.Error.UnknownMethod: Unknown method Trigger on /opi\n",
    );
    let (code, _, stderr) = run_ctl(&dir, &["lang"]);
    assert_eq!(code, 4, "stderr={stderr}");
    assert!(stderr.contains("接收侧没注册"), "stderr={stderr}");
}

/// dbus-send 根本不在 PATH 上（没装 dbus 的命令行包）→ 3，且告诉用户装什么。
/// ⚠️ PATH 只能给这一个空目录：带系统目录的话会找到**真** dbus-send
/// （本机实测：那一版会把这次调用真的发到会话总线上，拿到
/// `UnknownObject '/opi'` → 退出码 4 —— 测试就变成了在量另一件事）。
#[test]
fn missing_dbus_send_maps_to_3() {
    let empty = std::env::temp_dir().join(format!("opi-ctl-empty-{}", std::process::id()));
    std::fs::create_dir_all(&empty).unwrap();
    let (code, _, stderr) = run_ctl_with_path(&empty.display().to_string(), &["punct"]);
    assert_eq!(code, 3, "stderr={stderr}");
    assert!(stderr.contains("dbus-send"), "stderr={stderr}");
}

/// 应答形状不认识（插件与本工具版本对不上？）→ 5，并把原始输出带出来。
#[test]
fn unrecognized_reply_maps_to_5() {
    let dir = fake(REPLY_TO_STDOUT, "method return time=1.0\n   int32 42\n");
    let (code, _, stderr) = run_ctl(&dir, &["fullwidth"]);
    assert_eq!(code, 5, "stderr={stderr}");
    assert!(stderr.contains("应答看不懂"), "stderr={stderr}");
}

/// 用法错在本地就判死（不发 D-Bus）：缺动作与未知动作都是 2 + usage。
#[test]
fn usage_errors_are_local_and_exit_2() {
    let dir = fake(REPLY_TO_STDOUT, OK_REPLY);
    for args in [vec![], vec!["frobnicate"]] {
        let (code, _, stderr) = run_ctl(&dir, &args);
        assert_eq!(code, 2, "args={args:?} stderr={stderr}");
        assert!(stderr.contains("usage:"), "stderr={stderr}");
    }
    assert!(
        !dir.join("argv").exists(),
        "用法错不该碰总线（假 dbus-send 的 argv 文件不该出现）"
    );
}
