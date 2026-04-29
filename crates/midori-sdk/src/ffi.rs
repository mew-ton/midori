//! C ABI 経由で SPSC リングバッファを操作するためのエクスポート群。
//!
//! Python (`PyO3`) や Node.js (`napi-rs`) などの他言語バインディングを書く際の
//! 基盤として、`extern "C"` 関数で SPSC の主要操作を公開する。
//!
//! # メモリ確保ポリシー
//!
//! [`SpscStorage`] の内部レイアウトは不透明バイト列として扱う。C 側はサイズと
//! アラインメントを問い合わせて自前で確保し、[`midori_sdk_spsc_init`] で
//! 初期化する。サイズは `slot_size` に依存するため、handshake で確定した値を
//! 渡す。
//!
//! ```c
//! uint32_t slot_size = 1032; /* default、または handshake 戻り値 */
//! size_t size  = midori_sdk_spsc_storage_size(slot_size);
//! size_t align = midori_sdk_spsc_storage_alignment();
//! void* storage = aligned_alloc(align, size);
//! if (midori_sdk_spsc_init(storage, slot_size) != 1) {
//!     /* slot_size が不正（4 byte 倍数でない / HARD 超過 等） */
//! }
//! ```
//!
//! # SPSC 規律
//!
//! [`midori_sdk_spsc_push`] は同時に 1 スレッドからのみ、
//! [`midori_sdk_spsc_pop`] も同時に 1 スレッドからのみ呼ばれること。
//! 違反時の挙動は未規定。これは Rust 側 API では `&mut Producer` /
//! `&mut Consumer` で型レベルに保証している規律と同じ。
//!
//! # FFI 戻り値
//!
//! `push` / `pop` / `init` の戻り値は `int32_t` を返す。`push` の戻り値
//! `-2` は payload size 超過（events.yaml 違反 / driver 実装バグ相当）を表す。

use std::ffi::c_void;

use midori_core::shm::{self, validate_slot_size};

use crate::spsc::{self, SpscStorage};
use midori_core::shm::ShmHeader;

/// 指定 `slot_size` の [`SpscStorage`] を確保するために必要なバイト数を返す。
///
/// `slot_size` が不正な値の場合は `0` を返す。caller はゼロ判定で確保失敗を
/// 検出する。
#[allow(unsafe_code)] // `#[no_mangle]` は安全な関数でも unsafe_code lint の対象
#[no_mangle]
pub extern "C" fn midori_sdk_spsc_storage_size(slot_size: u32) -> usize {
    if validate_slot_size(slot_size).is_err() {
        return 0;
    }
    shm::shm_total_size(slot_size)
}

/// [`SpscStorage`] を確保するときに必要なアラインメントを返す（`slot_size` に
/// 依存しない固定値）。
#[allow(unsafe_code)]
#[no_mangle]
pub extern "C" fn midori_sdk_spsc_storage_alignment() -> usize {
    std::mem::align_of::<shm::ShmHeader>()
}

/// 確保済みのメモリ領域に空の SPSC ストレージを書き込む。
///
/// 戻り値:
/// - `1`: 成功
/// - `0`: `slot_size` が不正、または `storage` が NULL
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
pub unsafe extern "C" fn midori_sdk_spsc_init(storage: *mut c_void, slot_size: u32) -> i32 {
    if storage.is_null() {
        return 0;
    }
    if validate_slot_size(slot_size).is_err() {
        return 0;
    }
    SpscStorage::init_in_place(storage.cast::<u8>(), slot_size);
    1
}

/// payload バイト列を 1 つ push する。
///
/// 戻り値:
/// - `1`: 成功
/// - `0`: バッファ満杯または引数 NULL
/// - `-2`: payload バイト長が `slot_size - 8` を超える（events.yaml 違反）
///
/// # Safety
///
/// - `storage` は [`midori_sdk_spsc_init`] 済みの領域を指すこと
/// - `payload` / `payload_len` は読み取り可能なバイト列を指すこと（`payload`
///   が NULL かつ `payload_len == 0` のときは empty payload として扱う）
/// - 同時に 1 スレッドからのみ呼ばれること（SPSC 生産者規律）
#[allow(unsafe_code)]
#[no_mangle]
pub unsafe extern "C" fn midori_sdk_spsc_push(
    storage: *const c_void,
    payload: *const u8,
    payload_len: usize,
) -> i32 {
    if storage.is_null() {
        return 0;
    }
    if payload.is_null() && payload_len != 0 {
        return 0;
    }
    let bytes: &[u8] = if payload_len == 0 {
        &[]
    } else {
        std::slice::from_raw_parts(payload, payload_len)
    };
    let header = &*storage.cast::<ShmHeader>();
    let slot_size = header.slot_size;
    spsc::push_raw(storage.cast::<u8>().cast_mut(), slot_size, bytes).as_ffi_code()
}

/// payload バイト列を 1 つ pop する。
///
/// 戻り値:
/// - `1`: 成功（`out_payload` に payload を書き、`*out_payload_len` に長さを書く）
/// - `0`: バッファ空、引数 NULL、または `out_payload_cap < slot_size - 8`
///
/// # Safety
///
/// - `storage` は [`midori_sdk_spsc_init`] 済みの領域を指すこと
/// - `out_payload` は `out_payload_cap` バイト以上の書き込み可能領域を指すこと
/// - `out_payload_len` は書き込み可能な `usize` を指すこと
/// - 同時に 1 スレッドからのみ呼ばれること（SPSC 消費者規律）
#[allow(unsafe_code)]
#[no_mangle]
pub unsafe extern "C" fn midori_sdk_spsc_pop(
    storage: *const c_void,
    out_payload: *mut u8,
    out_payload_cap: usize,
    out_payload_len: *mut usize,
) -> i32 {
    if storage.is_null() || out_payload_len.is_null() {
        return 0;
    }
    if out_payload.is_null() && out_payload_cap != 0 {
        return 0;
    }
    let header = &*storage.cast::<ShmHeader>();
    let slot_size = header.slot_size;
    let Some(payload) = spsc::pop_raw(storage.cast::<u8>(), slot_size) else {
        return 0;
    };
    if payload.len() > out_payload_cap {
        // capacity 不足で書き込めない。caller は `slot_size - 8` 以上の buffer
        // を渡す責務を持つ。本実装では payload は捨てた状態で 0 を返す
        // （SPSC 規律上、再 pop はできない）。これは driver SDK が
        // slot_size を知らずに小さい buffer を渡した契約違反を意味する。
        return 0;
    }
    if !payload.is_empty() {
        std::ptr::copy_nonoverlapping(payload.as_ptr(), out_payload, payload.len());
    }
    *out_payload_len = payload.len();
    1
}

#[cfg(test)]
#[allow(unsafe_code, clippy::cast_possible_truncation)]
mod tests {
    use super::*;
    use midori_core::shm::{DEFAULT_SLOT_SIZE, SLOT_HEADER_SIZE};

    fn alloc_storage(slot_size: u32) -> (*mut u8, std::alloc::Layout) {
        let size = midori_sdk_spsc_storage_size(slot_size);
        let align = midori_sdk_spsc_storage_alignment();
        let layout = std::alloc::Layout::from_size_align(size, align).expect("valid layout");
        let raw = unsafe { std::alloc::alloc(layout) };
        assert!(!raw.is_null());
        let ok = unsafe { midori_sdk_spsc_init(raw.cast::<c_void>(), slot_size) };
        assert_eq!(ok, 1);
        (raw, layout)
    }

    fn dealloc_storage(raw: *mut u8, layout: std::alloc::Layout) {
        unsafe { std::alloc::dealloc(raw, layout) };
    }

    #[test]
    fn it_should_round_trip_a_payload_through_ffi() {
        let (raw, layout) = alloc_storage(DEFAULT_SLOT_SIZE);

        let payload: [u8; 4] = 42u32.to_le_bytes();
        let pushed =
            unsafe { midori_sdk_spsc_push(raw.cast::<c_void>(), payload.as_ptr(), payload.len()) };
        assert_eq!(pushed, 1);

        let mut out_buf = vec![0u8; (DEFAULT_SLOT_SIZE - SLOT_HEADER_SIZE) as usize];
        let mut out_len: usize = 0;
        let popped = unsafe {
            midori_sdk_spsc_pop(
                raw.cast::<c_void>(),
                out_buf.as_mut_ptr(),
                out_buf.len(),
                &raw mut out_len,
            )
        };
        assert_eq!(popped, 1);
        assert_eq!(out_len, 4);
        assert_eq!(&out_buf[..out_len], &payload[..]);

        let popped_empty = unsafe {
            midori_sdk_spsc_pop(
                raw.cast::<c_void>(),
                out_buf.as_mut_ptr(),
                out_buf.len(),
                &raw mut out_len,
            )
        };
        assert_eq!(popped_empty, 0);

        dealloc_storage(raw, layout);
    }

    #[test]
    fn it_should_round_trip_payload_at_slot_max_capacity_through_ffi() {
        let (raw, layout) = alloc_storage(DEFAULT_SLOT_SIZE);

        let max_payload = (DEFAULT_SLOT_SIZE - SLOT_HEADER_SIZE) as usize;
        let payload: Vec<u8> = (0..max_payload).map(|i| (i % 251) as u8).collect();
        let pushed =
            unsafe { midori_sdk_spsc_push(raw.cast::<c_void>(), payload.as_ptr(), payload.len()) };
        assert_eq!(pushed, 1);

        let mut out_buf = vec![0u8; max_payload];
        let mut out_len: usize = 0;
        let popped = unsafe {
            midori_sdk_spsc_pop(
                raw.cast::<c_void>(),
                out_buf.as_mut_ptr(),
                out_buf.len(),
                &raw mut out_len,
            )
        };
        assert_eq!(popped, 1);
        assert_eq!(out_len, max_payload);
        assert_eq!(out_buf, payload);

        dealloc_storage(raw, layout);
    }

    #[test]
    fn it_should_return_payload_too_large_when_pushing_oversize_through_ffi() {
        let (raw, layout) = alloc_storage(DEFAULT_SLOT_SIZE);
        let max_payload = (DEFAULT_SLOT_SIZE - SLOT_HEADER_SIZE) as usize;
        let too_large = vec![0u8; max_payload + 1];
        let pushed = unsafe {
            midori_sdk_spsc_push(raw.cast::<c_void>(), too_large.as_ptr(), too_large.len())
        };
        assert_eq!(pushed, -2, "payload too large は -2 を返す（FFI breaking）");
        dealloc_storage(raw, layout);
    }

    #[test]
    fn it_should_reject_init_with_invalid_slot_size() {
        let bad_slot_size = 13u32; // 4 byte 倍数でない
        let layout = std::alloc::Layout::from_size_align(
            shm::shm_total_size(DEFAULT_SLOT_SIZE),
            midori_sdk_spsc_storage_alignment(),
        )
        .expect("layout");
        let raw = unsafe { std::alloc::alloc(layout) };
        assert!(!raw.is_null());

        let ok = unsafe { midori_sdk_spsc_init(raw.cast::<c_void>(), bad_slot_size) };
        assert_eq!(ok, 0, "alignment 違反は init で reject");
        unsafe { std::alloc::dealloc(raw, layout) };
    }

    #[test]
    fn it_should_return_zero_when_called_with_null_pointers() {
        // storage NULL
        let mut payload: [u8; 4] = [1, 2, 3, 4];
        let mut out_len: usize = 0;
        assert_eq!(
            unsafe {
                midori_sdk_spsc_push(std::ptr::null::<c_void>(), payload.as_ptr(), payload.len())
            },
            0
        );
        assert_eq!(
            unsafe {
                midori_sdk_spsc_pop(
                    std::ptr::null::<c_void>(),
                    payload.as_mut_ptr(),
                    payload.len(),
                    &raw mut out_len,
                )
            },
            0
        );

        // storage 有効、payload ポインタが NULL かつ len > 0 の不正
        let (raw, layout) = alloc_storage(DEFAULT_SLOT_SIZE);
        assert_eq!(
            unsafe { midori_sdk_spsc_push(raw.cast::<c_void>(), std::ptr::null::<u8>(), 4) },
            0
        );
        assert_eq!(
            unsafe {
                midori_sdk_spsc_pop(
                    raw.cast::<c_void>(),
                    std::ptr::null_mut::<u8>(),
                    4,
                    &raw mut out_len,
                )
            },
            0
        );
        dealloc_storage(raw, layout);
    }

    #[test]
    fn it_should_storage_size_be_zero_for_invalid_slot_size() {
        assert_eq!(midori_sdk_spsc_storage_size(13), 0);
        assert_eq!(midori_sdk_spsc_storage_size(0), 0);
    }

    #[test]
    fn it_should_storage_size_match_layout_for_default_slot() {
        let size = midori_sdk_spsc_storage_size(DEFAULT_SLOT_SIZE);
        let align = midori_sdk_spsc_storage_alignment();
        assert!(size > 0);
        assert!(align.is_power_of_two());
        assert_eq!(
            size,
            std::mem::size_of::<shm::ShmHeader>() + shm::RING_CAPACITY * DEFAULT_SLOT_SIZE as usize
        );
    }

    /// `build.rs` が生成した C ヘッダに想定の関数宣言と新型シンボルが
    /// 含まれていることを検証する。
    #[test]
    fn it_should_generate_c_header_with_expected_ffi_symbols() {
        const GENERATED_HEADER: &str = include_str!(concat!(env!("OUT_DIR"), "/midori_sdk.h"));
        assert!(GENERATED_HEADER.contains("MIDORI_SDK_H"));
        // FFI 関数群
        assert!(GENERATED_HEADER.contains("midori_sdk_spsc_storage_size"));
        assert!(GENERATED_HEADER.contains("midori_sdk_spsc_storage_alignment"));
        assert!(GENERATED_HEADER.contains("midori_sdk_spsc_init"));
        assert!(GENERATED_HEADER.contains("midori_sdk_spsc_push"));
        assert!(GENERATED_HEADER.contains("midori_sdk_spsc_pop"));
        // 新レイアウトの構造体型
        assert!(GENERATED_HEADER.contains("ShmHeader"));
        assert!(GENERATED_HEADER.contains("SlotHeader"));
        // 戻り値型の breaking change：旧 u8 ではなく i32 (= int32_t)
        assert!(GENERATED_HEADER.contains("int32_t midori_sdk_spsc_push"));
        assert!(GENERATED_HEADER.contains("int32_t midori_sdk_spsc_pop"));
        // 旧レイアウト象徴シンボルの残骸が無いこと
        assert!(!GENERATED_HEADER.contains("PAYLOAD_INLINE_MAX"));
        assert!(!GENERATED_HEADER.contains("side_offset"));
        assert!(!GENERATED_HEADER.contains("side_len"));
    }
}
