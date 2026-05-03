//! OS 非依存の SPSC ring consumer 実装。
//!
//! `mmap` で得たページ整列領域 (`MmapMut`) と handshake で確定した
//! `slot_size` だけを材料に、producer が書いた slot を pop / 観測する
//! 経路を提供する。Linux の `memfd_create(2)` 系経路と macOS の
//! `shm_open(2)` 系経路はどちらも本構造体 [`RingConsumerCore`] を内部に
//! 持ち、shm 確保まわりの差異だけが OS 別 module に切り出されている。
//!
//! 本 module 単独では shm の確保 (`memfd_create` / `shm_open`) は行わない。
//! 既存 `MmapMut` を受け取り、`init_header` で初期化してから `read` /
//! `test_push` を提供する。
//!
//! # Safety
//!
//! `header()` は `mmap.as_ptr()` から `&ShmHeader` を組み立てるため
//! `unsafe` を 1 箇所だけ使う。詳細は `header()` の `# Safety` 注記を参照。
//!
//! 設計参照: `design/17-driver-comm/01-inline-ring.md`「API スケッチ」
//! 「Bridge 側」「メモリ順序」。

use std::sync::atomic::Ordering;

use memmap2::MmapMut;
use midori_core::shm::{
    slot_offset_in_shm, ShmHeader, RING_CAPACITY, SHM_LAYOUT_VERSION, SLOT_HEADER_SIZE,
};

/// OS 非依存の consumer 状態。`mmap` 領域と `slot_size` だけを保持する。
///
/// `Drop` で `MmapMut` が自動的に `munmap` する。**スレッドモデル**: SPSC
/// の S（single consumer）として扱うこと。本構造体は `Send` だが `Sync`
/// を要求する API は提供しない。consumer ループは 1 スレッド内で `read()`
/// を呼ぶ前提。
pub(super) struct RingConsumerCore {
    /// shm 全体（ヘッダ + 全スロット）にまたがる mmap region。
    /// `Drop` で `munmap` を発行する。
    mmap: MmapMut,
    /// handshake で確定した slot 全体サイズ（byte）。stride 計算に使う。
    slot_size: u32,
}

impl RingConsumerCore {
    /// 既に shm を確保し `mmap` 済みの領域を受け取り、ヘッダ / 各 slot の
    /// 初期値を書き込んだ上で本構造体を返す。
    pub(super) fn from_mmap(mmap: MmapMut, slot_size: u32) -> Self {
        let mut core = Self { mmap, slot_size };
        core.init_header();
        core
    }

    /// このリングが扱う slot 全体サイズ（byte）。テスト / 観測用。
    pub(super) fn slot_size(&self) -> u32 {
        self.slot_size
    }

    /// mmap 領域の長さ（byte）。`Debug` 実装やテストでの観測用。
    pub(super) fn shm_bytes(&self) -> usize {
        self.mmap.len()
    }

    /// 1 slot 分を pop し、payload バイト列を `Vec<u8>` に複製して返す。
    /// リングが空のときは `None`。
    ///
    /// 戻り値の `Vec<u8>` は新規 allocate する（slot 領域は次の producer 書き込みで
    /// 上書きされ得るため、所有を pop 側に切り離す必要がある）。
    pub(super) fn read(&self) -> Option<Vec<u8>> {
        let header = self.header();
        // Acquire で write_index を読んだ後、対応する slot の payload までを
        // 同じ Acquire 観測の中に取り込む（producer 側の Release 書き込みと対）。
        let write_index = header.write_index.load(Ordering::Acquire);
        let read_index = header.read_index.load(Ordering::Relaxed);
        if write_index == read_index {
            return None;
        }

        // RING_CAPACITY (= 256) で剰余を取るため、上位ビットは破棄して問題
        // ない。usize::MAX は 32-bit プラットフォームでも 2^32-1 ≧ 256 なので
        // truncation 後の値は剰余演算と同じ結果になる。
        #[allow(clippy::cast_possible_truncation)]
        let slot_index = (read_index as usize) % RING_CAPACITY;
        let payload = self.copy_slot_payload(slot_index);

        // 消費完了を producer に通知。Release で「slot 内データを読み終えた」
        // 事実を公開する。
        header
            .read_index
            .store(read_index.wrapping_add(1), Ordering::Release);

        payload
    }

    /// shm 領域先頭の `ShmHeader` への参照を取り出す。`mmap.as_ptr()` から
    /// 直接組み立てる必要があり、`unsafe` を 1 箇所封じ込めるためのヘルパー。
    pub(super) fn header(&self) -> &ShmHeader {
        // SAFETY: `mmap` は本構造体作成時に `shm_total_size` 以上の領域を
        // 確保しており、先頭 `size_of::<ShmHeader>()` byte は `ShmHeader` の
        // memory layout として `init_header()` で書き込み済み。alignment は
        // `MmapMut` がページ境界（4 KiB）に揃え、`ShmHeader` の `align_of`
        // は 8 byte なので満たす（cast_ptr_alignment lint はそれを静的に
        // 検出できないので個別 allow）。lifetime は `&self` に紐付くので、
        // 戻り値の参照中に mmap 領域が unmap されることはない。
        #[allow(unsafe_code, clippy::cast_ptr_alignment)]
        unsafe {
            &*self.mmap.as_ptr().cast::<ShmHeader>()
        }
    }

    /// 指定 slot index の `payload_len` / `occupied` を読み、payload バイト列を
    /// `Vec<u8>` に複製して返す。`payload_len == 0` または `occupied == 0` の
    /// ときは `None`（防衛的: producer 側 race で実質空のスロットを pop した）。
    fn copy_slot_payload(&self, slot_index: usize) -> Option<Vec<u8>> {
        let slot_offset = slot_offset_in_shm(self.slot_size, slot_index);
        let header_bytes = SLOT_HEADER_SIZE as usize;
        let slot_bytes_total = self.slot_size as usize;
        let slot_bytes = &self.mmap[slot_offset..slot_offset + slot_bytes_total];

        // slot offset 0:  occupied: u8
        // slot offset 4:  payload_len: u32 (LE; ABI は host endian だが
        //                  Bridge と driver は同一マシン同一 endian 前提)
        let occupied = slot_bytes[0];
        let payload_len =
            u32::from_ne_bytes([slot_bytes[4], slot_bytes[5], slot_bytes[6], slot_bytes[7]])
                as usize;

        if occupied == 0 || payload_len == 0 {
            return None;
        }
        let payload_cap = slot_bytes_total - header_bytes;
        if payload_len > payload_cap {
            // ABI 違反: producer が `slot_size - 8` を超えて書いた。本来
            // emit_event 側で -2 を返して driver で防がれる経路だが、
            // 防衛的に drop（None で skip）する。
            return None;
        }
        let payload = slot_bytes[header_bytes..header_bytes + payload_len].to_vec();
        Some(payload)
    }

    /// `ShmHeader` の `slot_size` / `version` / 両 index を初期値で書き込む。
    fn init_header(&mut self) {
        // ヘッダ部のバイト列を直書きする。`AtomicU64` は `from_ne_bytes` で
        // load/store するレイアウトと同一なので、初期化として 0u64 を 8 byte
        // 連続で書けばよい。slot_size / version は ne バイトで書く。
        let header_bytes = std::mem::size_of::<ShmHeader>();
        debug_assert!(self.mmap.len() >= header_bytes, "mmap < ShmHeader size");

        // write_index = 0 (offset 0..8), read_index = 0 (offset 8..16)
        self.mmap[0..16].fill(0);
        // slot_size (offset 16..20)
        self.mmap[16..20].copy_from_slice(&self.slot_size.to_ne_bytes());
        // version (offset 20..24)
        self.mmap[20..24].copy_from_slice(&SHM_LAYOUT_VERSION.to_ne_bytes());
        // _pad (offset 24..56)
        self.mmap[24..header_bytes].fill(0);

        // 各 slot の `occupied` / `payload_len` を 0 で初期化する。これで
        // 万が一 `read()` が write 前に走っても確実に空判定される。
        for slot_index in 0..RING_CAPACITY {
            let off = slot_offset_in_shm(self.slot_size, slot_index);
            self.mmap[off..off + (SLOT_HEADER_SIZE as usize)].fill(0);
        }

        // 念のため kernel に書き戻しを促す（mmap region 内の書き込みは
        // 同一プロセス内では即見えるが、別プロセス（driver）が attach する
        // 前のここで一度 sync しておくと debug 時にレースが見える）。
        let _ = self.mmap.flush();
    }

    /// テスト専用: producer 側のように 1 件 push する。in-process
    /// での単体テスト用ヘルパー。
    ///
    /// 本 crate のテスト (`cfg(test)`) と、`feature = "test-helpers"` を
    /// dev-dependencies で有効化した上位 crate のテストからのみ可視。
    /// プロダクション経路では使わない。
    #[cfg(any(test, feature = "test-helpers"))]
    pub(super) fn test_push(&mut self, payload: &[u8]) -> bool {
        let header_bytes_view: ShmHeaderView = ShmHeaderView::from_slice(&self.mmap);
        let write_index = header_bytes_view.write_index;
        let read_index = header_bytes_view.read_index;
        if write_index.wrapping_sub(read_index) >= RING_CAPACITY as u64 {
            return false;
        }
        // テスト経路。剰余を取るため上位ビットの破棄は影響なし。
        #[allow(clippy::cast_possible_truncation)]
        let slot_index = (write_index as usize) % RING_CAPACITY;
        let off = slot_offset_in_shm(self.slot_size, slot_index);
        let header_bytes = SLOT_HEADER_SIZE as usize;
        let cap = self.slot_size as usize - header_bytes;
        if payload.len() > cap {
            return false;
        }

        let payload_len_bytes =
            (u32::try_from(payload.len()).expect("payload < u32::MAX")).to_ne_bytes();
        self.mmap[off] = 1; // occupied
        self.mmap[off + 1..off + 4].fill(0); // _pad
        self.mmap[off + 4..off + 8].copy_from_slice(&payload_len_bytes);
        self.mmap[off + header_bytes..off + header_bytes + payload.len()].copy_from_slice(payload);

        // write_index を Release 相当で更新（ne バイト書き）。
        let new_index = write_index.wrapping_add(1);
        self.mmap[0..8].copy_from_slice(&new_index.to_ne_bytes());
        true
    }
}

#[cfg(any(test, feature = "test-helpers"))]
struct ShmHeaderView {
    write_index: u64,
    read_index: u64,
}

#[cfg(any(test, feature = "test-helpers"))]
impl ShmHeaderView {
    fn from_slice(bytes: &[u8]) -> Self {
        let mut w = [0u8; 8];
        let mut r = [0u8; 8];
        w.copy_from_slice(&bytes[0..8]);
        r.copy_from_slice(&bytes[8..16]);
        Self {
            write_index: u64::from_ne_bytes(w),
            read_index: u64::from_ne_bytes(r),
        }
    }
}
