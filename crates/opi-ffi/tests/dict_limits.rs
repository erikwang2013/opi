// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 导入词库的硬上限（2026-10-24 用户要求「超限就提示达到上限」）。
//! 一条测试跑完两档，避免同进程里多次 `api::install` / 文件夹具互相踩。

use std::fs;
use std::path::PathBuf;

use opi_ffi::api;

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("opi-dict-limit-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("建临时目录");
    dir.join(name)
}

#[test]
fn import_dict_rejects_oversized_file_and_too_many_entries() {
    api::install(None).expect("内置回退词库必装得上");

    // 1) 文件字节上限：8 MiB + 1（在解析之前就拒）。
    let big = temp("big.tsv");
    fs::write(&big, vec![b'a'; (api::MAX_IMPORT_DICT_BYTES + 1) as usize]).expect("写大文件");
    let err = api::with_engine(|a| a.import_dict(big.to_str().unwrap()))
        .expect("引擎已装载")
        .expect_err("超过字节上限必须拒收");
    assert!(err.contains("达到上限"), "字节上限原因要可读：{err}");

    // 2) 条目上限：上限 + 1 条**合法**行（引擎解析后按条数拒）。
    let many = temp("many.tsv");
    let mut body = String::with_capacity((api::MAX_IMPORT_DICT_ENTRIES + 1) * 4);
    for _ in 0..=api::MAX_IMPORT_DICT_ENTRIES {
        body.push_str("a\tb\n");
    }
    fs::write(&many, body).expect("写多条文件");
    let err = api::with_engine(|a| a.import_dict(many.to_str().unwrap()))
        .expect("引擎已装载")
        .expect_err("超过条目上限必须拒收");
    assert!(err.contains("达到上限"), "条目上限原因要可读：{err}");

    let _ = fs::remove_file(big);
    let _ = fs::remove_file(many);
}
