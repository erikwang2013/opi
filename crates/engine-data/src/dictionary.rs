//! 内置 fallback 词库与 load_or_fallback 回退逻辑（M2 Task 8）。

use std::path::Path;
use engine_core::dictionary::Dictionary;
use crate::loader::{load_bytes, load_mmap, MmapDictionary};

/// 编译提交的内置回退词库（data/raw/fallback.tsv → opi-tools → data/generated/fallback.opid）。
/// 提交时经 opi-tools 校验，运行期解析失败即仓库损坏，直接 panic。
fn load_fallback() -> Result<MmapDictionary, String> {
    let bytes: &[u8] = include_bytes!("../../../data/generated/fallback.opid");
    load_bytes(bytes.to_vec()).map_err(|e| format!("内置 fallback 词库损坏: {e:?}"))
}

pub fn fallback_dict() -> MmapDictionary {
    load_fallback().expect("内置 fallback 词库损坏（提交时已校验）")
}

/// 加载指定词库；文件缺失/损坏一律回退内置词库。仅当内置词库本身损坏才返回 Err。
pub fn load_or_fallback(path: Option<&Path>) -> Result<Box<dyn Dictionary>, String> {
    if let Some(p) = path
        && let Ok(d) = load_mmap(p)
    {
        return Ok(Box::new(d));
    }
    load_fallback().map(|d| Box::new(d) as Box<dyn Dictionary>)
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine_core::dictionary::Dictionary;

    #[test]
    fn fallback_has_35_common_words() {
        let d = fallback_dict();
        assert_eq!(d.len(), 35);
        assert!(!d.is_empty());
    }

    #[test]
    fn fallback_queries_by_pinyin() {
        let d = fallback_dict();
        let wo = d.query("wo", 8);
        assert_eq!(wo[0].word, "我");
        let ni = d.query("n", 8);
        assert!(ni.iter().any(|e| e.word == "你"));
    }

    #[test]
    fn fallback_freqs_descending() {
        let d = fallback_dict();
        let hao = d.query("hao", 8);
        assert!(hao.windows(2).all(|w| w[0].freq >= w[1].freq));
    }

    #[test]
    fn load_or_fallback_none_returns_fallback() {
        let d = load_or_fallback(None).unwrap();
        assert_eq!(d.len(), 35);
    }

    #[test]
    fn load_or_fallback_missing_file_returns_fallback() {
        let d = load_or_fallback(Some(Path::new("/nonexistent/opi.opid"))).unwrap();
        assert_eq!(d.len(), 35);
    }

    #[test]
    fn load_or_fallback_corrupt_file_returns_fallback() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("opi-fb-{}", std::process::id()));
        std::fs::write(&path, b"OPID\x01\x00\x00garbage").unwrap();
        let d = load_or_fallback(Some(&path)).unwrap();
        let _ = std::fs::remove_file(&path);
        let wo = d.query("wo", 8);
        assert_eq!(wo[0].word, "我");
    }

    #[test]
    fn load_or_fallback_valid_file_uses_it() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("opi-fb-valid-{}", std::process::id()));
        let bytes = crate::format::serialize(&crate::format::OpDict {
            entries: vec![crate::format::RawEntry {
                pinyin: "opa".into(),
                word: "测试".into(),
                freq: 1,
            }],
            pinyin_total: 3,
        });
        std::fs::write(&path, &bytes).unwrap();
        let d = load_or_fallback(Some(&path)).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(d.len(), 1);
        assert_eq!(d.query("opa", 8)[0].word, "测试");
    }
}
