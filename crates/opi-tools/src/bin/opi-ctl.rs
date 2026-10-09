// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
//
// `opi-ctl`：桌面状态栏（desktop-ui 用 ProcessBuilder 按名字调 /usr/bin/opi-ctl）
// 与 fcitx5-opi 插件之间的**唯一** CLI 面。动作集是**冻结契约**（任务书）：
//     opi-ctl symbols|trad|punct|fullwidth|lang
// 新增动作要先找 lead 定 —— 别在客户端先加一个服务端还没有的名字（那只会多出
// 一条「未知动作」的失败路径）。
//
// 传输：本进程**不**自己说 D-Bus，而是起一个 `dbus-send` 把一条方法调用交给插件
// 自己挂的 D-Bus 对象（接收侧的 spike 结论与拒绝理由在
// crates/fcitx5-opi/cpp/opi_ctl.h —— 两边是一对）。
// 为什么客户端也用外部命令而不是 Rust D-Bus 客户端（zbus/…）：
//   · 插件侧选路的理由就是「零新构建依赖」，客户端同理：加 zbus 会把重异步栈拉进
//     opi-tools 的依赖图，还牵动 Cargo.lock 与本仓打包镜像里的离线构建；
//   · dbus-send 随 dbus 装到所有目标发行版上（Debian/Ubuntu：dbus-bin 是 dbus 的
//     硬依赖；Fedora：dbus-tools；Arch/openSUSE：dbus 本体）—— 对本仓支持的桌面
//     发行版是既有事实，不是希望；
//   · 起不来的那条路有清晰诊断（见下），不做哑失败。
//   （gdbus / busctl 是「换一个第三方 CLI」，不解决根本问题，先不加。）
// ⚠️ 已知怪癖（memory: fcitx5-live-desktop-not-harness）：装了 D-Bus 激活文件
// 的系统上，发给 org.fcitx.Fcitx5 的调用**会把 fcitx5 拉起来**（激活）——
// 这是 dbus 语义，不是本工具能改的；正常桌面会话里 fcitx5 本来就在跑。
//
// 应答协议（与 opi_ctl.h 的注释成对，改要两侧同时动）：
//   插件的方法 Trigger(string) -> string：
//     "ok"                成功
//     "err:<码>:<原因>"    失败 —— <码> 由服务端分类（4=功能不可用，5=动作被拒），
//                         本进程原样把它当**自己的退出码**。
//
// 退出码（**契约**，lead 2026-10-10 钉死：0=成功；非 0 必须带可区分的 stderr 原因；
// desktop-ui 按「非零 = 可见反馈」实现，不许只有哑失败）：
//   0  成功
//   2  用法错误（动作缺失 / 未知动作）—— 本地就能判，不发 D-Bus
//   3  D-Bus 通道不通：会话总线连不上 / fcitx5 不在总线上（没在运行）/ dbus-send 缺失
//   4  功能不可用：服务端 err:4（符号浏览入口未接线、类目数据缺失）或对象/接口
//      不存在（插件没装 / 没加载 ⇒ 接收侧没注册）
//   5  动作被拒 / 其他：服务端 err:5（未知动作、内部异常）或应答格式不认识
// 3/4 的分界是「通道」与「功能」；4/5 的分界是「这东西没有」与「这东西拒绝了」。

use std::process::Command;

// 与 cpp/opi_ctl.h 的 kOpiCtlPath / kOpiCtlInterface / D-Bus 服务名同名 —— 改一处
// 要两侧同时动。服务名 org.fcitx.Fcitx5 是 fcitx5 自己的会话总线名（实测）。
const DEST: &str = "org.fcitx.Fcitx5";
const PATH: &str = "/opi";
const IFACE: &str = "org.fcitx.Fcitx5.Opi1";

// 冻结的动作集（顺序即 usage 里印的顺序）。
const ACTIONS: [&str; 5] = ["symbols", "trad", "punct", "fullwidth", "lang"];

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        // 附加项（动作集之外）：支持排障时确认装的是哪一版。桌面 UI 不用它。
        Some("-V" | "--version") => {
            println!("opi-ctl {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Some("-h" | "--help") => {
            usage();
            0
        }
        Some(a) if ACTIONS.contains(&a) => send(a),
        Some(a) => {
            eprintln!("opi-ctl: 未知动作：{a}");
            usage();
            2
        }
        None => {
            eprintln!("opi-ctl: 缺少动作");
            usage();
            2
        }
    }
}

fn usage() {
    eprintln!("usage: opi-ctl symbols|trad|punct|fullwidth|lang");
}

/// 发一条 Trigger(action)，把应答翻成退出码。参数顺序与落点是硬契约 ——
/// `tests/opi_ctl_cli.rs` 拿假 dbus-send 把这条命令行的**逐字 argv**钉住。
fn send(action: &str) -> i32 {
    // 先把会进 argv 的串都建好（引用临时值的数组活得比语句短，写在一行里太脆）。
    let dest = format!("--dest={DEST}");
    let method = format!("{IFACE}.Trigger");
    let arg = format!("string:{action}");
    // --reply-timeout：动作都要在插件主循环上瞬间做完，5 秒足够；缺了它 dbus-send
    // 会按 libdbus 缺省等下去 —— 状态栏按钮不能挂几十秒。
    let out = Command::new("dbus-send")
        .args([
            "--session",
            "--print-reply",
            "--reply-timeout=5000",
            &dest,
            PATH,
            &method,
            &arg,
        ])
        .output();
    let out = match out {
        Ok(o) => o,
        Err(e) => {
            // dbus-send 不在 PATH 上（或起不来）—— 装它，不是重试。
            eprintln!("opi-ctl: 起不了 dbus-send（{e}）");
            eprintln!(
                "       装 D-Bus 的命令行包再试：Debian/Ubuntu 是 dbus-bin，Fedora 是 dbus-tools"
            );
            return 3;
        }
    };
    if !out.status.success() {
        return transport_error(&String::from_utf8_lossy(&out.stderr));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let reply = match parse_reply(&stdout) {
        Ok(r) => r,
        Err(why) => {
            eprintln!("opi-ctl: 插件的应答看不懂（{why}）—— 插件与本工具版本对不上？");
            let first = stdout.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
            eprintln!("       原始输出：{first}");
            return 5;
        }
    };
    handle_reply(&reply)
}

/// 服务端应答 → 退出码。协议见文件头。
fn handle_reply(reply: &str) -> i32 {
    if reply == "ok" {
        return 0;
    }
    if let Some(rest) = reply.strip_prefix("err:") {
        let (num, msg) = rest.split_once(':').unwrap_or((rest, ""));
        // 服务端已分类的码原样透传（4/5）；越界的码不当作退出码用（125 以上 shell
        // 语义会扭曲，0 会伪装成功），退回 5。
        let code = num.parse::<i32>().unwrap_or(5);
        let code = if (1..=125).contains(&code) { code } else { 5 };
        let msg = if msg.is_empty() {
            "插件报错（没给原因）"
        } else {
            msg
        };
        eprintln!("opi-ctl: {msg}");
        return code;
    }
    eprintln!("opi-ctl: 插件的应答不认识：{reply:?}");
    5
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
