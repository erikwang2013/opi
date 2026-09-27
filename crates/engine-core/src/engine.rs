// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

use crate::candidates::{Candidate, DEFAULT_TOP_N, USER_BOOST, rank_and_pick};
use crate::composer::{Composer, KeyEffect, Mode};
use crate::dictionary::Dictionary;
use crate::learner::Learner;
use crate::punctuation::{self, Quotes};
use crate::symbols::{Block, BlockId, SymbolEngine, SymbolEntry};

/// 引擎门面：输入法 UI 层（经 FFI）交互的唯一入口。
pub struct Engine {
    dict: Box<dyn Dictionary>,
    /// 繁体词典（trad.opid）；None 时 Traditional 模式回退 dict（spec 错误处理）。
    trad_dict: Option<Box<dyn Dictionary>>,
    composer: Composer,
    symbols: SymbolEngine,
    learner: Learner,
    /// 用户词频权重，按词典最大静态词频动态缩放（一次选词即压过所有静态词）。
    user_boost: u64,
    /// 全角开关：`true` = ASCII 标点出**机械全角**（U+FF01..U+FF5E 平移），
    /// `false` = 直通半角。只管宽度 —— 中文标点表是另一档（[`Self::chinese_punct`]）。
    /// 切模式重置为该模式的默认值（[`Mode::default_fullwidth`]）。
    fullwidth: bool,
    /// 中文标点表开关（`,` → `，`、`\` → `、`，含成对引号交替）：`true` = 表生效。
    ///
    /// **全局布尔、不随模式重置**（与 [`Self::fullwidth`] 有意不同）：表只被
    /// Pinyin / Traditional 读，对它而言这是**用户偏好**而不是某个模式的默认值 ——
    /// 切模式重置一个没有 per-mode 含义的偏好是错的（切到英文再切回来，
    /// 用户关掉的表会自己打开）。默认 `true` = 保持既有行为。
    chinese_punct: bool,
    /// 成对引号的交替状态（复位时机见 [`Quotes`]）。
    quotes: Quotes,
}

impl Engine {
    /// 单词典构造（trad=None），JVM/现有调用向后兼容。
    pub fn new(dict: Box<dyn Dictionary>, symbols: SymbolEngine, learner_enabled: bool) -> Self {
        Self::with_dictionaries(dict, None, symbols, learner_enabled)
    }

    /// 双词典构造：Traditional 模式查 trad（None 时回退 dict），其余模式查 dict。
    pub fn with_dictionaries(
        dict: Box<dyn Dictionary>,
        trad: Option<Box<dyn Dictionary>>,
        symbols: SymbolEngine,
        learner_enabled: bool,
    ) -> Self {
        let max_freq = dict
            .max_freq()
            .max(trad.as_ref().map_or(0, |d| d.max_freq()));
        let user_boost = USER_BOOST.max(max_freq.saturating_mul(2));
        Engine {
            dict,
            trad_dict: trad,
            composer: Composer::new(),
            symbols,
            learner: Learner::new(learner_enabled),
            user_boost,
            fullwidth: Mode::default().default_fullwidth(),
            chinese_punct: true,
            quotes: Quotes::default(),
        }
    }

    /// 换装繁体词典（FFI install_trad 用）。None = 清除（回退简体）。
    /// 重算 user_boost：trad.opid 静态最大词频 4e9，保持"一次选词压过全部静态词"。
    pub fn set_trad_dict(&mut self, dict: Option<Box<dyn Dictionary>>) {
        let max_freq = self
            .dict
            .max_freq()
            .max(dict.as_ref().map_or(0, |d| d.max_freq()));
        self.user_boost = USER_BOOST.max(max_freq.saturating_mul(2));
        self.trad_dict = dict;
    }

    /// 当前模式生效的词典。
    fn active_dict(&self) -> &dyn Dictionary {
        match self.composer.session().mode {
            Mode::Traditional => self.trad_dict.as_deref().unwrap_or(&*self.dict),
            _ => &*self.dict,
        }
    }

    /// 处理一次击键，返回需要提交的文本（空串 = 无提交）。
    /// 空格键在 Engine 层拦截：拼音模式选首候选，其他模式提交缓冲。
    ///
    /// **不吞标点**：既没进缓冲、也没有标点映射的 ASCII 标点，原样交回调用方。
    /// 调用方是「没有客户端可交」的那些端（Android 面板经 JNI、C ABI 直调）——
    /// 三轨的「直通」是把自己交还给客户端应用，Android 没有这一层，返回空串
    /// 就等于吞键（NumberPad 的 `,` `.` 正是靠这条活在引擎里）。
    pub fn input_key(&mut self, ch: char) -> String {
        if ch == ' ' {
            return self.input_space();
        }
        if let Some(text) = self.input_punct(ch) {
            return text;
        }
        let (effect, _session) = self.composer.input_key(ch);
        match effect {
            KeyEffect::Updated => String::new(),
            // 标点类**永远有归宿**：要么被标点表映射（上面那条），要么原样交回调用方。
            // 其余字符（字母/数字/控制符/非 ASCII）不在内 —— 它们各有去处（进缓冲，
            // 或这个模式明确不收），**且不能靠这条救**：缓冲满时 composer 同样返回
            // `Ignored`，那时交回一个字母会把「缓冲满」误当成「这个字符没处放」，
            // 顺手 flush 掉用户打到一半的拼音（实测：满缓冲再打一个字母会把 16 个
            // 字符全提交掉）。
            // 与标点表同一条收尾：先走空格的收尾，标点落在待提交的缓冲**后面**。
            KeyEffect::Ignored if ch.is_ascii_punctuation() => self.input_space() + &ch.to_string(),
            KeyEffect::Ignored => String::new(),
        }
    }

    /// 标点键 → 需要上屏的文本；`None` = 本模式/本状态下不归标点表管，交调用方直通。
    ///
    /// **四端的单一判定点**：三轨（fcitx5 / TSF / 平台中立路由）在可打印分支开头问它，
    /// [`Self::input_key`] 也问它。加进来的是引擎层，不是某一端 —— 用户裁决
    /// 2026-09-27：「引擎层加标点表，四端走同一条路」。
    pub fn input_punct(&mut self, ch: char) -> Option<String> {
        let text = self.punct_text(ch)?;
        // 缓冲非空：先走与空格**同一条**收尾（有候选提首候选、无候选提原文），
        // 否则 `ni,` 上屏成 `，你` —— 标点抢在待提交的拼音前面。
        // 不映射 ⇒ 不走这里：那是「直通」，由调用方自己插入，与三轨的 PassThrough 同形
        // （两个开关都关时才是这一档，见 `punct_text`）。
        let pending = if self.composer.session().buffer.is_empty() {
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
        if ch == '\'' && !self.composer.session().buffer.is_empty() {
            return None;
        }
        let mapped = match self.composer.session().mode {
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

    /// 空格键：拼音/繁体/符号模式选中首候选；英文/数字模式提交缓冲。
    ///
    /// 无候选时的行为**有意分叉**：拼音/繁体提交原始缓冲（这是「打英文」的逃生口），
    /// 符号模式什么都不提交 —— 缓冲里是关键字（`dunx`），不是要上屏的文本。
    pub fn input_space(&mut self) -> String {
        match self.composer.session().mode {
            Mode::Pinyin | Mode::Traditional => {
                let buffer = self.composer.session().buffer.clone();
                let cands = self.candidates(DEFAULT_TOP_N);
                if cands.is_empty() {
                    self.composer.commit_buffer();
                    buffer
                } else {
                    // 走 select_from：候选提交的收尾（记学习 / 清缓冲 / 记页码）只此一份
                    self.select_from(&cands, 0)
                }
            }
            // 符号模式：候选就是符号（缓冲是关键字）。无候选 → 空串，路由层据此回
            // EngineHandled，**且不清缓冲**：关键字仍留在预编辑里，用户退格改一个
            // 字母就能重来（清了等于静默吞掉用户打的字）。
            Mode::Symbol => {
                let cands = self.candidates(DEFAULT_TOP_N);
                if cands.is_empty() {
                    String::new()
                } else {
                    self.select_from(&cands, 0)
                }
            }
            _ => {
                let buffer = self.composer.session().buffer.clone();
                self.composer.commit_buffer();
                buffer
            }
        }
    }

    pub fn backspace(&mut self) {
        self.composer.backspace();
    }

    pub fn clear(&mut self) {
        self.composer.clear();
    }

    /// 切模式：全角开关跟着模式默认值重置、引号交替复位（与「清 shift」同一条理由 ——
    /// 跨模式残留的粘滞态会让用户切回来发现「打字变成另一个样子」）。
    ///
    /// **`chinese_punct` 不在此列**：它是用户偏好而非模式默认值（理由见字段注释）。
    pub fn switch_mode(&mut self, mode: Mode) {
        self.fullwidth = mode.default_fullwidth();
        self.quotes.reset();
        self.composer.switch_mode(mode);
    }

    /// 触发键：在拼音与符号模式之间切换，返回**需要上屏的文本**（空串 = 无提交）。
    ///
    /// 切模式前先把未提交的缓冲处理掉 —— 照抄 Android
    /// `ImeState.commitPendingBuffer()`（`ImeState.kt:180-189`，`openSymbol` 开面板前先调它）：
    /// 有候选则提交首候选；无候选的乱码缓冲（如 `zzz`）直接清掉而**不上屏**
    /// ——把拼音原文塞进文档比丢掉更糟。`Composer::switch_mode` 只清缓冲，
    /// 拿到这里就是「打到一半的拼音被静默丢弃」，所以这层必须在它之前。
    ///
    /// 放 Engine 层是因为它同时被四端共用（与 [`Self::select_from`] 同一个理由：
    /// 各端各写一份必漂）；**键位不在本层**，本层只出语义。
    ///
    /// ⚠️ **桌面两轨目前没有任何按键指向本函数**（2026-09-28 核对的调用点）：
    /// `Ctrl+\` 两轨都走 [`Self::switch_mode`] —— `tsf-opi/src/vk.rs` 的 `mode_hotkey`
    /// → `tsf.rs` 的 `toggle_mode`、`fcitx5-opi/cpp/opi_fcitx5.cpp` 的
    /// `handleModeHotkey` —— 那条路**不收尾**：`Composer::switch_mode` 只清缓冲，
    /// 打到一半、甚至已经有候选的拼音都一起丢掉（本函数与 `switch_mode` 的语义差别
    /// 就是这个收尾）。它是给**有插入通道、且要保住待提交文本**的端用的，现有入口
    /// 只有两个出口（C ABI `opi_toggle_symbol`、JNI `toggleSymbol`）。Apple 两端接
    /// 热键时要在「照桌面丢缓冲」与「先上屏」之间**明选一个**，别照抄 `Ctrl+\`，
    /// 也别选可打印键（理由同上 [`Self::toggle_fullwidth`]）。
    /// 调用方负责把非空返回插入文档；拿不到插入通道的端
    /// 不要调它 —— 调了等于只记学习记录却不上屏。
    pub fn toggle_symbol(&mut self) -> String {
        let target = if self.composer.session().mode == Mode::Symbol {
            Mode::Pinyin
        } else {
            Mode::Symbol
        };
        let pending = if self.composer.session().buffer.is_empty() {
            String::new()
        } else {
            let cands = self.candidates(DEFAULT_TOP_N);
            if cands.is_empty() {
                self.composer.clear();
                String::new()
            } else {
                // 与 input_space 同一条收尾；符号模式下 select_from 顺带回拼音，
                // 下面的 switch_mode(target) 此时是幂等的
                self.select_from(&cands, 0)
            }
        };
        self.switch_mode(target);
        pending
    }

    pub fn set_shift(&mut self, on: bool) {
        self.composer.set_shift(on);
    }

    pub fn buffer(&self) -> &str {
        &self.composer.session().buffer
    }

    pub fn mode(&self) -> Mode {
        self.composer.session().mode
    }

    pub fn candidates(&self, limit: usize) -> Vec<Candidate> {
        let s = self.composer.session();
        rank_and_pick(
            self.active_dict(),
            &self.symbols,
            &self.learner,
            &s.buffer,
            s.mode,
            limit,
            self.user_boost,
        )
    }

    /// 选中候选项。越界返回空串。记录学习（若开启）。
    /// limit 512：rank_and_pick 仍全量排序（正确性），这里只限 FFI 载荷。
    pub fn select(&mut self, index: usize) -> String {
        let cands = self.candidates(512);
        self.select_from(&cands, index)
    }

    /// 在**调用方已算好的**候选表上选中第 `index` 项（全局下标）。
    /// 语义与 [`Self::select`] 逐字相同（越界 → 空串，命中 → 记学习 + 清缓冲），
    /// 唯一区别是不重算候选表 —— 数字选词在 `digit_select` 里已经抓过一次，
    /// 再排一遍整表是纯浪费。`Engine::select` 保留原签名（C 出口的全局索引用它）。
    pub fn select_from(&mut self, cands: &[Candidate], index: usize) -> String {
        match cands.get(index) {
            Some(c) => {
                let text = c.text.clone();
                self.learner.record_selection(&text);
                self.composer.commit_buffer();
                // 符号模式一次一符号（用户裁决 2026-09-27）：提交后回拼音，不跨键保持
                // ——下一次击键就是拼音，不需要先手动切回。放这一层是因为候选提交的收尾
                // 只此一处：四端（fcitx5 / TSF / Android / Apple）自动一致，
                // 前端各写一份必漂（`switch_mode` 顺带清 shift，与本层语义一致）。
                if self.composer.session().mode == Mode::Symbol {
                    self.switch_mode(Mode::Pinyin);
                }
                text
            }
            None => String::new(),
        }
    }

    pub fn set_learner(&mut self, enabled: bool) {
        self.learner.set_enabled(enabled);
    }

    pub fn learner_enabled(&self) -> bool {
        self.learner.is_enabled()
    }

    pub fn remove_user_word(&mut self, text: &str) {
        self.learner.remove_word(text);
    }

    pub fn clear_user_words(&mut self) {
        self.learner.clear();
    }

    pub fn export_user_words(&self) -> String {
        self.learner.export_json()
    }

    /// 导入 [`Engine::export_user_words`] 的产物（Android 侧启动时读文件后传入，
    /// engine 不做 IO）。合并语义、幂等性与失败原子性见 [`Learner::import_json`]。
    /// 返回导入条数；非法输入返回 Err 且不改动内存状态。
    pub fn import_user_words(&mut self, json: &str) -> Result<usize, String> {
        self.learner.import_json(json)
    }

    pub fn symbol_blocks(&self) -> Vec<Block> {
        self.symbols.common_blocks()
    }

    pub fn symbols_in_block(&self, id: BlockId) -> Vec<SymbolEntry> {
        self.symbols.entries_in_block(id)
    }

    pub fn search_symbols(&self, keyword: &str) -> Vec<SymbolEntry> {
        self.symbols.search(keyword)
    }
}
