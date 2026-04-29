//! 共有メモリ上の SPSC リングバッファレイアウト定義。
//!
//! Driver から Bridge へ inline tier の raw event を運ぶスロット形式。
//! raw event は driver の `events.yaml` に沿った key-value 構造を msgpack
//! で encode したバイト列としてスロットの payload 領域に格納する。
//!
//! スロットサイズは driver lifetime 内で固定だが driver ごとに異なる。
//! Bridge と driver は handshake で `slot_size` を確定し、それ以降
//! [`ShmHeader::slot_size`] を起点に stride 計算で raw memory にアクセスする。
//!
//! 詳細設計: `design/17-driver-comm/01-inline-ring.md`。
//!
//! # スロットレイアウト
//!
//! ```text
//! slot offset 0:  occupied: u8
//! slot offset 1:  _pad: [u8; 3]
//! slot offset 4:  payload_len: u32
//! slot offset 8:  payload: [u8; slot_size - 8]
//! ```
//!
//! ヘッダ部（[`SlotHeader`]）は 8 byte で固定。payload は `slot_size - 8`
//! byte 続く。`slot_size` は 4 byte 倍数であること、[`MIN_SLOT_SIZE`] 以上
//! [`HARD_SLOT_SIZE`] 以下であることを Bridge / driver の双方で検証する。
//!
//! # ABI version
//!
//! [`ShmHeader::version`] は単調増加する整数カウンタで、フィールド追加・
//! 意味変更のたびに増分する。Bridge は
//! [`MIN_SUPPORTED_SHM_VERSION`]..=[`MAX_SUPPORTED_SHM_VERSION`] の範囲で
//! 受け入れ、範囲外は reject する。

use std::sync::atomic::AtomicU64;

/// SPSC リングバッファのスロット数。
///
/// raw event 1 件 = 1 スロット。1 tick 分のドライバー出力を満杯にせず
/// 受け取れるサイズを目安にしている。
pub const RING_CAPACITY: usize = 256;

/// driver の要求が無いときに Bridge が確保する slot 全体サイズ（byte）。
///
/// payload 容量は `1032 - 8 = 1024 byte` で MIDI `SysEx` 1 KiB 上限と一致する
/// ため、典型 driver は handshake で要求を出さずにこのサイズで動く。
pub const DEFAULT_SLOT_SIZE: u32 = 1032;

/// driver が handshake で要求できる slot 全体サイズの上限（byte）。
///
/// `slot_size > HARD_SLOT_SIZE` の要求は handshake で reject される。1 driver
/// あたり最大メモリ = `sizeof(ShmHeader) (56) + RING_CAPACITY (256) ×
/// HARD_SLOT_SIZE = 16,777,272 byte ≈ 16 MiB`。
pub const HARD_SLOT_SIZE: u32 = 65_536;

/// `slot_size` の最小値（byte）。
///
/// header 8 byte + payload 最低 4 byte = 12 byte が下限（4 byte alignment 倍数性
/// と payload 最低限の意味を兼ねる）。実用的には [`DEFAULT_SLOT_SIZE`] が
/// 想定されるため、この最小値はあくまで防衛的検証用。
pub const MIN_SLOT_SIZE: u32 = 12;

/// 各スロット先頭に置かれるヘッダのバイト数。
///
/// 8 byte 固定。stride 計算で payload 領域オフセットを求める際に用いる。
pub const SLOT_HEADER_SIZE: u32 = 8;

/// `ShmHeader::version` の初期値。
pub const SHM_LAYOUT_VERSION: u32 = 1;

/// Bridge が受け入れる `ShmHeader::version` の下限。
pub const MIN_SUPPORTED_SHM_VERSION: u32 = 1;

/// Bridge が受け入れる `ShmHeader::version` の上限。
///
/// append-only な ABI 拡張のたびに引き上げる。`MAX_SUPPORTED_SHM_VERSION`
/// より新しい driver は Bridge を更新するまで起動不可。
pub const MAX_SUPPORTED_SHM_VERSION: u32 = 1;

/// 単一スロット先頭に置かれる固定 8 byte のヘッダ。
///
/// payload 部はこのヘッダの直後 `slot_size - 8` byte 続く。本構造体は
/// `#[repr(C)]` で mmap / FFI レイアウトを保証する。
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SlotHeader {
    /// 0 = 空、1 = 占有。空判定は本来 `write_index` / `read_index` の比較で
    /// 十分だが、C 側の利便性（pop した直後に `slot.occupied == 1` で成功と
    /// 判定）のために残す。
    pub occupied: u8,
    #[allow(clippy::pub_underscore_fields)]
    pub _pad: [u8; 3],
    /// payload のバイト長。`<= slot_size - 8`。
    pub payload_len: u32,
}

const _: () = assert!(std::mem::size_of::<SlotHeader>() == SLOT_HEADER_SIZE as usize);
const _: () = assert!(std::mem::align_of::<SlotHeader>() == 4);

/// 共有メモリ領域の先頭に置かれるヘッダ。
///
/// レイアウト:
///
/// ```text
/// offset 0:  write_index (AtomicU64)
/// offset 8:  read_index  (AtomicU64)
/// offset 16: slot_size   (u32)
/// offset 20: version     (u32)
/// offset 24: _pad        ([u8; 32])
/// offset 56: <slots[RING_CAPACITY]; 各 slot_size byte>
/// ```
///
/// 両インデックスは単調増加。実際のスロット位置は `index % RING_CAPACITY`。
/// バッファ満杯条件は `write_index - read_index == RING_CAPACITY`。
///
/// 生産者はスロット書き込み後に `write_index` を [`std::sync::atomic::Ordering::Release`]
/// で公開し、消費者は `write_index` を [`std::sync::atomic::Ordering::Acquire`]
/// で読んだ後にスロットを読む。
///
/// `slot_size` / `version` は handshake 時に Bridge が書き込み、それ以降は
/// 不変。`_pad` は将来の append-only 拡張余地で、cache line に収めるため
/// 32 byte 確保している。
#[repr(C)]
pub struct ShmHeader {
    pub write_index: AtomicU64,
    pub read_index: AtomicU64,
    /// slot 全体のバイト数。handshake で確定し、driver / Bridge とも本値を
    /// 起点に stride 計算する。4 byte 倍数。
    pub slot_size: u32,
    /// ABI version。単調増加カウンタ。
    pub version: u32,
    /// 将来フィールド追加のための予約領域。
    #[allow(clippy::pub_underscore_fields)]
    pub _pad: [u8; 32],
}

const _: () = assert!(std::mem::size_of::<ShmHeader>() == 56);
const _: () = assert!(std::mem::align_of::<ShmHeader>() == 8);

// design/17-driver-comm/01-inline-ring.md の暫定値を ABI 定数として lock
// する。値変更は major bump 案件のため、compile-time でドリフトを検出する。
const _: () = assert!(DEFAULT_SLOT_SIZE == 1032);
const _: () = assert!(HARD_SLOT_SIZE == 65_536);
const _: () = assert!(SLOT_HEADER_SIZE == 8);

/// `slot_size` 検証で検出される失敗種別。
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SlotSizeError {
    /// `slot_size` が 4 byte 倍数でない。
    NotAligned { slot_size: u32 },
    /// `slot_size` が [`MIN_SLOT_SIZE`] 未満。
    TooSmall { slot_size: u32 },
    /// `slot_size` が [`HARD_SLOT_SIZE`] を超える。
    TooLarge { slot_size: u32 },
}

impl std::fmt::Display for SlotSizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAligned { slot_size } => write!(
                f,
                "slot_size {slot_size} は 4 byte 倍数ではありません（ABI 違反）"
            ),
            Self::TooSmall { slot_size } => write!(
                f,
                "slot_size {slot_size} は最小値 {MIN_SLOT_SIZE} を下回っています"
            ),
            Self::TooLarge { slot_size } => write!(
                f,
                "slot_size {slot_size} は HARD_SLOT_SIZE {HARD_SLOT_SIZE} を超えています"
            ),
        }
    }
}

impl std::error::Error for SlotSizeError {}

/// `slot_size` が ABI 制約を満たすかを検証する。
///
/// driver 側の handshake 計算結果と、Bridge 側で受領した値の双方でこの関数
/// を通すことで、4 byte alignment / 最小値 / 上限の 3 条件を一括検証できる。
///
/// # Errors
///
/// 4 byte 倍数性 / 最小値 / 上限のいずれかを満たさない場合に
/// 対応する [`SlotSizeError`] を返す。
pub fn validate_slot_size(slot_size: u32) -> Result<(), SlotSizeError> {
    if !slot_size.is_multiple_of(4) {
        return Err(SlotSizeError::NotAligned { slot_size });
    }
    if slot_size < MIN_SLOT_SIZE {
        return Err(SlotSizeError::TooSmall { slot_size });
    }
    if slot_size > HARD_SLOT_SIZE {
        return Err(SlotSizeError::TooLarge { slot_size });
    }
    Ok(())
}

/// driver 側で `max_payload_size` から要求 `slot_size` を導出する規約。
///
/// `((max_payload_size + SLOT_HEADER_SIZE) + 3) & !3` の式に従い、4 byte
/// alignment へ切り上げる。algebraic に等価な
/// `((max_payload_size + SLOT_HEADER_SIZE + 3) / 4) * 4` だが、bit mask の
/// ほうが driver 側 SDK の典型実装と一致する。
///
/// 戻り値が [`HARD_SLOT_SIZE`] を超えるかどうかの判定は本関数では行わない
/// （caller が [`validate_slot_size`] と組み合わせて使う）。
#[must_use]
pub const fn align_slot_size(max_payload_size: u32) -> u32 {
    let raw = max_payload_size.saturating_add(SLOT_HEADER_SIZE);
    raw.saturating_add(3) & !3u32
}

/// 1 driver ぶんの shm 領域の総バイト数を返す（ページ整列前）。
///
/// `sizeof(ShmHeader) + RING_CAPACITY × slot_size`。Bridge は本値を 4 KiB
/// ページに切り上げて mmap する。
///
/// # Panics
///
/// `slot_size` が `validate_slot_size` を通っていない値だと `usize` への
/// 拡張中にオーバーフローする可能性があるため、本関数を呼ぶ前に必ず
/// `validate_slot_size` を通すこと。実際には `HARD_SLOT_SIZE` を上限に
/// した値ならオーバーフローしない。
#[must_use]
pub const fn shm_total_size(slot_size: u32) -> usize {
    std::mem::size_of::<ShmHeader>() + RING_CAPACITY * slot_size as usize
}

/// 0-origin の slot 番号（`index % RING_CAPACITY`）から、shm 領域先頭
/// （`ShmHeader` 先頭 = base ポインタ）を起点としたスロット先頭までの
/// バイトオフセットを返す。
///
/// 戻り値には `sizeof(ShmHeader)` (= 56 byte) が常に含まれる。caller は
/// `base_ptr.add(slot_offset_in_shm(slot_size, idx))` で raw byte アクセスを
/// 開始する。
#[must_use]
pub const fn slot_offset_in_shm(slot_size: u32, slot_index: usize) -> usize {
    std::mem::size_of::<ShmHeader>() + slot_index * slot_size as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_should_have_nonzero_ring_capacity() {
        const { assert!(RING_CAPACITY > 0) };
    }

    #[test]
    fn it_should_lock_shm_header_size_to_56_bytes() {
        assert_eq!(std::mem::size_of::<ShmHeader>(), 56);
        assert_eq!(std::mem::align_of::<ShmHeader>(), 8);
    }

    #[test]
    fn it_should_lock_slot_header_size_to_8_bytes() {
        assert_eq!(std::mem::size_of::<SlotHeader>(), SLOT_HEADER_SIZE as usize);
    }

    #[test]
    fn it_should_lock_default_and_hard_slot_size_to_design_spec_values() {
        // design/17-driver-comm/01-inline-ring.md の暫定値と整合するか。
        // compile-time guard はモジュール末尾の `const _: () = assert!(...)`
        // で固定し、本テストは runtime 値も runtime で確認する dual-check。
        assert_eq!(DEFAULT_SLOT_SIZE, 1032);
        assert_eq!(HARD_SLOT_SIZE, 65_536);
    }

    #[test]
    fn it_should_render_slot_size_error_display_with_explanatory_text() {
        // Display に「offending slot_size 値」と「違反種別の意味」が両方
        // 含まれることを担保する（API consumer が log 経路で意味を読み取れる）。
        let err = validate_slot_size(13).expect_err("alignment");
        let rendered = err.to_string();
        assert!(rendered.contains("13"), "got: {rendered}");
        assert!(
            rendered.contains("4 byte 倍数"),
            "alignment 違反の説明を含む: {rendered}"
        );

        let err_small = validate_slot_size(8).expect_err("min");
        assert!(err_small.to_string().contains("最小値"));

        let err_large = validate_slot_size(HARD_SLOT_SIZE + 4).expect_err("hard");
        assert!(err_large.to_string().contains("HARD_SLOT_SIZE"));
    }

    #[test]
    fn it_should_validate_default_slot_size() {
        validate_slot_size(DEFAULT_SLOT_SIZE).expect("default は valid");
    }

    #[test]
    fn it_should_reject_slot_size_not_multiple_of_4() {
        let err = validate_slot_size(13).expect_err("alignment 違反");
        assert!(matches!(err, SlotSizeError::NotAligned { .. }));
    }

    #[test]
    fn it_should_reject_slot_size_below_min() {
        let err = validate_slot_size(8).expect_err("min 未満");
        assert!(matches!(err, SlotSizeError::TooSmall { .. }));
    }

    #[test]
    fn it_should_reject_slot_size_above_hard_limit() {
        let err = validate_slot_size(HARD_SLOT_SIZE + 4).expect_err("hard 超過");
        assert!(matches!(err, SlotSizeError::TooLarge { .. }));
    }

    #[test]
    fn it_should_align_slot_size_to_next_multiple_of_4() {
        // payload 1024 → header 8 加算 → 1032（既に 4 倍数）
        assert_eq!(align_slot_size(1024), 1032);
        // payload 1023 → 1031 → 切り上げ 1032
        assert_eq!(align_slot_size(1023), 1032);
        // payload 1 → 9 → 切り上げ 12
        assert_eq!(align_slot_size(1), 12);
        // payload 0 → 8 → 8（既に 4 倍数。但し validate_slot_size は MIN_SLOT_SIZE=12 で reject）
        assert_eq!(align_slot_size(0), 8);
    }

    #[test]
    fn it_should_compute_slot_offset_relative_to_shm_base() {
        let slot_size = DEFAULT_SLOT_SIZE;
        let header_size = std::mem::size_of::<ShmHeader>();
        assert_eq!(slot_offset_in_shm(slot_size, 0), header_size);
        assert_eq!(
            slot_offset_in_shm(slot_size, 1),
            header_size + slot_size as usize
        );
        assert_eq!(
            slot_offset_in_shm(slot_size, RING_CAPACITY - 1),
            header_size + (RING_CAPACITY - 1) * slot_size as usize
        );
    }

    #[test]
    fn it_should_compute_shm_total_size_for_default_slot() {
        let total = shm_total_size(DEFAULT_SLOT_SIZE);
        assert_eq!(
            total,
            std::mem::size_of::<ShmHeader>() + RING_CAPACITY * DEFAULT_SLOT_SIZE as usize
        );
    }
}
