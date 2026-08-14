//! XDG 数据目录 + luna.opid 词库装载层（B3）。
//!
//! 语义对照 Android EngineLoader.kt（权威参考）：
//! - 目标文件缺失或 size 与资产不一致 → 重拷（插件升级后资产更新而旧文件残留）
//! - size 一致 → 跳过拷贝（幂等，防陈旧词库）
//! - 本层不吞 I/O 错误：失败原样返回 Err，由调用方（lib.rs C 入口）回退内置
//!   35 词词库（对照 EngineLoader.loadAsset 抛错 → 调用方 load(null)）。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::install;

/// 词库文件名（镜像 EngineLoader.FILE_NAME = "luna.opid"）。
pub const DICT_FILE_NAME: &str = "luna.opid";

/// OPI 数据目录（与 C++ 胶水 `StandardPath::Type::Data` + "opi/..." 探测路径
/// 一致）：
/// - $XDG_DATA_HOME 非空 → $XDG_DATA_HOME/opi
/// - 否则 → $HOME/.local/share/opi（XDG Base Directory 规范默认值；
///   XDG_DATA_HOME 为空或未设置时同此；HOME 缺失 → 相对 ".local/share/opi"）
pub fn xdg_data_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(xdg).join("opi");
    }
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share/opi")
}

/// size 校验重拷规则（镜像 EngineLoader.needsCopy：缺失或 size 不一致即重拷）。
pub fn needs_copy(existing_size: Option<u64>, asset_size: u64) -> bool {
    match existing_size {
        None => true,
        Some(size) => size != asset_size,
    }
}

/// 建数据目录（递归）。镜像 EngineLoader 写盘行为：目录缺失即创建。
pub fn ensure_dirs(data_dir: &Path) -> io::Result<()> {
    fs::create_dir_all(data_dir)
}

/// size 校验后把 luna.opid 拷贝进数据目录，返回目标路径。
/// 目标缺失或 size 不一致 → 拷贝；一致 → 跳过（幂等）。
/// 失败原样返回 Err（对照 EngineLoader：write 抛 IOException → 调用方回退）。
pub fn ensure_dict(source: &Path, data_dir: &Path) -> io::Result<PathBuf> {
    let target = data_dir.join(DICT_FILE_NAME);
    let asset_size = source.metadata()?.len();
    let existing = fs::metadata(&target).ok().map(|m| m.len());
    if needs_copy(existing, asset_size) {
        fs::copy(source, &target)?;
    }
    Ok(target)
}

/// 初始化编排（对照 EngineLoader.loadAsset 顺序）：建目录 → size 校验拷贝 →
/// 装载引擎。任一步失败返回 Err（不吞错误，由 C 入口回退内置词库——
/// 对照 EngineLoader：loadAsset 失败 → load(null)）。
pub fn init_dict(source: &Path, data_dir: &Path) -> Result<(), String> {
    ensure_dirs(data_dir).map_err(|e| format!("建数据目录失败（{data_dir:?}）: {e}"))?;
    let target = ensure_dict(source, data_dir).map_err(|e| format!("词库拷贝失败: {e}"))?;
    let path = target
        .to_str()
        .ok_or_else(|| format!("数据目录路径非 UTF-8（{data_dir:?}）"))?;
    install(Some(path)).map_err(|e| format!("词库装载失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_data::format::{OpDict, RawEntry};
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// 环境变量互斥：set_var/remove_var 是全局副作用，串行防并行测试竞争
    /// （对照 opi-ffi cabi_test.rs 的 SERIAL 模式）。
    static ENV_LOCK: Mutex<()> = Mutex::new(());
    /// 引擎单例互斥：本模块内触碰 SINGLETON 的测试串行执行。
    static SINGLETON_LOCK: Mutex<()> = Mutex::new(());

    /// 唯一临时目录（并行测试互不冲突），测试结束清理。
    fn temp_dir(tag: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "opi-b3-{tag}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir); // 清理上次遗留
        fs::create_dir_all(&dir).expect("建临时目录");
        dir
    }

    /// 合法 luna.opid 内容（engine_data::format::serialize；含 wo→我，
    /// 与内置回退词库同命中，便于断言回退语义）。
    fn valid_dict_bytes() -> Vec<u8> {
        engine_data::format::serialize(&OpDict {
            entries: vec![
                RawEntry {
                    pinyin: "hao".into(),
                    word: "好".into(),
                    freq: 5000,
                },
                RawEntry {
                    pinyin: "wo".into(),
                    word: "我".into(),
                    freq: 5000,
                },
            ],
            pinyin_total: 5,
        })
    }

    #[test]
    fn needs_copy_matrix() {
        // 镜像 EngineLoader.needsCopy：缺失或 size 不一致 → 重拷
        assert!(needs_copy(None, 100));
        assert!(!needs_copy(Some(100), 100)); // size 一致 → 跳过
        assert!(needs_copy(Some(99), 100)); // 偏小 → 拷
        assert!(needs_copy(Some(101), 100)); // 偏大 → 拷
    }

    #[test]
    fn xdg_data_home_respected() {
        let _g = ENV_LOCK.lock().unwrap();
        let tmp = temp_dir("xdg");
        // Safety: edition 2024 起 set_var/remove_var 为 unsafe；ENV_LOCK 串行保护
        unsafe {
            std::env::set_var("XDG_DATA_HOME", &tmp);
            std::env::set_var("HOME", "/nonexistent/home");
        }
        assert_eq!(xdg_data_dir(), tmp.join("opi"));
        unsafe {
            std::env::remove_var("XDG_DATA_HOME");
            std::env::remove_var("HOME");
        }
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn xdg_falls_back_to_home_local_share() {
        let _g = ENV_LOCK.lock().unwrap();
        let tmp = temp_dir("home");
        // Safety: 同上（XDG_DATA_HOME 未设置 → HOME/.local/share/opi）
        unsafe {
            std::env::remove_var("XDG_DATA_HOME");
            std::env::set_var("HOME", &tmp);
        }
        assert_eq!(xdg_data_dir(), tmp.join(".local/share/opi"));
        unsafe { std::env::remove_var("HOME") };
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn xdg_empty_data_home_falls_back() {
        let _g = ENV_LOCK.lock().unwrap();
        let tmp = temp_dir("empty");
        // Safety: 同上（XDG 规范：XDG_DATA_HOME 为空等同未设置）
        unsafe {
            std::env::set_var("XDG_DATA_HOME", "");
            std::env::set_var("HOME", &tmp);
        }
        assert_eq!(xdg_data_dir(), tmp.join(".local/share/opi"));
        unsafe {
            std::env::remove_var("XDG_DATA_HOME");
            std::env::remove_var("HOME");
        }
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn ensure_dict_copies_only_when_needed() {
        let src_dir = temp_dir("src");
        let data_dir = temp_dir("data");
        let src = src_dir.join(DICT_FILE_NAME);
        fs::write(&src, valid_dict_bytes()).unwrap();
        // 首次：目标缺失 → 拷贝
        let target = ensure_dict(&src, &data_dir).unwrap();
        assert_eq!(target, data_dir.join(DICT_FILE_NAME));
        assert_eq!(fs::read(&target).unwrap(), fs::read(&src).unwrap());
        // size 一致 → 跳过拷贝：篡改目标（同 size）后重跑，内容不被覆盖
        let bytes = fs::read(&target).unwrap();
        fs::write(&target, vec![0xabu8; bytes.len()]).unwrap();
        ensure_dict(&src, &data_dir).unwrap();
        assert_eq!(fs::read(&target).unwrap()[..1], [0xab]);
        // size 不一致 → 重拷恢复
        fs::write(&target, b"stale").unwrap();
        ensure_dict(&src, &data_dir).unwrap();
        assert_eq!(fs::read(&target).unwrap(), fs::read(&src).unwrap());
        let _ = fs::remove_dir_all(&src_dir);
        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn ensure_dict_missing_source_errors() {
        let data_dir = temp_dir("bad");
        // 源缺失 → Err 原样上抛（不吞错误，由调用方决定回退）
        assert!(ensure_dict(Path::new("/nonexistent/luna.opid"), &data_dir).is_err());
        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn init_dict_copies_and_loads() {
        let _g = SINGLETON_LOCK.lock().unwrap();
        let src_dir = temp_dir("isrc");
        let data_dir = temp_dir("idata");
        let src = src_dir.join(DICT_FILE_NAME);
        fs::write(&src, valid_dict_bytes()).unwrap();
        init_dict(&src, &data_dir).expect("init 成功");
        // 拷贝完成 + 引擎在位；二次调用幂等（size 一致 → 跳过拷贝）
        assert_eq!(
            fs::read(data_dir.join(DICT_FILE_NAME)).unwrap(),
            fs::read(&src).unwrap()
        );
        init_dict(&src, &data_dir).expect("重跑成功");
        assert!(crate::with_state(|s| s.buffer()).is_some());
        let _ = fs::remove_dir_all(&src_dir);
        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn init_dict_bad_source_errors() {
        let _g = SINGLETON_LOCK.lock().unwrap();
        let data_dir = temp_dir("ierr");
        assert!(init_dict(Path::new("/nonexistent/luna.opid"), &data_dir).is_err());
        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn c_entry_init_dict_ok_and_fallback() {
        let _g = SINGLETON_LOCK.lock().unwrap();
        // 好源 → 成功且词库装载（wo → 我）
        let src_dir = temp_dir("csrc");
        let data_dir = temp_dir("cdata");
        let src = src_dir.join(DICT_FILE_NAME);
        fs::write(&src, valid_dict_bytes()).unwrap();
        let src_s = src.to_string_lossy().into_owned();
        let dir_s = data_dir.to_string_lossy().into_owned();
        let ok = unsafe {
            crate::opi_fcitx5_init_dict(src_s.as_ptr(), src_s.len(), dir_s.as_ptr(), dir_s.len())
        };
        assert!(ok);
        let hits_wo = crate::with_state(|s| {
            s.input_key('w');
            s.input_key('o');
            s.candidates().contains(&"我".to_string())
        });
        assert_eq!(hits_wo, Some(true));
        // 坏源 → false + 内置回退词库装载（不崩溃；wo → 我 仍命中）
        let bad = "/nonexistent/luna.opid";
        let ok = unsafe {
            crate::opi_fcitx5_init_dict(bad.as_ptr(), bad.len(), dir_s.as_ptr(), dir_s.len())
        };
        assert!(!ok);
        let hits_wo = crate::with_state(|s| {
            s.input_key('w');
            s.input_key('o');
            s.candidates().contains(&"我".to_string())
        });
        assert_eq!(hits_wo, Some(true));
        let _ = fs::remove_dir_all(&src_dir);
        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn c_entry_null_params_fallback() {
        let _g = SINGLETON_LOCK.lock().unwrap();
        // 参数缺失（null）→ false + 内置回退装载，不崩溃
        let ok = unsafe { crate::opi_fcitx5_init_dict(std::ptr::null(), 0, std::ptr::null(), 0) };
        assert!(!ok);
        assert!(crate::with_state(|s| s.buffer()).is_some());
    }
}
