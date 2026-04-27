//! 共有メモリ上の SPSC リングバッファレイアウト定義。
//!
//! Driver から Bridge へ raw event を運ぶための **post-MEW-40** のスロット
//! 形式。raw event は driver の `events.yaml` に沿った key-value 構造を
//! msgpack で encode したバイト列として `payload` に格納する。inline 容量
//! ([`PAYLOAD_INLINE_MAX`]) を超える payload は side channel（mmap プール、
//! 別 Issue MEW-43 で実装）に書き出し、スロットには `side_offset` /
//! `side_len` のみを格納する。
//!
//! 詳細設計: `design/15-sdk-bindings-api.md` 「SPSC スロットレイアウトの変更」。
//!
//! 旧スロット（`device_id` / `specifier` / `value_tag` / `value_i64` /
//! `value_f64` を持つ post-binding 形）は本リリースで撤廃され、
//! `midori-core` は major bump（0.2.0）となる。

/// SPSC リングバッファのスロット数。
///
/// raw event 1 件 = 1 スロット。1 tick 分のドライバー出力を満杯にせず
/// 受け取れるサイズを目安にしている。
pub const RING_CAPACITY: usize = 256;

/// 単一スロットに inline で格納できる msgpack バイト列の最大長。
///
/// この値を超える payload は side channel（別 mmap 領域）に書き出し、
/// `RingSlot::side_offset` / `RingSlot::side_len` のみがスロットに残る。
///
/// MIDI / OSC の通常イベントが余裕で収まる範囲として 240 byte に設定。
/// `SysEx` 1KB 級などはここを超えるため side channel 経由となる。
pub const PAYLOAD_INLINE_MAX: usize = 240;

/// SPSC リングバッファの単一スロット。
///
/// `#[repr(C)]` により mmap 可能な固定レイアウトを保証する。FFI で他言語
/// バインディングからも同レイアウトでアクセスする。
///
/// # フィールド
///
/// - `occupied`: 0 = 空、1 = 占有。空判定は本来 `write_index` /
///   `read_index` の比較で十分だが、C 側の利便性（pop した直後に
///   `slot.occupied == 1` で成功と判定）のために残す
/// - `payload_len`: msgpack バイト列の実長。`<= PAYLOAD_INLINE_MAX` のとき
///   inline 格納、超える場合は 0 を立てて side channel 経由とする
/// - `side_offset` / `side_len`: side channel 上の位置とバイト長。**side
///   channel の使用判定は `side_len > 0` を canonical とする**（`side_offset`
///   は 0 が side channel 先頭を指す有効値となるため、未使用フラグには使えない）
/// - `payload`: msgpack バイト列の inline 領域。`payload_len` バイト分のみ
///   有効で、それ以降の領域は未定義
///
/// # サイズ
///
/// 1 + 3 + 4 + 8 + 4 + 4 + 240 = 264 byte。
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RingSlot {
    /// 0 = 空、1 = 占有。
    pub occupied: u8,
    #[allow(clippy::pub_underscore_fields)]
    pub _pad: [u8; 3],
    /// inline payload に書かれた msgpack バイト列の長さ（`<= PAYLOAD_INLINE_MAX`）。
    /// side channel を使う場合は 0 にし、`side_len` 側を立てる。
    pub payload_len: u32,
    /// side channel 上の payload 開始オフセット（バイト）。`side_len > 0` の
    /// ときのみ有効（`side_offset == 0` でも `side_len > 0` なら side channel
    /// 先頭を指す有効ポインタ）。
    pub side_offset: u64,
    /// side channel 上の payload バイト長。**0 = side channel 未使用**。
    /// side channel 使用判定はこのフィールドを canonical とする。
    pub side_len: u32,
    #[allow(clippy::pub_underscore_fields)]
    pub _pad2: [u8; 4],
    /// inline 格納用の msgpack バイト列。`payload_len` バイト目までが有効。
    pub payload: [u8; PAYLOAD_INLINE_MAX],
}

/// 共有メモリ領域の先頭に置かれるヘッダ。
///
/// レイアウト（`AtomicU64` は `#[repr(C, align(8))]`、両フィールドとも 8 byte）:
///
/// ```text
/// offset 0:  write_index (AtomicU64)
/// offset 8:  read_index  (AtomicU64)
/// offset 16: slots[RING_CAPACITY] (RingSlot 配列)
/// ```
///
/// 両インデックスは単調増加。実際のスロット位置は `index % RING_CAPACITY`。
/// バッファ満杯条件は `write_index - read_index == RING_CAPACITY`。
///
/// 生産者はスロット書き込み後に `write_index` を [`std::sync::atomic::Ordering::Release`] で
/// 公開し、消費者は `write_index` を [`std::sync::atomic::Ordering::Acquire`] で読んだ後に
/// スロットを読む。
#[repr(C)]
pub struct ShmHeader {
    pub write_index: std::sync::atomic::AtomicU64,
    pub read_index: std::sync::atomic::AtomicU64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_capacity_nonzero() {
        const { assert!(RING_CAPACITY > 0) };
    }

    #[test]
    fn payload_inline_max_is_positive() {
        const { assert!(PAYLOAD_INLINE_MAX > 0) };
    }

    #[test]
    fn shm_header_size_and_align() {
        assert_eq!(std::mem::size_of::<ShmHeader>(), 16);
        assert_eq!(std::mem::align_of::<ShmHeader>(), 8);
    }

    #[test]
    fn ring_slot_is_repr_c_and_fixed_size() {
        // ドキュメントの 1 + 3 + 4 + 8 + 4 + 4 + 240 = 264 byte に固定。
        // C ヘッダ（cbindgen 生成）と共有されるレイアウトなので、padding の
        // 追加・並びの変更による ABI ドリフトをコンパイル時にも検出する。
        const { assert!(std::mem::size_of::<RingSlot>() == 264) };
        assert_eq!(std::mem::size_of::<RingSlot>(), 264);
        assert_eq!(
            std::mem::size_of::<RingSlot>() % std::mem::align_of::<RingSlot>(),
            0
        );
    }
}
