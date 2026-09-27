/* SPDX-FileCopyrightText: 2026 erik.xyz
 * SPDX-License-Identifier: MIT
 *
 * 真 C 消费者：**按头文件**声明去调 opi_* 导出。
 *
 * 为什么不是「Rust 里调 extern "C" fn」——那样验的是「Rust 以为的 ABI」，
 * 头文件写成什么样都照样绿。本文件是唯一会**编译 macos/OpiFFI.h** 的消费者，
 * 所以它能证明的是：Swift / ArkTS / 桌面的 C 侧照这份声明绑过去，接口真的能用
 * （签名错位、类型错位、返回值约定错，都会在这里露出）。
 *
 * 用法：./consumer /path/to/luna.opid     （缺参 → opi_load(NULL,0) 的内置 35 词库）
 * 失败即非 0 退出并打印具体哪一条断言。
 */
#include "OpiFFI.h" /* 经 -I 指向 macos/ —— 与 Swift bridging header 看到的是同一份 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int failures = 0;
static int checks = 0;

#define CHECK(cond, ...)                                                       \
    do {                                                                       \
        checks++;                                                              \
        if (!(cond)) {                                                         \
            failures++;                                                        \
            printf("  [FAIL] %s:%d ", __FILE__, __LINE__);                     \
            printf(__VA_ARGS__);                                               \
            printf("\n");                                                      \
        }                                                                      \
    } while (0)

/* 把 OpiString 打成 \uXXXX 便于诊断（不实现 UTF-8 解码，够用即可）。 */
static void dump(const char *tag, OpiString s) {
    printf("  %s: ptr=%s len=%zu ", tag, s.ptr ? "非空" : "NULL", s.len);
    if (s.ptr && s.len <= 32) {
        printf("\"");
        for (size_t i = 0; i < s.len; i++) {
            printf("\\u%04x", (unsigned)s.ptr[i]);
        }
        printf("\"");
    }
    printf("\n");
}

/* 与期望码点序列逐位比。 */
static int eq_cp(OpiString s, const uint16_t *want, size_t n) {
    if (s.len != n) return 0;
    if (n == 0) return 1;
    if (!s.ptr) return 0;
    return memcmp(s.ptr, want, n * sizeof(uint16_t)) == 0;
}

static const uint16_t WO[] = {0x6211};   /* 我 */
static const uint16_t NI[] = {0x4F60};   /* 你 */

static const uint16_t W = 'w', O = 'o', X = 'x';

int main(int argc, char **argv) {
    /* ---------- 装载 ---------- */
    printf("[1] opi_load\n");
    OpiString path16 = {NULL, 0};
    if (argc > 1) {
        size_t n = strlen(argv[1]);
        uint16_t *buf = (uint16_t *)malloc((n + 1) * sizeof(uint16_t));
        if (!buf) return 2;
        for (size_t i = 0; i < n; i++) buf[i] = (uint16_t)(unsigned char)argv[1][i];
        path16.ptr = buf;
        path16.len = n;
        printf("  词库路径：%s\n", argv[1]);
    } else {
        printf("  无参 → 内置回退词库（35 词）\n");
    }
    bool ok = opi_load(path16.ptr, path16.len);
    CHECK(ok, "opi_load 返回 false（词库没装上，后面的断言都会退化）");
    if (!ok) {
        printf("结果：%d/%d 通过\n", checks - failures, checks);
        return 1;
    }
    /* 路径缓冲是调用方分配的，Rust 侧只读不留引用 —— 这里立刻释放，验证该约定。 */
    free((void *)path16.ptr);

    /* ---------- 装载后的空状态 ---------- */
    printf("[2] 空状态\n");
    OpiString buf = opi_buffer();
    CHECK(eq_cp(buf, NULL, 0), "新引擎的缓冲应为空串");
    opi_ffi_free_string(buf); /* 空句柄也可无条件释放（头文件写明的约定） */

    int32_t mode = opi_mode();
    printf("  opi_mode()          = %d\n", mode);
    CHECK(mode == 0, "初始模式应为 0=Pinyin，实为 %d", mode);
    uint32_t pages = opi_page_count();
    uint32_t page = opi_page();
    printf("  opi_page()/count()  = %u/%u\n", page, pages);
    CHECK(pages == 0 && page == 0, "空缓冲下页码应为 0/0");
    printf("  opi_shift_state()   = %d\n", opi_shift_state());
    printf("  opi_learner_enabled() = %d\n", (int)opi_learner_enabled());

    /* ---------- 键路由：打 "wo" ---------- */
    printf("[3] 键路由 opi_key_event（可打印字符 = Unicode 码点，空格 = 0x20）\n");
    OpiKeyEventResult r1 = opi_key_event(W, 0);
    CHECK(r1.action == 1, "打 'w' 应 action=1（已处理、无提交），实为 %d", r1.action);
    CHECK(r1.text.ptr == NULL, "action!=2 时 text 应为空句柄（调用方可无条件释放）");
    opi_ffi_free_string(r1.text);

    OpiKeyEventResult r2 = opi_key_event(O, 0);
    CHECK(r2.action == 1, "打 'o' 应 action=1，实为 %d", r2.action);
    opi_ffi_free_string(r2.text);

    buf = opi_buffer();
    dump("  opi_buffer()", buf);
    CHECK(eq_cp(buf, (const uint16_t[]){'w', 'o'}, 2), "缓冲应为 \"wo\"");
    opi_ffi_free_string(buf);

    /* ---------- 候选：JSON 文本数组 ---------- */
    printf("[4] opi_candidates(8)\n");
    OpiString cands = opi_candidates(8);
    dump("  opi_candidates()", cands);
    CHECK(cands.ptr != NULL && cands.len > 2, "候选 JSON 不该是空串");
    if (cands.ptr) {
        CHECK(cands.ptr[0] == '[', "候选应是 JSON 数组（首字符 '['），实为 \\u%04x",
              (unsigned)cands.ptr[0]);
        /* 含 "我"（U+6211）—— 用逐位扫描代替 JSON 解析，够证明「真拿到了字」。 */
        int found = 0;
        for (size_t i = 0; i < cands.len; i++) {
            if (cands.ptr[i] == WO[0]) found = 1;
        }
        CHECK(found, "候选里应含 U+6211（我）");
    }
    opi_ffi_free_string(cands);

    OpiString cpage = opi_candidates_page();
    dump("  opi_candidates_page()", cpage);
    CHECK(cpage.ptr != NULL && cpage.ptr[0] == '[', "当前页候选也应是 JSON 数组");
    opi_ffi_free_string(cpage);

    /* ---------- 空格提交：验证头文件写明的「空格 = 0x20」约定 ---------- */
    printf("[5] 空格提交（头文件约定：空格在可打印段，不是 0x1_0020）\n");
    OpiKeyEventResult r3 = opi_key_event(0x20, 0);
    printf("  action=%d", r3.action);
    dump(" text", r3.text);
    CHECK(r3.action == 2, "缓冲非空时按空格应 action=2（提交），实为 %d —— 若为 0 说明键码"
                          "落进了非 ASCII 直通分支", r3.action);
    CHECK(eq_cp(r3.text, WO, 1),
          "提交文本应为 U+6211（我）；若只提交出一个空格说明键码空间用错");
    opi_ffi_free_string(r3.text);

    buf = opi_buffer();
    CHECK(eq_cp(buf, NULL, 0), "提交后缓冲应清空");
    opi_ffi_free_string(buf);

    /* ---------- 页内索引选词 ---------- */
    printf("[6] opi_select_page（页内索引）\n");
    opi_key_event(W, 0);
    opi_key_event(O, 0);
    OpiString sel = opi_select_page(0);
    dump("  opi_select_page(0)", sel);
    CHECK(sel.ptr != NULL && sel.len > 0, "第 0 个候选应选得出东西");
    opi_ffi_free_string(sel);

    /* 越界：空串、不改状态、不 panic */
    OpiString oob = opi_select_page(9999);
    CHECK(oob.ptr == NULL && oob.len == 0, "越界 select_page 应返回空句柄");
    opi_ffi_free_string(oob);
    opi_clear();

    /* ---------- 模式切换 ---------- */
    printf("[7] opi_switch_mode\n");
    opi_switch_mode(1);
    CHECK(opi_mode() == 1, "切到 English 后 opi_mode 应为 1");
    opi_switch_mode(0);
    CHECK(opi_mode() == 0, "切回 Pinyin 后 opi_mode 应为 0");
    opi_switch_mode(999); /* 越界：静默忽略（头文件写明的既有语义） */
    CHECK(opi_mode() == 0, "越界模式值应被忽略，模式不变");

    /* ---------- 符号 / 用户词：只要不崩且形状对 ---------- */
    printf("[8] 符号与用户词出口\n");
    OpiString blocks = opi_symbol_blocks();
    dump("  opi_symbol_blocks()", blocks);
    CHECK(blocks.ptr == NULL || blocks.ptr[0] == '[', "符号块应是 JSON 数组");
    opi_ffi_free_string(blocks);

    OpiString syms = opi_symbols_in_block(-1); /* 负 id = 越界 → 空数组 */
    CHECK(syms.ptr != NULL && eq_cp((OpiString){syms.ptr, 2}, (const uint16_t[]){'[', ']'}, 2),
          "负块 id 应返回空数组 []");
    opi_ffi_free_string(syms);

    OpiString search = opi_search_symbols(&X, 1); /* i18n 词 "x" 是合法 UTF-16，参数约定须为 u16* */
    dump("  opi_search_symbols(\"x\")", search);
    CHECK(search.ptr == NULL || search.ptr[0] == '[', "符号搜索结果应是 JSON 数组");
    opi_ffi_free_string(search);

    int32_t imported = opi_import_user_words((const uint16_t[]){'{', '}'}, 2); /* 非词表 JSON */
    printf("  opi_import_user_words(\"{}\") = %d\n", imported);
    CHECK(imported <= 0, "非法 JSON 应返回负值");
    OpiString exported = opi_export_user_words();
    dump("  opi_export_user_words()", exported);
    CHECK(exported.ptr != NULL, "导出用户词应是有效 JSON 文本（可为空对象/数组）");
    opi_ffi_free_string(exported);
    opi_clear_user_words();
    opi_set_learner(true);
    CHECK(opi_learner_enabled() == true, "set_learner(true) 后应读到 true");
    opi_set_learner(false);

    /* ---------- 多字符缓冲 + 逗号候选 ---------- */
    printf("[9] 双字候选 \"nihao\"\n");
    const uint16_t keys[] = {'n', 'i', 'h', 'a', 'o'};
    for (size_t i = 0; i < sizeof(keys) / sizeof(keys[0]); i++) {
        OpiKeyEventResult r = opi_key_event(keys[i], 0);
        CHECK(r.action == 1, "打 '%c' 应 action=1，实为 %d", (char)keys[i], r.action);
        opi_ffi_free_string(r.text);
    }
    cands = opi_candidates(8);
    dump("  opi_candidates(8) @nihao", cands);
    CHECK(cands.ptr != NULL, "nihao 应有候选");
    if (cands.ptr) {
        int has_ni = 0;
        for (size_t i = 0; i < cands.len; i++) {
            if (cands.ptr[i] == NI[0]) has_ni = 1;
        }
        CHECK(has_ni, "nihao 候选里应含 U+4F60（你）");
    }
    opi_ffi_free_string(cands);

    /* ---------- 键码空间：特殊键必须带 0x1_0000 基址 ---------- */
    printf("[10] 键码空间（头文件的键码表：特殊键 = 0x1_0000 | 低 16 位码）\n");
    OpiKeyEventResult raw = opi_key_event(0x1B, 0); /* 裸 0x1B：**不是** Esc 的编码 */
    printf("  裸 0x1B            action=%d\n", raw.action);
    CHECK(raw.action == 0, "裸 0x1B 不是特殊键编码，应落进直通分支（action=0），实为 %d —— "
                          "把特殊键按裸低字节送会静默失效，这正是头文件键码表的用意",
          raw.action);
    opi_ffi_free_string(raw.text);

    OpiKeyEventResult esc = opi_key_event((0x1u << 16) | 0x1Bu, 0); /* KEY_ESCAPE */
    printf("  KEY_ESCAPE         action=%d\n", esc.action);
    CHECK(esc.action == 0, "Esc 是直通键（路由不消费它），应 action=0，实为 %d", esc.action);
    opi_ffi_free_string(esc.text);

    buf = opi_buffer();
    CHECK(!eq_cp(buf, NULL, 0), "Esc 直通**不**清缓冲（路由不消费该键），缓冲应保持 \"nihao\"");
    opi_ffi_free_string(buf);

    opi_clear();
    buf = opi_buffer();
    CHECK(eq_cp(buf, NULL, 0), "opi_clear() 后缓冲应清空");
    opi_ffi_free_string(buf);

    /* ---------- 其余导出各调一次：证明每个导出都编得过、链得上、签名对 ---------- */
    printf("[11] 其余导出\n");
    OpiString ik = opi_input_key(&W, 1);
    dump("  opi_input_key(\"w\")", ik);
    opi_ffi_free_string(ik);
    buf = opi_buffer();
    CHECK(eq_cp(buf, (const uint16_t[]){'w'}, 1), "opi_input_key 后缓冲应为 \"w\"");
    opi_ffi_free_string(buf);

    OpiString sel0 = opi_select(0); /* 全局索引，与 opi_candidates 同序 */
    dump("  opi_select(0)", sel0);
    CHECK(sel0.ptr != NULL, "opi_select(0) 应选得出东西");
    opi_ffi_free_string(sel0);

    OpiString sp = opi_input_space();
    dump("  opi_input_space()", sp);
    opi_ffi_free_string(sp);
    opi_backspace();
    opi_set_shift(true);
    opi_set_shift(false);
    printf("  opi_shift_state() = %d\n", opi_shift_state());
    opi_remove_user_word(&W, 1);          /* 词不存在 → 无操作 */
    opi_remove_user_word(NULL, 0);        /* null → 无操作 */
    bool trad = opi_load_trad(NULL, 0);   /* 空路径 → false（回退简体库） */
    CHECK(trad == false, "opi_load_trad(NULL,0) 应返回 false，实为 %d", (int)trad);

    /* ---------- 全角 / 符号开关（B3 / B5 的三个出口） ----------
     * 这三条此前只被覆盖到「**符号存在**」（库里数得到名字），**没有一处真调过**。
     * 而 `opi_toggle_symbol` 恰恰是最需要这一层的那条：它**按值**返回 OpiString
     * （`*const u16` + `usize` 跨 ABI），且必须**恰好释放一次** —— 释放两次是 UB、
     * 不释放是泄漏，**两者在本机都测不出**。Rust 侧测试碰不到跨 ABI 的按值传递，
     * 这里是唯一能真碰的地方。 */
    printf("[11b] 全角 / 符号开关\n");
    opi_clear();

    /* 约定 4 的前置：Pinyin 的 default_fullwidth = 全角 ⇒ 装载后/切模式后读侧应为 true */
    bool fw0 = opi_fullwidth_state();
    printf("  opi_fullwidth_state() @Pinyin = %d\n", (int)fw0);
    CHECK(fw0 == true, "Pinyin 默认全角，读侧应为 true，实为 %d", (int)fw0);

    bool fw1 = opi_toggle_fullwidth();
    printf("  opi_toggle_fullwidth() = %d，紧接着读 = %d\n", (int)fw1,
           (int)opi_fullwidth_state());
    CHECK(fw1 == false, "全角→半角应返回**切换后**的新状态 false，实为 %d", (int)fw1);
    CHECK(opi_fullwidth_state() == fw1,
          "读侧应与 toggle 的返回值一致 —— 不一致说明两侧各自记了一份状态");
    CHECK(opi_toggle_fullwidth() == true, "再切回应返回 true");

    /* 约定 4：`toggle_symbol` 内部调 `switch_mode`，而 `switch_mode` **无条件**把全角
     * 重置为新模式的默认值。先把全角置开，这一步才可观测（Symbol 的默认是半角）。 */
    opi_toggle_fullwidth(); /* → false */
    opi_toggle_fullwidth(); /* → true */
    CHECK(opi_fullwidth_state() == true, "前置：先让全角处于开，否则看不出被重置");

    /* 切之前留一截未提交缓冲：返回的应是它的**待提交文本**（有候选 → 首候选） */
    opi_key_event(W, 0);
    opi_key_event(O, 0);
    OpiString ts = opi_toggle_symbol();
    dump("  opi_toggle_symbol()", ts);
    CHECK(eq_cp(ts, WO, 1),
          "返回的应是**切换前那截缓冲**的待提交文本 U+6211（我）—— "
          "名字里的 Symbol 是这次切换的目标，不是返回值的内容");
    opi_ffi_free_string(ts); /* 非空句柄必须恰好释放一次 */

    int32_t m_sym = opi_mode();
    bool fw_sym = opi_fullwidth_state();
    printf("  opi_mode() = %d，opi_fullwidth_state() = %d\n", m_sym, (int)fw_sym);
    CHECK(m_sym == 3, "toggle_symbol 后模式应为 3=Symbol，实为 %d", m_sym);
    CHECK(fw_sym == false,
          "Symbol 默认半角 ⇒ 全角必须已被重置为 false。读侧若仍为 true，说明平台侧"
          "自己缓存了全角、没在 toggle_symbol 之后重读（指示灯会亮错）");

    /* 缓冲已空：再切一次应返回空串（无提交），且**空句柄也要能安全 free** */
    OpiString ts2 = opi_toggle_symbol();
    dump("  opi_toggle_symbol() @空缓冲", ts2);
    CHECK(ts2.len == 0, "缓冲为空时应返回空串 = 无提交，实为 len=%zu", ts2.len);
    opi_ffi_free_string(ts2);
    CHECK(opi_mode() == 0, "从 Symbol 再切应回 Pinyin，实为 %d", opi_mode());
    CHECK(opi_fullwidth_state() == true,
          "回到 Pinyin ⇒ 全角随该模式默认值回到 true，实为 %d（这是约定 4 的反方向）",
          (int)opi_fullwidth_state());

    /* ---------- 中文标点开关（B3/B5 的另三个出口） ----------
     * 断言全部照头文件那三段写，不照「跑出来是什么」写：
     *   1. 默认开；set 之后**读侧**必须如实反映（本档必须有读侧）；
     *   2. toggle 返回的是**切换后**的新状态，且与读侧一致（不能两边各记一份）；
     *   3. ⚠️ 与全角那一档的**关键差别**：本档是用户偏好，
     *      `opi_switch_mode` / `opi_toggle_symbol` **都不重置它** —— 而全角是
     *      模式默认值、任何模式切换都会被重置（[11b] 刚实测过）。两者重读规则
     *      不通用；谁把这份「不重置」当 bug「修」掉，本段先红，而不是等平台侧
     *      的状态栏/勾选框亮错。
     *   4. 「只翻一个 bool」：不改 buffer / 候选集 / 页码。 */
    printf("[11c] 中文标点开关\n");

    bool cp0 = opi_chinese_punct();
    printf("  opi_chinese_punct() 默认 = %d\n", (int)cp0);
    CHECK(cp0 == true, "中文标点默认应为开，实为 %d", (int)cp0);

    opi_set_chinese_punct(false);
    CHECK(opi_chinese_punct() == false, "set_chinese_punct(false) 后读侧应为 false"
          "（读侧仍为 true ⇒ 写入口没落到同一份状态上）");
    opi_set_chinese_punct(true);
    CHECK(opi_chinese_punct() == true, "set_chinese_punct(true) 后读侧应为 true");

    /* 触发键入口：返回值就是**切换后**的新状态（状态栏/勾选框直接拿去刷新） */
    bool cp1 = opi_toggle_chinese_punct();
    printf("  opi_toggle_chinese_punct() = %d，紧接着读 = %d\n", (int)cp1,
           (int)opi_chinese_punct());
    CHECK(cp1 == false, "开→关应返回切换后的新状态 false，实为 %d", (int)cp1);
    CHECK(opi_chinese_punct() == cp1,
          "读侧应与 toggle 的返回值一致 —— 不一致说明两侧各自记了一份状态");
    CHECK(opi_toggle_chinese_punct() == true, "再切一次应回到 true（翻两次回原值）");

    /* 「只翻一个 bool」：全程不得动 buffer / 页码（头文件写明的语义） */
    OpiString cp_buf = opi_buffer();
    CHECK(eq_cp(cp_buf, NULL, 0), "标点开关不得动 buffer，实为 len=%zu", cp_buf.len);
    opi_ffi_free_string(cp_buf);
    CHECK(opi_page() == 0 && opi_page_count() == 0,
          "标点开关不得动页码，实为 %u/%u", opi_page(), opi_page_count());

    /* 用户偏好：switch_mode 不得重置（同一个出口，[11b] 里全角是被重置的那一个） */
    opi_set_chinese_punct(false);
    opi_switch_mode(1); /* English */
    CHECK(opi_chinese_punct() == false,
          "opi_switch_mode 不得重置中文标点（用户偏好 ≠ 模式默认值）");
    opi_switch_mode(0);
    CHECK(opi_chinese_punct() == false, "切回 Pinyin 也不得重置");

    /* toggle_symbol 内部调 switch_mode ⇒ 它重置全角，但**不重置**本档 */
    OpiString ts3 = opi_toggle_symbol(); /* Pinyin → Symbol */
    dump("  opi_toggle_symbol()（标点段）", ts3);
    CHECK(ts3.len == 0, "缓冲为空 ⇒ 本应返回空串，实为 len=%zu", ts3.len);
    opi_ffi_free_string(ts3);
    CHECK(opi_chinese_punct() == false,
          "opi_toggle_symbol 不得重置中文标点（它只重置全角那一档）");
    OpiString ts4 = opi_toggle_symbol(); /* Symbol → Pinyin */
    CHECK(eq_cp(ts4, NULL, 0), "再切回应返回空串（无提交）");
    opi_ffi_free_string(ts4);
    CHECK(opi_mode() == 0, "两次 toggle_symbol 后应回 Pinyin，实为 %d", opi_mode());
    opi_set_chinese_punct(true); /* 复位：别把状态漏给后面的段落 */

    /* ---------- 释放语义：空句柄可无条件释放 ---------- */
    printf("[12] 释放语义\n");
    opi_ffi_free_string((OpiString){NULL, 0});
    opi_ffi_free_string((OpiString){NULL, 0});
    CHECK(1, "空句柄重复释放未崩溃");

    printf("\n结果：%d/%d 通过（%d 失败）\n", checks - failures, checks, failures);
    return failures == 0 ? 0 : 1;
}
