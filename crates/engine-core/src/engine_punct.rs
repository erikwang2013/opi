// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! 标点表 / 全角一族（2026-10-09 从 `engine.rs` 拆出 —— 那里加「输入方案」后
//! 越了 500 行门禁，与 candidates_tests / opi_json.h 同一套拆分先例）。
//!
//! 拆的是**一族自洽的方法**：中文标点表（`chinese_punct` 字段把关）与机械全角
//! （`fullwidth` 字段把关）两档开关 + 它们的读写/触发口。语义一字未动 —— 搬移
//! 只动了缩进与它所在的文件；判据（四组合真值表、键位占用表）随注释一起走。
//!
//! 字段可见性：本文件要用 `Engine` 的三个私有配置字段（`chinese_punct` /
//! `fullwidth` / `quotes`），故它们升成 `pub(crate)`（**crate 内**，不泄漏给
//! 依赖方）。这是拆分的代价，别再为此往 engine.rs 塞回方法。

use crate::composer::Mode;
use crate::engine::Engine;
use crate::punctuation;

impl Engine {
    /// 标点键 → 需要上屏的文本；`None` = 本模式/本状态下不归标点表管，交调用方直通。
    ///
    /// **四端的单一判定点**：三轨（fcitx5 / TSF / 平台中立路由）在可打印分支开头问它，
    /// [`Self::input_key`] 也问它。加进来的是引擎层，不是某一端 —— 用户裁决
    /// 2026-09-27：「引擎层加标点表，四端走同一条路」。
    pub fn input_punct(&mut self, ch: char) -> Option<String> {
        // 浏览态：同 input_key —— 先退出浏览恢复真缓冲，再按正常流程处理（契约 ②）。
        self.leave_symbol_browse_if_needed();
        // 数字**紧邻**的句号出 ASCII `.`（2026-10-17 用户裁决，桌面端报障「数字后面
        // 点击 . 键默认只能是 .」）：每端路由的可打印分支都先问本函数 ⇒ 这里记
        // 「上一次可打印输入是不是数字」是四端共用的唯一观测点（桌面数字直通后，
        // 引擎只剩这一处能看到它）。缓冲为空才算数 —— 9 键方案里数字是**输入码**
        // （进缓冲），那时的 `.` 仍是中文句号。规则先于两道开关：中英标点/全角都
        // 不越过它（用户原话「只能是 .」）。
        let after_digit = std::mem::replace(&mut self.after_digit, ch.is_ascii_digit());
        if ch == '.' && after_digit && self.buffer().is_empty() {
            return Some(".".to_string());
        }
        let text = self.punct_text(ch)?;
        // 缓冲非空：先走与空格**同一条**收尾（有候选提首候选、无候选提原文），
        // 否则 `ni,` 上屏成 `，你` —— 标点抢在待提交的拼音前面。
        // 不映射 ⇒ 不走这里：那是「直通」，由调用方自己插入，与三轨的 PassThrough 同形
        // （两个开关都关时才是这一档，见 `punct_text`）。
        let pending = if self.buffer().is_empty() {
            String::new()
        } else {
            self.input_space()
        };
        Some(pending + &text)
    }

    /// 标点映射本尊。`None` = 直通。
    ///
    /// **两档开关，互不牵连**（用户裁决 2026-09-28）：`chinese_punct` 管中文标点表那一档
    /// （含引号交替），`fullwidth` 管机械全角那一档。本函数曾经的第一行是
    /// `if !self.fullwidth { return None; }` —— 关全角把整张中文标点表一起关掉，
    /// 两个本该独立的开关焊在一起；四组合真值表逐格实测见
    /// tests/punctuation_switches.rs 的 `chinese_punct_and_fullwidth_are_independent`。
    fn punct_text(&mut self, ch: char) -> Option<String> {
        // 撇号在拼音/繁体是**音节分隔符**（`xi'an`）：只有缓冲空时它没有分隔语义，
        // 这时才当引号 —— 两个需求都要，代价只有这一个条件。
        // 排在两道闸**之前**：分隔符语义与标点开关无关，且闸关时不动引号交替状态。
        if ch == '\'' && !self.buffer().is_empty() {
            return None;
        }
        let mapped = match self.mode() {
            // 中文标点：表命中由 `chinese_punct` 把关，表未命中落到机械全角、由 `fullwidth`
            // 把关（表里没有的键如 `^` 就是这条）。两闸各管一段，兜底不越过各自的闸。
            Mode::Pinyin | Mode::Traditional => {
                let table = if self.chinese_punct {
                    punctuation::chinese(ch, &mut self.quotes)
                } else {
                    None
                };
                table.or_else(|| self.mechanical_fullwidth(ch))?
            }
            // 西文模式：**没有中文标点表这一档**（西文里冒出中文句号是错的），
            // 只有机械全角，`.` 得 `．`(U+FF0E) 而不是 `。`
            Mode::English | Mode::Number => self.mechanical_fullwidth(ch)?,
            // 符号模式：**不映射**（我的裁决，依据是数据不是口味）：生产符号表的全部
            // 关键字一律 `^[a-z0-9]+$`（tests/punctuation.rs 的 every_symbol_keyword_is_alnum
            // 钉住），标点在那里既搜不出候选、也不是待上屏的中文文本。交调用方原样插入
            // （Android 面板 / [`Self::input_key`]）或直通客户端（三轨），与 Number 同档。
            // **两个开关在符号模式都不生效**：它是搜索模式，不是文本模式。
            Mode::Symbol => return None,
        };
        Some(mapped.to_string())
    }

    /// 机械全角那一档（`fullwidth` 闸）：关 → `None`（半角直通）。
    /// 表未命中的兜底与西文模式共用这一处 —— 「全角」这个开关的判定只此一份。
    fn mechanical_fullwidth(&self, ch: char) -> Option<char> {
        if self.fullwidth {
            punctuation::ascii_fullwidth(ch)
        } else {
            None
        }
    }

    /// 全角开关的当前值（`toggle_fullwidth` 的读侧，供状态栏/测试）。
    pub fn fullwidth(&self) -> bool {
        self.fullwidth
    }

    /// **全角化一段待上屏文本**（宽档：字母/数字/标点；空格与非 ASCII 不动）——
    /// 宿主**直传点**的唯一入口（用户裁决 2026-10-12「字母、数字也要」）。
    ///
    /// 为什么入口在宿主而不是 `input_key` 里：英文/数字模式的字母数字**各端都是直传**
    /// （不进引擎：Android `KeyRouter.handleKey`、fcitx5/TSF 的 `Mode::English` 分支、
    /// `router.rs` 的同名分支），引擎收不到那些字符 —— 想在引擎一处做完，就得把直传
    /// 全改成走引擎，那是另一场手术。所以规则这样切：**映射与开关状态唯一在引擎**
    /// （本函数），宿主只在直传点调用它，别自己写平移（会漂）。
    ///
    /// 开关关 ⇒ 原样返回。映射范围：**ASCII 可打印**（字母/数字/标点）平移 +0xFEE0；
    /// **空格不宽化**（有意，别「顺手补上」）：全角开关在拼音/繁体模式是**默认开**的
    /// （`Mode::default_fullwidth`），宽空格会把「中文模式按一下空格」这个高频动作
    /// 悄悄换成全角空格 U+3000 —— 用户裁决 2026-10-12 只点名了字母与数字；要全角空格
    /// 是一条独立的裁决（改这里 + 三个宿主空格口）。汉字等非 ASCII 原样。
    pub fn fullwidthize(&self, text: &str) -> String {
        if !self.fullwidth {
            return text.to_string();
        }
        text.chars()
            .map(|c| punctuation::ascii_fullwidth_wide(c).unwrap_or(c))
            .collect()
    }

    /// 触发键：全角 ↔ 半角，返回**切换后的状态**（状态栏显示用）。
    ///
    /// 语义只在本层，**键位不在本层**。桌面两轨的键位**已定：`Shift+Space`**
    /// （`tsf-opi/src/vk.rs` 的 `fullwidth_hotkey` ≡ `fcitx5-opi/cpp/opi_fcitx5.cpp`
    /// 的 `handleFullwidthHotkey`；两轨同构，改一处必须改另一处）—— Apple 两端接线时
    /// 照这两处，别自创。为什么不用可打印键：见 `vk.rs` 里 `mode_hotkey` 的
    /// 「为什么不改成可打印键」（2026-09-27 已裁决，裸键归标点层，别再评估）。
    /// 调用方拿到状态后自行刷新 UI；不需要插入通道，故比 `toggle_symbol` 更容易接线。
    pub fn toggle_fullwidth(&mut self) -> bool {
        self.fullwidth = !self.fullwidth;
        self.fullwidth
    }

    /// 中文标点表开关的当前值（[`Self::set_chinese_punct`] / [`Self::toggle_chinese_punct`]
    /// 的读侧，供状态栏/设置页/测试）。
    ///
    /// **必须有读侧**：设置项会被切模式以外的路径改动（设置页、下次启动的持久化），
    /// 平台侧只记一份本地副本必然漂 —— 与 `opi_fullwidth_state` 同一条理由。
    pub fn chinese_punct(&self) -> bool {
        self.chinese_punct
    }

    /// **生效的**中文标点状态（供状态栏/宿主显示）：true = 此刻按标点键会出中文标点。
    ///
    /// 与 [`Self::punct_text`] 的模式分派**逐字一致**（同一组 Mode 常量）：中文类模式
    /// （拼音/繁体）读 `chinese_punct` 设置，其余模式（英文/数字/符号）恒 `false` ——
    /// 那几档根本没有中文标点表这一条路。
    ///
    /// 2026-10-15 用户裁决「切换中英文的时候，标点符号也联动切换」：**行为本来就对**
    /// （`punct_text` 一按模式分派，切英文后按 `.` 就是半角 `.`），**这条是给显示用的**
    /// —— 桌面状态栏那个「。，」按钮原先读的是**配置值** `ChinesePunct`，切到英文后
    /// 它仍旧报 on、按钮还画「。，」，看着像没联动。显示面一律读这个值；配置值只管
    /// 中文模式那一侧（它不随模式变），**别**再拿它当显示值。
    pub fn effective_chinese_punct(&self) -> bool {
        matches!(self.mode(), Mode::Pinyin | Mode::Traditional) && self.chinese_punct
    }

    /// 设置中文标点表是否生效（设置页勾选框那一侧）。
    ///
    /// **本开关有两个入口**：设置项是这一个，触发键是 [`Self::toggle_chinese_punct`] ——
    /// 两者指向同一个字段，没有先后。⚠️ 别把它写成「设置项所以没有 `toggle_` 形态」：
    /// 那句话在加触发键之后就假了。
    /// （对照：`fullwidth` **只有** `toggle` 一个入口，没有 `set_fullwidth` —— 这个不对称
    /// 是有意的：全角有模式默认值、且切模式会重置，本来就不适合当持久设置项。）
    /// 语义只在本层，**键位不在本层**（理由见 [`Self::toggle_chinese_punct`]）。
    pub fn set_chinese_punct(&mut self, on: bool) {
        self.chinese_punct = on;
    }

    /// 触发键：中文标点表 开 ↔ 关，返回**切换后的状态**（状态栏显示用），
    /// 与 [`Self::toggle_fullwidth`] 同形。
    ///
    /// 语义只在本层，**键位不在本层** —— 与 [`Self::toggle_fullwidth`] 同一条约定。
    /// 桌面两轨的键位**暂定 `Ctrl+/`**（未定稿，须两轨各自实测后确认）。
    ///
    /// **为什么是 `Ctrl+/`**（2026-09-28 本机三层实测均无占用）：
    /// ① `strings` 扫 `/usr/lib/x86_64-linux-gnu/fcitx5/**/*.so`（34 个文件），Ctrl 类占用只有
    /// `7` `8` `.` `;` `Return`（±Shift、±KP_Enter）`Shift+U` `Alt+E` `Alt+Shift+U`，
    /// 非 Ctrl 类另有 `Shift+Tab` `Super+grave` `Super+semicolon`；
    /// ② `~/.config/fcitx5/config` 的全局热键：`Control+space`、`Control+Shift+space`、
    /// `Control+Shift_L/R`、`Super+space`、`Shift+Super+space`、`Control+Alt+P`；
    /// ③ `/usr/share/fcitx5/addon/*.conf`（26 个）与 `~/.config/fcitx5/conf/*.conf`（4 个）：
    /// 无任何热键式条目。单修饰键、无 Shift ⇒ 组合面最小。
    /// （每层都留了阳性对照，否则「没搜到」可能是**空输入**而非「无占用」：`Control+semicolon`
    /// 与 `Control+period` 各命中 1 次 ⇒ 扫库管道能开火；`Category` 在 26/26 个 conf 里命中
    /// ⇒ 那组 glob 非空。）
    ///
    /// ⚠️ **三层缺一不可，别只扫第 ① 层。** **全局热键是配置值、不编进库** ⇒ 只扫库必然漏，
    /// 且当场可复现：`Control+space` 在层 ① 全库扫是 **0 命中**，却在层 ② 的
    /// `[Hotkey/TriggerKeys]` 里活着。⇒ **「库里没有」推不出「没被占用」**；同理，日后换键位
    /// 时只跑一条 `strings` 就宣布「干净」是错的，那正是本轮 `Ctrl+;` 那次错前提的成因。
    ///
    /// 应用级惯例：`Ctrl+/` = 编辑器「切换注释」，只在编辑器内生效；故不取**同样三层干净**的
    /// `Ctrl+,` —— 那是 GNOME/多数应用的「偏好设置」，**全局动作**、在哪都会按。
    ///
    /// ⚠️ **三层只是 fcitx5 内部**，别把「三层干净」读成「系统级干净」：DE/WM 与应用程序抢的
    /// 键不在其中（`Super+grave` 能扫到是特例）。⚠️ 且这是 **Linux 一侧** —— Windows/TSF 按
    /// `crates/tsf-opi/src/vk.rs` 自己的话是**本机核实不了**。⇒ **两轨各自实测后再定稿**，
    /// 别拿一端的结论当另一端。
    /// **备选 `Ctrl+Shift+.`**（未实测）：助记更强 —— Sogou 的「中英文标点切换」正是 `Ctrl+.`，
    /// 而精确组合 `Ctrl+.` 被 fcitx5 的标点模块占着（`Control+period`）；但**它尚未按上面三层
    /// 方法测过**。若某轨否掉 `Ctrl+/`，**先测它再换**，别直接换。
    ///
    /// ⚠️ **别照抄 `Ctrl+;`**：`crates/fcitx5-opi/cpp/opi_fcitx5.cpp` 的 `handleModeHotkey`
    /// 记着它是**实测**被抢走的键（`Control+semicolon` 被恒加载的 PreInputMethod 模块
    /// libclipboard 占着，`filterAndAccept()` 拦不住，e2e 阴性对照里实测到剪贴板面板弹出），
    /// `crates/tsf-opi/src/vk.rs` 的 `mode_hotkey` 有同一条。
    pub fn toggle_chinese_punct(&mut self) -> bool {
        self.chinese_punct = !self.chinese_punct;
        self.chinese_punct
    }
}
