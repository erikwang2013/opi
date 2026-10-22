// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
//
// opi-ctl 的**总线寻址/防激活**契约测试 —— 从 `opi_ctl_cli.rs` 拆出来的子模块
// （主文件加上它们会顶破 500 行门禁；器具 fake / run_ctl / reply_of / REPLY_* /
// FIXTURE_LOCK 全在主模块里，这里 `use super::*` 取用）。
use super::*;

/// 跑 `opi-ctl state`，总线环境由调用方定：`bus` = Some 地址（None = 抹掉），
/// `xdg` = Some 目录（None = 抹掉）。
fn run_ctl_state_env(
    fake_dir: &Path,
    bus: Option<&str>,
    xdg: Option<&str>,
) -> (i32, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_opi-ctl"));
    cmd.arg("state")
        .env("PATH", format!("{}:/usr/bin:/bin", fake_dir.display()))
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .env_remove("XDG_RUNTIME_DIR");
    if let Some(a) = bus {
        cmd.env("DBUS_SESSION_BUS_ADDRESS", a);
    }
    if let Some(d) = xdg {
        cmd.env("XDG_RUNTIME_DIR", d);
    }
    let out = {
        let _g = FIXTURE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        cmd.output().expect("起不了 opi-ctl")
    };
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// 会话总线地址缺失（两个环境变量都空）→ 3，且**绝不**退到 X11 autolaunch：
/// 那会另造一条私有总线，再经激活文件拉起第二个 fcitx5（抢 XIM、没有 OPI 插件）。
/// 连 dbus-send 都不许起 —— 起了就说明地址兜底又溜回去了。
#[test]
fn missing_bus_address_is_channel_down_without_autolaunch() {
    let dir = fake(
        REPLY_TO_STDOUT,
        &reply_of("lang=zh;trad=off;punct=on;fw=on;im=;mode=pinyin"),
    );
    let (code, stdout, stderr) = run_ctl_state_env(&dir, None, None);
    assert_eq!(code, 3, "stderr={stderr}");
    assert_eq!(stdout, "");
    assert!(stderr.contains("私有总线"), "stderr={stderr}");
    assert!(!dir.join("argv").exists(), "没地址就不该起 dbus-send");
}

/// 只有 XDG_RUNTIME_DIR 时推标准会话总线（不是 autolaunch）。
#[test]
fn xdg_runtime_dir_derives_standard_session_bus() {
    let dir = fake(
        REPLY_TO_STDOUT,
        &reply_of("lang=zh;trad=off;punct=on;fw=on;im=;mode=pinyin"),
    );
    let (code, _, stderr) = run_ctl_state_env(&dir, None, Some("/run/user/12345"));
    assert_eq!(code, 0, "stderr={stderr}");
    let addr = std::fs::read_to_string(dir.join("addr")).unwrap();
    assert_eq!(addr.trim(), "unix:path=/run/user/12345/bus");
}

/// 显式 `autolaunch:` 也拒（libdbus 会另造私有总线）—— 一次 dbus-send 都不许起。
#[test]
fn explicit_autolaunch_address_is_refused_without_spawning() {
    let dir = fake(REPLY_TO_STDOUT, &reply_of("[]"));
    let (code, _, stderr) = run_ctl_state_env(&dir, Some("autolaunch:"), None);
    assert_eq!(code, 3, "stderr={stderr}");
    assert!(stderr.contains("autolaunch"), "stderr={stderr}");
    assert!(!dir.join("argv").exists(), "拒绝的地址不该起 dbus-send");
}

/// 属主查不到（fcitx5 没在跑）→ 3，且**不**向知名名发目标调用：知名名会触发
/// D-Bus 激活，拉起的实例继承调用方环境（实际报障的第二个 fcitx5 就是这么来的）。
#[test]
fn missing_owner_never_sends_to_the_activatable_name() {
    let dir = fake(
        NO_OWNER,
        "Error org.freedesktop.DBus.Error.NameHasNoOwner: The name org.fcitx.Fcitx5 was not provided by any .service files\n",
    );
    let (code, _, stderr) = run_ctl(&dir, &["trad"]);
    assert_eq!(code, 3, "stderr={stderr}");
    assert!(stderr.contains("通道不通"), "stderr={stderr}");
    let argv = std::fs::read_to_string(dir.join("argv")).unwrap();
    assert!(
        argv.contains("GetNameOwner"),
        "预检那条要真的发出去：{argv}"
    );
    assert!(
        !argv.contains("/opi"),
        "没属主时不许碰目标对象（有激活文件时会把 fcitx5 拉起来）：{argv}"
    );
}
