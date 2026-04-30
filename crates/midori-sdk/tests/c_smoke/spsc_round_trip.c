/*
 * midori-sdk SPSC ring buffer C smoke test.
 *
 * `midori_sdk.h` を include して midori-sdk の C ABI 経由で
 *   1. storage 確保 (aligned_alloc + midori_sdk_spsc_init)
 *   2. push (4 byte payload)
 *   3. pop  (out_buf に書き戻し / out_len 更新)
 *   4. 値一致検証
 *   5. 空状態の pop が 0 を返すこと
 * を確認する。失敗時は非ゼロで exit する。Rust 側 integration test
 * (`tests/c_smoke.rs`) が exit code を assert する。
 */

#include "midori_sdk.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define EXPECT(cond, msg)                                                       \
    do {                                                                        \
        if (!(cond)) {                                                          \
            fprintf(stderr, "[c-smoke] FAIL: %s (line %d)\n", (msg), __LINE__); \
            return 1;                                                           \
        }                                                                       \
    } while (0)

int main(void) {
    /* DEFAULT_SLOT_SIZE = 1032 を使う（C 側に const は出力されないため
     * 数値を直接書く。値は midori-core::shm::DEFAULT_SLOT_SIZE と一致）。 */
    const uint32_t slot_size = 1032;

    /* `midori_sdk.h` の signature と一致させるため `uintptr_t` を使う。
     * LP64 環境では size_t と同サイズだが、type-correctness のため厳密に。 */
    uintptr_t storage_size = midori_sdk_spsc_storage_size(slot_size);
    EXPECT(storage_size > 0, "storage_size > 0");

    uintptr_t storage_align = midori_sdk_spsc_storage_alignment();
    EXPECT(storage_align >= 8, "storage_align >= 8");
    EXPECT((storage_align & (storage_align - 1)) == 0, "alignment is power of two");

    /* aligned_alloc は size が alignment の倍数であることを要求する。 */
    uintptr_t alloc_size = (storage_size + storage_align - 1) & ~(storage_align - 1);
    void *storage = aligned_alloc((size_t)storage_align, (size_t)alloc_size);
    EXPECT(storage != NULL, "aligned_alloc succeeded");

    int32_t init_rc = midori_sdk_spsc_init(storage, slot_size);
    EXPECT(init_rc == 1, "init returned 1");

    /* 空状態では pop が 0 を返す。 */
    uint8_t out_buf[1024];
    uintptr_t out_len = 0;
    int32_t empty_rc = midori_sdk_spsc_pop(storage, out_buf, sizeof(out_buf), &out_len);
    EXPECT(empty_rc == 0, "empty pop returns 0");

    /* 4 byte の payload を push → pop して値一致を確認。 */
    const uint8_t payload[4] = {0xDE, 0xAD, 0xBE, 0xEF};
    int32_t push_rc = midori_sdk_spsc_push(storage, payload, sizeof(payload));
    EXPECT(push_rc == 1, "push returned 1");

    int32_t pop_rc = midori_sdk_spsc_pop(storage, out_buf, sizeof(out_buf), &out_len);
    EXPECT(pop_rc == 1, "pop returned 1");
    EXPECT(out_len == sizeof(payload), "pop wrote expected length");
    EXPECT(memcmp(out_buf, payload, sizeof(payload)) == 0, "pop bytes match push bytes");

    /* もう一度 pop すると 0（空）。 */
    int32_t empty_rc2 = midori_sdk_spsc_pop(storage, out_buf, sizeof(out_buf), &out_len);
    EXPECT(empty_rc2 == 0, "subsequent pop on empty returns 0");

    /* 最大サイズ payload (slot_size - 8 = 1024 byte) でも round-trip する。 */
    uint8_t large_payload[1024];
    for (size_t i = 0; i < sizeof(large_payload); ++i) {
        large_payload[i] = (uint8_t)(i % 251);
    }
    int32_t push_max_rc =
        midori_sdk_spsc_push(storage, large_payload, sizeof(large_payload));
    EXPECT(push_max_rc == 1, "push max-size payload returned 1");

    int32_t pop_max_rc =
        midori_sdk_spsc_pop(storage, out_buf, sizeof(out_buf), &out_len);
    EXPECT(pop_max_rc == 1, "pop max-size returned 1");
    EXPECT(out_len == sizeof(large_payload), "max payload length matches");
    EXPECT(memcmp(out_buf, large_payload, sizeof(large_payload)) == 0,
           "max payload bytes match");

    /* slot 容量超過の push は -2 を返す（events.yaml 違反 / driver 実装バグ
     * 相当。本テストでは契約検証のみ）。 */
    uint8_t too_large[1025];
    memset(too_large, 0, sizeof(too_large));
    int32_t over_rc = midori_sdk_spsc_push(storage, too_large, sizeof(too_large));
    EXPECT(over_rc == -2, "oversize push returns -2");

    /* caller buffer が payload より小さいケースは -3 を返し、`*out_len` には
     * 本来必要だった byte 数が書かれる。SPSC 規律上 slot は消費されるため、
     * 直後の pop は empty (0) を返す。 */
    const uint8_t small_payload[64] = {0};
    int32_t push_small_rc =
        midori_sdk_spsc_push(storage, small_payload, sizeof(small_payload));
    EXPECT(push_small_rc == 1, "push 64-byte payload returned 1");
    uint8_t tiny_buf[32];
    uintptr_t tiny_len = 0;
    int32_t cap_rc =
        midori_sdk_spsc_pop(storage, tiny_buf, sizeof(tiny_buf), &tiny_len);
    EXPECT(cap_rc == -3, "capacity-insufficient pop returns -3");
    EXPECT(tiny_len == sizeof(small_payload), "out_len reports required bytes");
    int32_t empty_rc3 =
        midori_sdk_spsc_pop(storage, out_buf, sizeof(out_buf), &out_len);
    EXPECT(empty_rc3 == 0, "slot was consumed even though pop returned -3");

    free(storage);
    fprintf(stdout, "[c-smoke] OK\n");
    return 0;
}
