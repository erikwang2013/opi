// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 读者先退出时的行为（`opi-tools … | head`）：**不能 panic**，也不该丢下工作。
//!
//! 为什么单独一个文件：`cli_boundary.rs` 已经 494 行（500 行硬规矩），且这里测的是
//! 进程的**流**行为，不是参数 / 路径 / 格式边界。
//!
//! 成因是 Rust 的既有语义：runtime 把 SIGPIPE 设成 SIG_IGN，于是往「读者已走」的管道
//! 写会拿到 `BrokenPipe` 错误，而 `println!` / `eprintln!` 对这个错误的反应是 **panic**
//! ——`thread 'main' panicked at .../stdio.rs: failed printing to stdout: Broken pipe`，
//! 退出码 101。对一个日常会被用的用法（`| head`）来说，那是崩溃不是失败。
//!
//! 构造必须**确定性**：stdout/stderr 交给我们自己造的一对 socketpair 的写端，
//! **在 spawn 之前就把读端关掉** —— 子进程的第一次写必然 EPIPE，不抢时序。
//! （靠 `head` 去抢时序的写法在输出小于管道缓冲时会变成假绿：子进程早就写完了。）
//!
//! **这里钉不住的一条**：「非 EPIPE 的写失败仍要报错退出」（`emit` 的 else 分支）。
//! 试过用只读 fd 当 stdout 造 EBADF（raw `write(2)` 确实返回 -1 / EBADF），但本机
//! rustc 1.97.1 上 `std::io::stdout().lock().write_fmt(..)` 对同一个 fd 返回的是
//! **`Ok(())`** —— 失败压根到不了 `main`，CLI 层无从断言，硬写只会得到一条
//! 「永远绿但什么也没测」的假钉子。那条策略因此只以代码形态留在 `main.rs::emit`。

/// 一个「读者已经走掉」的 fd：写它就 EPIPE。
#[cfg(unix)]
fn dead_writer() -> std::process::Stdio {
    use std::os::fd::OwnedFd;
    let (write_end, read_end) = std::os::unix::net::UnixStream::pair().expect("socketpair");
    drop(read_end);
    std::process::Stdio::from(OwnedFd::from(write_end))
}

#[cfg(unix)]
fn run_with(
    streams: (std::process::Stdio, std::process::Stdio),
    args: &[&str],
) -> std::process::Output {
    use std::process::Command;
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_opi-tools"));
    cmd.args(args).stdout(streams.0).stderr(streams.1);
    let child = cmd.spawn().expect("跑 opi-tools");
    // `wait_with_output` 只收 piped 的那条流；另一条已交给我们造的 fd，不会被改写。
    child.wait_with_output().expect("等 opi-tools")
}

#[cfg(unix)]
fn run_with_dead_stream(dead: Stream, args: &[&str]) -> std::process::Output {
    use std::process::Stdio;
    match dead {
        Stream::Stdout => run_with((dead_writer(), Stdio::piped()), args),
        Stream::Stderr => run_with((Stdio::piped(), dead_writer()), args),
    }
}

#[cfg(unix)]
#[derive(Clone, Copy)]
enum Stream {
    Stdout,
    Stderr,
}

#[cfg(unix)]
struct Dir(std::path::PathBuf);

#[cfg(unix)]
impl Dir {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!("opi-pipe-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("建临时目录");
        Dir(p)
    }
}

#[cfg(unix)]
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 成功路径：读者先走不是错误，**但产物必须照常落盘**。
///
/// 「安静收工」有两种写法，只有一种是对的：写失败就当场 `exit(0)` 会让
/// `opi-tools compile a.tsv b.opid | head -1` 在**产物还没写**的情况下返回 0 ——
/// 那正是本项目最怕的「exit 0 但什么也没做」。所以这里两条一起钉：
/// 退出码 0 + 产物真的是一个完整的 `.opid`。
#[cfg(unix)]
#[test]
fn dead_stdout_must_not_panic_and_must_not_abandon_the_work() {
    let dir = Dir::new("stdout");
    let tsv = dir.0.join("t.tsv");
    let opid = dir.0.join("t.opid");
    std::fs::write(&tsv, "好\thao\n号\thao\t1200\n").unwrap();

    let out = run_with_dead_stream(
        Stream::Stdout,
        &["compile", tsv.to_str().unwrap(), opid.to_str().unwrap()],
    );
    let se = String::from_utf8_lossy(&out.stderr);
    assert!(
        !se.contains("panicked at"),
        "读者先退出不是崩溃的理由（现状：stdio.rs 的 Broken pipe panic）：{se}"
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "EPIPE 该安静收工（101 = panic，其它非 0 = 把读者的离开当成了自己的失败）：{se}"
    );
    assert!(
        !se.contains("write stdout"),
        "broken pipe 不该被当成写错误报出来：{se}"
    );
    let bytes = std::fs::read(&opid).expect("读者先退出不该让产物不落盘");
    assert_eq!(
        &bytes[0..4],
        b"OPID",
        "产物必须完整写出（少了这条，一个「EPIPE 就 exit(0)」的实现也能全绿）"
    );
}

/// 失败路径：stderr 的读者走掉时，**退出码不能跟着消失**。
///
/// 退出码是与读者的唯一契约（文本可以没人看，`$?` 永远有人看）：
/// 坏文件仍须 exit 1 —— 既不能 panic 成 101，也不能因为「报错写不出去」就变成 0。
#[cfg(unix)]
#[test]
fn dead_stderr_must_not_turn_a_failure_into_a_panic_or_a_success() {
    let dir = Dir::new("stderr");
    let bad = dir.0.join("bad.opid");
    std::fs::write(&bad, b"not an opid at all\n").unwrap();

    let out = run_with_dead_stream(Stream::Stderr, &["verify", bad.to_str().unwrap()]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "坏文件仍须 exit 1（101 = panic 在报错路上，0 = 把「报不出去」当成了没事）"
    );
}
