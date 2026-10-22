// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **导入词库层**（M3，2026-10-14）：`importDict` / `dictList` / `setDictEnabled` /
//! `removeDict` 的薄壳实现 —— 两个 ABI 面（JNI `jni_dict.rs` + C `cabi_dict.rs`）
//! 共用这一份，上接 `Engine::set_extra_dicts` / `Engine::extra_dicts`。
//!
//! 单独一个文件是 500 行硬规矩（同 `convert.rs` 的分家思路）：`api/mod.rs` 已经
//! 461 行，这批再加进去必越线。
//!
//! ## 三条语义裁决（是契约，别当实现细节看）
//!
//! 1. **数据留在本层内存里**。`setDictEnabled(name, false)` 只把这条从**引擎的**
//!    额外层里拿掉，本层的词典数据不丢 ⇒「导入 → 停用 → 再启用」**不依赖原文件
//!    还在**（宿主删了/改了文件也照样能再启用）。代价如实记：启用中的词库内存 ×2，
//!    且每次同步要把启用子集**重建**一份交给引擎 —— `set_extra_dicts` 按值收、没有
//!    回读口，交出去就拿不回来了（`extra_dicts()` 只回名字与条数）。同步只在导入 /
//!    开关 / 删除这些**设置页冷路径**上发生，不在按键热路径。
//!    （将来若省这份内存：让 `set_extra_dicts` 收 `Arc` 或让它返回旧集合。）
//! 2. **同名重导入 = 原位覆盖**，并**置为启用** —— 导入动作的语义就是「我要用它」；
//!    宿主的停用配置在导入之后重放（fcitx5 扫目录导入完再按 `DisabledDicts` 停用）。
//! 3. **系统词典（luna/trad/stroke）不接受 `setDictEnabled`**：它们不是本层的层，
//!    返回 false。宿主 UI 按 `system: true` 把开关隐藏掉（它们启不启用是宿主自己的
//!    事：fcitx5/Android 装不装 trad.opid、TsfLogic 读不读 stroke）。
//!
//! 4. **排序与兜底归引擎侧统一裁决**（2026-10-17 用户裁决「是词库就要抢位」「超时
//!    就优先使用自带词库」；2026-10-22 总账 30ms）：抢位加成（**广覆盖**，重复项取 max）与 20ms 扫描截止
//!    都在 `engine-core/src/dict_layers.rs`（`boosted_import_freq` /
//!    `IMPORT_SCAN_BUDGET`），本层不重复实现、不给宿主开预算开关
//!    （测试口是 `Engine::set_import_scan_budget`）。宿主可见的变化只是候选表本身。
//!
//! 名字 = **文件名词干**（`foo.tsv` → `foo`，`a.b.tsv` → `a.b`）：与桌面设置页
//! （`desktop/.../Dicts.kt`）和 `DisabledDicts` 停用名单里写的那个名字同一份口径。

use std::path::Path;

use engine_core::dictionary::{Dictionary, InMemoryDictionary};
use engine_core::router::KeyRouter;
use serde_json::json;

use super::Api;

/// 引擎未装载时 `dictList` 的哨兵：**JSON 空数组**。
///
/// 家族约定：数组类出口的哨兵是 `[]` 而不是空串（同 `opi_candidates` /
/// `opi_symbols_in_block`）—— 宿主直接 `parse` 就得到空列表，不必为「未装载」特判。
pub const DICT_LIST_UNLOADED: &str = "[]";

/// 一条导入词库：名字 + 数据 + 启用位 + 条目数。
pub(crate) struct ExtraDict {
    pub(crate) name: String,
    pub(crate) dict: InMemoryDictionary,
    /// 条目数（导入时刻算好）：`dictList` 直接用，不必替宿主每次再数一遍。
    pub(crate) count: usize,
    /// 启用位：true = 已交给引擎的额外层；false = 只留在本层（裁决 1）。
    pub(crate) enabled: bool,
}

/// 词库层状态（[`Api`] 的一个字段）。`Default` = 空层：没有导入词库、
/// luna 条数为 0（真值由装载入口采集，见 [`Api::with_router`]）。
#[derive(Default)]
pub(crate) struct DictLayer {
    /// 导入层，**导入顺序**（`dictList` 的次序稳定）；同名重导入**原位**替换。
    extra: Vec<ExtraDict>,
    /// 系统词典 luna（主词典）的条目数 —— 装载时刻采集（engine-core 不暴露读口）。
    luna_count: usize,
    /// 系统词典 trad / stroke：`Some(n)` = 已装载（= `dictList` 的 `enabled`）。
    /// 写口只有 `install_trad` / `install_stroke` 两处，故状态与本层同步维护。
    trad_count: Option<usize>,
    stroke_count: Option<usize>,
}

impl Api {
    /// 构造（带 luna 条目数）：三个装载入口（`install` / `load_sync` /
    /// `load_fallback_sync`）**都走这里**，免得将来多一个入口时忘了采集系统词典条数
    /// （`dictList` 里 `{name:"luna", count}` 的真源就在这一步）。
    pub(crate) fn with_router(router: KeyRouter, luna_count: usize) -> Self {
        Api {
            router,
            dicts: DictLayer {
                luna_count,
                ..DictLayer::default()
            },
        }
    }

    /// 记下繁体词典（trad.opid）的条目数。`Some`/`None` 就是 `dictList` 里
    /// `{name:"trad", enabled}` 的真源（engine-core 只有写口 `set_trad_dict`，没读口）。
    pub(crate) fn note_trad_count(&mut self, n: usize) {
        self.dicts.trad_count = Some(n);
    }

    /// 记下笔画词典（stroke.opid）的条目数（同 [`Self::note_trad_count`]）。
    pub(crate) fn note_stroke_count(&mut self, n: usize) {
        self.dicts.stroke_count = Some(n);
    }

    /// importDict(path)：读文件 →（同名则）原位替换 → 进引擎。返回**条目数**。
    ///
    /// 名字 = 文件名词干；宿主传的是**绝对路径**（Android 侧 `filesDir/dicts/foo.tsv`），
    /// 相对路径也照收（本层不拼目录 —— 目录口径是宿主的事，见桌面 `Dicts.kt`）。
    /// 失败（读不到 / 全坏 / 一条有效条目都没有）→ `Err`，两个 ABI 面都折成 `-1`。
    pub fn import_dict(&mut self, path: &str) -> Result<usize, String> {
        let p = Path::new(path);
        let name = p
            .file_stem()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| format!("词库路径取不出文件名词干：{path}"))?
            .to_string();
        // ⚠️ `load_extra_dict` 对「全坏」已经返 Err（见 dict_text.rs），但对**空文件**
        // 返 Ok(空词典) —— 这里补上「一条有效条目都没有 = 失败」：空词库装上只会让
        // 用户以为导成功了（宿主 UI 显示 0 条、打什么都不出字）。
        let (dict, skipped) = engine_data::load_extra_dict(p)?;
        let count = dict.len();
        if count == 0 {
            return Err(format!("导入词库 {path} 没有有效条目"));
        }
        if skipped > 0 {
            // 部分坏行**不**失败（其余条目照样能用），但不能一声不吭：本仓有过
            // 「坏词库静默失效」的前车之鉴（fcitx5 的 loadDictionary，见其注释）。
            eprintln!("[opi] 导入词库 {path}：跳过 {skipped} 行坏行，载入 {count} 条");
        }
        match self.dicts.extra.iter_mut().find(|e| e.name == name) {
            Some(e) => {
                e.dict = dict;
                e.count = count;
                e.enabled = true; // 同名重导入 = 覆盖并启用（文件头裁决 2）
            }
            None => self.dicts.extra.push(ExtraDict {
                name,
                dict,
                count,
                enabled: true,
            }),
        }
        self.sync_extra_dicts();
        Ok(count)
    }

    /// setDictEnabled(name, on)：把一条导入词库装进 / 移出引擎的额外层。
    ///
    /// **原子**：一次 `set_extra_dicts` 全量替换，引擎看不到半个集合（裁决 1 的数据
    /// 留存保证「停用后再启用」拿得到原数据）。名字认不得（含系统词典名
    /// luna/trad/stroke）→ false；**已经是目标态 → true**（幂等，且不重复同步）。
    pub fn set_dict_enabled(&mut self, name: &str, on: bool) -> bool {
        let Some(e) = self.dicts.extra.iter_mut().find(|e| e.name == name) else {
            return false;
        };
        if e.enabled == on {
            return true;
        }
        e.enabled = on;
        self.sync_extra_dicts();
        true
    }

    /// removeDict(name)：把一条导入词库**从内存里**删掉（引擎的额外层跟着重算）。
    ///
    /// **文件不归本层管**：调用方（宿主）自己删 `*.tsv`（桌面/Android 的删除键）。
    /// 删掉之后本条再 `setDictEnabled` 找不到 → false（要重新 importDict）。
    /// 名字认不得（含系统词典）→ false。
    pub fn remove_dict(&mut self, name: &str) -> bool {
        let before = self.dicts.extra.len();
        self.dicts.extra.retain(|e| e.name != name);
        if self.dicts.extra.len() == before {
            return false;
        }
        self.sync_extra_dicts();
        true
    }

    /// dictList()：JSON 数组，**系统词典在前**（luna / trad / stroke，`system: true`），
    /// 导入层随后（导入顺序）。每项 `{name, count, enabled, system}`。
    ///
    /// ⚠️ 键序是 serde_json 的**字典序**（`count, enabled, name, system`）—— JSON 对象
    /// 的键序无意义，宿主按**键**取；门禁（`tests/dict_import.rs`）也只按键取，
    /// 谁都不许按位置下标解析。
    ///
    /// 系统词典的 count 是**装载时刻**采集的条目数；trad/stroke 未装载 → 0 + enabled
    /// false（不是「读不出来」，是真的没这层）。
    pub fn dict_list(&self) -> String {
        let mut arr: Vec<serde_json::Value> = vec![
            json!({ "name": "luna", "count": self.dicts.luna_count, "enabled": true, "system": true }),
            json!({
                "name": "trad",
                "count": self.dicts.trad_count.unwrap_or(0),
                "enabled": self.dicts.trad_count.is_some(),
                "system": true,
            }),
            json!({
                "name": "stroke",
                "count": self.dicts.stroke_count.unwrap_or(0),
                "enabled": self.dicts.stroke_count.is_some(),
                "system": true,
            }),
        ];
        arr.extend(self.dicts.extra.iter().map(
            |e| json!({ "name": e.name, "count": e.count, "enabled": e.enabled, "system": false }),
        ));
        serde_json::Value::Array(arr).to_string()
    }

    /// 把**当前启用**的导入层整体交给引擎（`set_extra_dicts` 是全量替换语义）。
    ///
    /// 数据留在本层（裁决 1），所以这里是**重建副本**：`visit_entries` 零拷贝遍历 +
    /// `insert`（同一 (拼音, 词) 重复插入在 Trie 里取更高频，故重建幂等）。
    fn sync_extra_dicts(&mut self) {
        let enabled: Vec<(String, InMemoryDictionary)> = self
            .dicts
            .extra
            .iter()
            .filter(|e| e.enabled)
            .map(|e| (e.name.clone(), rebuild(&e.dict)))
            .collect();
        self.router.engine_mut().set_extra_dicts(enabled);
    }
}

/// 按内存里的数据重建一份词典副本（见 [`Api::sync_extra_dicts`] 头注）。
fn rebuild(src: &InMemoryDictionary) -> InMemoryDictionary {
    let mut out = InMemoryDictionary::new();
    src.visit_entries(&mut |py, w, f| out.insert(py, w, f));
    out
}
