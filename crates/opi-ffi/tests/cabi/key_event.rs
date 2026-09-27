// SPDX-FileCopyrightText: 2026 erik.xyz
// SPDX-License-Identifier: MIT

//! `opi_key_event` 的 C ABI 集成测试（`#[path]` 引入 cabi_test.rs，同一测试二进制 ——
//! 与其余 cabi 用例共用 SERIAL 互斥，单例不串扰；拆文件只为守住 <500 行）。

use super::*;

/// 调一次 opi_key_event，返回 (action, text)。text 按 OpiString 约定读取并释放。
fn key_event(keyval: u32, states: u32) -> (i32, String) {
    let r = unsafe { opi_key_event(keyval, states) };
    (r.action, read(r.text))
}

/// 拼音链路：字母入缓冲（action=1 无文本）→ 空格/数字选词（action=2 带文本），
/// 且提交文本与 opi_candidates 同源（同一个引擎单例，不是第二份引擎）。
#[test]
fn cabi_key_event_pinyin_buffer_and_commit() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_switch_mode(0) };
    let (a, t) = key_event('w' as u32, 0);
    assert_eq!((a, t.as_str()), (1, ""), "字母入缓冲：已处理、无提交文本");
    key_event('o' as u32, 0);
    assert_eq!(read(unsafe { opi_buffer() }), "wo");

    // 空格提交首候选（缓冲非空 ≠ 直传空格）
    let top = read_texts(unsafe { opi_candidates(8) });
    assert!(!top.is_empty(), "前置：wo 应有候选");
    let (a, t) = key_event(KEY_SPACE, 0);
    assert_eq!(a, 2, "空格的提交路径：action=2");
    assert_eq!(t, top[0], "提交文本应与 opi_candidates 首位同源");
    assert_eq!(read(unsafe { opi_buffer() }), "");

    // 非提交不得分配文本（调用方可以无条件 opi_ffi_free_string）
    let r = unsafe { opi_key_event('w' as u32, 0) };
    assert_eq!(r.action, 1);
    assert!(
        r.text.ptr.is_null() && r.text.len == 0,
        "action≠2 时 text 必须是空句柄"
    );
    unsafe { opi_clear() };
}

/// 数字选词是**页内**索引：翻页后 '1' 指向该页首位。两种词库下都成立
/// （候选 >8 时是全局第 9 个，不足一页时被钳到首页）。
#[test]
fn cabi_key_event_digit_selects_page_relative() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_switch_mode(0) };
    key_event('w' as u32, 0);
    key_event('o' as u32, 0);
    let texts = read_texts(unsafe { opi_candidates(64) });
    assert!(!texts.is_empty(), "前置：wo 应有候选");
    let want = if texts.len() > 8 {
        texts[8].clone()
    } else {
        texts[0].clone()
    };
    assert_eq!(key_event(KEY_PAGE_DOWN, 0).0, 1, "翻页被路由消费");
    let (a, t) = key_event('1' as u32, 0);
    assert_eq!((a, t.as_str()), (2, want.as_str()), "翻页后 '1' = 页内首位");
    unsafe { opi_clear() };
}

/// 英文模式：空缓冲直传（action=2 + 该字符）；⇧ Lock 大写；抬起不二次提交。
#[test]
fn cabi_key_event_english_pass_through_and_shift() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_switch_mode(1) };
    let (a, t) = key_event('a' as u32, 0);
    assert_eq!((a, t.as_str()), (2, "a"), "英文空缓冲直传：提交该字符");
    assert_eq!(read(unsafe { opi_buffer() }), "", "直传路径不进缓冲");
    let (a, _) = key_event('a' as u32, KEY_STATE_RELEASED);
    assert_eq!(a, 1, "抬起不得二次提交（否则 a → aa）");

    let (a, _) = key_event(KEY_SHIFT, KEY_STATE_LONG_PRESSED);
    assert_eq!(a, 1, "⇧ 长按被消费");
    let (a, t) = key_event('a' as u32, 0);
    assert_eq!((a, t.as_str()), (2, "A"), "Lock 下转大写");

    unsafe { opi_switch_mode(0) };
    let (a, t) = key_event('a' as u32, 0);
    assert_eq!((a, t.as_str()), (1, ""), "拼音模式下字母进缓冲");
    unsafe { opi_clear() };
}

/// ⇧ Lock 不得跨模式残留：`opi_switch_mode` 必须清**前端** ⇧ 状态。
/// 直传路径的大小写由前端三态决定（不查 composer 的 shift），引擎侧清了不够 ——
/// 漏了这条，Apple 侧「⇧ 长按 → 切拼音 → 切回英文」后打出的全是大写。
/// 对标两轨的 switch_mode_clears_frontend_shift_lock。
#[test]
fn cabi_key_event_switch_mode_clears_frontend_shift_lock() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_switch_mode(1) };
    key_event(KEY_SHIFT, KEY_STATE_LONG_PRESSED); // Lock
    unsafe { opi_switch_mode(0) }; // 离开 English → 清前端 ⇧
    unsafe { opi_switch_mode(1) }; // 回到 English
    let (a, t) = key_event('a' as u32, 0);
    assert_eq!(
        (a, t.as_str()),
        (2, "a"),
        "Lock 不得跨模式残留（否则全大写）"
    );
    unsafe { opi_switch_mode(0) };
}

/// 可打印字符不得被特殊键抢走码位：'.'=0x2E / '!'=0x21 / '"'=0x22 与
/// TSF 轨的 VK_DELETE / VK_PRIOR / VK_NEXT 同值（那里真出过 bug：拼音缓冲非空时
/// 敲 '.' 走退格分支删掉拼音字母）。Apple 侧走 Unicode 码点。
///
/// 2026-09-27 标点表落地后它们**出中文标点、并先把待提交的拼音上屏**：旧版钉的
/// 「一律交系统且缓冲原样」已不成立，这里改钉真正要保的三件：文本交得出去、
/// 缓冲是被 flush 上屏而不是被退格吃掉、抬起不重复提交。半角态（用户按了全角
/// 切换键）下才回到直通 —— 那条在 engine-core 的路由层测试里。
#[test]
fn cabi_key_event_printables_not_stolen_by_special_key_codes() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_switch_mode(0) };
    for c in ['h', 'a', 'o'] {
        key_event(c as u32, 0);
    }
    assert_eq!(read(unsafe { opi_buffer() }), "hao", "前置：缓冲已有拼音");
    let (a, t) = key_event('.' as u32, 0);
    assert_eq!(a, 2, "'.' 必须是标点上屏，不是退格：{t:?}");
    assert!(t.ends_with('。'), "'.' 出中文句号：{t:?}");
    assert!(
        t.chars().count() > 1,
        "待提交的拼音必须先上屏（标点排在其后）：{t:?}"
    );
    assert_eq!(
        key_event('.' as u32, KEY_STATE_RELEASED),
        (1, String::new()),
        "抬起不重复提交"
    );
    for c in ['!', '"'] {
        let (a, t) = key_event(c as u32, 0);
        assert_eq!(a, 2, "{c:?} 被特殊键抢走了码位：{t:?}");
        assert!(!t.is_empty(), "{c:?} 上屏文本不得为空");
    }
    assert_eq!(read(unsafe { opi_buffer() }), "", "缓冲已上屏，不是被吃掉");
    unsafe { opi_clear() };
}

/// 空缓冲的退格/回车：按下交系统，抬起也必须交系统 —— 否则客户端只有 keydown
/// 没有 keyup，依赖键状态的控件卡键（对标两轨 empty_buffer_backspace_and_enter_release_*）。
#[test]
fn cabi_key_event_empty_buffer_backspace_and_enter_release_pass_through() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_switch_mode(0) };
    for key in [KEY_BACK_SPACE, KEY_RETURN] {
        assert_eq!(key_event(key, 0).0, 0, "空缓冲按下放行（{key}）");
        assert_eq!(key_event(key, KEY_STATE_RELEASED).0, 0, "抬起同判（{key}）");
    }
}

/// 页码是路由的自有状态，而 ABI 还留着 raw 出口（`opi_input_key` / `opi_backspace`）
/// —— 它们直接改引擎、绕过路由。若不在出口处对齐页码，「翻页 → 清空 → 用 raw 出口
/// 重打 → 按 1」会用过期页码选到第 9 个候选而不是第 1 个（buffer 内容与快照相同，
/// 路由内部的重置判据察觉不到）。
/// 前置：wo 候选 >8（luna 词库），否则页码恒为 0，观测不到差别。
#[test]
fn cabi_key_event_page_aligns_across_raw_abi_exports() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_switch_mode(0) };
    key_event('w' as u32, 0);
    key_event('o' as u32, 0);
    let texts = read_texts(unsafe { opi_candidates(64) });
    assert!(
        texts.len() > 8,
        "前置：需要 >8 个候选（luna 词库），实际 {}",
        texts.len()
    );
    assert_eq!(key_event(KEY_PAGE_DOWN, 0).0, 1, "翻页被路由消费");

    // raw 出口重填缓冲（缓冲区内容与翻页前相同 —— 路由内部判据看不出来）
    unsafe { opi_clear() };
    for c in ["w", "o"] {
        let u = to_units(c);
        unsafe { opi_input_key(u.as_ptr(), u.len()) };
    }
    assert_eq!(
        read(unsafe { opi_buffer() }),
        "wo",
        "前置：缓冲已由 raw 出口重建"
    );
    let (a, t) = key_event('1' as u32, 0);
    assert_eq!(
        (a, t.as_str()),
        (2, texts[0].as_str()),
        "页码必须已归零：选到第 9 个说明 raw 出口没对齐页码"
    );
    unsafe { opi_clear() };
}

/// `opi_shift_state` 出口：0=OFF 1=SINGLE 2=LOCK。**前端**状态 —— Swift 的 ⇧ 高亮
/// 只能从这里读（英文直传路径的大小写由三态决定，引擎侧 shift 位看不出来；
/// 没有出口 UI 只能猜，且永远错）。
#[test]
fn cabi_shift_state_reflects_frontend_three_states() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_switch_mode(1) }; // English：大小写由前端三态决定
    assert_eq!(unsafe { opi_shift_state() }, 0, "初始 OFF");
    key_event(KEY_SHIFT, 0); // tap
    assert_eq!(unsafe { opi_shift_state() }, 1, "tap → SINGLE");
    let (_, t) = key_event('a' as u32, 0);
    assert_eq!(t, "A", "前置：SINGLE 下直传转大写");
    assert_eq!(unsafe { opi_shift_state() }, 0, "单次大写后自动复位");
    key_event(KEY_SHIFT, 0);
    key_event(KEY_SHIFT, KEY_STATE_LONG_PRESSED); // 长按 → LOCK
    assert_eq!(unsafe { opi_shift_state() }, 2, "长按 → LOCK");
    key_event('a' as u32, 0);
    assert_eq!(unsafe { opi_shift_state() }, 2, "LOCK 不因一次输入复位");
    unsafe { opi_switch_mode(0) }; // 离开 English → 清前端 ⇧
    assert_eq!(
        unsafe { opi_shift_state() },
        0,
        "切走必须清（否则回到英文全大写）"
    );
}

/// `opi_page` 出口：候选栏本地页码必须与引擎一致（自己维护的话，末页被钳制时
/// 会与引擎漂移 —— 高亮的页 ≠ 实际选词所在的页）。交叉验证用「翻页后按 '1'
/// 选中哪个候选」，那是页码唯一可观测的下游效果。
/// 前置：wo 候选 >8（luna 词库），否则页码恒为 0，测不出差别。
#[test]
fn cabi_page_matches_page_relative_selection() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_switch_mode(0) };
    key_event('w' as u32, 0);
    key_event('o' as u32, 0);
    let texts = read_texts(unsafe { opi_candidates(64) });
    assert!(
        texts.len() > 8,
        "前置：需要 >8 个候选（luna 词库），实际 {}",
        texts.len()
    );
    assert_eq!(unsafe { opi_page() }, 0, "起始页 0");
    key_event(KEY_PAGE_DOWN, 0);
    assert_eq!(unsafe { opi_page() }, 1, "PageDown 后页码 1");
    let (a, t) = key_event('1' as u32, 0);
    assert_eq!(
        (a, t.as_str()),
        (2, texts[8].as_str()),
        "第 1 页首位 = 全局第 9 个"
    );
    assert_eq!(
        unsafe { opi_page() },
        0,
        "提交清空缓冲 → 页码归零（与选词路径同源）"
    );
    unsafe { opi_clear() };
}

/// `opi_candidates_page` / `opi_page_count` 出口：前端要显示当前页与「共 N 页」，
/// 不该自己按硬编码的 PAGE_SIZE 去切全局列表（`PAGE_SIZE` 一改，UI 与引擎就静默错位）。
/// 本用例**不写 8**：页大小由出口自己给出（= 首页长度），再拿它交叉验证全局列表切片、
/// 总页数与「按 '1' 提交的正是该页首位」。前置：wo 需多于一页（luna 词库）。
#[test]
fn cabi_candidates_page_and_page_count_match_global_slice() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_switch_mode(0) };
    // 空缓冲 → 无候选 → 0 页（分页状态与候选集同源；实测空缓冲 candidates=0）
    assert_eq!(unsafe { opi_page_count() }, 0, "空缓冲 → 0 页");
    assert!(
        read_texts(unsafe { opi_candidates_page() }).is_empty(),
        "空缓冲 → 空页"
    );

    key_event('w' as u32, 0);
    key_event('o' as u32, 0);
    // 抓取**不设上限**：本用例要的是「全局列表」本身去反推总页数，写死一个抓取
    // 上限就是同一个错的上一层 —— 它防住了硬编码 `PAGE_SIZE`，却曾把抓取上限
    // 写成 64，于是只对「前 64 条」负责（引擎侧 `FETCH_LIMIT` 一放开，这条断言
    // 反过来假红）。`usize::MAX` = `.take()` 不设上限，判据因此变成
    // 「`FETCH_LIMIT` 不得小于引擎排名总数」：上限一旦低于全量两侧立刻不等
    // （实测把上限调回 64 时本用例红），而不是像旧版那样跟着上限一起缩。
    let global = read_texts(unsafe { opi_candidates(usize::MAX) });
    let p0 = read_texts(unsafe { opi_candidates_page() });
    assert!(!p0.is_empty(), "首页非空");
    let n = p0.len(); // 页大小 = 出口给出的首页长度，测试不硬编码 8
    assert!(
        global.len() > n,
        "前置：需要多于一页（luna 词库），全局 {} / 首页 {n}",
        global.len()
    );
    assert_eq!(p0[..], global[..n], "首页 = 全局列表前缀");
    assert_eq!(
        unsafe { opi_page_count() },
        (global.len() as u32).div_ceil(n as u32),
        "总页数 = ceil(全局/页大小)"
    );

    key_event(KEY_PAGE_DOWN, 0);
    assert_eq!(unsafe { opi_page() }, 1, "翻到第 1 页");
    let p1 = read_texts(unsafe { opi_candidates_page() });
    assert!(!p1.is_empty(), "第 1 页非空（前置保证全局多于一页）");
    assert_eq!(
        p1[..],
        global[n..n + p1.len()],
        "第 1 页 = 全局列表从 n 开始的一段"
    );

    // 交叉验证：该页首位就是按 '1' 会提交的词（页码唯一可观测的下游效果）
    let (a, t) = key_event('1' as u32, 0);
    assert_eq!(
        (a, t.as_str()),
        (2, p1[0].as_str()),
        "第 1 页首位 = '1' 提交的词"
    );
    assert_eq!(unsafe { opi_page() }, 0, "提交后页码归零");
    assert!(
        read_texts(unsafe { opi_candidates_page() }).is_empty(),
        "提交后无候选"
    );
    assert_eq!(unsafe { opi_page_count() }, 0, "提交后 0 页");
    unsafe { opi_clear() };
}

/// 页码契约：**凡改了 buffer 的出口都必须对齐页码**，否则 `opi_page()` 与
/// `opi_page_count()` 会互相矛盾 —— UI 显示「第 2 页 / 共 0 页」。
/// 三条 raw 出口（`opi_select` / `opi_input_space` / `opi_clear`）此前漏了对齐：
/// 它们清空缓冲后页码停在旧值，而候选数已归零。观测点就是下面这个组合
/// （有缓冲 → 翻到第 1 页 → 出口清空缓冲）。
#[test]
fn cabi_page_resets_after_raw_exports_that_change_buffer() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe { opi_switch_mode(0) };
    let type_wo = || {
        key_event('w' as u32, 0);
        key_event('o' as u32, 0);
    };
    let page_size = || read_texts(unsafe { opi_candidates_page() }).len(); // 不硬编码 8

    // ① opi_select（全局索引；第 1 页首位 = 全局第 n 个）
    type_wo();
    let n = page_size();
    assert!(
        read_texts(unsafe { opi_candidates(64) }).len() > n,
        "前置：需要多于一页（luna 词库）"
    );
    key_event(KEY_PAGE_DOWN, 0);
    assert_eq!(unsafe { opi_page() }, 1, "前置：已翻到第 1 页");
    assert!(
        !read(unsafe { opi_select(n) }).is_empty(),
        "前置：选择成功并清空缓冲"
    );
    assert_eq!(
        unsafe { opi_page() },
        0,
        "opi_select 改了缓冲 → 页码必须归零"
    );
    assert_eq!(
        unsafe { opi_page_count() },
        0,
        "无候选 → 0 页（与上一行必须自洽）"
    );

    // ② opi_input_space（拼音态有缓冲 → 走引擎提交）
    type_wo();
    key_event(KEY_PAGE_DOWN, 0);
    assert!(
        !read(unsafe { opi_input_space() }).is_empty(),
        "前置：空格提交并清空缓冲"
    );
    assert_eq!(
        unsafe { opi_page() },
        0,
        "opi_input_space 改了缓冲 → 页码必须归零"
    );

    // ③ opi_clear
    type_wo();
    key_event(KEY_PAGE_DOWN, 0);
    unsafe { opi_clear() };
    assert_eq!(
        unsafe { opi_page() },
        0,
        "opi_clear 改了缓冲 → 页码必须归零"
    );

    unsafe { opi_clear() };
}

/// `opi_select_page(k)` 出口：**页内**索引选词（点击候选走这条），与数字选词、
/// 回车提交同源 —— UI 因此不需要知道 PAGE_SIZE。交叉验证用**全局**列表：
/// `opi_select_page(k)` 必须提交全局第 `page * n + k` 个（n = 页大小，由
/// `opi_candidates_page()` 给出，测试不硬编码 8）。关掉 learner 让排序稳定
/// （本用例验的是分页语义，不是学习）。
#[test]
fn cabi_select_page_is_page_relative_and_reuses_one_conversion() {
    let _g = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
    load_any();
    unsafe {
        opi_switch_mode(0);
        opi_set_learner(false);
    }
    let type_wo = || {
        key_event('w' as u32, 0);
        key_event('o' as u32, 0);
    };

    // 第 0 页：k=1 → 全局第 1 个
    type_wo();
    let global = read_texts(unsafe { opi_candidates(64) });
    let n = read_texts(unsafe { opi_candidates_page() }).len();
    assert!(
        global.len() > n,
        "前置：需要多于一页（luna 词库），全局 {} / 页大小 {n}",
        global.len()
    );
    assert_eq!(
        read(unsafe { opi_select_page(1) }),
        global[1],
        "第 0 页 k=1 = 全局第 1 个"
    );
    assert_eq!(read(unsafe { opi_buffer() }), "", "提交后缓冲清空");
    assert_eq!(unsafe { opi_page() }, 0, "提交后页码归零");

    // 第 1 页：k=0 → 全局第 n 个（**页内**，不是全局第 0 个）
    type_wo();
    key_event(KEY_PAGE_DOWN, 0);
    assert_eq!(unsafe { opi_page() }, 1, "前置：已翻到第 1 页");
    let global = read_texts(unsafe { opi_candidates(64) });
    let got = read(unsafe { opi_select_page(0) });
    assert_eq!(got, global[n], "第 1 页 k=0 = 全局第 n 个（页内索引）");
    assert_ne!(got, global[0], "不得退化成全局索引");

    // 越界：空串、状态原样、不崩
    type_wo();
    assert_eq!(read(unsafe { opi_select_page(999) }), "", "越界 → 空串");
    assert_eq!(read(unsafe { opi_buffer() }), "wo", "越界不得改状态");
    unsafe {
        opi_set_learner(true);
        opi_clear();
    }
}
