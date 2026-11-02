// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **导入词库装载**（M3 2c，2026-10-14）：扫 `%LOCALAPPDATA%\opi\dicts` 的 `*.tsv`，
//! 跳过 `DisabledDicts` 名单里的名字，整体交给引擎（`Engine::set_extra_dicts`）。
//!
//! **与另外三端同一份契约**（正式文本在 `desktop/.../candidate/Dicts.kt` 的头注释；
//! Linux 轨的镜像在 `crates/fcitx5-opi/cpp/opi_dicts.h`）：
//!   · 目录：Windows = `%LOCALAPPDATA%\opi\dicts`（推导在 `dict_path::dicts_dir`）；
//!   · 文件 = `*.tsv` 的普通文件，列序 **词<TAB>拼音[<TAB>频次]** —— 解析器
//!     （`engine-data/src/dict_text.rs` 的 `cols[0] = word`）就是按这个顺序读的，
//!     别照 Dicts.kt 头注释里那句「拼音<TAB>词」抄；
//!   · 词库名 = 文件名去 `.tsv`（停用名单里写的就是这个名字）；
//!   · `DisabledDicts` = 逗号分隔、去空白、丢空段；**空 = 全部启用**（与 `FuzzyPairs`
//!     反着来 —— 那是「空 = 回落总开关」）；
//!   · 名单在 `Activate` 与控制面 `reload` 时读（TSF 侧 = `TsfLogic::apply_settings`，
//!     与桌面窗写的提示文案一字不差）。
//!
//! **与 fcitx5 轨 `dict_layer.rs` 的不对称（刻意，不是漏做）**：本轨**没有**
//! `importDict` / `setDictEnabled` 出口 —— 没有宿主会调它们（导入 = 桌面设置窗往目录里
//! 拷文件；停用 = 同一个窗写 settings.ini 的 `DisabledDicts`）。于是 [`load`] 本身
//! **零状态**：每次调用都从目录重算并**整体替换**引擎里的导入层（`set_extra_dicts`
//! 本就是全集替换语义）。如实记两条后果：
//!   · 文件**删掉/改名** ⇒ 下次重读（激活或 `reload`）这层就没了（fcitx5 轨做不到：
//!     它的数据留在内存层里。本轨不需要「停用后仍保数据」—— 重读那一刻文件本来就该在）；
//!   · **代价**原为每次激活重读目录里的全部词库文件（Windows 上切窗口即 `Activate`）。
//!     现由 [`fingerprint`]（路径 + mtime + 大小，外加归一后的 `DisabledDicts`，存在
//!     `TsfLogic`）挡掉未变化的目录 —— **只缓存指纹，不缓存解析结果**：引擎里的导入层
//!     仍是唯一真源，指纹只回答「目录变过没有」。别顺手把词典也记进来：那是第二份
//!     真源（fcitx5 轨头注说的「记了就是第二份真源，必漂」），必与文件静默漂开。
//!
//! 反常识处同 fcitx5 轨：**装载结果必须接住**。本仓出过「坏词库静默失效」的事故
//! （插件照常加载、按键被接受、一个字不出、任何地方都没报错，用户侧只表现为候选质量
//! 骤降）—— [`ExtraDicts::lines`] 就是那条防线：每个没装上的词库都带原因，调用方逐行
//! 打出来（[`TsfLogic::sync_extra_dicts`] 里那三行）。

use std::path::Path;

use engine_core::{Dictionary, InMemoryDictionary};

/// 一次扫描的结果。
#[derive(Default)]
pub struct ExtraDicts {
    /// 装上的层（名字 + 数据），顺序 = `dict_path::list_dicts` 的名字序（确定性）。
    pub dicts: Vec<(String, InMemoryDictionary)>,
    /// 一行一个词库的日志：装上的报条数、没装上的报原因。**停用的不记**
    /// （按名单跳过不是错误，每次激活都报才是噪音）。
    /// 格式留在本模块（`DictLoad::log_line` 同一条取舍：放 Windows 胶水里就等于没测过）。
    pub lines: Vec<String>,
}

/// `DisabledDicts` → 名字表：逗号切、去空白、丢空段。**与 `Dicts.kt` 的
/// `parseDisabledDicts`、`opi_dicts.h` 的 `opiParseDisabledDicts` 同口径**。
/// 空白只按 ASCII 判：桌面窗写不出首尾带空白的名字（`withDictEnabled` 的 round-trip
/// 判据），这里只需接住手改 ini 的那点空白。
pub fn parse_disabled(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// 扫目录 + 逐条装载（跳过停用名单）。**纯判定**：目录与名单都由参数给（不读环境）；
/// 文件 IO 只到 `engine_data::load_extra_dict` 为止。目录不在/打不开 ⇒ 空报告、
/// 零日志（还没导入过词库是常态，不是错误）。
pub fn load(dir: &Path, disabled_raw: &str) -> ExtraDicts {
    let disabled = parse_disabled(disabled_raw);
    let mut out = ExtraDicts::default();
    for (name, path) in crate::dict_path::list_dicts(dir) {
        if disabled.contains(&name) {
            continue;
        }
        // 路径带上（`file_stem` 一定非空 —— 名字就是从它来的，`list_dicts` 已挡掉空词干）。
        match engine_data::load_extra_dict(&path) {
            Ok((dict, skipped)) => {
                let n = dict.len();
                if n == 0 {
                    // 空文件（`load_extra_dict` 只拒「有坏行却一条都没解析出来」那种形态）。
                    // 收下一层空的 = 界面照常报成功、一个词也打不出来 —— 正是本仓在防的
                    // 静默失效，故按「没装上」处理（与 fcitx5/opi-ffi 的 count == 0 同裁决）。
                    out.lines
                        .push(format!("词库 {name} 未装载（没有一条有效条目）"));
                    continue;
                }
                out.lines.push(if skipped > 0 {
                    // 坏行如实报数：「装上了」与「装上了、另有 N 行没读进去」是两种知情程度。
                    format!("词库 {name} 已装载 {n} 条（跳过 {skipped} 行坏行）")
                } else {
                    format!("词库 {name} 已装载 {n} 条")
                });
                out.dicts.push((name, dict));
            }
            // 原因串（含路径）来自 engine-data：读不到 / 编码不对 / 全坏行。
            Err(e) => out.lines.push(format!("词库 {name} 未装载（{e}）")),
        }
    }
    out
}

/// 目录指纹：**只**用来判断能否跳过下一次装载，不存任何解析结果。
/// 文件按 `dict_path::list_dicts` 的名字序（确定性），每项带 mtime + 大小；
/// 外加归一后的停用名单（` a , b ` 与 `a,b` 同值 —— 与装载判定同一份
/// [`parse_disabled`]，不另写一套口径）。
///
/// 任一文件读不到 metadata → `None`：调用方必须重装，绝不把「读不到」当成「没变」。
/// shortcut: mtime + 大小看不见「同大小、且落在文件系统时间戳粒度内」的改写
/// （FAT 2s）；真遇到再升级为内容哈希。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint {
    files: Vec<(std::path::PathBuf, std::time::SystemTime, u64)>,
    disabled: Vec<String>,
}

/// 目录当前指纹（见 [`Fingerprint`]）。目录不在 = 空文件表（与 [`load`] 同口径）。
pub fn fingerprint(dir: &Path, disabled_raw: &str) -> Option<Fingerprint> {
    let mut files = Vec::new();
    for (_, path) in crate::dict_path::list_dicts(dir) {
        let meta = std::fs::metadata(&path).ok()?;
        files.push((path, meta.modified().ok()?, meta.len()));
    }
    Some(Fingerprint {
        files,
        disabled: parse_disabled(disabled_raw),
    })
}

impl crate::logic::TsfLogic {
    /// 扫词库目录 + **整体替换**引擎里的导入层（判定在本文模块，这里只转发 + 日志）。
    /// 指纹（[`Fingerprint`]）与上次相同 ⇒ **整段跳过**（不重读文件、不碰引擎）。
    /// 返回装上的层数（跳过时 = 上次的层数；生产侧不用，门禁/排障要看）。
    /// 配对：fcitx5-opi 的 `opi_fcitx5_import_dict` + `opi_fcitx5_set_dict_enabled`
    /// 那一对出口（那边由 C++ 宿主逐条调 —— 不对称的理由见模块头注）。
    pub(crate) fn sync_extra_dicts(&mut self, dir: &Path, disabled_raw: &str) -> usize {
        let current = fingerprint(dir, disabled_raw);
        if let Some((cached, layers)) = &self.extra_dicts_fingerprint
            && Some(cached) == current.as_ref()
        {
            return *layers;
        }
        let report = load(dir, disabled_raw);
        // 「装载结果必须接住」（本仓的坏词库静默失效前车之鉴）：没装上的在这里现形。
        for line in &report.lines {
            tlog!("tsf-opi: {line}");
        }
        let layers = report.dicts.len();
        self.invalidate_fetch_cache();
        self.engine.set_extra_dicts(report.dicts);
        // 读不到指纹（metadata 失败）⇒ 不记，下次照常重装。
        self.extra_dicts_fingerprint = current.map(|f| (f, layers));
        layers
    }

    /// 控制面 `Reload` 用：忘掉指纹 ⇒ 紧接着的 `apply_settings` 一定重装。
    /// 设置窗保存/导入词库后必须看见新文件，哪怕 mtime + 大小恰好没变。
    pub(crate) fn invalidate_extra_dict_cache(&mut self) {
        self.extra_dicts_fingerprint = None;
    }
}

// 单测独立成文件（`#[path]` 引入）以保持各文件 <500 行：extra_dicts_tests.rs
#[cfg(test)]
#[path = "extra_dicts_tests.rs"]
mod tests;
