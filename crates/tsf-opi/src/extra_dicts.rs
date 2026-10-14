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
//!   · 名单在**下次激活**时读（TSF 侧 = `Activate` → `TsfLogic::apply_settings`，
//!     与桌面窗写的提示文案一字不差）。
//!
//! **与 fcitx5 轨 `dict_layer.rs` 的不对称（刻意，不是漏做）**：本轨**没有**
//! `importDict` / `setDictEnabled` 出口 —— 没有宿主会调它们（导入 = 桌面设置窗往目录里
//! 拷文件；停用 = 同一个窗写 settings.ini 的 `DisabledDicts`）。于是本层**零状态**：
//! 每次 [`load`] 都从目录重算并**整体替换**引擎里的导入层（`set_extra_dicts` 本就是
//! 全集替换语义）。如实记两条后果：
//!   · 文件**删掉/改名** ⇒ 下次激活这层就没了（fcitx5 轨做不到：它的数据留在内存层里。
//!     本轨不需要「停用后仍保数据」—— 停用同样要等下次激活，那一刻文件本来就该在）；
//!   · **代价** = 每次激活重读目录里的全部词库文件（Windows 上切窗口即 `Activate`）。
//!     ponytail: 目录里 N 个文件就读 N 次；真到激活卡手再加 (路径, mtime, 大小) 缓存 ——
//!     那是第二份真源（fcitx5 轨头注说的「记了就是第二份真源，必漂」），别顺手加。
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

// 单测独立成文件（`#[path]` 引入）以保持各文件 <500 行：extra_dicts_tests.rs
#[cfg(test)]
#[path = "extra_dicts_tests.rs"]
mod tests;
