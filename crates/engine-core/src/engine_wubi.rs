// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! **五笔**的引擎侧（2026-10-14，M4 计划 3b）：输入风格、候选装配（纯五笔 / 拼音混输）、
//! 引擎侧特殊习惯。独立成文件是 500 行门禁（`engine.rs` 加完这些块后越线，
//! 同 `engine_shuangpin.rs` / `engine_segment.rs` 一套先例）。
//!
//! 五笔与**笔画**同型（lead 冻结契约 1）：键是普通 ASCII 字母、匹配即**纯前缀**
//! （`Scheme::matches` 对 Stroke/Wubi 就是 `starts_with`）⇒ 候选直接复用既有前缀
//! 查询管线（[`rank_and_pick_with`]），不新造通路。与笔画的区别只在两处：
//! 1. **输入风格**（[`WubiStyle`]，本文件）：混输要把拼音主库的候选并进同一候选层；
//! 2. **特殊习惯**（本文件下半）：四码唯一自动上屏 / 候选个数档位 / 编码提示出口 /
//!    回车行为设置位。
//!
//! ⚠️ **分词回退与逐音节汤对五笔关闸**（`RankOptions::segment_fallback = false`，
//! 2026-10-14）：那两档的判据建立在拼音音节上（`segment`/`boundary_aligned`），
//! `hehe` 这类码串会被切出 `he|he` 而捞回「更短码前缀」的候选 —— 候选层显示的是
//! `he` 的候选、收尾层却按整段消耗（`engine_segment.rs` 对 Wubi 给 None），两层
//! 「匹配了多少」对不上。码串的候选只该来自「码是缓冲的前缀」这一条。
//!
//! ⚠️ 混输的拼音侧**就是主库**（[`Engine::active_dict`]：主库 + 导入层，繁体模式走
//! trad）—— **不新增拼音槽**（lead 冻结契约 5）；那 K 就是用户在拼音模式里看到的同一个
//! 库，`Engine::set_extra_dicts` 的导入词在混输里同样可见。
//!
//! 字段可见性：本文件要用 `Engine` 的 `wubi_dict` / `wubi_style` /
//! `wubi_candidate_limit` / `wubi_return_style` / `composer` / `symbols` / `learner` /
//! `user_boost`（前四个新增 `pub(crate)`，其余早因别的拆分升过）—— 同一条拆分裂代价。

use crate::candidates::{Candidate, RankOptions, rank_and_pick_with};
use crate::composer::Mode;
use crate::engine::Engine;
use crate::scheme::Scheme;

/// 五笔输入风格（lead 冻结契约 2，2026-10-14）。**默认 [`WubiStyle::Pure`]**
/// （对齐搜狗：纯五笔）。运行期状态存 `Engine::wubi_style`（同 `shuangpin_layout`
/// 的切法：枚举管「匹配语义」，风格只是候选装配轴）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WubiStyle {
    /// 纯五笔：候选**只来自五笔码表**；没装码表 = 没有候选（落普通路径，不假装）。
    #[default]
    Pure,
    /// 五笔拼音混输：码表候选 + 拼音（主库）候选**在同一候选层按同一分数序合并**
    /// —— 判定规则（怎么分辨码/拼音）见 [`Engine::candidates_wubi`] 头注。
    Mixed,
}

impl WubiStyle {
    /// 配置面**名字**（四端同一套，与 `Scheme::name` 同一条纪律；桌面 ini / TSF ini 里
    /// 存的就是它）。**名字↔枚举的换算只在 engine-core 这一处**。
    pub fn name(self) -> &'static str {
        match self {
            WubiStyle::Pure => "纯五笔",
            WubiStyle::Mixed => "五笔拼音",
        }
    }

    /// 名字 → 枚举（认不得 → None；调用方按「认不得就不动引擎」处理）。
    pub fn from_name(name: &str) -> Option<WubiStyle> {
        match name.trim() {
            "纯五笔" => Some(WubiStyle::Pure),
            "五笔拼音" => Some(WubiStyle::Mixed),
            _ => None,
        }
    }

    /// **对外协议码**（四端同名同值：JNI/`OpiEngine`、C ABI、fcitx5/TSF 配置枚举均取它；
    /// 与 `Scheme::code` 同一条纪律 —— 表只有这一份，别在别的 crate 再抄一张）。
    /// 改这张表 = 改协议。
    pub fn code(self) -> i32 {
        match self {
            WubiStyle::Pure => 0,
            WubiStyle::Mixed => 1,
        }
    }

    /// 协议码 → 枚举（越界 → None：写侧忽略）。
    pub fn from_code(v: i32) -> Option<WubiStyle> {
        match v {
            0 => Some(WubiStyle::Pure),
            1 => Some(WubiStyle::Mixed),
            _ => None,
        }
    }
}

/// 混输时**拼音怎么进**（搜狗「拼音进入方式」一档，2026-10-14 lead 收货时补）。
/// **默认 [`Self::DirectLetters`]**。只在 [`WubiStyle::Mixed`] 下有意义 —— 纯五笔没有
/// 拼音侧，本档开了也与关着等价（缓冲一律按码解释，z 码照打）。
///
/// | 取值 | 缓冲 `znihao` | 缓冲 `nihao` | 缓冲 `zzjp` |
/// |---|---|---|---|
/// | [`Self::DirectLetters`] | 空（两边都没这前缀） | 码候选 + 拼音候选**同层合并** | z 码候选（字面解释） |
/// | [`Self::ZKey`] | 「你好」（z 后按拼音） | **码候选**（非 z 段不查拼音） | 空（z 段按拼音查 `zjp`） |
///
/// 两条**写死的子决定**（不是抄来的，是这里定的；要改成别的是一行的事，但改之前先裁决）：
/// 1. z 段查拼音时**去掉前导 `z`**（`znihao` → 查 `nihao`），z 是标志不是输入；
///    `z` 单独一个字符 ⇒ 余下为空 ⇒ **无候选**（与拼音输入法里 z 无输出一致）。
/// 2. z 段**只出拼音候选**，非 z 段**只出码候选** —— 这正是本档存在的理由（把
///    「两边都像」的模糊段拆成两个明确段）。代价如实记：**混输 + ZKey 下 z 码
///    （677 行 zz 分类表）打不出来**（切回纯五笔或 DirectLetters 照打）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WubiMixedEntry {
    /// 直接字母（默认，对齐搜狗缺省）：字母串两边都查、同层合并。
    #[default]
    DirectLetters,
    /// z 键：z 开头的那段按拼音解释，非 z 段按码解释（见上表）。
    ZKey,
}

impl WubiMixedEntry {
    /// 配置面名字（纪律同 [`WubiStyle::name`]）。
    pub fn name(self) -> &'static str {
        match self {
            WubiMixedEntry::DirectLetters => "直接字母",
            WubiMixedEntry::ZKey => "z键",
        }
    }

    pub fn from_name(name: &str) -> Option<WubiMixedEntry> {
        match name.trim() {
            "直接字母" => Some(WubiMixedEntry::DirectLetters),
            "z键" => Some(WubiMixedEntry::ZKey),
            _ => None,
        }
    }

    /// 对外协议码（0=直接字母 1=z键；纪律同 [`WubiStyle::code`]）。
    pub fn code(self) -> i32 {
        match self {
            WubiMixedEntry::DirectLetters => 0,
            WubiMixedEntry::ZKey => 1,
        }
    }

    pub fn from_code(v: i32) -> Option<WubiMixedEntry> {
        match v {
            0 => Some(WubiMixedEntry::DirectLetters),
            1 => Some(WubiMixedEntry::ZKey),
            _ => None,
        }
    }
}

/// 五笔回车键行为（搜狗「特殊习惯」里的「回车键」一档）。**默认 [`Self::Clear`]**。
///
/// **消费判据已接线**（2026-10-14）：引擎侧仍不处理回车（`Engine::input_key` 从不收
/// `'\r'`），但**判据**集中在一处 —— [`crate::Engine::enter_action`]（实现与理由见
/// `engine_enter.rs`），三处宿主的 handleEnter（`router.rs`、`crates/fcitx5-opi`、
/// `crates/tsf-opi`）都问它；**Android 待其 JNI 出口**（在那之前 Android 上本设置不生效，
/// 设置页文案如实这么写）。语义按下表：
///
/// | 取值 | 缓冲非空时回车 |
/// |---|---|
/// | [`Self::Clear`] | 丢弃缓冲，**不上屏**（打废了就清掉重来） |
/// | [`Self::CommitRaw`] | 把**缓冲原文**上屏（正在打的码，含半码）—— 与空格「无候选时提交原文」的逃生口语义同源 |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WubiReturnStyle {
    /// 清空缓冲（不上屏）。
    #[default]
    Clear,
    /// 上屏缓冲原文（正在打的码）。
    CommitRaw,
}

impl WubiReturnStyle {
    /// 配置面名字（纪律同 [`WubiStyle::name`]）。
    pub fn name(self) -> &'static str {
        match self {
            WubiReturnStyle::Clear => "清空",
            WubiReturnStyle::CommitRaw => "上屏原文",
        }
    }

    pub fn from_name(name: &str) -> Option<WubiReturnStyle> {
        match name.trim() {
            "清空" => Some(WubiReturnStyle::Clear),
            "上屏原文" => Some(WubiReturnStyle::CommitRaw),
            _ => None,
        }
    }

    /// 对外协议码（0=清空 1=上屏原文；纪律同 [`WubiStyle::code`]）。
    pub fn code(self) -> i32 {
        match self {
            WubiReturnStyle::Clear => 0,
            WubiReturnStyle::CommitRaw => 1,
        }
    }

    pub fn from_code(v: i32) -> Option<WubiReturnStyle> {
        match v {
            0 => Some(WubiReturnStyle::Clear),
            1 => Some(WubiReturnStyle::CommitRaw),
            _ => None,
        }
    }
}

/// 五笔候选个数的默认档位（对齐搜狗「候选个数」默认 5）。
pub const WUBI_CANDIDATE_LIMIT_DEFAULT: usize = 5;
/// 五笔候选个数的上限：9（数字选词键 1–9 的物理上限；与搜狗档位表同量级）。
pub const WUBI_CANDIDATE_LIMIT_MAX: usize = 9;

impl Engine {
    // ---- 输入风格 ----

    /// 换五笔风格。**不清缓冲**：风格只改「同一缓冲怎么装配候选」，缓冲里已有的码
    /// 按新风格重解即可（与 `set_shuangpin_layout` 同一条理由 —— 与 `set_scheme`
    /// 的区别正在这里，那条必须清）。
    pub fn set_wubi_style(&mut self, style: WubiStyle) {
        self.wubi_style = style;
    }

    pub fn wubi_style(&self) -> WubiStyle {
        self.wubi_style
    }

    // ---- 混输的拼音进入方式（只在 Mixed 下有语义，见 WubiMixedEntry 头注表）----

    /// 换「拼音进入方式」。**不清缓冲**（与风格同：同一缓冲按新档重解）。
    pub fn set_wubi_mixed_entry(&mut self, entry: WubiMixedEntry) {
        self.wubi_mixed_entry = entry;
    }

    pub fn wubi_mixed_entry(&self) -> WubiMixedEntry {
        self.wubi_mixed_entry
    }

    // ---- 四码唯一自动上屏的开关 ----

    /// 四码唯一自动上屏开关。**默认 `true`**（对齐搜狗缺省：开）。关掉后四码唯一也
    /// **不**自动上屏 —— 缓冲原样留着，用户自己选（空格/数字键/回车/继续打）。
    /// 判据其余部分（[`Self::wubi_autocommit`]）一字不动，本开关只是它的总闸。
    /// **不清缓冲**（与其余设置位同：关掉不会把正在打的码吐出去）。
    pub fn set_wubi_auto_commit(&mut self, on: bool) {
        self.wubi_auto_commit = on;
    }

    pub fn wubi_auto_commit(&self) -> bool {
        self.wubi_auto_commit
    }

    // ---- 候选个数档位 ----

    /// 五笔候选个数档位（引擎侧截断：`Engine::candidates` 在 `Scheme::Wubi` 下把
    /// 调用方的 `limit` 与本值取小）。**越界钳到 [1, [`WUBI_CANDIDATE_LIMIT_MAX`]]**：
    /// 0 个候选是自相矛盾（钳到 1），过大的值钳到 9（数字选词键上限）。
    /// 默认 = [`WUBI_CANDIDATE_LIMIT_DEFAULT`]（5）。**不清缓冲**（与风格同：只改截断）。
    ///
    /// ⚠️ 截断在**引擎侧**生效 ⇒ `Engine::select` 的下标空间在五笔下随之收窄
    /// （越界返回空串）；「候选总数不设上限」的承诺是拼音路的（见 `Engine::select`），
    /// 五笔的档位是用户显式设的上限，不矛盾。
    pub fn set_wubi_candidate_limit(&mut self, n: usize) {
        self.wubi_candidate_limit = n.clamp(1, WUBI_CANDIDATE_LIMIT_MAX);
    }

    pub fn wubi_candidate_limit(&self) -> usize {
        self.wubi_candidate_limit
    }

    // ---- 回车行为（设置位；消费判据见 `Engine::enter_action` / `engine_enter.rs`）----

    pub fn set_wubi_return_style(&mut self, style: WubiReturnStyle) {
        self.wubi_return_style = style;
    }

    pub fn wubi_return_style(&self) -> WubiReturnStyle {
        self.wubi_return_style
    }

    // ---- 编码提示（剩余码）----

    /// **编码提示**：当前码还能接哪些键（`'a'..='z'` 里使「缓冲 + 该键」成为某个码
    /// 前缀的那些，升序；无码表/空缓冲/缓冲含非小写字母/非五笔方案 ⇒ 空数组）。
    /// 判据逐键自证：`词库.query(缓冲+键, 1)` 非空 = 存在以它为前缀的码（含**精确
    /// 就是它**的码 —— 提示的是「还能接」，不是「必须接」）。
    ///
    /// 读侧出口，**确定性**（同一缓冲同一词库必同结果）；宿主 UI（五笔键盘上方
    /// 提示条）下一波接线。⚠️ 提示按**全码表**算，不认识「用户只想打 4 码」的档位
    /// 意图 —— 与搜狗「编码提示」同姿态（它提示的也是表里接得下去键）。
    pub fn wubi_next_keys(&self) -> Vec<char> {
        if self.scheme() != Scheme::Wubi {
            return Vec::new();
        }
        let Some(dict) = self.wubi_dict.as_deref() else {
            return Vec::new();
        };
        let buffer = &self.composer.session().buffer;
        if buffer.is_empty() || !buffer.chars().all(|c| c.is_ascii_lowercase()) {
            return Vec::new();
        }
        let mut probe = buffer.clone();
        ('a'..='z')
            .filter(|&c| {
                probe.push(c);
                let hit = !dict.query(&probe, 1).is_empty();
                probe.pop();
                hit
            })
            .collect()
    }

    // ---- 候选装配 ----

    /// 五笔候选（`Engine::candidates` 的 `Scheme::Wubi` 臂）。空槽守卫在**调用点**
    /// （`engine.rs`：槽为空落普通路径 —— 与 stroke 同一条「可选增强」姿态），
    /// 本函数对 None 仍防御性返回空。
    ///
    /// ## 纯五笔（[`WubiStyle::Pure`]，默认）
    /// 候选只来自五笔码表：纯前缀匹配 + 精确加成（**码长**相等）+ 符号档，
    /// 全走 [`rank_and_pick_with`]；拼音主库**不参与**。
    ///
    /// ## 五笔拼音混输（[`WubiStyle::Mixed`]）
    /// **「怎么分辨码还是拼音」的判据**（本文件的设计决定，2026-10-14，M4 3b）：
    ///
    /// 1. 缓冲**同时**喂两条既有前缀路：五笔码表（`self.wubi_dict`）与拼音主库
    ///    （[`Engine::active_dict`]，**不新增拼音槽**，见文件头）。
    /// 2. 两边的命中都由**各自词库的前缀查询自证** —— 没有形态识别、没有启发式：
    ///    「出拼音候选」的充要条件 = 缓冲本身是某个拼音键的前缀；「出码候选」=
    ///    缓冲是某个码的前缀。这就是可写进门禁的判据（tests/wubi.rs 的混输用例
    ///    正是按它断言的：五笔专属码只出码候选、拼音专属串只出拼音候选）。
    /// 3. 拼音侧**关掉模糊/纠错/简拼**（与码侧同一份 opts）：那三档是给标准全拼
    ///    纠偏的，在混输里对码串同样开着只会把码展开成拼音词组 —— 正是
    ///    「打一个字出两种体系噪声」的形态（lead 契约 2 点名的体验）。
    /// 4. 合并 = **同一候选层、同一分数量纲**：两路都走 `rank_and_pick_with`
    ///    （`rank_score` + 精确加成是同一套算法），拼起来按分数降序、文本升序排、
    ///    去重、截断（与 [`Engine::candidates_shuangpin`] 的多解析合并同一条形状）。
    ///    **不另造排序**（lead 契约 2：混输要 walk 同一条 merge）。
    /// 5. 两边都无命中 ⇒ 空（不假装）。
    ///
    /// 截断：档位（[`Engine::set_wubi_candidate_limit`]）在两层生效 —— 各子路先截
    /// 一轮、合并后按档位定截。
    ///
    /// ⚠️ 四码自动上屏（[`Engine::wubi_autocommit`]）**只在 Pure 风格**生效：
    /// 混输下缓冲天然是双重解释（可能同时是拼音前缀），自动提交会拿码表的单候选
    /// 压过用户的拼音意图（`gong` 这类四字母串可能既是某字的**唯一**码、又是合法
    /// 拼音）—— 保守取向，判据留待产品裁决，改法是一行（去掉风格条件即可）。
    pub(crate) fn candidates_wubi(&self, buffer: &str, mode: Mode, limit: usize) -> Vec<Candidate> {
        let Some(dict) = self.wubi_dict.as_deref() else {
            return Vec::new();
        };
        let cap = limit.min(self.wubi_candidate_limit);
        // 三档与分词回退对码串全关，理由见文件头与 `RankOptions::segment_fallback`。
        // ⚠️ `tiers` 也在这里关掉**且对混输那条拼音支同样生效**（它复用同一个 `opts`）：
        // 五笔下候选栏的主语是码候选，拼音候选是叠加上去的一路 —— 给它单开分档是另一条
        // 产品裁决，本轮（2026-10-14）没做、如实记在交付说明里。
        let opts = RankOptions {
            fuzzy: 0,
            correction: false,
            jianpin: false,
            prefer_simplified: mode == Mode::Pinyin,
            segment_fallback: false,
            tiers: false,
        };
        // z 键进入方式（只混输；表与两条子决定见 `WubiMixedEntry` 头注）：
        // z 段**只出拼音**（余串空 ⇒ 无候选，`z` 本身不是输入），非 z 段**只出码** ——
        // 本档下不走下面那条「两边都查再合并」的路，各段各走各的。
        if self.wubi_style == WubiStyle::Mixed {
            match self.wubi_mixed_entry {
                // `buffer[1..]` 字节切片安全：前导是 ASCII `z`（1 字节）。
                WubiMixedEntry::ZKey if buffer.starts_with('z') => {
                    let py = &buffer[1..];
                    return if py.is_empty() {
                        Vec::new()
                    } else {
                        rank_and_pick_with(
                            self.active_dict(),
                            &self.symbols,
                            &self.learner,
                            py,
                            mode,
                            cap,
                            self.user_boost,
                            opts,
                        )
                    };
                }
                // 非 z 段：只出码候选（不查拼音）—— 与 Pure 的码侧装配同形。
                WubiMixedEntry::ZKey => {
                    return rank_and_pick_with(
                        dict,
                        &self.symbols,
                        &self.learner,
                        buffer,
                        mode,
                        cap,
                        self.user_boost,
                        opts,
                    );
                }
                WubiMixedEntry::DirectLetters => {} // 落到下面「两边都查再合并」
            }
        }
        let mut merged = rank_and_pick_with(
            dict,
            &self.symbols,
            &self.learner,
            buffer,
            mode,
            cap,
            self.user_boost,
            opts,
        );
        if self.wubi_style == WubiStyle::Mixed {
            merged.extend(rank_and_pick_with(
                self.active_dict(),
                &self.symbols,
                &self.learner,
                buffer,
                mode,
                cap,
                self.user_boost,
                opts,
            ));
            merged.sort_by(|a, b| b.score.cmp(&a.score).then(a.text.cmp(&b.text)));
            let mut seen = std::collections::HashSet::new();
            merged.retain(|c| seen.insert(c.text.clone()));
            merged.truncate(cap);
        }
        merged
    }

    // ---- 四码唯一自动上屏 ----

    /// **四码唯一自动上屏**（特殊习惯 1；搜狗同名开关）：缓冲满四码且**码表只给一个
    /// 候选**时提交该词并清缓冲 —— 返回值经 `Engine::input_key` 的既有「单键返回待
    /// 上屏文本」通道出引擎，四端自动一致（调用点在 `engine.rs` 的
    /// `KeyEffect::Updated` 分支；宿主不需要任何新接线，也**没有**第二条提交通路）。
    ///
    /// 判据（全成立才自动；顺序 = 从廉到贵）：
    /// 0. **总闸开着**（[`Engine::set_wubi_auto_commit`]，默认开）；
    /// 1. 方案 `Wubi` **且风格 `Pure`** —— 混输的理由见 [`Self::candidates_wubi`] 尾注；
    /// 2. 模式 Pinyin/Traditional（符号/英文/数字模式的缓冲不是码，见 composer 的门）；
    /// 3. **恰好 4 个字符**（「四码」的字面义；1–3 码是简码，绝不自动）；
    /// 4. 装了五笔码表（None ⇒ 不自动 —— 没码表就没有「码表唯一」可言）；
    /// 5. 码表前缀查询**整档恰好 1 条、且是精确命中**（`pinyin_len == 4`）——
    ///    同码多词（≥2 条）= 不自动（用户要选）；还有更长的码以它为前缀（延伸码）
    ///    ⇒ 整档不止 1 条 ⇒ 不自动；
    /// 6. **符号层也对这个缓冲无命中**（`symbols.search` 是**前缀**匹配，四码撞上
    ///    `comma`/`heart` 这类关键字的前四个字母时候选栏还挂着符号）—— 判据是
    ///    「**候选层**恰好一条」，不是「码表里恰好一条」：用户看得见第二条候选，
    ///    就不该替他上屏。
    ///
    /// ⚠️ 判据基于**真实候选**（词库查询结果），不是「4 个字母就猜唯一」。
    /// **不记学习**（`Learner::record_selection`）：自动上屏不是用户选词，且唯一码
    /// 本无排序可言 —— 记了只是往用户词表里灌垃圾；一词在其他码下的排名也不该被
    /// 这次自动提交悄悄改写。
    ///
    /// 变异自检（2026-10-14 实测，三次）：判据 5 放宽成「整档非空」（`len != 1`
    /// → `is_empty()`）⇒ **3 条红**（同码多词 / 延伸码 / 空格首候选）；去掉「精确
    /// 命中」半条（只留 `len != 1`）⇒ 唯一延伸码那条红；去掉判据 6 ⇒ 符号那条红。
    ///
    /// 判据 5 **各半条的实际射程**（2026-10-14 实测，别照抄成更宽的话）：86 表四键码
    /// 93,032 个里可自动上屏 71,244（76.6%）；「**延伸码**」那半条在 as-shipped 的
    /// **86 / 98 上都挡 0 个**（两表最长码都是 4，四码处无更长的码可延伸）。它挡的是
    /// **含 >4 键码的表** —— 那种表今天只能经 `set_wubi_dict` 装进来（宿主读盘那条路
    /// 见下一波）；用户导入的词典走 `set_extra_dicts`（**主库层**，不参与本判据）。
    /// 判据留着是纵深：真遇到 >4 键表时，4 码唯一不该抢在「再多打一键」前面提交。
    pub(crate) fn wubi_autocommit(&mut self) -> Option<String> {
        if !self.wubi_auto_commit {
            return None;
        }
        if self.scheme() != Scheme::Wubi
            || self.wubi_style != WubiStyle::Pure
            || !matches!(self.mode(), Mode::Pinyin | Mode::Traditional)
        {
            return None;
        }
        let buffer = &self.composer.session().buffer;
        if buffer.chars().count() != 4 {
            return None;
        }
        let dict = self.wubi_dict.as_deref()?;
        let hits = dict.query(buffer, usize::MAX);
        if hits.len() != 1
            || hits[0].pinyin_len != buffer.len()
            || !self.symbols.search(buffer).is_empty()
        {
            return None;
        }
        let text = hits[0].word.clone();
        self.composer.commit_buffer();
        Some(text)
    }
}
