/* SPDX-FileCopyrightText: 2026 erik.xyz
 * SPDX-License-Identifier: MIT
 *
 * consumer.c 的 **#include 片段**（不是独立 TU）：`[13] 五笔旋钮族` 那一段。
 * 拆出去的理由与 `consumer_dicts.c` 同（500 行硬门禁 + 段职责独立）。
 * ⚠️ 必须经 `#include "consumer_wubi.c"` 编译（在 consumer.c 里、`int main` 之前）：
 * 单独 -fsyntax-only 会缺 OpiFFI.h 的类型与 CHECK/计数变量 —— 且本段依赖
 * `consumer_dicts.c` 准备的**导入词库目录**（`opi_load_wubi` 要一个真 .opid）。
 *
 * 段职责：照 **macos/OpiFFI.h** 的声明绑过去，证明这 12 条出口真能用（签名/返回值约定/
 * UTF-16 串都在这里露），并走一遍「装载 wubi 码表 → 方案切到五笔 → 打字出候选 →
 * 旋钮读写回环 → 编码提示」。逐字段行为矩阵在 Rust 侧 `crates/engine-core/tests/wubi.rs`。
 */

static const uint16_t WQVB[] = {'w', 'q', 'v', 'b'};

/* argv 里没有 wubi 表路径时跳过本段（照 [11b] 笔画那段的「可选增强」姿态：
 * 表没带 ≠ 引擎坏）。返回 1 = 本段已跑。 */
static int wubi_segment(const char *wubi_path) {
    printf("[13] 五笔旋钮族（loadWubi/风格/混入方式/个数/回车/自动上屏/编码提示）\n");
    if (wubi_path == NULL) {
        printf("  无 wubi 表路径参数 → 跳过（本段可选，见用法注释）\n");
        return 0;
    }
    size_t n = strlen(wubi_path);
    uint16_t *buf = (uint16_t *)malloc((n + 1) * sizeof(uint16_t));
    if (!buf) return 0;
    for (size_t i = 0; i < n; i++) buf[i] = (uint16_t)(unsigned char)wubi_path[i];

    bool ok = opi_load_wubi(buf, n);
    free(buf);
    CHECK(ok, "opi_load_wubi 返回 false（表存在却装不上？路径：%s）", wubi_path);
    if (!ok) return 0;

    /* 方案切到五笔（整数表：5=五笔，见 api::scheme_from_int）。 */
    opi_set_scheme(5);
    CHECK(opi_scheme() == 5, "set_scheme(5) 后应读到五笔，实为 %d", opi_scheme());

    /* 旋钮缺省值（读侧哨兵与真缺省要分清：引擎已装载 ⇒ 读到的是真缺省）。 */
    CHECK(opi_wubi_style() == 0, "风格缺省 = 纯五笔（0），实为 %d", opi_wubi_style());
    CHECK(opi_wubi_mixed_entry() == 0, "混入方式缺省 = 直接字母（0），实为 %d",
          opi_wubi_mixed_entry());
    CHECK(opi_wubi_return_style() == 0, "回车缺省 = 清空（0），实为 %d", opi_wubi_return_style());
    CHECK(opi_wubi_auto_commit() == true, "自动上屏缺省 = 开");
    CHECK(opi_wubi_candidate_limit() == 5, "候选个数缺省 = 5，实为 %d", opi_wubi_candidate_limit());

    /* 写回环 + 越界语义（写侧越界忽略 / 个数钳区间）。 */
    opi_set_wubi_style(1);
    CHECK(opi_wubi_style() == 1, "set_wubi_style(1) 后应读到混输");
    opi_set_wubi_style(99);
    CHECK(opi_wubi_style() == 1, "越界风格忽略（仍为 1）");
    opi_set_wubi_style(0);

    opi_set_wubi_mixed_entry(1);
    CHECK(opi_wubi_mixed_entry() == 1, "set_wubi_mixed_entry(1) 后应读到 z 键档");
    opi_set_wubi_mixed_entry(-1);
    CHECK(opi_wubi_mixed_entry() == 1, "越界混入方式忽略（仍为 1）");
    opi_set_wubi_mixed_entry(0);

    opi_set_wubi_return_style(1);
    CHECK(opi_wubi_return_style() == 1, "set_wubi_return_style(1) 后应读到上屏原文");
    opi_set_wubi_return_style(0);

    opi_set_wubi_auto_commit(false);
    CHECK(opi_wubi_auto_commit() == false, "关掉后应读到 false");
    opi_set_wubi_auto_commit(true);

    opi_set_wubi_candidate_limit(3);
    CHECK(opi_wubi_candidate_limit() == 3, "set_wubi_candidate_limit(3) 后应读到 3");
    opi_set_wubi_candidate_limit(99);
    CHECK(opi_wubi_candidate_limit() == 9, "过大钳到 9，实为 %d", opi_wubi_candidate_limit());
    opi_set_wubi_candidate_limit(0);
    CHECK(opi_wubi_candidate_limit() == 1, "0/负数归 1，实为 %d", opi_wubi_candidate_limit());
    opi_set_wubi_candidate_limit(5);

    /* 真打一个码：wqvb（86 表里是「你」字族 —— 值是数据事实，这里只断「有候选」）。 */
    for (size_t i = 0; i < 4; i++) {
        OpiKeyEventResult r = opi_key_event(WQVB[i], 0);
        opi_ffi_free_string(r.text);
    }
    OpiString cands = opi_candidates(8);
    CHECK(cands.ptr != NULL && cands.len > 2, "五笔四码应出候选（wqvb）");
    opi_ffi_free_string(cands);

    /* 回车判据（2026-10-14 接线）：四码在手 + 缺省档（清空）⇒ 1；
       CommitRaw ⇒ 2；缓冲清掉 ⇒ 0（安全默认 = 各端既有行为）。 */
    CHECK(opi_enter_action() == 1, "五笔 + 缓冲非空 + 缺省档 ⇒ 清空(1)，实为 %d",
          opi_enter_action());
    opi_set_wubi_return_style(1);
    CHECK(opi_enter_action() == 2, "上屏原文档 ⇒ 2，实为 %d", opi_enter_action());
    opi_set_wubi_return_style(0);
    CHECK(opi_enter_action() == 1, "切回清空档 ⇒ 1，实为 %d", opi_enter_action());

    /* 编码提示：四码之后多半没有接续键（86 最长 4 码）⇒ 断言「取得到」（空串合法）。 */
    OpiString hint = opi_wubi_next_keys();
    printf("  opi_wubi_next_keys() len=%zu\n", hint.len);
    opi_ffi_free_string(hint);

    opi_clear();
    CHECK(opi_enter_action() == 0, "空缓冲 ⇒ 0（选首候选，四端既有行为）");
    opi_set_scheme(0);
    return 1;
}
