/* SPDX-FileCopyrightText: 2026 erik.xyz
 * SPDX-License-Identifier: MIT
 *
 * consumer.c 的 **#include 片段**（不是独立 TU）：`[14] 自定义短语` 那一段。
 * 拆出去的理由与 `consumer_dicts.c` / `consumer_wubi.c` 同（500 行硬门禁 + 段职责独立）。
 * ⚠️ 必须经 `#include "consumer_phrases.c"` 编译（在 consumer.c 里、`int main` 之前）：
 * 单独 -fsyntax-only 会缺 OpiFFI.h 的类型、CHECK 与 `has_ascii`。
 *
 * 段职责：按 macos/OpiFFI.h 的声明调四条出口，验签名/返回值约定/句柄释放。
 * 逐字段行为矩阵（校验原子性、精确优先、9/14 键编码）在
 * `crates/engine-core/tests/custom_phrases.rs` —— 这里不重造一套。
 */

static const char PHRASE_JSON[] =
    "{\"version\":1,\"phrases\":[{\"trigger\":\"dz\",\"text\":\"addr\"}]}";

static void check_custom_phrases(void) {
    printf("[14] 自定义短语（set/export/enabled）\n");
    CHECK(opi_custom_phrases_enabled() == true, "短语开关缺省应为开");

    uint16_t buf[sizeof(PHRASE_JSON)];
    size_t n = strlen(PHRASE_JSON);
    for (size_t i = 0; i < n; i++) buf[i] = (uint16_t)(unsigned char)PHRASE_JSON[i];
    int32_t added = opi_set_custom_phrases(buf, n);
    CHECK(added == 1, "合法 JSON 应导入 1 条，实为 %d", added);

    OpiString exported = opi_export_custom_phrases();
    CHECK(has_ascii(exported, "\"version\":1"), "导出应含 version:1");
    CHECK(has_ascii(exported, "\"trigger\":\"dz\""), "导出应含归一后的 trigger");
    opi_ffi_free_string(exported);

    opi_set_custom_phrases_enabled(false);
    CHECK(opi_custom_phrases_enabled() == false, "set_custom_phrases_enabled(false) 生效");
    opi_set_custom_phrases_enabled(true);
    CHECK(opi_custom_phrases_enabled() == true, "再开回 true");

    /* 失败原子性：坏 JSON 返回负值，旧表逐字节不变。 */
    int32_t bad = opi_set_custom_phrases((const uint16_t[]){'{', '}'}, 2);
    CHECK(bad < 0, "坏 JSON 应返回负值，实为 %d", bad);
    CHECK(opi_set_custom_phrases(NULL, 0) < 0, "NULL 入参应返回负值");
    exported = opi_export_custom_phrases();
    CHECK(has_ascii(exported, "\"trigger\":\"dz\""), "失败后旧表必须原样保留");
    opi_ffi_free_string(exported);

    /* 英文套（2026-10-24）：同 schema，独立表/独立开关。 */
    printf("[14b] 英文自定义短语（独立一套）\n");
    CHECK(opi_english_custom_phrases_enabled() == true, "英文短语开关缺省应为开");
    CHECK(opi_set_english_custom_phrases(buf, n) == 1, "英文套导入 1 条");
    exported = opi_export_english_custom_phrases();
    CHECK(has_ascii(exported, "\"trigger\":\"dz\""), "英文套导出应含 trigger");
    opi_ffi_free_string(exported);
    opi_set_english_custom_phrases_enabled(false);
    CHECK(opi_english_custom_phrases_enabled() == false, "英文套开关可独立关闭");
    CHECK(opi_custom_phrases_enabled() == true, "关英文套不影响中文套开关");
    opi_set_english_custom_phrases_enabled(true);
    CHECK(opi_set_english_custom_phrases(NULL, 0) < 0, "英文套 NULL 入参应返回负值");
}
