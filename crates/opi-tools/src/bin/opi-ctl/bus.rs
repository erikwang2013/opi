// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
//
// `opi-ctl` 的 D-Bus 传输层：进程外 `dbus-send` 的两步调用（GetNameOwner →
// 唯一名）、总线地址兜底与退出码分类。从 opi-ctl.rs 拆出（500 行门禁）；
// 协议与退出码契约写在 opi-ctl.rs 文件头，改要两侧一起看。

use std::process::Command;

use super::{DEST, PATH};

/// 会话总线地址：只用环境里已有的，或从 XDG_RUNTIME_DIR 推标准会话总线。
/// **绝不**让 dbus-send 走 X11 autolaunch（`autolaunch:` 也拒）：libdbus 会凭空
/// 造一条私有总线，D-Bus 激活文件随即拉起一个继承该环境的 fcitx5。
fn session_bus_address() -> Result<String, i32> {
    let env_addr = std::env::var("DBUS_SESSION_BUS_ADDRESS")
        .ok()
        .filter(|a| !a.is_empty());
    let canonical = std::env::var("XDG_RUNTIME_DIR")
        .ok()
        .filter(|d| !d.is_empty())
        .map(|d| format!("unix:path={d}/bus"));
    // 进程可能从桌面/自启动继承了 dbus-launch 的私有总线（`/tmp/dbus-*`）。
    // 那种环境里 fcitx5 是另一个实例（通常没装 OPI），状态栏会一直问错对象；
    // systemd 会话存在标准总线时优先用它。普通 `/run/user/...` 或测试给的
    // 任意地址仍按环境走，避免改变既有语义。
    let address = match env_addr {
        Some(a) if a.starts_with("unix:path=/tmp/") => canonical
            .filter(|c| std::path::Path::new(c.trim_start_matches("unix:path=")).exists())
            .unwrap_or(a),
        Some(a) => a,
        None => canonical.ok_or_else(|| {
            eprintln!(
                "opi-ctl: 没有会话总线地址（DBUS_SESSION_BUS_ADDRESS 与 XDG_RUNTIME_DIR 都为空）—— 不自动创建私有总线"
            );
            3
        })?,
    };
    if address.starts_with("autolaunch:") {
        eprintln!("opi-ctl: 会话总线地址是 autolaunch:（会另造一条私有总线），拒绝使用");
        return Err(3);
    }
    Ok(address)
}

/// 起一次 dbus-send，取回应答 string。Err(码) = 传输面已判死（3/4/5）。
/// `dest` 可为知名名或唯一名：唯一名不触发激活（见 [`call_bus`]）。
fn bus_send(
    address: &str,
    dest: &str,
    path: &str,
    method: &str,
    arg: Option<&str>,
) -> Result<String, i32> {
    let dest = format!("--dest={dest}");
    let mut cmd = Command::new("dbus-send");
    // 显式给地址：不给的话 libdbus 在无地址时会走 X11 autolaunch（另造私有总线）。
    cmd.env("DBUS_SESSION_BUS_ADDRESS", address);
    // --reply-timeout：都要在插件主循环上瞬间做完，5 秒足够；缺了它 dbus-send
    // 会按 libdbus 缺省等下去 —— 状态栏按钮不能挂几十秒。
    cmd.args([
        "--session",
        "--print-reply",
        "--reply-timeout=5000",
        &dest,
        path,
        method,
    ]);
    if let Some(a) = arg {
        cmd.arg(a);
    }
    let out = cmd.output();
    let out = match out {
        Ok(o) => o,
        Err(e) => {
            // dbus-send 不在 PATH 上（或起不来）—— 装它，不是重试。
            eprintln!("opi-ctl: 起不了 dbus-send（{e}）");
            eprintln!(
                "       装 D-Bus 的命令行包再试：Debian/Ubuntu 是 dbus-bin，Fedora 是 dbus-tools"
            );
            return Err(3);
        }
    };
    if !out.status.success() {
        return Err(transport_error(&String::from_utf8_lossy(&out.stderr)));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    match parse_reply(&stdout) {
        Ok(r) => Ok(r),
        Err(why) => {
            eprintln!("opi-ctl: 插件的应答看不懂（{why}）—— 插件与本工具版本对不上？");
            let first = stdout.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
            eprintln!("       原始输出：{first}");
            Err(5)
        }
    }
}

/// 发一条方法调用，取回应答 string。Err(码) = 传输面已判死（3/4/5，见 transport_error）。
/// 参数带不带由调用方定：动作走 `Trigger(string:<动作>)`（那条命令行是**逐字契约**），
/// state 走无参的 `State` —— 两者在总线上是同一个对象（/opi）。
pub(super) fn call_bus(method: &str, arg: Option<&str>) -> Result<String, i32> {
    let address = session_bus_address()?;
    // 先查属主（纯查询，不触发激活）；再打返回的唯一名 —— 唯一名不可能被激活。
    // 不给 fcitx5 没在跑时留任何激活机会（激活出的实例会抢 XIM 且没有 OPI）。
    let owner = bus_send(
        &address,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus.GetNameOwner",
        Some(&format!("string:{DEST}")),
    )?;
    let owner = owner.trim();
    if !owner.starts_with(':') {
        eprintln!("opi-ctl: 总线给回的属主名不是唯一名：{owner:?}");
        return Err(3);
    }
    bus_send(&address, owner, PATH, method, arg)
}

/// dbus-send 失败（非 0 退出）→ 退出码。判据是 stderr 里的 D-Bus 错误名 / 本地
/// 连接失败文本（dbus-send 的措辞在 dbus 各版本间稳定，且这里只看**子串**，
/// 不认识的一律落到 5 并原样带出底层第一行 —— 不静默、也不假装分类成功）。
fn transport_error(stderr: &str) -> i32 {
    let channel = [
        "Failed to open connection", // 本地：连不上会话总线
        "Unable to autolaunch",      // 本地：没有 DISPLAY 也没地址
        "Could not connect",
        "Disconnected",
        "ServiceUnknown", // 服务名没有属主 = fcitx5 没在跑
        "NameHasNoOwner",
        "NoReply", // 送达了但没回（插件主循环卡住 / 超时）
    ];
    let missing = [
        "UnknownObject", // /opi 不在总线上 = 接收侧没注册
        "UnknownInterface",
        "UnknownMethod", // 接口/方法不对（插件旧版、或没加载）
    ];
    let (code, why) = if channel.iter().any(|p| stderr.contains(p)) {
        (3, "D-Bus 通道不通（会话总线连不上，或 fcitx5 没在运行）")
    } else if missing.iter().any(|p| stderr.contains(p)) {
        (
            4,
            "插件没挂上这条通道（接收侧没注册 —— 插件没装 / 没加载？）",
        )
    } else {
        (5, "这次调用被总线/插件拒绝")
    };
    eprintln!("opi-ctl: {why}");
    let detail = stderr.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    if !detail.is_empty() {
        eprintln!("       底层报错：{detail}");
    }
    code
}

/// 从 `--print-reply` 的输出里取回 string 返回值：
///     method return time=… sender=… -> destination=… serial=… reply_serial=…
///        string "ok"
/// 取**最后一个**含 `string "` 的行（应答排在最后；多行输出里更早的不该是它），
/// 再去掉 dbus-send 的 C 风格转义（\" \\ \n \t \r）—— 服务端会把用户输入原样
/// 回显进 err 消息（未知动作那条），不反转义就可能漏字符。
fn parse_reply(stdout: &str) -> Result<String, &'static str> {
    const MARK: &str = "string \"";
    let line = stdout
        .lines()
        .rev()
        .find(|l| l.contains(MARK))
        .ok_or("输出里没有 string 返回值")?;
    let start = line.find(MARK).unwrap_or(0) + MARK.len();
    let rest = &line[start..];
    let end = rest.rfind('"').ok_or("返回值没有收尾引号")?;
    Ok(unescape(&rest[..end]))
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some('"') => out.push('"'),
            // 不认识的转义按字面保留（包括结尾孤立的反斜杠）。
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}
