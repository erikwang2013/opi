// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **导入词库层**（M3 2c，2026-10-14）：fcitx5 轨的 `importDict` / `setDictEnabled` 状态。
//!
//! 与 opi-ffi 的 `api/dict.rs` **同语义同裁决**（那是 Android/桌面/TSF 共用的另一份
//! 单例）—— 三条裁决（数据留在 FFI 层内存、同名重导入 = 原位覆盖、系统词典整条出口
//! 都拒）与它们的理由（含「启用中的词库内存 ×2」这笔代价与升级路径）写在那份文件头，
//! **这里不复制第二遍**；改裁决必须两边同时改。
//!
//! 与那份的唯一区别：本轨**没有列表出口**（`dictList` 归宿主自己扫目录，契约见
//! `cpp/opi_rust_ffi.h`），所以本层不采集「条数给谁看」——条数只出现在 import 的返回值里。
//!
//! 单独一个文件是 500 行硬规矩（`candidate.rs` 421 行，这批再加进去必越线）。

use engine_core::Engine;
use engine_core::dictionary::{Dictionary, InMemoryDictionary};

/// 一条导入词库：名字 + **数据本体** + 启用位。
///
/// 数据留在本层（裁决 1）：停用只是把它从**引擎的**层集合里拿掉，再启用不依赖原文件
/// 还在（宿主删了文件也照样能再启用）。
struct ExtraDict {
    name: String,
    dict: InMemoryDictionary,
    enabled: bool,
}

/// 导入层集合（[`crate::CandidateState`] 的一个字段）。
#[derive(Default)]
pub(crate) struct DictLayer {
    /// 导入顺序；同名重导入**原位**替换（见 `import`）。
    extra: Vec<ExtraDict>,
}

impl DictLayer {
    /// `importDict(path)`：读一份词库文件导入成一层。返回条目数；失败 → **-1**，
    /// 且**既有导入层不动**（读不到 / 格式全坏 / 一条有效条目都没有 / 没有文件名）。
    ///
    /// 与 opi-ffi 那份逐条同语义：名字 = **文件名词干**、导入即启用、同名重导入 =
    /// 原位覆盖并置为启用。
    pub(crate) fn import(&mut self, engine: &mut Engine, path: &str) -> i32 {
        let p = std::path::Path::new(path);
        // 文件名先定：没有词干的路径连导入层都收不下（后面还要用它当名字）。
        let Some(name) = p
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
        else {
            eprintln!("fcitx5-opi: 导入词库被拒绝（路径没有文件名，定不了词库名）: {path}");
            return -1;
        };
        let (dict, skipped) = match engine_data::load_extra_dict(p) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("fcitx5-opi: 导入词库失败（既有导入层不动）: {e}");
                return -1;
            }
        };
        let count = dict.len();
        if count == 0 {
            // 空文件/全坏行：导进来是一层**空壳** —— 界面照常报成功，用户一个词也打不出来。
            // 这正是本仓防的那种「静默失效」，故按失败处理而不是收下一层空的。
            eprintln!("fcitx5-opi: 导入词库被拒绝（没有一条有效条目）: {path}");
            return -1;
        }
        if skipped > 0 {
            // 「坏词库静默失效」的前车之鉴：坏行如实报数，不静默吞（行号与摘要在
            // engine-data 的 Err 里；这里只报总数）。用户看到「导入了 100 条」而不是
            // 「导入了 100 条、另有 20 行没读进去」是两种知情程度。
            eprintln!("fcitx5-opi: 导入词库 {path} 跳过了 {skipped} 行坏行（列数不符）");
        }
        match self.extra.iter_mut().find(|e| e.name == name) {
            // 同名重导入 = 原位覆盖并置为启用（不 push 第二层，否则同一个名字会出现两条）。
            Some(e) => {
                e.dict = dict;
                e.enabled = true;
            }
            None => self.extra.push(ExtraDict {
                name,
                dict,
                enabled: true,
            }),
        }
        self.sync(engine);
        count as i32
    }

    /// `setDictEnabled(name, on)`：装载 / 卸载一条**导入**词库（原子）。
    /// 已是目标态 → true（幂等）；名字认不得（含系统词典）→ false —— 系统词典不是本层
    /// 的层（裁决 3），`on` 与它当前状态一致时也照样 false，别按「已是目标态」推。
    pub(crate) fn set_enabled(&mut self, engine: &mut Engine, name: &str, on: bool) -> bool {
        let Some(e) = self.extra.iter_mut().find(|e| e.name == name) else {
            return false;
        };
        if e.enabled == on {
            return true;
        }
        e.enabled = on;
        self.sync(engine);
        true
    }

    /// 把**当前启用**的层整体交给引擎（`set_extra_dicts` 是全集替换语义，见它的注释）。
    ///
    /// 数据留在本层（裁决 1），所以这里是**重建副本**：`visit_entries` 遍历 + `insert`
    /// （同一 (拼音, 词) 重复插入在 Trie 里取更高频 ⇒ 重建幂等）。只在导入/开关这些
    /// **冷路径**上发生，不在按键热路径。
    fn sync(&mut self, engine: &mut Engine) {
        let enabled: Vec<(String, InMemoryDictionary)> = self
            .extra
            .iter()
            .filter(|e| e.enabled)
            .map(|e| (e.name.clone(), rebuild(&e.dict)))
            .collect();
        engine.set_extra_dicts(enabled);
    }
}

/// 按内存里的数据重建一份副本（见 [`DictLayer::sync`] 头注；与 opi-ffi 的 `rebuild`
/// 逐字同形 —— 两轨都靠 `Trie::insert` 取高频的幂等性）。
fn rebuild(src: &InMemoryDictionary) -> InMemoryDictionary {
    let mut out = InMemoryDictionary::new();
    src.visit_entries(&mut |py, w, f| out.insert(py, w, f));
    out
}
