// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 「清空用户词」的**世代标记**（`%LOCALAPPDATA%\opi\clear_generation`）。
//!
//! 为什么需要：TSF 的 Learner 在**每个宿主进程的内存里各一份**，控制面管道一次
//! 只能清当时连上的那个宿主。设置窗每次清词写一个新的 token 到这里；每个引擎在
//! 下次激活/线程焦点时读它，发现与自己上次见过的不一样，就清本进程的 Learner 并
//! 记下 —— 即「按宿主、下次焦点生效」。不枚举进程，也不会误清之后新学的词
//! （引擎只在焦点到来时比对一次）。
//!
//! 平台中立：路径推导与「变了没有」正是静默失败的部分（读错文件/永远相等 =
//! 用户以为清了其实没清），故放在 Linux 主机也能单测的模块里。写侧在 desktop
//! 的 `UserWords.kt`（文件名与目录两边必须一致）；消费侧在 `state.rs` 的
//! `EngineShared::check_clear_generation`（Windows 目标专属，本机跑不到）。
//!
//! 用 token 而不是单调计数器：设置窗可能并发清词，读-改-写计数器会互相盖回；
//! 唯一 token 每次落盘都不同，谁最后写谁生效，不需要锁也不需要 fsync ——
//! 读到半个 token 的最坏结果是多清一次（幂等），不会漏清。

use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::Mutex;

/// 与 desktop `UserWords.kt::clearGenerationFile` 的文件名**必须一致**。
pub const CLEAR_GENERATION_FILE_NAME: &str = "clear_generation";

/// `%LOCALAPPDATA%\opi\clear_generation`（与 `settings::path_in` 同目录同口径：
/// 缺省/空串 → 相对路径 `opi/clear_generation`，不 panic，读不到就当没清过）。
pub fn path_in(local_appdata: Option<&OsStr>) -> PathBuf {
    let base = local_appdata
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_default();
    base.join("opi").join(CLEAR_GENERATION_FILE_NAME)
}

/// 读当前世代。**读不到 / 空 / 纯空白 = None**（没清过，或上次写入失败）：
/// 引擎保持现状，下一次焦点再读。读取失败不报错、不落日志 —— 它是每次焦点都
/// 会发生的常规路径，不是异常。
pub fn read(local_appdata: Option<&OsStr>) -> Option<String> {
    let text = std::fs::read_to_string(path_in(local_appdata)).ok()?;
    let token = text.trim();
    (!token.is_empty()).then(|| token.to_string())
}

/// 本引擎「上次见过并清过」的 token。每个宿主进程的每个引擎各一份
/// （挂在 `EngineShared` 上）。
#[derive(Default)]
pub struct SeenGeneration {
    seen: Mutex<Option<String>>,
}

impl SeenGeneration {
    /// 新 token → 记下并返回 true（调用方负责清 Learner）；None / 与上次相同 /
    /// 锁中毒 → false。
    ///
    /// **先记后清**：`TsfLogic::clear_user_words` 没有失败路径，故记录不会抢在
    /// 一次失败之前。并发焦点同时到达时只有一个调用拿到 true，避免同一宿主重复清；
    /// 极端窗口里另一线程刚学到的词可能被这次清掉，可接受（清词本来就是用户刚点的）。
    pub fn take_if_changed(&self, current: Option<&str>) -> bool {
        let Some(current) = current else { return false };
        match self.seen.lock() {
            Ok(mut seen) if seen.as_deref() != Some(current) => {
                *seen = Some(current.to_string());
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "opi-clear-gen-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn path_mirrors_settings_dir() {
        assert_eq!(
            PathBuf::from("C:/L/opi/clear_generation"),
            path_in(Some(OsStr::new("C:/L")))
        );
        // 缺省/空串 = 未设置（与 settings.rs / dict_path.rs 同口径）。
        assert_eq!(PathBuf::from("opi/clear_generation"), path_in(None));
        assert_eq!(
            PathBuf::from("opi/clear_generation"),
            path_in(Some(OsStr::new("")))
        );
    }

    #[test]
    fn read_ignores_missing_blank_and_trims() {
        let base = tmp_dir("read");
        let opi = base.join("opi");
        std::fs::create_dir_all(&opi).unwrap();
        let file = opi.join(CLEAR_GENERATION_FILE_NAME);
        let base_s = base.as_os_str();

        // 文件不存在 = 没清过。
        assert_eq!(None, read(Some(base_s)));

        std::fs::write(&file, "  \r\n\t").unwrap();
        assert_eq!(None, read(Some(base_s)), "空白 token 不能触发清词");

        std::fs::write(&file, "token-1\r\n").unwrap();
        assert_eq!(Some("token-1".to_string()), read(Some(base_s)));

        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn seen_generation_fires_once_per_token_and_ignores_none() {
        let seen = SeenGeneration::default();
        assert!(!seen.take_if_changed(None), "没有标记不能清");
        assert!(seen.take_if_changed(Some("a")), "第一次见到 token 要清");
        assert!(!seen.take_if_changed(Some("a")), "同一 token 不重复清");
        assert!(seen.take_if_changed(Some("b")), "新 token 再清一次");
        assert!(seen.take_if_changed(Some("a")), "旧 token 又出现也算新世代");
    }
}
