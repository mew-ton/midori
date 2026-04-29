//! Bridge 側で driver からの `request_ring(slot_size)` 受領を処理する経路。
//!
//! `design/17-driver-comm/01-inline-ring.md` の handshake プロトコルに従い:
//!
//! 1. driver が events.yaml の inline tier 全イベントから `max_payload_size`
//!    と必要 `slot_size = ((max_payload_size + 8) + 3) & !3` を計算
//! 2. driver → Bridge: `slot_size` 送信。`<= DEFAULT_SLOT_SIZE` のときは
//!    sentinel `0` で「default 要求なし」を示す
//! 3. Bridge: 受信値が `0` なら `DEFAULT_SLOT_SIZE` を採用、それ以外は受信値
//!    をそのまま採用したうえで alignment / 上限を validate
//! 4. Bridge: shm 領域全体を 4 KiB ページに切り上げて mmap
//!
//! 本 module はステップ 3-4 のうち「`slot_size` の解決と検証」「ページ整列
//! 計算」までを担う。**範囲外**: control channel の wire format（serde 等で
//! 通信する `request_ring` メッセージ構造）と、実 driver process spawn 経由
//! での shm fd 確保 / `mmap(2)` 呼び出し。これらは driver lifecycle 管理が
//! 入った段階で別 module から本 module の関数を呼ぶ形で接続する。
//!
//! 公開 API は driver process spawn / control channel 経由で接続される予定で、
//! 現状 main から caller が居ないため module 全体で `dead_code` を抑制する。
//! 単体テストで挙動を担保している。
#![allow(dead_code)]

use std::error::Error;
use std::fmt;

use midori_core::shm::{shm_total_size, validate_slot_size, SlotSizeError, DEFAULT_SLOT_SIZE};

/// メモリマップに用いるページサイズ。design では 4 KiB を仮定している。
///
/// Bridge は `shm_total_size(slot_size)` を本値に切り上げて mmap する。
/// 4 KiB は Linux / macOS / Windows いずれの典型ページサイズの最小公倍数で、
/// driver 側 SDK は本値を仮定して buffer alignment を計算する規約になっている。
pub const PAGE_SIZE: usize = 4096;

// `page_aligned_shm_size` の bit-mask 計算は `PAGE_SIZE` が 2 のべき乗である
// ことを前提とする。値変更時に静かに崩れないよう compile-time で固定する。
const _: () = assert!(PAGE_SIZE.is_power_of_two());

/// driver の `request_ring` メッセージで sentinel として使う値。
///
/// `requested_slot_size == 0` のとき driver は「default で確保してくれ」を
/// 意味する。実 `slot_size` = 0 は `MIN_SLOT_SIZE = 12` 未満で validator が
/// reject するため、sentinel として安全に使える。
pub const REQUEST_DEFAULT_SLOT_SIZE: u32 = 0;

/// driver からの `request_ring(slot_size)` 受領時に発生する失敗。
#[derive(Debug, PartialEq, Eq)]
pub enum HandshakeError {
    /// 受信した `slot_size` が ABI 制約（alignment / 最小値 / 上限）を
    /// 満たさない。具体内容は内包する [`SlotSizeError`] を参照。
    InvalidSlotSize {
        requested_slot_size: u32,
        source: SlotSizeError,
    },
}

impl fmt::Display for HandshakeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSlotSize {
                requested_slot_size,
                source,
            } => write!(
                f,
                "driver からの request_ring が拒否されました（slot_size={requested_slot_size}）: {source}"
            ),
        }
    }
}

impl Error for HandshakeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidSlotSize { source, .. } => Some(source),
        }
    }
}

/// driver から受け取った `requested_slot_size` を Bridge 側で解決する。
///
/// - `requested_slot_size == REQUEST_DEFAULT_SLOT_SIZE` (= 0) のとき:
///   `DEFAULT_SLOT_SIZE` を採用する
/// - それ以外: 受信値をそのまま採用し、alignment / 上限を validate
///
/// # Errors
///
/// alignment 違反 / `MIN_SLOT_SIZE` 未満 / `HARD_SLOT_SIZE` 超過のいずれか
/// で [`HandshakeError::InvalidSlotSize`] を返す。
pub fn resolve_requested_slot_size(requested_slot_size: u32) -> Result<u32, HandshakeError> {
    let resolved = if requested_slot_size == REQUEST_DEFAULT_SLOT_SIZE {
        DEFAULT_SLOT_SIZE
    } else {
        requested_slot_size
    };
    validate_slot_size(resolved).map_err(|source| HandshakeError::InvalidSlotSize {
        requested_slot_size,
        source,
    })?;
    Ok(resolved)
}

/// 解決済み `slot_size` から、4 KiB ページに切り上げた shm 全体サイズ
/// （byte）を返す。Bridge は本値を mmap 引数として使う。
///
/// 計算式: `(shm_total + PAGE_SIZE - 1) & !(PAGE_SIZE - 1)`。bit mask は
/// `PAGE_SIZE` が 2 のべき乗であることを利用した整列イディオムで、
/// `((shm_total + PAGE_SIZE - 1) / PAGE_SIZE) * PAGE_SIZE` と等価。
#[must_use]
pub const fn page_aligned_shm_size(slot_size: u32) -> usize {
    let total = shm_total_size(slot_size);
    (total + PAGE_SIZE - 1) & !(PAGE_SIZE - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use midori_core::shm::{HARD_SLOT_SIZE, RING_CAPACITY};

    #[test]
    fn it_should_resolve_sentinel_to_default_slot_size() {
        let resolved = resolve_requested_slot_size(REQUEST_DEFAULT_SLOT_SIZE).expect("default");
        assert_eq!(resolved, DEFAULT_SLOT_SIZE);
    }

    #[test]
    fn it_should_pass_through_valid_custom_slot_size() {
        let resolved = resolve_requested_slot_size(4_104).expect("4 KiB slot");
        assert_eq!(resolved, 4_104);
    }

    #[test]
    fn it_should_reject_alignment_violation() {
        let err = resolve_requested_slot_size(13).expect_err("alignment 違反");
        assert!(matches!(
            err,
            HandshakeError::InvalidSlotSize {
                source: SlotSizeError::NotAligned { .. },
                ..
            }
        ));
    }

    #[test]
    fn it_should_reject_above_hard_limit() {
        let err = resolve_requested_slot_size(HARD_SLOT_SIZE + 4).expect_err("hard 超過");
        assert!(matches!(
            err,
            HandshakeError::InvalidSlotSize {
                source: SlotSizeError::TooLarge { .. },
                ..
            }
        ));
    }

    #[test]
    fn it_should_round_shm_total_up_to_page_size() {
        // DEFAULT_SLOT_SIZE での実 shm 容量
        let aligned = page_aligned_shm_size(DEFAULT_SLOT_SIZE);
        // shm_total = 56 + 256 * 1032 = 264,248 byte. 4 KiB ページに切り上げ → 65 ページ = 266,240 byte
        assert!(
            aligned.is_multiple_of(PAGE_SIZE),
            "ページ整列していること: {aligned}"
        );
        let raw_total = std::mem::size_of::<midori_core::shm::ShmHeader>()
            + RING_CAPACITY * DEFAULT_SLOT_SIZE as usize;
        assert!(aligned >= raw_total, "raw_total を下回らない");
        assert!(
            aligned < raw_total + PAGE_SIZE,
            "1 ページ超過しない（最小切り上げ）"
        );
    }

    #[test]
    fn it_should_render_handshake_error_display_with_slot_size_and_inner_reason() {
        // 4 byte 倍数違反のケース。Display は外側の handshake 文脈に加えて
        // 内側 SlotSizeError の理由フレーズも含むことを担保する。
        let err = resolve_requested_slot_size(13).expect_err("alignment");
        let rendered = err.to_string();
        assert!(rendered.contains("13"), "Display に値を含む: {rendered}");
        assert!(
            rendered.contains("request_ring"),
            "Display に handshake 文脈を含む: {rendered}"
        );
        assert!(
            rendered.contains("4 byte 倍数"),
            "Display に SlotSizeError の理由フレーズを含む: {rendered}"
        );
    }
}
