// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT
//
// `opi-ctl`：桌面状态栏（desktop-ui 用 ProcessBuilder 按名字调 /usr/bin/opi-ctl）
// 与 fcitx5-opi 插件之间的**唯一** CLI 面。动作集是**冻结契约**（任务书）：
//     opi-ctl symbols|trad|punct|fullwidth|lang|englishpro|clearuserwords （动作：会改状态）
//     opi-ctl insert <文本>                        （大页面的上屏通道，2026-10-12 加）
//     opi-ctl mode                                 （模式循环，2026-10-14 用户裁决加）
//     opi-ctl state                                 （只读查询，2026-10-10 用户裁决加）
//     opi-ctl search <关键词>                       （只读查询，2026-10-13 加）
// 新增动作要先找 lead 定 —— 别在客户端先加一个服务端还没有的名字（那只会多出
// 一条「未知动作」的失败路径）。
// 两条路的**协议不同**：动作发 `Trigger(string:<动作>)`，应答 "ok" / "err:…"，
// 成功静默；state 发 `State`（**无参数**）、search 发 `Search(string:<关键词>)`，
// 应答就是载荷本身 —— 校验形状后原样打到 stdout（细节在 send_state /
// check_state_shape / send_search / check_search_shape）。动作集仍是那五个：
// state/search 不改任何状态，故不进 ACTIONS（desktop 的按钮映射也不含它们）。
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
// ⚠️ 激活怪癖（memory: fcitx5-live-desktop-not-harness）：装了 D-Bus 激活文件
// 的系统上，发给 org.fcitx.Fcitx5 的调用**会把 fcitx5 拉起来**（激活）—— 拉起的
// 实例继承的是**调用方**的总线/环境（root 维护脚本、私有总线），实测会抢走 XIM
// 且没有 OPI 插件。故本工具两步走：先 GetNameOwner（纯查询）确认它**已经在跑**，
// 再打返回的**唯一名** —— 唯一名不可能触发激活。同理，环境没给总线地址时**绝不**
// 退到 X11 autolaunch（会另造一条私有总线），只从 XDG_RUNTIME_DIR 推标准会话总线。
//
// 应答协议（与 opi_ctl.h 的注释成对，改要两侧同时动）：
//   插件的方法 Trigger(string) -> string：
//     "ok"                成功
//     "err:<码>:<原因>"    失败 —— <码> 由服务端分类（4=功能不可用，5=动作被拒），
//                         本进程原样把它当**自己的退出码**。
//     ⚠️ **唯一例外**：动作 `mode`（2026-10-14 加）成功时**不是** "ok"，而是新状态
//     token：`pinyin` / `en26`（未来 `wubi`）—— 桌面状态栏的模式按钮要拿它立即显示
//     新档位，所以它同 state/search 一样把载荷打进 stdout（形状检查见
//     check_mode_shape）。其余动作一字不动。
//   插件的方法 State() -> string（只读查询，无参数）：
//     一行**冻结格式** `lang=zh|en;trad=on|off;punct=on|off;fw=on|off;im=…;mode=…`
//     （键序固定，桌面端按位置解析；im = 当前输入法唯一名，**可为空** = 没有可报的
//     输入法（无 IC 且 ShareInputState=No 没兜底）—— 前端约定按「未知 = 维持现状」，
//     2026-10-10 追加在行尾 —— 旧解析只读前四段，多一段不会读错位；mode = 档位
//     token（取值域同 `mode` 动作的应答：pinyin / en26 / 未来 wubi），2026-10-14
//     **同样追加在行尾**，桌面拿它定状态栏模式按钮的初始档），
//     或同样的 "err:<码>:<原因>"。
//   插件的方法 Search(string) -> string（只读查询，一个入参，2026-10-13 加）：
//     **JSON 文本数组**（`["…"]`，serde_json 输出；`[]` = 没搜到），或同样的
//     "err:<码>:<原因>"。空关键词在**本地**就拒（不发 D-Bus，退出码 2）—— 服务端
//     也拒一次（err:2），手发 dbus-send 的人不会绕过。载荷的完整解析归桌面
//     （JParse），这里只做轻量形状检查（理由见 check_search_shape）。
//
// 退出码（**契约**，lead 2026-10-10 钉死：0=成功；非 0 必须带可区分的 stderr 原因；
// desktop-ui 按「非零 = 可见反馈」实现，不许只有哑失败）：
//   0  成功
//   2  用法错误（动作缺失 / 未知动作 / insert-search 缺参数）—— 本地就能判，不发 D-Bus
//   3  D-Bus 通道不通：会话总线连不上 / fcitx5 不在总线上（没在运行）/ dbus-send 缺失
//   4  功能不可用：服务端 err:4（符号浏览入口未接线、类目数据缺失、搜索入口未接线）
//      或对象/接口/方法不存在（插件没装 / 没加载 ⇒ 接收侧没注册、或插件版本旧到
//      还没有 Search 方法）
//   5  动作被拒 / 其他：服务端 err:5（未知动作、内部异常）或应答格式不认识
// 3/4 的分界是「通道」与「功能」；4/5 的分界是「这东西没有」与「这东西拒绝了」。
// state/search/mode 走**同一张表**：0 = 拿到载荷（只有这三条会写 stdout，各写各的
// 形状）；3/4 同上面两条；5 多一档含义 —— 应答不是那个冻结形状（插件与本工具版本
// 对不上：宁可在这里红，也别把错位的值交给桌面画图标 / 显示成搜索结果 / 当模式显示）。

// 子模块放同名目录（`src/bin/bus.rs` 会被 Cargo 当成另一个 bin target）。
#[path = "opi-ctl/bus.rs"]
mod bus;

use bus::call_bus;

// 与 cpp/opi_ctl.h 的 kOpiCtlPath / kOpiCtlInterface / D-Bus 服务名同名 —— 改一处
// 要两侧同时动。服务名 org.fcitx.Fcitx5 是 fcitx5 自己的会话总线名（实测）。
const DEST: &str = "org.fcitx.Fcitx5";
const PATH: &str = "/opi";
const IFACE: &str = "org.fcitx.Fcitx5.Opi1";

// 冻结的动作集（顺序即 usage 里印的顺序）。state 不在其中 —— 它不改状态，
// 走的是另一条协议（见文件头），单独分派。**mode 也不在其中**（2026-10-14）：
// 它改状态，但成功应答是状态 token 而不是 "ok"，与 insert 一样单独分派。
const ACTIONS: [&str; 7] = [
    "symbols",
    "trad",
    "punct",
    "fullwidth",
    "lang",
    "englishpro",
    // 2026-10-23：设置窗「清空用户词」（引擎学习记忆）。服务端分支见
    // cpp/opi_engine.h 的 clearuserwords；两端动作名必须逐字一致。
    "clearuserwords",
];

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
        Some("state") => send_state(),
        // `insert <文本>`：大页面（桌面符号/表情窗）的**上屏通道**（用户裁决 2026-10-12
        // 「符号按钮点击后要是个大的页面」）。自绘窗拿不到 fcitx5 的提交通道（那在插件
        // 进程里），所以文本经 `Trigger("insert:<text>")` 下沉给插件提交到当前输入框。
        // **带参数的动作**：不在冻结集里，单独分派（空文本/缺参数 = 用法错误 2）。
        Some("insert") => match args.get(2).map(String::as_str) {
            Some(text) if !text.is_empty() => send(&format!("insert:{text}")),
            _ => {
                eprintln!("opi-ctl: insert 需要一个非空文本参数");
                usage();
                2
            }
        },
        // `search <关键词>`：桌面符号/表情页搜索框的数据来源（2026-10-13）。桌面进程
        // 够不着引擎（引擎在插件进程里），所以搜索与上屏一样走 CLI。**空关键词 = 用法
        // 错误**（2）：引擎侧 search("") 的语义是「全部条目」，那是浏览不是搜索 ——
        // 桌面清空搜索框时该回浏览态，不发这一问。服务端也拒一次（见文件头协议段）。
        Some("search") => match args.get(2).map(String::as_str) {
            Some(kw) if !kw.is_empty() => send_search(kw),
            _ => {
                eprintln!("opi-ctl: search 需要一个非空关键词");
                usage();
                2
            }
        },
        // `mode`：模式循环（用户裁决 2026-10-14；桌面状态栏的「模式」按钮 = 循环
        // 一次）。动作名搭既有的 `Trigger(string)` 走，但成功应答是**新状态 token**
        // （不是 "ok"）—— 单独分派，理由与形状检查都在 send_mode / check_mode_shape。
        Some("mode") => send_mode(),
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
    eprintln!(
        "usage: opi-ctl symbols|trad|punct|fullwidth|lang|englishpro|clearuserwords|mode [insert <文本>] | state | search <关键词>"
    );
}

/// 发一条 Trigger(action)，把应答翻成退出码。参数顺序与落点是硬契约 ——
/// `tests/opi_ctl_cli.rs` 拿假 dbus-send 把这条命令行的**逐字 argv**钉住。
fn send(action: &str) -> i32 {
    let arg = format!("string:{action}");
    match call_bus(&format!("{IFACE}.Trigger"), Some(&arg)) {
        Ok(reply) => handle_reply(&reply),
        Err(code) => code,
    }
}

/// state（只读查询）：State（无参）→ 那一行状态串（或 err:…）。
/// 与动作路径的三处分歧都是**有意的**：无参数、方法名不同、成功时把状态行打到
/// stdout（所以这是唯一会写 stdout 的路径；stderr 在成功时保持静默 —— 桌面端
/// 每秒轮询一次，成功刷 stderr 会把日志刷满）。
fn send_state() -> i32 {
    let reply = match call_bus(&format!("{IFACE}.State"), None) {
        Ok(r) => r,
        Err(code) => return code,
    };
    // 服务端分类的失败与动作路径同一套映射（err:4 之类原样透传成退出码）。
    if reply.starts_with("err:") {
        return handle_reply(&reply);
    }
    match check_state_shape(&reply) {
        Ok(()) => {
            println!("{reply}");
            0
        }
        Err(why) => {
            eprintln!("opi-ctl: state 的输出不符合冻结格式（{why}）—— 插件与本工具版本对不上？");
            eprintln!("       原始输出：{reply}");
            5
        }
    }
}

/// 搜索（只读查询）：`Search(string:<关键词>)` → 命中条目的 JSON 文本数组（或 err:…）。
/// 与 state 同路（只读、方法名不同、成功时打 stdout）；与动作路径的分歧多一处 ——
/// **带一个参数**（state 是无参那半）。
///
/// 载荷的**完整**解析归桌面（那边有 JParse 递归下降解析器，能把元素逐个拿出来建
/// 网格）；这里只查形状。为什么不做完整解析：opi-tools **没有 serde_json 依赖**
/// （Cargo.toml 里就四个依赖，加一个穿透到打包镜像的离线构建 —— 为一个「看一眼」
/// 的形状检查不值得），而轻量检查已经够本层的用途：把「插件与本工具版本对不上 /
/// 应答被截断」与「真的拿到了数组」分开，别把错位的值当搜索结果涂到屏幕上。
fn send_search(keyword: &str) -> i32 {
    let arg = format!("string:{keyword}");
    let reply = match call_bus(&format!("{IFACE}.Search"), Some(&arg)) {
        Ok(r) => r,
        Err(code) => return code,
    };
    // 服务端分类的失败与动作路径同一套映射（err:2 空关键词 / err:4 未接线 原样透传）。
    if reply.starts_with("err:") {
        return handle_reply(&reply);
    }
    match check_search_shape(&reply) {
        Ok(()) => {
            println!("{reply}");
            0
        }
        Err(why) => {
            eprintln!(
                "opi-ctl: search 的输出不像 JSON 文本数组（{why}）—— 插件与本工具版本对不上？"
            );
            eprintln!("       原始输出：{reply}");
            5
        }
    }
}

/// 模式循环（`mode`，2026-10-14 用户裁决）：`Trigger(string:mode)` → **新状态
/// token**（`pinyin` / `en26`，未来 `wubi`），或 err:…。与动作路径的分歧只有一处
/// 但很关键：成功应答**不是** "ok" 而是 token —— 桌面的模式按钮靠它立即显示新档位
/// （见文件头协议段）。落点、退出码表与 state/search 同族：方法名 `Trigger`（不是
/// 新方法）、rc 0 + 载荷进 stdout、形状不对 = 5。
///
/// 不识别的动作（老插件没有 mode 分支）由**服务端**回 err:5 未知动作 —— 措辞那边
/// 更准，本层不重复判（mode 是合法的动词名，本地拒了反而把「插件旧」误报成用法错）。
fn send_mode() -> i32 {
    let reply = match call_bus(&format!("{IFACE}.Trigger"), Some("string:mode")) {
        Ok(r) => r,
        Err(code) => return code,
    };
    // 服务端分类的失败与动作路径同一套映射（err:5 未知动作/内部异常原样透传）。
    if reply.starts_with("err:") {
        return handle_reply(&reply);
    }
    match check_mode_shape(&reply) {
        Ok(()) => {
            println!("{reply}");
            0
        }
        Err(why) => {
            eprintln!("opi-ctl: mode 的输出不是状态 token（{why}）—— 插件与本工具版本对不上？");
            eprintln!("       原始输出：{reply}");
            5
        }
    }
}

/// 档位 token 的取值域：`mode` 动作的应答**与** state 的 `mode=` 格同域（服务端
/// 由同一个函数派生，见 cpp/opi_ctl_mode.h）—— 放一处，免得两份取值域各自漂。
const MODE_TOKENS: [&str; 3] = ["pinyin", "en26", "wubi"];

/// mode 载荷的形状检查：**钉死取值域** `{pinyin, en26, wubi}`（不是「非空即可」）。
/// 钉死而不是放行的理由同 check_state_shape：桌面会把返回值直接当档位显示，放过去
/// 就是把错位的值画到条上。最要防的一种错接线是插件回 `"ok"`（Trigger 的通用成功
/// 应答 —— 说明它把 mode 接到了旧路径）—— 桌面会把 "ok" 当模式名显示出来。
/// `wubi` 服务端现在**还不会发**（五笔未落地）：先放行是前向兼容 —— 引擎侧加档时
/// 客户端不必跟着发版才不红（另一方向不成立：老客户端遇新 token 会判 5，那是
/// 「版本对不上」的如实分类）。
fn check_mode_shape(reply: &str) -> Result<(), String> {
    // **不 trim**：取值域是枚举，前后空白不是「服务端口气」而是另一个值 ——
    // 放行它就等于把「形状检查」降级成「非空检查」（同 check_state_shape 的口径）。
    if MODE_TOKENS.contains(&reply) {
        Ok(())
    } else {
        Err(format!("不是 {MODE_TOKENS:?} 之一：{reply:?}"))
    }
}

/// 搜索载荷的**轻量**形状检查：去首尾空白后 `[` 开头、`]` 结尾。
/// 刻意**只**查这两头：数组里每个元素是不是合法 JS 字符串（转义、代理对）归桌面的
/// 真解析器判 —— 这里手写一个半吊子 JSON 词法器才是真正的风险（两份实现必然漂，
/// 且漂的那份是错的没人知道）。空数组 `[]` 合法（= 没搜到）。
fn check_search_shape(reply: &str) -> Result<(), String> {
    let t = reply.trim();
    if !t.starts_with('[') {
        return Err("开头不是 `[`".to_string());
    }
    if !t.ends_with(']') {
        return Err("结尾不是 `]`".to_string());
    }
    Ok(())
}

/// 冻结的状态串键序与取值域（与 cpp/opi_ctl.h 的 State 注释成对；桌面端按 `;`
/// 分段、按位置取值）。形状漂了**必须**红：宁可让状态栏显示「读不到状态」，也别
/// 让它拿错位的值去画图标 —— 位置解析下 `trad=on` 跑到第一位就是「语言变繁体」。
/// im 那一格没有取值域（输入法名是开放集合，且**空串合法** = 没有可报的输入法：
/// 没有 IC 且 ShareInputState=No 没兜底 —— 见 cpp/opi_ctl.h；前端约定按
/// 「未知 = 维持现状」处理）—— 但**段数与位置**照样钉死。
/// mode（2026-10-14 追加，**行尾**）：取值域 = [`MODE_TOKENS`]（与 `mode` 动作的
/// 应答同域，服务端同一份派生）。段数照旧**钉死** —— 老插件的五行输出在本工具这里
/// 照样判 5（「插件与本工具版本对不上」的如实分类）；对**缺这一格**的容错在桌面
/// 解析侧（Actions.kt 的 [ImeState.mode] 回落「拼音」），不在这一层。
fn check_state_shape(line: &str) -> Result<(), String> {
    const FIELDS: [(&str, Option<&[&str]>); 7] = [
        ("lang", Some(&["zh", "en"])),
        ("trad", Some(&["on", "off"])),
        ("punct", Some(&["on", "off"])),
        ("fw", Some(&["on", "off"])),
        ("im", None),
        ("mode", Some(MODE_TOKENS.as_slice())),
        ("enpro", Some(&["on", "off"])),
    ];
    let parts: Vec<&str> = line.split(';').collect();
    // enpro 是 2026-10-19 追加在 mode 之后的可选末段：老插件 6 段仍可读
    // （桌面按 off 处理），新插件 7 段必须形状正确。
    if !(6..=FIELDS.len()).contains(&parts.len()) {
        return Err(format!("应为 6 或 7 段，实际 {} 段", parts.len()));
    }
    for (i, (key, domain)) in FIELDS.iter().take(parts.len()).enumerate() {
        let prefix = format!("{key}=");
        let Some(val) = parts[i].strip_prefix(prefix.as_str()) else {
            return Err(format!(
                "第 {} 段不是 `{key}=…`（键序错了？）：{:?}",
                i + 1,
                parts[i]
            ));
        };
        if let Some(domain) = domain
            && !domain.contains(&val)
        {
            return Err(format!("{key} 的取值 {val:?} 不在 {domain:?} 里"));
        }
    }
    Ok(())
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
