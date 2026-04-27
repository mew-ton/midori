//! Single-producer single-consumer ring buffer over the shared-memory layout
//! defined in [`midori_core::shm`].
//!
//! # 同期モデル
//!
//! - `write_index` と `read_index` は単調増加する [`AtomicU64`]。スロットの
//!   位置は `index % RING_CAPACITY` で得る。
//! - 生産者は (1) スロットへ書き込み → (2) `write_index` を `Release` で公開
//!   する。消費者は (1) `write_index` を `Acquire` で読み → (2) スロットを
//!   読む。Release/Acquire ペアによりスロットへの書き込みが消費側から正しく
//!   観測できる。
//! - SPSC 規律（生産者 1・消費者 1）は [`SpscStorage::split`] が
//!   `&mut self` を取ることで型レベルに enforce している。両ハンドルが生存
//!   する間は `split` を再呼び出しできない。
//!
//! # ロックフリー性
//!
//! 生産者と消費者は分離したインデックスをそれぞれ排他的に書き込むだけで、
//! 競合する書き込みは発生しない。スロット側は同じインデックスへの同時
//! アクセスをインデックス比較で排除している。

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU64, Ordering};

use midori_core::shm::{RingSlot, ShmHeader, PAYLOAD_INLINE_MAX, RING_CAPACITY};

/// 共有メモリ上に置かれることを意図した SPSC リングバッファのストレージ。
///
/// `#[repr(C)]` により mmap 可能な固定レイアウトを保証し、C FFI 経由で他言語
/// からも同レイアウトでアクセスできる。
#[repr(C)]
pub struct SpscStorage {
    header: ShmHeader,
    slots: [UnsafeCell<RingSlot>; RING_CAPACITY],
}

// SAFETY: SpscStorage は SPSC 規律下で 1 スレッド (生産者) と 1 スレッド (消費者) に
// より共有される。`split(&mut self)` がペアの一意性を保証し、各スレッドは
// 異なるスロットインデックス（write/read）にしかアクセスしないため、
// UnsafeCell 経由の同時アクセスはレースしない。インデックス更新は AtomicU64
// で同期される。
#[allow(unsafe_code)]
unsafe impl Sync for SpscStorage {}

impl SpscStorage {
    /// すべてのスロットがゼロ埋めの空のストレージを生成する。
    #[must_use]
    pub fn new() -> Self {
        Self {
            header: ShmHeader {
                write_index: AtomicU64::new(0),
                read_index: AtomicU64::new(0),
            },
            slots: std::array::from_fn(|_| UnsafeCell::new(EMPTY_SLOT)),
        }
    }

    /// 確保済みの領域に SPSC ストレージを **その場で** 初期化する。
    ///
    /// `Self::new()` と異なり、`SpscStorage` 全体（~75 KB）を呼び出し側スタック上に
    /// 一時生成しない。FFI 経由で C 側が用意した領域へ書き込む経路（小さなスレッド
    /// スタックや組込み環境）でのオーバーフローを避けるために用意する。
    ///
    /// インデックス管理の不変条件（`read..write` 範囲のスロットだけが読まれる）に
    /// より、`slots` 配列は未初期化のままで安全。`header` のみ Atomic を 0 で書く。
    ///
    /// # Safety
    ///
    /// `ptr` は以下を満たすこと:
    /// - 非 NULL
    /// - `size_of::<SpscStorage>()` バイト以上の書き込み可能領域を指す
    /// - `align_of::<SpscStorage>()` のアラインメントを満たす
    /// - 呼び出し時点で生存する Producer/Consumer がない
    #[allow(unsafe_code)]
    pub unsafe fn init_in_place(ptr: *mut Self) {
        // SAFETY: 呼び出し側契約により ptr は SpscStorage 全体を覆う書き込み可能
        // 領域を指し、適切にアラインされている。`header` フィールドへの addr_of_mut!
        // は未初期化のフィールドに対しても合法。ShmHeader は 16 バイトなのでスタック
        // 経由で書いても問題ない。
        let header_ptr = std::ptr::addr_of_mut!((*ptr).header);
        header_ptr.write(ShmHeader {
            write_index: AtomicU64::new(0),
            read_index: AtomicU64::new(0),
        });
        // `slots` は意図的に未初期化のまま残す。pop は read < write の不変条件で
        // 守られているため、未書込みのスロットを読むことはない。
    }

    /// SPSC 規律に従って Producer / Consumer のペアに分割する。
    ///
    /// `&mut self` を取るため、ペアが生存する間は再分割が型レベルで禁止される。
    pub fn split(&mut self) -> (Producer<'_>, Consumer<'_>) {
        let storage: &Self = self;
        (Producer { storage }, Consumer { storage })
    }
}

impl Default for SpscStorage {
    fn default() -> Self {
        Self::new()
    }
}

/// バッファが満杯で push できなかったことを示すエラー。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Full;

impl std::fmt::Display for Full {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SPSC ring buffer is full")
    }
}

impl std::error::Error for Full {}

/// 生産者の push 処理本体。`Producer::push` と FFI からの両方で呼ばれる。
///
/// `slot` は `&RingSlot` を受け取り内部でコピーする。新レイアウト
/// （`PAYLOAD_INLINE_MAX = 240` 込みで ~264 byte）の値渡しはスタックを
/// 無駄に消費するため。
///
/// 呼び出し側は SPSC 規律（任意の時刻に生産者は 1 つだけ）を守ること。
/// Rust API では [`SpscStorage::split`] が型レベルで保証する。FFI からの
/// 呼び出しでは C 側が規律を守る責務を負う。
pub(crate) fn try_push(storage: &SpscStorage, slot: &RingSlot) -> Result<(), Full> {
    let header = &storage.header;
    // 自プロセス内の生産者専用インデックスは Relaxed で十分（書き手は自分だけ）。
    let write = header.write_index.load(Ordering::Relaxed);
    // 消費者の進捗を Acquire で取得し、満杯判定の根拠とする。
    let read = header.read_index.load(Ordering::Acquire);
    if write.wrapping_sub(read) >= RING_CAPACITY as u64 {
        return Err(Full);
    }
    // `as usize` は 32-bit ターゲットで上位ビットを落とすが、
    // `RING_CAPACITY` は 2 のべき乗のため `% RING_CAPACITY` でその差は消える。
    #[allow(clippy::cast_possible_truncation)]
    let idx = (write as usize) % RING_CAPACITY;
    // SAFETY: SPSC 規律により消費者は `slots[read % CAP]` までしか読まず、
    // ここで書く `slots[write % CAP]` は満杯判定により消費者の追跡範囲外。
    // よって同一スロットへの同時アクセスは発生しない。
    #[allow(unsafe_code)]
    unsafe {
        *storage.slots[idx].get() = *slot;
    }
    // スロット書き込みより後に必ず観測されるよう Release で公開する。
    header
        .write_index
        .store(write.wrapping_add(1), Ordering::Release);
    Ok(())
}

/// 消費者の pop 処理本体。`Consumer::pop` と FFI からの両方で呼ばれる。
///
/// 呼び出し側は SPSC 規律（任意の時刻に消費者は 1 つだけ）を守ること。
pub(crate) fn try_pop(storage: &SpscStorage) -> Option<RingSlot> {
    let header = &storage.header;
    // 自プロセス内の消費者専用インデックスは Relaxed で十分。
    let read = header.read_index.load(Ordering::Relaxed);
    // 生産者の進捗を Acquire で取得（対応する Release はスロット書き込みの後）。
    let write = header.write_index.load(Ordering::Acquire);
    if read == write {
        return None;
    }
    // 同上: 2 のべき乗での剰余に守られているのでターゲット間で結果は同じ。
    #[allow(clippy::cast_possible_truncation)]
    let idx = (read as usize) % RING_CAPACITY;
    // SAFETY: SPSC 規律により生産者は `slots[write % CAP]` までしか書かず、
    // ここで読む `slots[read % CAP]` は read < write の不変条件より既に
    // 書き込みが完了している。Acquire ロードによりその書き込みが可視。
    #[allow(unsafe_code)]
    let slot = unsafe { *storage.slots[idx].get() };
    header
        .read_index
        .store(read.wrapping_add(1), Ordering::Release);
    Some(slot)
}

/// 単一の生産者ハンドル。`push` のみを提供する。
pub struct Producer<'a> {
    storage: &'a SpscStorage,
}

impl Producer<'_> {
    /// スロットを末尾に追加する。バッファが満杯なら [`Full`] を返す。
    pub fn push(&mut self, slot: &RingSlot) -> Result<(), Full> {
        try_push(self.storage, slot)
    }
}

/// 単一の消費者ハンドル。`pop` のみを提供する。
pub struct Consumer<'a> {
    storage: &'a SpscStorage,
}

impl Consumer<'_> {
    /// 先頭スロットを取り出す。バッファが空なら `None` を返す。
    pub fn pop(&mut self) -> Option<RingSlot> {
        try_pop(self.storage)
    }
}

const EMPTY_SLOT: RingSlot = RingSlot {
    occupied: 0,
    _pad: [0; 3],
    payload_len: 0,
    side_offset: 0,
    side_len: 0,
    _pad2: [0; 4],
    payload: [0; PAYLOAD_INLINE_MAX],
};

#[cfg(test)]
#[allow(
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::items_after_statements
)]
mod tests {
    use super::*;

    const THREAD_TEST_COUNT: usize = 10_000;

    /// inline payload に「seq の下位バイトを 4 byte little-endian で詰めた」
    /// テスト用 [`RingSlot`] を作る。msgpack そのものではなく、test 用にバイト列の
    /// ラウンドトリップだけを確認するためのプレースホルダ。
    fn slot_with_seq(seq: u32) -> RingSlot {
        let mut s = EMPTY_SLOT;
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

    #[test]
    fn it_should_return_none_when_consumer_pops_empty_buffer() {
        let mut storage = SpscStorage::new();
        let (_p, mut c) = storage.split();
        assert!(c.pop().is_none());
    }

    #[test]
    fn it_should_pop_pushed_slots_in_fifo_order() {
        let mut storage = SpscStorage::new();
        let (mut p, mut c) = storage.split();
        p.push(&slot_with_seq(1)).unwrap();
        p.push(&slot_with_seq(2)).unwrap();
        p.push(&slot_with_seq(3)).unwrap();
        assert_eq!(read_seq(&c.pop().unwrap()), 1);
        assert_eq!(read_seq(&c.pop().unwrap()), 2);
        assert_eq!(read_seq(&c.pop().unwrap()), 3);
        assert!(c.pop().is_none());
    }

    #[test]
    fn it_should_return_full_when_buffer_holds_capacity_items() {
        let mut storage = SpscStorage::new();
        let (mut p, _c) = storage.split();
        for i in 0..RING_CAPACITY {
            p.push(&slot_with_seq(i as u32)).unwrap();
        }
        assert_eq!(p.push(&slot_with_seq(u32::MAX)), Err(Full));
    }

    #[test]
    fn it_should_wrap_around_after_consuming() {
        let mut storage = SpscStorage::new();
        let (mut p, mut c) = storage.split();
        // 3 周分書いて読む。インデックスは 3 * RING_CAPACITY まで進む。
        let total = RING_CAPACITY * 3;
        for i in 0..total {
            p.push(&slot_with_seq(i as u32)).unwrap();
            assert_eq!(read_seq(&c.pop().unwrap()), i as u32);
        }
    }

    #[test]
    fn it_should_allow_refill_after_drain() {
        let mut storage = SpscStorage::new();
        let (mut p, mut c) = storage.split();
        // 一度満杯にし、全部抜き、再度満杯にできる
        for i in 0..RING_CAPACITY {
            p.push(&slot_with_seq(i as u32)).unwrap();
        }
        for i in 0..RING_CAPACITY {
            assert_eq!(read_seq(&c.pop().unwrap()), i as u32);
        }
        for i in 0..RING_CAPACITY {
            p.push(&slot_with_seq((i + 1000) as u32)).unwrap();
        }
        for i in 0..RING_CAPACITY {
            assert_eq!(read_seq(&c.pop().unwrap()), (i + 1000) as u32);
        }
    }

    #[test]
    fn it_should_round_trip_inline_payload_at_max_capacity() {
        // payload_len <= PAYLOAD_INLINE_MAX のラウンドトリップ（バイト列が一致）
        let mut storage = SpscStorage::new();
        let (mut p, mut c) = storage.split();

        let mut slot = EMPTY_SLOT;
        slot.occupied = 1;
        slot.payload_len = PAYLOAD_INLINE_MAX as u32;
        for (i, byte) in slot.payload.iter_mut().enumerate() {
            *byte = (i % 251) as u8;
        }
        p.push(&slot).unwrap();

        let popped = c.pop().expect("slot should be available");
        assert_eq!(popped.occupied, 1);
        assert_eq!(popped.payload_len as usize, PAYLOAD_INLINE_MAX);
        for (i, byte) in popped.payload.iter().enumerate() {
            assert_eq!(*byte, (i % 251) as u8, "byte at {i} mismatched");
        }
        // side channel は使用していない
        assert_eq!(popped.side_offset, 0);
        assert_eq!(popped.side_len, 0);
    }

    #[test]
    fn it_should_carry_side_channel_offsets_when_inline_is_unused() {
        // payload_len > PAYLOAD_INLINE_MAX 相当のケース: payload_len=0 で
        // side_offset/side_len のみが立ち、inline payload 領域は使われない。
        // 本テストは RingSlot フィールドのセマンティクスのみを検証する
        // （side channel 本体の確保は本クレートのスコープ外）。
        let mut storage = SpscStorage::new();
        let (mut p, mut c) = storage.split();

        let mut slot = EMPTY_SLOT;
        slot.occupied = 1;
        slot.payload_len = 0;
        slot.side_offset = 4096;
        slot.side_len = 1024;
        p.push(&slot).unwrap();

        let popped = c.pop().expect("slot should be available");
        assert_eq!(popped.occupied, 1);
        assert_eq!(popped.payload_len, 0);
        assert_eq!(popped.side_offset, 4096);
        assert_eq!(popped.side_len, 1024);
    }

    #[test]
    fn it_should_transfer_data_between_threads() {
        let mut storage = SpscStorage::new();
        let (mut p, mut c) = storage.split();

        std::thread::scope(|s| {
            s.spawn(move || {
                let mut sent: u32 = 0;
                while (sent as usize) < THREAD_TEST_COUNT {
                    if p.push(&slot_with_seq(sent)).is_ok() {
                        sent += 1;
                    } else {
                        std::thread::yield_now();
                    }
                }
            });
            s.spawn(move || {
                let mut expected: u32 = 0;
                while (expected as usize) < THREAD_TEST_COUNT {
                    if let Some(slot) = c.pop() {
                        assert_eq!(read_seq(&slot), expected);
                        expected += 1;
                    } else {
                        std::thread::yield_now();
                    }
                }
            });
        });
    }
}
