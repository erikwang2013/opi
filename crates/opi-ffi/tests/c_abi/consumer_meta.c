/* SPDX-FileCopyrightText: 2026 erik.xyz
 * SPDX-License-Identifier: MIT
 *
 * consumer.c 的 **#include 片段**（不是独立 TU）：`[15] 版本 / 页大小 / 待提交 /
 * 语言切换` 那一段。拆出去的唯一理由是 500 行硬门禁（crates/opi-ffi/tests/line_limit.rs
 * 扫全仓源码）—— 与 consumer_dicts.c 同一种拆法。
 * ⚠️ 本文件**必须**经 `#include "consumer_meta.c"` 编译（在 consumer.c 里、`int main`
 * 之前）：单独 -fsyntax-only 一个片段会缺 OpiFFI.h 的类型与 CHECK/`dump`/计数变量。
 *
 * 段职责：证明 2026-10-28 新加的四个出口在 C 面**真的链得上、签名对**（只声明不算）：
 * 版本 / 页大小不依赖装载，待提交文本即使是空句柄也必须能安全释放，语言切换是
 * `void` 且不凭空造出状态。行为细节的权威在 Rust 侧 `cabi_unloaded.rs` / `cabi_test.rs`。
 */

/* [15]：装载后、空状态时调用一次（冷进程语义由 cabi_unloaded.rs 钉）。 */
static void check_meta_exports(void) {
    printf("[15] 版本 / 页大小 / 待提交 / 语言切换\n");

    OpiString ver = opi_engine_version();
    dump("  opi_engine_version()", ver);
    CHECK(ver.len > 0, "版本串不应为空（workspace 版本随包写入）");
    opi_ffi_free_string(ver);

    uint32_t page_size = opi_page_size();
    printf("  opi_page_size()     = %u\n", page_size);
    /* 不抄 8：页大小的唯一载体就是这个出口（见 macos/OpiFFI.h 的注释）。 */
    CHECK(page_size > 0, "页大小应非 0，实为 %u", page_size);

    OpiString pend = opi_pending_text();
    dump("  opi_pending_text()", pend);
    CHECK(pend.len == 0, "新引擎没有待提交片段，实为 len=%zu", pend.len);
    opi_ffi_free_string(pend);

    /* 语言切换（void）：合法参数、不 panic、不改模式、不建出待提交文本。 */
    opi_switch_language(0, 0);
    CHECK(opi_mode() == 0, "switch_language(0,0) 后模式应仍为 0，实为 %d", opi_mode());
    pend = opi_pending_text();
    CHECK(pend.len == 0, "switch_language 不该凭空产生待提交片段");
    opi_ffi_free_string(pend);
}
