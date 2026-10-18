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
//   4. state（只读查询，2026-10-10）：**无参数**的 State 方法名、状态行原样进
//      stdout，以及冻结形状的**负向**面（键序换位/少段/多段/取值域外 → 5：
//      桌面按位置解析，放过去就是把值读错位）。
//   5. mode（模式循环，2026-10-14）：成功应答**不是 "ok"** 而是状态 token
//      （pinyin/en26，未来 wubi）—— 唯一这样的动作；token 原样进 stdout，取值域
//      钉死（"ok"/大小写/带空白/多词都判 5，见 mode_broken_payload_maps_to_5）。
//      这三条测试在子模块 `tests/opi_ctl_cli/mode.rs`（本文件加上它们顶破 500 行门禁）。
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

/// 夹具与 spawn 的**共用锁**（2026-10-14 修一个真竞态，见 `run_ctl_with_path` 的注）。
static FIXTURE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// 造一个只含假 dbus-send 的目录。每例一个独立目录（测试并行跑）。
///
/// ⚠️ 写脚本这一段**必须在 [`FIXTURE_LOCK`] 里**：并行测试时，本线程 `fork`（见
/// `run_ctl_with_path`）会继承**别的线程**此刻正打开的写 fd —— 那个 fd 指向的是
/// 另一个用例的假脚本；若那个用例随后 exec 同一个 inode，内核给的就是
/// `ETXTBSY (Text file busy)`（本仓实测：全量 workspace 并行跑时偶尔红在
/// `state_service_unknown_maps_to_3`，单独跑 5/5 绿）。两侧同锁 ⇒ fork 与「写可执行
/// 文件」不再可能重叠。
fn fake(script: &str, reply: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "opi-ctl-test-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let bin = dir.join("dbus-send");
    {
        let _g = FIXTURE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        std::fs::write(&bin, script).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    std::fs::write(dir.join("reply"), reply).unwrap();
    dir
}

/// 跑 opi-ctl，返回 (退出码, stdout, stderr)，PATH **原样**给（调用方自己定）。
fn run_ctl_with_path(path: &str, args: &[&str]) -> (i32, String, String) {
    // ⚠️ spawn+等待也在 [`FIXTURE_LOCK`] 里（与 `fake()` 同锁）：`output()` 内部
    // fork 会继承全进程的 fd —— 锁住的正是「别的线程正拿着某个假脚本的写 fd」那个
    // 窗口（成因与实测见 `fake()` 的注）。串行化的代价：本文件全是用例级快跑，
    // 合并耗时 <1s。
    let out = {
        let _g = FIXTURE_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        Command::new(env!("CARGO_BIN_EXE_opi-ctl"))
            .args(args)
            .env("PATH", path)
            .output()
            .expect("起不了 opi-ctl")
    };
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

// mode 那一组（文件头第 5 条）拆成子模块文件，见其文件头：本文件加上那三条
// 会顶破 500 行门禁。[path] 是必须的：**测试文件是 crate root**，`mod x;` 会在
// 本层目录找 `tests/x.rs`，不自带「同名子目录」规则（同 opi-ffi 的
// `#[path = "c_abi/checker.rs"]` 先例）。
#[path = "opi_ctl_cli/mode.rs"]
mod mode;

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

/// `insert <文本>`（2026-10-12 加）：**带参数的动作** —— 文本经 `insert:` 前缀搭
/// 既有 `Trigger(string)` 契约走（不新增 D-Bus 方法），逐字钉住落点；缺参/空参 = 用法
/// 错误 2（**本地**判定，不上总线 —— 空文本没有任何意义）。
#[test]
fn insert_action_carries_the_text_through_the_trigger_slot() {
    let dir = fake(REPLY_TO_STDOUT, OK_REPLY);
    let (code, _, stderr) = run_ctl(&dir, &["insert", "好"]);
    assert_eq!(code, 0, "stderr={stderr}");
    let argv = std::fs::read_to_string(dir.join("argv")).unwrap();
    assert_eq!(
        argv.lines().last(),
        Some("string:insert:好"),
        "文本要逐字跟在 insert: 前缀后面"
    );

    for bad in [vec!["insert"], vec!["insert", ""]] {
        let dir = fake(REPLY_TO_STDOUT, OK_REPLY);
        let (code, stdout, stderr) = run_ctl(&dir, &bad);
        assert_eq!(code, 2, "{bad:?}: 用法错误应为 2（stderr={stderr}）");
        assert_eq!(stdout, "", "{bad:?}: 不许往 stdout 写东西");
        assert!(
            !dir.join("argv").exists(),
            "{bad:?}: 用法错误不该上总线（实发了：{:?}）",
            std::fs::read_to_string(dir.join("argv")).ok()
        );
    }
}

/// 冻结动作都能原样走到总线上（动作名不许在客户端被改写成别的）。
#[test]
fn all_frozen_actions_pass_through() {
    for action in [
        "symbols",
        "trad",
        "punct",
        "fullwidth",
        "lang",
        "englishpro",
    ] {
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

/// state 的成功路径：**无参数**的 `State` 方法、那一行原样进 stdout、rc 0、
/// stderr 静默（桌面每秒轮询 —— 成功刷屏等于刷日志）。命令行逐字如下：比动作那条
/// **少**一个 `string:` 参数（服务端签名是空入参，多带一个参数会被总线判成坏调用）。
#[test]
fn state_query_sends_exact_wire_command_and_prints_line() {
    let dir = fake(
        REPLY_TO_STDOUT,
        &reply_of("lang=zh;trad=off;punct=on;fw=on;im=opi;mode=pinyin"),
    );
    let (code, stdout, stderr) = run_ctl(&dir, &["state"]);
    assert_eq!(code, 0, "stderr={stderr}");
    assert_eq!(
        stdout, "lang=zh;trad=off;punct=on;fw=on;im=opi;mode=pinyin\n",
        "状态行要原样进 stdout"
    );
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
            "org.fcitx.Fcitx5.Opi1.State",
        ]
    );
}

/// 两端值都原样透传（正、负两面）：on/off 是服务端给的，客户端不缓存、不改写 ——
/// 客户端自己造状态正是这条查询要避免的事。im 那一格**没有取值域**：别的输入法名
/// 要放行，**空串也要放行**（= 没有 IC / 查不到，前端按「未知=维持现状」处理；
/// 在这里把空串判成形状错，等于把「查不到」变成状态栏可见的失败）。mode 那一格
/// （2026-10-14 追加，行尾）取值域是 token 表：`wubi` 服务端现在还不会发，但**先
/// 放行**（前向兼容 —— 引擎加档时客户端不必跟着发版，同 check_mode_shape）。
#[test]
fn state_values_pass_through_both_ways() {
    for line in [
        "lang=zh;trad=off;punct=on;fw=on;im=opi;mode=pinyin",
        "lang=en;trad=on;punct=off;fw=off;im=keyboard-us;mode=en26",
        "lang=zh;trad=off;punct=on;fw=on;im=;mode=pinyin",
        "lang=zh;trad=off;punct=on;fw=on;im=;mode=wubi", // 预放行的未来档
        "lang=en;trad=off;punct=off;fw=off;im=opi;mode=en26;enpro=on", // 2026-10-19 专业态
    ] {
        let dir = fake(REPLY_TO_STDOUT, &reply_of(line));
        let (code, stdout, stderr) = run_ctl(&dir, &["state"]);
        assert_eq!(code, 0, "{line}: stderr={stderr}");
        assert_eq!(stdout.trim_end(), line);
    }
}

/// 冻结形状的**负向**面：键序换位、少段、多段、取值域外（fw 与 mode 各一）、
/// 键名重复 —— 一律 5 且 stdout 为空。桌面端按位置解析，放过去就是把 `trad=on`
/// 读成「语言变繁体」。**少一段那条现在少的是 mode**（= 老插件的输出）：本工具
/// 这一层**照样判 5**（「插件与本工具版本对不上」的如实分类）；对缺这一格的**容错
/// 在桌面解析侧**（Actions.kt 的 ImeState.mode 回落「拼音」），别把两层的口径搞混。
#[test]
fn state_broken_shapes_map_to_5() {
    for bad in [
        "trad=off;lang=zh;punct=on;fw=on;im=opi;mode=pinyin", // 键序换位（同一个值集合）
        "lang=zh;trad=off;punct=on;fw=on;im=opi",             // 少一段（缺 mode —— 老插件）
        "lang=zh;trad=off;punct=on;fw=on;im=opi;mode=pinyin;x=y", // 多一段
        "lang=zh;trad=off;punct=on;fw=maybe;im=;mode=pinyin", // 取值域外（im 空不背锅）
        "lang=zh;trad=off;punct=on;fw=on;im=;mode=quanpin",   // mode 取值域外（token 表外）
        "lang=zh;trad=off;punct=on;fw=on;im=opi;lang=zh",     // 键名重复（末位不是 mode）
        "",                                                   // 空串
    ] {
        let dir = fake(REPLY_TO_STDOUT, &reply_of(bad));
        let (code, stdout, stderr) = run_ctl(&dir, &["state"]);
        assert_eq!(code, 5, "{bad:?}: stderr={stderr}");
        assert_eq!(stdout, "", "{bad:?}: 形状不对就不许往 stdout 写东西");
        assert!(stderr.contains("格式"), "{bad:?}: stderr={stderr}");
    }
}

/// 服务端分类的失败对 state 同样生效（与动作同一张退出码表）：通道通了但插件
/// 内部异常时，桌面拿到的是非零 + 原因，不是一行假状态。
#[test]
fn state_server_error_propagates() {
    let dir = fake(REPLY_TO_STDOUT, &reply_of("err:5:内部异常：std::bad_alloc"));
    let (code, stdout, stderr) = run_ctl(&dir, &["state"]);
    assert_eq!(code, 5, "stderr={stderr}");
    assert_eq!(stdout, "");
    assert!(stderr.contains("内部异常"), "原因要原样带出来：{stderr}");
}

/// 通道不通对 state 是 3（「通道」与「功能」的分界不变），且 stdout 不许留半行。
#[test]
fn state_service_unknown_maps_to_3() {
    let dir = fake(
        REPLY_TO_STDERR,
        "Error org.freedesktop.DBus.Error.ServiceUnknown: The name org.fcitx.Fcitx5 was not provided by any .service files\n",
    );
    let (code, stdout, stderr) = run_ctl(&dir, &["state"]);
    assert_eq!(code, 3, "stderr={stderr}");
    assert_eq!(stdout, "", "通道不通时 stdout 不许有残留状态行");
    assert!(stderr.contains("通道不通"), "stderr={stderr}");
}

/// search 的成功路径：`Search(string:<关键词>)`（**带参数**、方法名与 State 同族）、
/// 载荷（JSON 文本数组）原样进 stdout、rc 0、stderr 静默。命令行逐字如下 ——
/// 与 cpp/opi_ctl.h 的 `FCITX_OBJECT_VTABLE_METHOD(search, "Search", "s", "s")` 成对，
/// 任一侧改名/改签名这条就红。
#[test]
fn search_query_sends_exact_wire_command_and_prints_payload() {
    let dir = fake(REPLY_TO_STDOUT, &reply_of(r#"["♥","、"]"#));
    let (code, stdout, stderr) = run_ctl(&dir, &["search", "heart"]);
    assert_eq!(code, 0, "stderr={stderr}");
    assert_eq!(stdout, "[\"♥\",\"、\"]\n", "载荷要原样进 stdout");
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
            "org.fcitx.Fcitx5.Opi1.Search",
            "string:heart",
        ]
    );
}

/// 空关键词 = **用法错误 2**，本地判定、不上总线 —— 引擎侧 `search("")` 的语义是
/// 「全部条目」（浏览不是搜索），桌面清空搜索框时该回浏览态，不发这一问。
/// 服务端也拒一次（err:2，另见 opi_engine.h），这里是客户端那一次。
#[test]
fn search_empty_keyword_is_local_usage_error() {
    for bad in [vec!["search"], vec!["search", ""]] {
        let dir = fake(REPLY_TO_STDOUT, &reply_of("[]"));
        let (code, stdout, stderr) = run_ctl(&dir, &bad);
        assert_eq!(code, 2, "{bad:?}: 用法错误应为 2（stderr={stderr}）");
        assert_eq!(stdout, "", "{bad:?}: 不许往 stdout 写东西");
        assert!(stderr.contains("usage:"), "{bad:?}: stderr={stderr}");
        assert!(
            !dir.join("argv").exists(),
            "{bad:?}: 用法错误不该上总线（实发了：{:?}）",
            std::fs::read_to_string(dir.join("argv")).ok()
        );
    }
}

/// 载荷形状不对 → 5 + 原始输出带出来，且**不许**往 stdout 写半行。负向形态里
/// `ok` 是**版本对不上**的指纹：旧插件（没有 Search 方法）或把 search 接回
/// Trigger 那种接线错，回的就是 "ok" —— 桌面会把 "ok" 当搜索结果画成两个字母。
#[test]
fn search_broken_payload_maps_to_5() {
    for bad in [
        "ok",           // 旧/错接线的指纹（Trigger 的应答）
        "[",            // 截断
        "[\"a\",\"b\"", // 少收尾
        "{}",           // 对象而不是数组
        "",             // 空串
    ] {
        let dir = fake(REPLY_TO_STDOUT, &reply_of(bad));
        let (code, stdout, stderr) = run_ctl(&dir, &["search", "heart"]);
        assert_eq!(code, 5, "{bad:?}: stderr={stderr}");
        assert_eq!(stdout, "", "{bad:?}: 形状不对就不许往 stdout 写东西");
        assert!(stderr.contains("JSON"), "{bad:?}: stderr={stderr}");
    }
}

/// 空数组是**合法**载荷（= 没搜到）：rc 0、`[]` 进 stdout —— 桌面据此显示
/// 「没有匹配」；把它判成形状错会让「搜不到」变成一条可见的失败。
#[test]
fn search_empty_array_is_a_valid_payload() {
    let dir = fake(REPLY_TO_STDOUT, &reply_of("[]"));
    let (code, stdout, stderr) = run_ctl(&dir, &["search", "zzzz"]);
    assert_eq!(code, 0, "stderr={stderr}");
    assert_eq!(stdout, "[]\n");
}

/// 服务端分类的失败对 search 同样生效（与动作/state 同一张退出码表）：
/// err:4（搜索入口未接线）/ err:2（空关键词，服务端那一次）原样透传成退出码。
#[test]
fn search_server_error_propagates() {
    for (n, msg) in [
        (4, "符号搜索入口未接线（引擎侧未落地），本次查询无效"),
        (
            2,
            "搜索关键字为空（用法错误：opi-ctl search 需要一个关键词）",
        ),
    ] {
        let dir = fake(REPLY_TO_STDOUT, &reply_of(&format!("err:{n}:{msg}")));
        let (code, stdout, stderr) = run_ctl(&dir, &["search", "heart"]);
        assert_eq!(code, n, "stderr={stderr}");
        assert_eq!(stdout, "", "服务端报错时 stdout 不许有残留");
        assert!(stderr.contains(msg), "原因要原样带出来：{stderr}");
    }
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
