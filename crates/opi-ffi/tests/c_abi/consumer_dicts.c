/* SPDX-FileCopyrightText: 2026 erik.xyz
 * SPDX-License-Identifier: MIT
 *
 * consumer.c 的 **#include 片段**（不是独立 TU）：`[11d] 导入词库` 那一段。
 * 拆出去的唯一理由是 500 行硬门禁（crates/opi-ffi/tests/line_limit.rs 扫全仓源码，
 * consumer.c 加完那段是 612 行）—— 与 fcitx5-opi 的 `opi_ctl_*_driver.cpp` 同一种拆法。
 * ⚠️ 本文件**必须**经 `#include "consumer_dicts.c"` 编译（在 consumer.c 里、`int main`
 * 之前）：单独 -fsyntax-only 一个片段会缺 OpiFFI.h 的类型与 CHECK/`dump`/计数变量。
 *
 * 段职责（与 consumer.c 头注释同一条）：照 **macos/OpiFFI.h** 的声明绑过去，证明
 * importDict/dictList/setDictEnabled/removeDict 这四条真能用（签名错位 / 返回值约定错 /
 * UTF-16 路径解错都在这里露），并真跑一遍「导入 → 候选里真有这个词 → 停用 → 没了 →
 * 删掉」。四条出口的**逐字段行为矩阵**由 Rust 侧 `tests/dict_import.rs` 钉住。
 */

/* UTF-16 句柄里找一段码元子序列（本文件一直用逐位扫描代替 JSON 解析，见 [4]）。 */
static int has_units(OpiString s, const uint16_t *needle, size_t n) {
    if (!s.ptr || s.len < n || n == 0) return 0;
    for (size_t i = 0; i + n <= s.len; i++) {
        size_t j = 0;
        while (j < n && s.ptr[i + j] == needle[j]) j++;
        if (j == n) return 1;
    }
    return 0;
}

/* 同上，针是 ASCII 字面量（dictList 的 JSON 键与词库名都是 ASCII）。 */
static int has_ascii(OpiString s, const char *lit) {
    uint16_t buf[64];
    size_t n = strlen(lit);
    if (n >= sizeof buf / sizeof buf[0]) return 0;
    for (size_t i = 0; i < n; i++) buf[i] = (uint16_t)(unsigned char)lit[i];
    return has_units(s, buf, n);
}

/* [11d]：夹具落临时目录、四条出口逐条真调、收尾清干净（宿主视角的删文件也在内）。 */
static void check_import_dicts(void) {
    printf("[11d] 导入词库（importDict/dictList/setDictEnabled/removeDict）\n");

    /* 夹具落临时目录。**文件名必须正好是 `smoke.tsv`**（词库名 = 文件名词干是
     * 本族契约里被钉住的一条），所以唯一性只能落在**目录**上 —— 按 pid 造一个，
     * 并发跑的两个进程不会撞名。 */
    const char *tmp = getenv("TMPDIR");
    if (!tmp || !*tmp) tmp = "/tmp";
    char ddir[512];
    snprintf(ddir, sizeof ddir, "%s/opi_consumer_dict_%u", tmp, (unsigned)getpid());
    (void)mkdir(ddir, 0700); /* 已存在（同一 pid 的上次残骸）即复用，不判失败 */
    char tsv[512];
    snprintf(tsv, sizeof tsv, "%s/smoke.tsv", ddir);
    /* 词条是 `词<TAB>拼音<TAB>频次` —— **词在前**（列序真源是 engine-data 的
     * dict_text.rs：`cols[0]=word`；`data/raw/brand_terms.tsv` 同为「腾讯/engxun」）。
     * 频次贴 u32 上限：CI 拿的是完整 luna.opid，`ce` 一族的候选比本地回退库多得多，
     * 得保证这个词排得进前 64。 */
    FILE *fx = fopen(tsv, "wb");
    CHECK(fx != NULL, "写不出夹具 %s", tsv);
    if (fx) {
        fputs("测词\tce\t999999999\n", fx);
        fclose(fx);
    }

    size_t pn = strlen(tsv);
    uint16_t *p16 = (uint16_t *)malloc((pn + 1) * sizeof(uint16_t));
    CHECK(p16 != NULL, "malloc 路径缓冲失败");
    for (size_t i = 0; i < pn; i++) p16[i] = (uint16_t)(unsigned char)tsv[i];

    static const uint16_t WORD[] = {0x6D4B, 0x8BCD}; /* 测词 */
    static const uint16_t SMOKE[] = {'s', 'm', 'o', 'k', 'e'};
    static const uint16_t LUNA[] = {'l', 'u', 'n', 'a'};
    static const uint16_t CE[] = {'c', 'e'};

    int32_t dn = opi_import_dict(p16, pn);
    printf("  opi_import_dict() = %d\n", dn);
    CHECK(dn == 1, "1 条有效行应返回 1，实为 %d（-1 = 失败：先看列序是不是写反了）", dn);

    /* 真进了引擎 —— 本段最承重的一条：ABI 通了 ≠ 词到了引擎里。 */
    opi_clear();
    opi_ffi_free_string(opi_input_key(CE, 1)); /* ⚠️ 一次一个字符：本出口多字符入参**整体忽略** */
    opi_ffi_free_string(opi_input_key(CE + 1, 1));
    OpiString c_on = opi_candidates(64);
    CHECK(has_units(c_on, WORD, 2), "导入层的词应出现在 `ce` 的候选里（词没进引擎？）");
    opi_ffi_free_string(c_on);

    /* dictList：导入层与系统层都在；键序是 serde 字典序（count/enabled/name/system），
     * 所以按「键值相邻」扫，不按位置 —— 这条 C 侧写法与解析约定同一条。 */
    OpiString dl = opi_dict_list();
    dump("  opi_dict_list()", dl);
    CHECK(has_ascii(dl, "\"name\":\"luna\""), "系统词典 luna 应在列表里");
    CHECK(has_ascii(dl, "\"name\":\"smoke\""), "导入层名字 = 文件名词干（smoke.tsv → smoke）");
    CHECK(has_ascii(dl, "\"enabled\":true,\"name\":\"smoke\""), "刚导入 ⇒ enabled=true");
    opi_ffi_free_string(dl);

    /* 停用 = 从引擎的额外层拿掉；幂等；系统词典整条出口都拒。 */
    CHECK(opi_set_dict_enabled(SMOKE, 5, false) == true, "停用已导入的词库 → true");
    opi_clear();
    opi_ffi_free_string(opi_input_key(CE, 1)); /* ⚠️ 一次一个字符：本出口多字符入参**整体忽略** */
    opi_ffi_free_string(opi_input_key(CE + 1, 1));
    OpiString c_off = opi_candidates(64);
    CHECK(!has_units(c_off, WORD, 2), "停用后候选里不该再有它");
    opi_ffi_free_string(c_off);
    CHECK(opi_set_dict_enabled(SMOKE, 5, false) == true, "已是目标态 ⇒ 幂等 true");
    CHECK(opi_set_dict_enabled(LUNA, 4, false) == false, "系统词典不可停用 → false");
    CHECK(opi_set_dict_enabled(LUNA, 4, true) == false,
          "⚠️ 系统词典这条出口**一律**拒（它不是导入层）—— 即便 on 与它当前状态一致"
          "也是 false；别按「已是目标态 ⇒ true」推");

    /* 裁决 1 的执行点：**先把文件删掉**再启用 —— 数据在 FFI 层内存里，不依赖原文件。 */
    CHECK(remove(tsv) == 0, "删夹具文件");
    CHECK(opi_set_dict_enabled(SMOKE, 5, true) == true, "原文件没了也要能再启用");
    opi_clear();
    opi_ffi_free_string(opi_input_key(CE, 1)); /* ⚠️ 一次一个字符：本出口多字符入参**整体忽略** */
    opi_ffi_free_string(opi_input_key(CE + 1, 1));
    OpiString c_back = opi_candidates(64);
    CHECK(has_units(c_back, WORD, 2), "再启用后候选里应有它（依赖原文件就会在这里空）");
    opi_ffi_free_string(c_back);

    /* 读不到 → -1（路径已不存在），且**不改动**既有导入层。 */
    int32_t dn2 = opi_import_dict(p16, pn);
    printf("  opi_import_dict()（文件已删）= %d\n", dn2);
    CHECK(dn2 == -1, "读不到 → -1，实为 %d", dn2);
    OpiString dl2 = opi_dict_list();
    CHECK(has_ascii(dl2, "\"enabled\":true,\"name\":\"smoke\""),
          "失败的导入不得改动成功的那个（连启用位带名字）");
    opi_ffi_free_string(dl2);

    /* removeDict：从内存删掉（**宿主删文件是宿主的事**，上面那步就是宿主视角）。 */
    CHECK(opi_remove_dict(SMOKE, 5) == true, "删已导入的词库 → true");
    CHECK(opi_remove_dict(SMOKE, 5) == false, "已经不在 ⇒ false");
    CHECK(opi_remove_dict(LUNA, 4) == false, "系统词典 ⇒ false");
    OpiString dl3 = opi_dict_list();
    CHECK(!has_ascii(dl3, "\"name\":\"smoke\""), "删掉之后列表里没有它");
    opi_ffi_free_string(dl3);
    opi_clear();
    opi_ffi_free_string(opi_input_key(CE, 1)); /* ⚠️ 一次一个字符：本出口多字符入参**整体忽略** */
    opi_ffi_free_string(opi_input_key(CE + 1, 1));
    OpiString c_gone = opi_candidates(64);
    CHECK(!has_units(c_gone, WORD, 2), "删掉之后候选里不该再有它的词");
    opi_ffi_free_string(c_gone);

    free(p16);
    remove(tsv); /* 幂等（上面已删）：防中途 CHECK 失败提前退出时留残骸 */
    remove(ddir); /* `remove` 对**空目录**等于 rmdir —— 别把空壳留在 /tmp */
    opi_clear();

}
