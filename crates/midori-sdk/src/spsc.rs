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
//! # スロットサイズ
//!
//! [`SpscStorage::new`] に渡す `slot_size` は driver lifetime 内で固定だが
//! driver ごとに異なる。値は handshake で決まり、4 byte 倍数 / `MIN_SLOT_SIZE`
//! 以上 / `HARD_SLOT_SIZE` 以下である必要がある（`midori_core::shm` の
//! `validate_slot_size` で検証）。
//!
//! # ロックフリー性
//!
//! 生産者と消費者は分離したインデックスをそれぞれ排他的に書き込むだけで、
//! 競合する書き込みは発生しない。スロット側は同じインデックスへの同時
//! アクセスをインデックス比較で排除している。

use std::cell::UnsafeCell;
use std::sync::atomic::Ordering;

use midori_core::shm::{
    self, validate_slot_size, ShmHeader, SlotHeader, SlotSizeError, RING_CAPACITY,
    SHM_LAYOUT_VERSION, SLOT_HEADER_SIZE,
};

/// 共有メモリ上に置かれることを意図した SPSC リングバッファのストレージ。
///
/// 内部レイアウトは:
///
/// ```text
/// offset 0:  ShmHeader (56 byte)
/// offset 56: slot[0] (slot_size byte)
/// offset 56+slot_size: slot[1]
/// ...
/// ```
///
/// `Box<[UnsafeCell<u64>]>` で確保することで、`ShmHeader` の 8 byte alignment
/// を満たしつつ、Producer / Consumer が共有 `&SpscStorage` 経由で interior
/// mutability を行使する根拠を型レベルで明示する。`u64` 単位の配列なので
/// 合計バイト数は `slot_size × RING_CAPACITY + 56` を 8 byte 境界へ切り上げ。
///
/// FFI 経由で C 側が確保した buffer に対しては、本構造体ではなく
/// [`push_raw`] / [`pop_raw`] を直接使う。両者は内部で共通の low-level
/// 関数を呼び出すため、Rust API と FFI で挙動が一致する。
pub struct SpscStorage {
    // 各セルを `UnsafeCell` で包むことで「共有参照経由の内部書き込み」が
    // Rust の aliasing rule に違反しない正規ルートになる。生バイト経由で
    // ヘッダ領域と各スロットを stride 計算で参照する。
    buffer: Box<[UnsafeCell<u64>]>,
    // ヘッダ書き込み / stride 計算のキャッシュ。`ShmHeader.slot_size` と
    // 一致する。
    slot_size: u32,
}

// SAFETY: SpscStorage は SPSC 規律下で 1 スレッド (生産者) と 1 スレッド (消費者) に
// より共有される。`split(&mut self)` がペアの一意性を保証し、各スレッドは
// 異なるスロットインデックス（write/read）にしかアクセスしないため、
// 生バイト経由の同時アクセスはレースしない。インデックス更新は AtomicU64
// で同期される。
#[allow(unsafe_code)]
unsafe impl Sync for SpscStorage {}

impl SpscStorage {
    /// 指定 `slot_size` で空のストレージを生成する。
    ///
    /// # Errors
    ///
    /// `slot_size` が 4 byte 倍数 / `MIN_SLOT_SIZE` 以上 / `HARD_SLOT_SIZE`
    /// 以下のいずれかを満たさない場合は対応する [`SlotSizeError`] を返す。
    pub fn new(slot_size: u32) -> Result<Self, SlotSizeError> {
        validate_slot_size(slot_size)?;
        let total_bytes = shm::shm_total_size(slot_size);
        // u64 単位に切り上げて確保することで 8-byte alignment を保証する。
        let total_u64 = total_bytes.div_ceil(std::mem::size_of::<u64>());
        let buffer: Box<[UnsafeCell<u64>]> = (0..total_u64)
            .map(|_| UnsafeCell::new(0u64))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let storage = Self { buffer, slot_size };
        // ヘッダ初期化（write/read = 0、slot_size、version、_pad は 0 のまま）
        #[allow(unsafe_code)]
        unsafe {
            storage.write_header();
        }
        Ok(storage)
    }

    /// backing buffer 先頭を指す raw pointer。`UnsafeCell::get()` 経由で
    /// 取り出すことで、共有参照下の内部書き込みが Rust aliasing rule を
    /// 守った形になる。`buffer` が空の場合は dangling だが本構造体は
    /// 必ず `ShmHeader` を含む `total_u64 >= 7` を確保しているため空にならない。
    fn buffer_base(&self) -> *mut u8 {
        // SAFETY: buffer は非空（new で必ず ShmHeader 分以上確保）。
        // UnsafeCell::get は安全な API で、得たポインタへの実 access は
        // SPSC 規律下の caller 責任。
        debug_assert!(!self.buffer.is_empty());
        self.buffer[0].get().cast::<u8>()
    }

    /// 確保済みのメモリ領域に SPSC ストレージを **その場で** 初期化する。
    ///
    /// `Self::new()` と異なり、Rust 側で backing store を確保しない。FFI 経由
    /// で C 側が用意した領域へ書き込む経路（小さなスレッドスタックや組込み
    /// 環境）で使う。
    ///
    /// # Safety
    ///
    /// `ptr` は以下を満たすこと:
    /// - 非 NULL
    /// - `shm_total_size(slot_size)` バイト以上の書き込み可能領域を指す
    /// - `align_of::<ShmHeader>() = 8` のアラインメントを満たす
    /// - 呼び出し時点で生存する Producer/Consumer がない
    ///
    /// 加えて呼び出し側は `validate_slot_size(slot_size)` を事前に通過させる
    /// こと。本関数は防衛的検証を行わない。
    #[allow(unsafe_code, clippy::cast_ptr_alignment)]
    pub unsafe fn init_in_place(ptr: *mut u8, slot_size: u32) {
        // backing 領域全体をゼロ埋め。slot 領域は SPSC の read..write 不変条件に
        // より未読セルが読まれない契約だが、初期化未定義値で `occupied=1` のような
        // ゴミが乗ることを避ける防衛のためここで一括 0 埋めしておく。
        let total = shm::shm_total_size(slot_size);
        std::ptr::write_bytes(ptr, 0, total);
        // ShmHeader を書き込み。AtomicU64 はゼロ初期化で 0 を表せるため、
        // 上の write_bytes で write/read = 0 が成立済み。slot_size と version
        // を直接書き込む。
        let header_ptr = ptr.cast::<ShmHeader>();
        let slot_size_ptr = std::ptr::addr_of_mut!((*header_ptr).slot_size);
        let version_ptr = std::ptr::addr_of_mut!((*header_ptr).version);
        slot_size_ptr.write(slot_size);
        version_ptr.write(SHM_LAYOUT_VERSION);
    }

    /// `Self::new` 内で `buffer` 先頭に `ShmHeader` を初期化する。
    ///
    /// # Safety
    ///
    /// `buffer` は `shm_total_size(slot_size)` バイト以上を確保済みで、
    /// 8-byte alignment を満たしていること。`Box<[u64]>` で確保されている
    /// ためこの条件は型レベルで担保されている。
    #[allow(unsafe_code)]
    unsafe fn write_header(&self) {
        // SAFETY: buffer は u64 配列なので 8-byte aligned かつ `shm_total_size`
        // 以上を確保している。UnsafeCell::get 経由で取り出した raw pointer
        // への書き込みは aliasing rule 上正規ルート。`init_in_place` と
        // 同じ手順で in-place 初期化する。
        Self::init_in_place(self.buffer_base(), self.slot_size);
    }

    /// 確定済みの `slot_size`。
    #[must_use]
    pub fn slot_size(&self) -> u32 {
        self.slot_size
    }

    /// SPSC 規律に従って Producer / Consumer のペアに分割する。
    ///
    /// `&mut self` を取るため、ペアが生存する間は再分割が型レベルで禁止される。
    pub fn split(&mut self) -> (Producer<'_>, Consumer<'_>) {
        let storage: &Self = self;
        (Producer { storage }, Consumer { storage })
    }

    /// テスト専用の `ShmHeader` アクセサ。本番経路では `push_raw` / `pop_raw`
    /// が直接 buffer 先頭からアクセスするため不要だが、`SpscStorage::new` が
    /// 初期化したヘッダ値（`slot_size` / `version`）の検証に使う。
    #[cfg(test)]
    fn header(&self) -> &ShmHeader {
        // SAFETY: buffer 先頭は ShmHeader としてアラインかつ初期化済み。
        // UnsafeCell::get で取り出した raw pointer 経由で参照する。
        #[allow(unsafe_code, clippy::cast_ptr_alignment)]
        unsafe {
            &*self.buffer_base().cast::<ShmHeader>()
        }
    }
}

/// `push_raw` の結果。FFI 戻り値（`i32`）は `as_ffi_code` で取り出す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushResult {
    /// payload を slot に書き込めた。
    Ok,
    /// ring が満杯で書き込めない。caller は再試行 / drop を判断する。
    Full,
    /// payload バイト長が `slot_size - SLOT_HEADER_SIZE` を超える。
    PayloadTooLarge,
}

impl PushResult {
    /// FFI 経由で C 側に返す数値コード。
    /// `1` = success、`0` = ring full、`-2` = payload too large。
    #[must_use]
    pub const fn as_ffi_code(self) -> i32 {
        match self {
            Self::Ok => 1,
            Self::Full => 0,
            Self::PayloadTooLarge => -2,
        }
    }
}

/// shm 領域 base ポインタ (`ShmHeader` 先頭) からスロット先頭までのオフセット
/// を返す helper。
fn slot_ptr_at(base: *mut u8, slot_size: u32, slot_index: usize) -> *mut u8 {
    let offset = shm::slot_offset_in_shm(slot_size, slot_index);
    // SAFETY: caller が shm_total_size 範囲内を保証する。
    #[allow(unsafe_code)]
    unsafe {
        base.add(offset)
    }
}

/// shm 領域 base ポインタから `ShmHeader` 共有参照を取り出す helper。
///
/// # Safety
///
/// caller は以下を満たす責任を負う:
///
/// - `base` は初期化済みの shm 領域先頭で、`shm_total_size(slot_size)` バイトの
///   書き込み可能領域を指す
/// - `base` は `ShmHeader` の 8-byte alignment を満たす（`SpscStorage::new` の
///   `Box<[u64]>` および FFI `aligned_alloc` 経由で担保）
/// - **戻り値の `&'a ShmHeader` が backing storage より長生きしないこと**。
///   返却される lifetime `'a` は caller 指定で型レベルでは無拘束のため、
///   実際の生存期間は呼び出し側コンテキストで担保すること
#[allow(unsafe_code, clippy::cast_ptr_alignment)]
unsafe fn shm_header_ref<'a>(base: *const u8) -> &'a ShmHeader {
    &*base.cast::<ShmHeader>()
}

/// shm 領域 base ポインタを起点に payload を push する low-level 関数。
/// Rust 側 [`SpscStorage`] と FFI の双方から呼ばれる共通実装。
///
/// # Safety
///
/// - `base` は初期化済みの shm 領域先頭で、`shm_total_size(slot_size)` バイトを
///   覆う書き込み可能領域を指すこと
/// - `slot_size` は対象 shm の [`ShmHeader::slot_size`] と一致すること
/// - `base` は `ShmHeader` の 8-byte alignment を満たすこと
/// - SPSC 規律: 同時に 1 スレッドからのみ呼ばれること
#[allow(unsafe_code, clippy::cast_ptr_alignment)]
pub(crate) unsafe fn push_raw(base: *mut u8, slot_size: u32, payload: &[u8]) -> PushResult {
    let max_payload = slot_size as usize - SLOT_HEADER_SIZE as usize;
    if payload.len() > max_payload {
        return PushResult::PayloadTooLarge;
    }
    let header = shm_header_ref(base);
    // 自プロセス内の生産者専用インデックスは Relaxed で十分（書き手は自分だけ）。
    let write = header.write_index.load(Ordering::Relaxed);
    // 消費者の進捗を Acquire で取得し、満杯判定の根拠とする。
    let read = header.read_index.load(Ordering::Acquire);
    if write.wrapping_sub(read) >= RING_CAPACITY as u64 {
        return PushResult::Full;
    }
    // `as usize` は 32-bit ターゲットで上位ビットを落とすが、
    // `RING_CAPACITY` は 2 のべき乗のため `% RING_CAPACITY` でその差は消える。
    #[allow(clippy::cast_possible_truncation)]
    let idx = (write as usize) % RING_CAPACITY;
    let slot_ptr = slot_ptr_at(base, slot_size, idx);
    // SAFETY: SPSC 規律により消費者は `slots[read % CAP]` までしか読まず、
    // ここで書く `slots[write % CAP]` は満杯判定により消費者の追跡範囲外。
    // よって同一スロットへの同時アクセスは発生しない。slot_ptr は
    // shm_total_size の範囲内かつ SlotHeader 互換アラインメント (>= 4) を
    // 満たす（slot_size が 4 byte 倍数のため）。
    let payload_dst = slot_ptr.add(SLOT_HEADER_SIZE as usize);
    std::ptr::copy_nonoverlapping(payload.as_ptr(), payload_dst, payload.len());
    let header_ptr = slot_ptr.cast::<SlotHeader>();
    std::ptr::write(
        header_ptr,
        SlotHeader {
            occupied: 1,
            _pad: [0; 3],
            #[allow(clippy::cast_possible_truncation)]
            payload_len: payload.len() as u32,
        },
    );
    // スロット書き込みより後に必ず観測されるよう Release で公開する。
    header
        .write_index
        .store(write.wrapping_add(1), Ordering::Release);
    PushResult::Ok
}

/// `pop_raw_into` の戻り値。FFI hot path で Vec allocate を回避するため
/// `&mut [u8]` 互換の caller buffer を取り、結果だけ enum で返す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopOutcome {
    /// ring が空。`out_buf` には書き込まれない。
    Empty,
    /// payload を `out_buf` に書き込んだ。`usize` は実際に書いた byte 数。
    Copied(usize),
    /// `out_buf` の容量が payload より小さく、書き込めなかった。slot は
    /// SPSC 規律に従い消費済み（再 pop 不可）。`usize` は本来必要だった
    /// payload byte 数で、caller が次回 buffer を見積もる材料にする。
    Truncated(usize),
}

/// shm 領域 base ポインタを起点に payload を caller buffer へ直接コピーする
/// low-level 関数。FFI hot path 用に Vec allocate を回避する。
///
/// # Safety
///
/// - `base` は初期化済みの shm 領域先頭で、`shm_total_size(slot_size)` バイトを
///   覆う読み取り可能領域を指すこと
/// - `slot_size` は対象 shm の [`ShmHeader::slot_size`] と一致すること
/// - `out_buf` は `out_buf_cap` バイト以上の書き込み可能領域を指すこと
///   （`out_buf_cap == 0` のときは書き込みを行わない）
/// - SPSC 規律: 同時に 1 スレッドからのみ呼ばれること
#[allow(unsafe_code, clippy::cast_ptr_alignment)]
pub(crate) unsafe fn pop_raw_into(
    base: *const u8,
    slot_size: u32,
    out_buf: *mut u8,
    out_buf_cap: usize,
) -> PopOutcome {
    let header = shm_header_ref(base);
    // 自プロセス内の消費者専用インデックスは Relaxed で十分。
    let read = header.read_index.load(Ordering::Relaxed);
    // 生産者の進捗を Acquire で取得（対応する Release はスロット書き込みの後）。
    let write = header.write_index.load(Ordering::Acquire);
    if read == write {
        return PopOutcome::Empty;
    }
    #[allow(clippy::cast_possible_truncation)]
    let idx = (read as usize) % RING_CAPACITY;
    let slot_ptr = slot_ptr_at(base.cast_mut(), slot_size, idx);
    // SAFETY: SPSC 規律により生産者は `slots[write % CAP]` までしか書かず、
    // ここで読む `slots[read % CAP]` は read < write の不変条件より既に
    // 書き込みが完了している。Acquire ロードによりその書き込みが可視。
    let slot_header_ptr = slot_ptr.cast::<SlotHeader>();
    let raw_payload_len = (*slot_header_ptr).payload_len as usize;
    // 防衛的 bounds check: 共有メモリ上の SlotHeader.payload_len が
    // shm 汚染や悪意ある producer により slot 容量を超える値になっている
    // ケースで、そのまま信用すると `copy_nonoverlapping` で隣接スロット
    // 領域 / shm 範囲外を読み取ってしまう。最大 payload 容量
    // (slot_size - SLOT_HEADER_SIZE) で clamp して被害を slot 内に閉じる。
    let max_payload = (slot_size as usize).saturating_sub(SLOT_HEADER_SIZE as usize);
    let payload_len = raw_payload_len.min(max_payload);
    // capacity check の結果に関わらず slot は消費する（SPSC 規律上、消費者
    // は自分の read_index を進める権限を持ち、再 pop はできない）。
    if payload_len > out_buf_cap {
        header
            .read_index
            .store(read.wrapping_add(1), Ordering::Release);
        return PopOutcome::Truncated(payload_len);
    }
    let payload_src = slot_ptr.add(SLOT_HEADER_SIZE as usize);
    if payload_len > 0 {
        std::ptr::copy_nonoverlapping(payload_src, out_buf, payload_len);
    }
    header
        .read_index
        .store(read.wrapping_add(1), Ordering::Release);
    PopOutcome::Copied(payload_len)
}

/// shm 領域 base ポインタを起点に payload を `Vec<u8>` として pop する。
/// Rust API の [`Consumer::pop`] が利用する経路。FFI 経由は Vec 経由を
/// 避けるため [`pop_raw_into`] を直接使うこと。
///
/// # Safety
///
/// [`pop_raw_into`] と同じ契約。
#[allow(unsafe_code)]
pub(crate) unsafe fn pop_raw(base: *const u8, slot_size: u32) -> Option<Vec<u8>> {
    // payload 長は最大でも slot_size - SLOT_HEADER_SIZE。最初に最大容量で
    // 確保しておき、Truncated は発生しない経路にする。
    let max_payload = (slot_size as usize).saturating_sub(SLOT_HEADER_SIZE as usize);
    let mut buf = vec![0u8; max_payload];
    match pop_raw_into(base, slot_size, buf.as_mut_ptr(), buf.len()) {
        PopOutcome::Copied(len) => {
            buf.truncate(len);
            Some(buf)
        }
        // SAFETY: out_buf_cap = max_payload を渡しているため pop_raw_into
        // は Truncated を返さない（payload_len は内部で max_payload に
        // clamp 済み）。理論上到達しない Truncated と Empty をまとめて
        // None で表現する。
        PopOutcome::Empty | PopOutcome::Truncated(_) => None,
    }
}

/// 単一の生産者ハンドル。`push` のみを提供する。
pub struct Producer<'a> {
    storage: &'a SpscStorage,
}

impl Producer<'_> {
    /// payload バイト列を末尾スロットに書き込む。
    pub fn push(&mut self, payload: &[u8]) -> PushResult {
        // SAFETY: SpscStorage は新規生成時に slot_size を validate 済みで、
        // buffer は shm_total_size(slot_size) バイト以上を確保している。
        // SPSC 生産者規律は &mut Producer により型レベルで担保される。
        // buffer_base は UnsafeCell::get 経由で aliasing rule を守る。
        #[allow(unsafe_code)]
        unsafe {
            push_raw(self.storage.buffer_base(), self.storage.slot_size, payload)
        }
    }
}

/// 単一の消費者ハンドル。`pop` のみを提供する。
pub struct Consumer<'a> {
    storage: &'a SpscStorage,
}

impl Consumer<'_> {
    /// 先頭スロットの payload バイト列を返す。空なら `None`。
    pub fn pop(&mut self) -> Option<Vec<u8>> {
        // SAFETY: 同上。SPSC 消費者規律は &mut Consumer で担保。
        // buffer_base は UnsafeCell::get 経由で aliasing rule を守る。
        #[allow(unsafe_code)]
        unsafe {
            pop_raw(self.storage.buffer_base(), self.storage.slot_size)
        }
    }
}

#[cfg(test)]
#[allow(clippy::cast_possible_truncation, clippy::items_after_statements)]
mod tests {
    use super::*;
    use midori_core::shm::{DEFAULT_SLOT_SIZE, HARD_SLOT_SIZE};

    const THREAD_TEST_COUNT: usize = 10_000;

    fn payload_with_seq(seq: u32) -> Vec<u8> {
        seq.to_le_bytes().to_vec()
    }

    fn read_seq(payload: &[u8]) -> u32 {
        assert_eq!(payload.len(), 4);
        let mut buf = [0u8; 4];
        buf.copy_from_slice(payload);
        u32::from_le_bytes(buf)
    }

    #[test]
    fn it_should_return_none_when_consumer_pops_empty_buffer() {
        let mut storage = SpscStorage::new(DEFAULT_SLOT_SIZE).expect("default");
        let (_p, mut c) = storage.split();
        assert!(c.pop().is_none());
    }

    #[test]
    fn it_should_pop_pushed_payloads_in_fifo_order() {
        let mut storage = SpscStorage::new(DEFAULT_SLOT_SIZE).expect("default");
        let (mut p, mut c) = storage.split();
        assert_eq!(p.push(&payload_with_seq(1)), PushResult::Ok);
        assert_eq!(p.push(&payload_with_seq(2)), PushResult::Ok);
        assert_eq!(p.push(&payload_with_seq(3)), PushResult::Ok);
        assert_eq!(read_seq(&c.pop().unwrap()), 1);
        assert_eq!(read_seq(&c.pop().unwrap()), 2);
        assert_eq!(read_seq(&c.pop().unwrap()), 3);
        assert!(c.pop().is_none());
    }

    #[test]
    fn it_should_return_full_when_buffer_holds_capacity_items() {
        let mut storage = SpscStorage::new(DEFAULT_SLOT_SIZE).expect("default");
        let (mut p, _c) = storage.split();
        for i in 0..RING_CAPACITY {
            assert_eq!(p.push(&payload_with_seq(i as u32)), PushResult::Ok);
        }
        assert_eq!(p.push(&payload_with_seq(u32::MAX)), PushResult::Full);
    }

    #[test]
    fn it_should_wrap_around_after_consuming() {
        let mut storage = SpscStorage::new(DEFAULT_SLOT_SIZE).expect("default");
        let (mut p, mut c) = storage.split();
        // 3 周分書いて読む。インデックスは 3 * RING_CAPACITY まで進む。
        let total = RING_CAPACITY * 3;
        for i in 0..total {
            assert_eq!(p.push(&payload_with_seq(i as u32)), PushResult::Ok);
            assert_eq!(read_seq(&c.pop().unwrap()), i as u32);
        }
    }

    #[test]
    fn it_should_allow_refill_after_drain() {
        let mut storage = SpscStorage::new(DEFAULT_SLOT_SIZE).expect("default");
        let (mut p, mut c) = storage.split();
        for i in 0..RING_CAPACITY {
            assert_eq!(p.push(&payload_with_seq(i as u32)), PushResult::Ok);
        }
        for i in 0..RING_CAPACITY {
            assert_eq!(read_seq(&c.pop().unwrap()), i as u32);
        }
        for i in 0..RING_CAPACITY {
            assert_eq!(p.push(&payload_with_seq((i + 1000) as u32)), PushResult::Ok);
        }
        for i in 0..RING_CAPACITY {
            assert_eq!(read_seq(&c.pop().unwrap()), (i + 1000) as u32);
        }
    }

    #[test]
    fn it_should_round_trip_payload_at_default_slot_max_capacity() {
        // payload_len = slot_size - 8 のラウンドトリップ
        let mut storage = SpscStorage::new(DEFAULT_SLOT_SIZE).expect("default");
        let (mut p, mut c) = storage.split();

        let max_payload = (DEFAULT_SLOT_SIZE - SLOT_HEADER_SIZE) as usize;
        let payload: Vec<u8> = (0..max_payload).map(|i| (i % 251) as u8).collect();
        assert_eq!(p.push(&payload), PushResult::Ok);

        let popped = c.pop().expect("slot should be available");
        assert_eq!(popped.len(), max_payload);
        for (i, byte) in popped.iter().enumerate() {
            assert_eq!(*byte, (i % 251) as u8, "byte at {i} mismatched");
        }
    }

    #[test]
    fn it_should_reject_payload_exceeding_slot_capacity() {
        let mut storage = SpscStorage::new(DEFAULT_SLOT_SIZE).expect("default");
        let (mut p, _c) = storage.split();
        let too_large = vec![0u8; (DEFAULT_SLOT_SIZE - SLOT_HEADER_SIZE) as usize + 1];
        assert_eq!(p.push(&too_large), PushResult::PayloadTooLarge);
    }

    #[test]
    fn it_should_round_trip_payload_with_custom_slot_size() {
        // 4 KiB slot で driver-requested サイズのケース
        let slot_size = 4_104; // payload 容量 4096 + header 8、4 byte align
        let mut storage = SpscStorage::new(slot_size).expect("custom");
        assert_eq!(storage.slot_size(), slot_size);
        let (mut p, mut c) = storage.split();

        let payload: Vec<u8> = (0..4096u32).map(|i| (i % 251) as u8).collect();
        assert_eq!(p.push(&payload), PushResult::Ok);
        let popped = c.pop().expect("slot should be available");
        assert_eq!(popped.len(), 4096);
        assert_eq!(popped, payload);
    }

    #[test]
    fn it_should_reject_storage_with_invalid_slot_size() {
        // alignment 違反
        assert!(matches!(
            SpscStorage::new(13),
            Err(SlotSizeError::NotAligned { .. })
        ));
        // hard 超過
        assert!(matches!(
            SpscStorage::new(HARD_SLOT_SIZE + 4),
            Err(SlotSizeError::TooLarge { .. })
        ));
        // min 未満
        assert!(matches!(
            SpscStorage::new(8),
            Err(SlotSizeError::TooSmall { .. })
        ));
    }

    #[test]
    fn it_should_record_slot_size_and_layout_version_in_header() {
        let storage = SpscStorage::new(DEFAULT_SLOT_SIZE).expect("default");
        let header = storage.header();
        assert_eq!(header.slot_size, DEFAULT_SLOT_SIZE);
        assert_eq!(header.version, SHM_LAYOUT_VERSION);
    }

    #[test]
    fn it_should_ffi_code_match_design_contract() {
        assert_eq!(PushResult::Ok.as_ffi_code(), 1);
        assert_eq!(PushResult::Full.as_ffi_code(), 0);
        assert_eq!(PushResult::PayloadTooLarge.as_ffi_code(), -2);
    }

    #[test]
    fn it_should_transfer_data_between_threads() {
        let mut storage = SpscStorage::new(DEFAULT_SLOT_SIZE).expect("default");
        let (mut p, mut c) = storage.split();

        std::thread::scope(|s| {
            s.spawn(move || {
                let mut sent: u32 = 0;
                while (sent as usize) < THREAD_TEST_COUNT {
                    if matches!(p.push(&payload_with_seq(sent)), PushResult::Ok) {
                        sent += 1;
                    } else {
                        std::thread::yield_now();
                    }
                }
            });
            s.spawn(move || {
                let mut expected: u32 = 0;
                while (expected as usize) < THREAD_TEST_COUNT {
                    if let Some(payload) = c.pop() {
                        assert_eq!(read_seq(&payload), expected);
                        expected += 1;
                    } else {
                        std::thread::yield_now();
                    }
                }
            });
        });
    }
}
