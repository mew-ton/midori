//! C ABI 経由で SPSC リングバッファを操作するためのエクスポート群。
//!
//! Python (`PyO3`) や Node.js (`napi-rs`) などの他言語バインディングを書く際の
//! 基盤として、`extern "C"` 関数で SPSC の主要操作を公開する。
//!
//! # メモリ確保ポリシー
//!
//! [`SpscStorage`] の内部レイアウト
//! （`UnsafeCell` の配列など）は不透明型として扱う。C 側はサイズと
//! アラインメントを問い合わせて自前で確保し、[`midori_sdk_spsc_init`] で
//! 初期化する。
//!
//! ```c
//! size_t size  = midori_sdk_spsc_storage_size();
//! size_t align = midori_sdk_spsc_storage_alignment();
//! void* storage = aligned_alloc(align, size);
//! midori_sdk_spsc_init(storage);
//! ```
//!
//! # SPSC 規律
//!
//! [`midori_sdk_spsc_push`] は同時に 1 スレッドからのみ、
//! [`midori_sdk_spsc_pop`] も同時に 1 スレッドからのみ呼ばれること。
//! 違反時の挙動は未規定。これは Rust 側 API では `&mut Producer` /
//! `&mut Consumer` で型レベルに保証している規律と同じ。

use std::ffi::c_void;

use midori_core::shm::RingSlot;

use crate::spsc::{self, SpscStorage};

/// [`SpscStorage`] を確保するために必要なバイト数を返す。
#[allow(unsafe_code)] // `#[no_mangle]` は安全な関数でも unsafe_code lint の対象
#[no_mangle]
pub extern "C" fn midori_sdk_spsc_storage_size() -> usize {
    std::mem::size_of::<SpscStorage>()
}

/// [`SpscStorage`] を確保するときに必要なアラインメントを返す。
#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn midori_sdk_spsc_storage_alignment() -> usize {
    std::mem::align_of::<SpscStorage>()
}

/// 確保済みのメモリ領域に空の SPSC ストレージを書き込む。
///
/// # Safety
///
/// `storage` は以下を満たすメモリを指していること:
/// - [`midori_sdk_spsc_storage_size`] バイト以上の書き込み可能領域
/// - [`midori_sdk_spsc_storage_alignment`] でアラインされている
/// - 呼び出し中・以後の使用中、有効であり続ける
///
/// 既に初期化済みの領域を再初期化する場合、Producer/Consumer が同時に
/// アクセスしていないことを呼び出し側が保証すること。
#[allow(unsafe_code)]
#[no_mangle]
pub unsafe extern "C" fn midori_sdk_spsc_init(storage: *mut c_void) {
    if storage.is_null() {
        return;
    }
    // ~75 KB の `SpscStorage` を呼び出し側スタックに一時生成せず、宛先ポインタへ
    // 直接書き込む。小さなスレッドスタックから呼ばれた場合のオーバーフローを回避。
    SpscStorage::init_in_place(storage.cast::<SpscStorage>());
}

/// スロットを 1 つ push する。
///
/// 戻り値:
/// - `1`: 成功
/// - `0`: バッファ満杯または引数 NULL
///
/// # Safety
///
/// - `storage` は [`midori_sdk_spsc_init`] 済みの領域を指すこと
/// - `slot` は有効な [`RingSlot`] を指すこと（読み取りは
///   [`std::ptr::read_unaligned`] を使うためアラインメントは不問）
/// - 同時に 1 スレッドからのみ呼ばれること（SPSC 生産者規律）
#[allow(unsafe_code)]
#[no_mangle]
pub unsafe extern "C" fn midori_sdk_spsc_push(storage: *const c_void, slot: *const RingSlot) -> u8 {
    if storage.is_null() || slot.is_null() {
        return 0;
    }
    let storage = &*storage.cast::<SpscStorage>();
    // C 側で `#pragma pack` 等によりパックされたポインタを渡されても UB を
    // 起こさないよう unaligned read を採用する。
    let slot = std::ptr::read_unaligned(slot);
    u8::from(spsc::try_push(storage, &slot).is_ok())
}

/// スロットを 1 つ pop する。
///
/// 戻り値:
/// - `1`: 成功（`out_slot` に書き込み済み）
/// - `0`: バッファ空または引数 NULL
///
/// # Safety
///
/// - `storage` は [`midori_sdk_spsc_init`] 済みの領域を指すこと
/// - `out_slot` は書き込み可能な [`RingSlot`] を指すこと（書き込みは
///   [`std::ptr::write_unaligned`] を使うためアラインメントは不問）
/// - 同時に 1 スレッドからのみ呼ばれること（SPSC 消費者規律）
#[allow(unsafe_code)]
#[no_mangle]
pub unsafe extern "C" fn midori_sdk_spsc_pop(
    storage: *const c_void,
    out_slot: *mut RingSlot,
) -> u8 {
    if storage.is_null() || out_slot.is_null() {
        return 0;
    }
    let storage = &*storage.cast::<SpscStorage>();
    if let Some(slot) = spsc::try_pop(storage) {
        // push と同じ理由（C 側パック構造体）で unaligned write を採用。
        std::ptr::write_unaligned(out_slot, slot);
        1
    } else {
        0
    }
}

#[cfg(test)]
#[allow(unsafe_code, clippy::cast_possible_truncation)]
mod tests {
    use super::*;
    use midori_core::shm::{PAYLOAD_INLINE_MAX, RING_CAPACITY};

    /// inline payload に 4 byte の little-endian シーケンス番号を詰めた
    /// テスト用 [`RingSlot`] を作る。
    fn slot_with_seq(seq: u32) -> RingSlot {
        let mut s: RingSlot = unsafe { std::mem::zeroed() };
        s.occupied = 1;
        s.payload_len = 4;
        s.payload[..4].copy_from_slice(&seq.to_le_bytes());
        s
    }

    fn read_seq(slot: &RingSlot) -> u32 {
        assert_eq!(slot.occupied, 1);
        assert_eq!(slot.payload_len, 4);
        let mut buf = [0u8; 4];
        buf.copy_from_slice(&slot.payload[..4]);
        u32::from_le_bytes(buf)
    }

    /// FFI 経由で確保→初期化→push→pop が成立することを検証する結合テスト。
    #[test]
    fn it_should_round_trip_a_slot_through_ffi() {
        // C の aligned_alloc 相当を Rust 側で再現
        let layout = std::alloc::Layout::from_size_align(
            midori_sdk_spsc_storage_size(),
            midori_sdk_spsc_storage_alignment(),
        )
        .expect("valid layout");

        // SAFETY: layout は size/alignment 共に正で、後で同じ layout で dealloc する
        let raw = unsafe { std::alloc::alloc(layout) };
        assert!(!raw.is_null());

        unsafe {
            midori_sdk_spsc_init(raw.cast::<c_void>());
        }

        let pushed = slot_with_seq(42);
        let ok = unsafe {
            midori_sdk_spsc_push(
                raw.cast::<c_void>(),
                std::ptr::from_ref::<RingSlot>(&pushed),
            )
        };
        assert_eq!(ok, 1);

        let mut popped: RingSlot = unsafe { std::mem::zeroed() };
        let ok =
            unsafe { midori_sdk_spsc_pop(raw.cast::<c_void>(), std::ptr::from_mut(&mut popped)) };
        assert_eq!(ok, 1);
        assert_eq!(read_seq(&popped), 42);

        // 空状態では pop が 0 を返す
        let ok =
            unsafe { midori_sdk_spsc_pop(raw.cast::<c_void>(), std::ptr::from_mut(&mut popped)) };
        assert_eq!(ok, 0);

        // SAFETY: 同じ layout で dealloc
        unsafe {
            std::alloc::dealloc(raw, layout);
        }
    }

    /// `payload_len` = [`PAYLOAD_INLINE_MAX`] 上限ぴったりで FFI 経由のラウンドトリップを検証。
    #[test]
    fn it_should_round_trip_inline_payload_at_max_through_ffi() {
        let layout = std::alloc::Layout::from_size_align(
            midori_sdk_spsc_storage_size(),
            midori_sdk_spsc_storage_alignment(),
        )
        .expect("valid layout");
        // SAFETY: layout は size/alignment 共に正で、後で同じ layout で dealloc する
        let raw = unsafe { std::alloc::alloc(layout) };
        assert!(!raw.is_null());
        unsafe { midori_sdk_spsc_init(raw.cast::<c_void>()) };

        let mut pushed: RingSlot = unsafe { std::mem::zeroed() };
        pushed.occupied = 1;
        pushed.payload_len = PAYLOAD_INLINE_MAX as u32;
        for (i, byte) in pushed.payload.iter_mut().enumerate() {
            *byte = (i % 251) as u8;
        }

        let ok = unsafe {
            midori_sdk_spsc_push(
                raw.cast::<c_void>(),
                std::ptr::from_ref::<RingSlot>(&pushed),
            )
        };
        assert_eq!(ok, 1);

        let mut popped: RingSlot = unsafe { std::mem::zeroed() };
        let ok =
            unsafe { midori_sdk_spsc_pop(raw.cast::<c_void>(), std::ptr::from_mut(&mut popped)) };
        assert_eq!(ok, 1);
        assert_eq!(popped.payload_len as usize, PAYLOAD_INLINE_MAX);
        for (i, byte) in popped.payload.iter().enumerate() {
            assert_eq!(*byte, (i % 251) as u8, "byte at {i} mismatched");
        }
        // side channel は未使用
        assert_eq!(popped.side_offset, 0);
        assert_eq!(popped.side_len, 0);

        // SAFETY: 同じ layout で dealloc
        unsafe { std::alloc::dealloc(raw, layout) };
    }

    /// `payload_len` > [`PAYLOAD_INLINE_MAX`] 相当のケース: `payload_len` = 0 で
    /// `side_offset` / `side_len` のみ立てたスロットがそのまま運ばれることを検証。
    /// 本テストは FFI 経由のフィールド輸送のみを確認する（side channel 本体の
    /// 確保は本クレートのスコープ外）。
    #[test]
    fn it_should_carry_side_channel_offsets_through_ffi() {
        let layout = std::alloc::Layout::from_size_align(
            midori_sdk_spsc_storage_size(),
            midori_sdk_spsc_storage_alignment(),
        )
        .expect("valid layout");
        // SAFETY: layout は size/alignment 共に正で、後で同じ layout で dealloc する
        let raw = unsafe { std::alloc::alloc(layout) };
        assert!(!raw.is_null());
        unsafe { midori_sdk_spsc_init(raw.cast::<c_void>()) };

        let mut pushed: RingSlot = unsafe { std::mem::zeroed() };
        pushed.occupied = 1;
        pushed.payload_len = 0;
        pushed.side_offset = 4096;
        pushed.side_len = 1024;

        let ok = unsafe {
            midori_sdk_spsc_push(
                raw.cast::<c_void>(),
                std::ptr::from_ref::<RingSlot>(&pushed),
            )
        };
        assert_eq!(ok, 1);

        let mut popped: RingSlot = unsafe { std::mem::zeroed() };
        let ok =
            unsafe { midori_sdk_spsc_pop(raw.cast::<c_void>(), std::ptr::from_mut(&mut popped)) };
        assert_eq!(ok, 1);
        assert_eq!(popped.occupied, 1);
        assert_eq!(popped.payload_len, 0);
        assert_eq!(popped.side_offset, 4096);
        assert_eq!(popped.side_len, 1024);

        // SAFETY: 同じ layout で dealloc
        unsafe { std::alloc::dealloc(raw, layout) };
    }

    #[test]
    fn it_should_return_zero_when_called_with_null_pointers() {
        let mut slot: RingSlot = unsafe { std::mem::zeroed() };
        let dummy_slot = slot_with_seq(1);
        let nul = std::ptr::null::<c_void>();

        // storage が NULL のケース
        assert_eq!(
            unsafe { midori_sdk_spsc_push(nul, std::ptr::from_ref::<RingSlot>(&dummy_slot)) },
            0
        );
        assert_eq!(
            unsafe { midori_sdk_spsc_pop(nul, std::ptr::from_mut(&mut slot)) },
            0
        );

        // storage は有効だが slot/out_slot 側が NULL のケース。
        // OR の short-circuit で後段の NULL チェックも動作することを確認する。
        let layout = std::alloc::Layout::from_size_align(
            midori_sdk_spsc_storage_size(),
            midori_sdk_spsc_storage_alignment(),
        )
        .expect("valid layout");
        // SAFETY: layout は size/alignment 共に正で、後で同じ layout で dealloc する
        let raw = unsafe { std::alloc::alloc(layout) };
        assert!(!raw.is_null());
        unsafe { midori_sdk_spsc_init(raw.cast::<c_void>()) };

        assert_eq!(
            unsafe { midori_sdk_spsc_push(raw.cast::<c_void>(), std::ptr::null::<RingSlot>()) },
            0
        );
        assert_eq!(
            unsafe { midori_sdk_spsc_pop(raw.cast::<c_void>(), std::ptr::null_mut::<RingSlot>()) },
            0
        );

        // SAFETY: 同じ layout で dealloc
        unsafe { std::alloc::dealloc(raw, layout) };
    }

    /// `build.rs` が生成した C ヘッダに想定の関数宣言と新 [`RingSlot`] フィールドが
    /// 含まれていることを検証する。ヘッダ生成自体は `cargo build` 時にチェック済み。
    #[test]
    fn it_should_generate_c_header_with_expected_ffi_symbols() {
        const GENERATED_HEADER: &str = include_str!(concat!(env!("OUT_DIR"), "/midori_sdk.h"));
        assert!(GENERATED_HEADER.contains("MIDORI_SDK_H"));
        assert!(GENERATED_HEADER.contains("midori_sdk_spsc_storage_size"));
        assert!(GENERATED_HEADER.contains("midori_sdk_spsc_storage_alignment"));
        assert!(GENERATED_HEADER.contains("midori_sdk_spsc_init"));
        assert!(GENERATED_HEADER.contains("midori_sdk_spsc_push"));
        assert!(GENERATED_HEADER.contains("midori_sdk_spsc_pop"));
        assert!(GENERATED_HEADER.contains("RingSlot"));
        // 新レイアウトのフィールドがヘッダに公開されていること
        assert!(GENERATED_HEADER.contains("payload_len"));
        assert!(GENERATED_HEADER.contains("side_offset"));
        assert!(GENERATED_HEADER.contains("side_len"));
        assert!(GENERATED_HEADER.contains("payload"));
        assert!(GENERATED_HEADER.contains("PAYLOAD_INLINE_MAX"));
    }

    #[test]
    fn it_should_report_storage_size_and_alignment_consistent_with_layout() {
        let size = midori_sdk_spsc_storage_size();
        let align = midori_sdk_spsc_storage_alignment();
        assert!(size > 0);
        assert!(align.is_power_of_two());
        // SpscStorage の中身は ShmHeader (16 byte) + RING_CAPACITY 個の RingSlot
        assert!(size >= 16 + RING_CAPACITY * std::mem::size_of::<RingSlot>());
    }
}
