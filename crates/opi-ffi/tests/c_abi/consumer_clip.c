/* SPDX-FileCopyrightText: 2026 erik.xyz
 * SPDX-License-Identifier: MIT
 *
 * [16] opi-clip 的 C ABI：真调全部 13 个导出（consumer.c 的 #include 片段，
 * 不是独立 TU —— 500 行门禁，拆法同 consumer_dicts.c）。
 *
 * 只验证 FFI 骨架与哨兵约定：无对端、不依赖真实局域网。数据目录留在 /tmp
 * （身份/信任表由 start 建），不在 C 里递归删目录 —— 那是测试夹具，不是产品路径。
 */

/* ASCII → UTF-16；本片段只喂 ASCII 路径与十六进制实例 ID。 */
static size_t clip_ascii_to_utf16(const char *s, uint16_t *out, size_t cap) {
    size_t n = 0;
    while (s[n] != '\0' && n < cap) {
        out[n] = (uint16_t)(unsigned char)s[n];
        n++;
    }
    return n;
}

static void check_clip_exports(void) {
    printf("[16] opi-clip FFI\n");

    static const uint16_t WO_CLIP[] = {0x6211}; /* 我 */
    static const char PEER_HEX[] = "01010101010101010101010101010101";

    char dir[128];
    snprintf(dir, sizeof dir, "/tmp/opi-clip-ffi-consumer-%ld", (long)getpid());
    char cfg[512];
    int cfg_n = snprintf(cfg, sizeof cfg,
                         "{\"data_dir\":\"%s\",\"tcp_port\":0,"
                         "\"discovery_port\":45987,\"device_name\":\"c-consumer\"}",
                         dir);
    CHECK(cfg_n > 0 && (size_t)cfg_n < sizeof cfg, "clip 配置 JSON 放不下");
    if (cfg_n <= 0 || (size_t)cfg_n >= sizeof cfg) return;

    uint16_t cfg16[512];
    size_t cfg_len = clip_ascii_to_utf16(cfg, cfg16, sizeof cfg16 / sizeof cfg16[0]);
    CHECK(cfg_len == (size_t)cfg_n, "clip 配置 JSON 被截断");
    if (cfg_len != (size_t)cfg_n) return;

    uint16_t peer16[32];
    size_t peer_len = clip_ascii_to_utf16(PEER_HEX, peer16, sizeof peer16 / sizeof peer16[0]);
    CHECK(peer_len == 32, "peer 十六进制 ID 应为 32 个码元");

    OpiClip *clip = opi_clip_create(cfg16, cfg_len);
    if (!clip) {
        OpiString err = opi_clip_last_error();
        dump("  opi_clip_create 失败", err);
        opi_ffi_free_string(err);
        CHECK(0, "opi_clip_create 返回 NULL");
        return;
    }

    /* 未 start：peers=[]、status 有 JSON、local_text=-1 且 last_error 非空。 */
    OpiString peers = opi_clip_peers(clip);
    CHECK(peers.len == 2 && peers.ptr && peers.ptr[0] == '[' && peers.ptr[1] == ']',
          "未 start 的 peers 应为 []，实为 len=%zu", peers.len);
    opi_ffi_free_string(peers);

    OpiString status = opi_clip_status(clip);
    CHECK(status.len > 0 && status.ptr != NULL, "未 start 的 status 应返回非空 JSON");
    opi_ffi_free_string(status);

    CHECK(opi_clip_local_text(clip, WO_CLIP, 1) == -1,
          "未 start 的 local_text 应为 -1（原因走 last_error）");
    OpiString err = opi_clip_last_error();
    CHECK(err.len > 0 && err.ptr != NULL, "失败后 last_error 应为非空");
    opi_ffi_free_string(err);

    /* start：幂等；无对端。 */
    CHECK(opi_clip_start(clip) == true, "opi_clip_start 应成功");
    CHECK(opi_clip_start(clip) == true, "opi_clip_start 重复调用应幂等");

    CHECK(opi_clip_local_text(clip, WO_CLIP, 1) == 0,
          "start 后 local_text 应为 0=已发布");

    OpiString event = opi_clip_poll_event(clip, 0);
    CHECK(event.len == 0, "timeout=0 应立即返回空串（无事件），实为 len=%zu", event.len);
    opi_ffi_free_string(event);

    /* 未发现的对端：pair 三连失败；forget 对未知对端幂等成功。 */
    CHECK(opi_clip_pair_start(clip, peer16, peer_len) == false,
          "未发现的对端 pair_start 应为 false");
    CHECK(opi_clip_pair_confirm(clip, peer16, peer_len, true) == false,
          "无待确认配对时 pair_confirm 应为 false");
    CHECK(opi_clip_pair_cancel(clip, peer16, peer_len) == false,
          "无待确认配对时 pair_cancel 应为 false");
    CHECK(opi_clip_forget_peer(clip, peer16, peer_len) == true,
          "forget_peer 对未知对端应幂等成功");

    err = opi_clip_last_error();
    CHECK(err.len > 0 && err.ptr != NULL, "pair_start 失败后 last_error 应为非空");
    opi_ffi_free_string(err);

    peers = opi_clip_peers(clip);
    CHECK(peers.len == 2, "start 后无对端，peers 应为 []，实为 len=%zu", peers.len);
    opi_ffi_free_string(peers);

    status = opi_clip_status(clip);
    CHECK(status.len > 0 && status.ptr != NULL, "start 后 status 应非空");
    opi_ffi_free_string(status);

    /* stop/destroy 幂等；NULL 句柄安全。 */
    opi_clip_stop(clip);
    opi_clip_stop(clip);
    opi_clip_destroy(clip);
    opi_clip_destroy(NULL);
    CHECK(1, "opi_clip_stop/destroy 重复调用与 destroy(NULL) 未崩溃");
}
