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
    let storage = storage.cast::<SpscStorage>();
    storage.write(SpscStorage::new());
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
/// - `slot` は有効な [`RingSlot`] を指すこと
/// - 同時に 1 スレッドからのみ呼ばれること（SPSC 生産者規律）
#[allow(unsafe_code)]
#[no_mangle]
pub unsafe extern "C" fn midori_sdk_spsc_push(storage: *const c_void, slot: *const RingSlot) -> u8 {
    if storage.is_null() || slot.is_null() {
        return 0;
    }
    let storage = &*storage.cast::<SpscStorage>();
    let slot = *slot;
    u8::from(spsc::try_push(storage, slot).is_ok())
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
/// - `out_slot` は書き込み可能な [`RingSlot`] を指すこと
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
        out_slot.write(slot);
        1
    } else {
        0
    }
}

#[cfg(test)]
#[allow(unsafe_code)]
mod tests {
    use super::*;
    use midori_core::shm::value_tag;

    fn slot_with_int(n: i64) -> RingSlot {
        let mut s: RingSlot = unsafe { std::mem::zeroed() };
        s.occupied = 1;
        s.value_tag = value_tag::INT;
        s.value_i64 = n;
        s
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

        let pushed = slot_with_int(42);
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
        assert_eq!(popped.value_i64, 42);
        assert_eq!(popped.value_tag, value_tag::INT);

        // 空状態では pop が 0 を返す
        let ok =
            unsafe { midori_sdk_spsc_pop(raw.cast::<c_void>(), std::ptr::from_mut(&mut popped)) };
        assert_eq!(ok, 0);

        // SAFETY: 同じ layout で dealloc
        unsafe {
            std::alloc::dealloc(raw, layout);
        }
    }

    #[test]
    fn it_should_return_zero_when_called_with_null_pointers() {
        let mut slot: RingSlot = unsafe { std::mem::zeroed() };
        let dummy_slot = slot_with_int(1);
        let nul = std::ptr::null::<c_void>();

        assert_eq!(
            unsafe { midori_sdk_spsc_push(nul, std::ptr::from_ref::<RingSlot>(&dummy_slot)) },
            0
        );
        assert_eq!(
            unsafe { midori_sdk_spsc_pop(nul, std::ptr::from_mut(&mut slot)) },
            0
        );
    }

    /// `build.rs` が生成した C ヘッダに想定の関数宣言が含まれていることを検証する。
    /// ヘッダ生成自体（`cargo build` で発火）はビルド時にチェック済み。
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
    }

    #[test]
    fn it_should_report_storage_size_and_alignment_consistent_with_layout() {
        let size = midori_sdk_spsc_storage_size();
        let align = midori_sdk_spsc_storage_alignment();
        assert!(size > 0);
        assert!(align.is_power_of_two());
        // SpscStorage の中身は ShmHeader (16 byte) + 256 個の RingSlot
        assert!(size >= 16 + 256 * std::mem::size_of::<RingSlot>());
    }
}
