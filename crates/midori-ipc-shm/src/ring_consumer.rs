//! Bridge 側の SPSC リング consumer 実装。
//!
//! driver からの handshake `request_ring(slot_size)` を受けて:
//!
//! 1. [`ring_handshake::resolve_requested_slot_size`] で受領値を validate
//!    （sentinel `0` → `DEFAULT_SLOT_SIZE`、alignment / 上限を検査）
//! 2. `memfd_create(2)` で anonymous shm を確保し
//!    [`ring_handshake::page_aligned_shm_size`] のページ整列サイズへ truncate
//! 3. `mmap(2)` で Bridge プロセス内に書き込み可能でマップ
//! 4. `ShmHeader` の `slot_size` / `version` / 両 index を初期化
//! 5. driver に渡すための [`OwnedFd`] と本構造体 [`RingConsumer`] を返す
//!
//! consumer 側 API は [`RingConsumer::read`] のみ:
//!
//! - 1 slot 分の payload を pop して `Vec<u8>` に複製して返す
//! - リングが空のときは `None`（caller は spin / sleep 戦略を上に被せる）
//!
//! `RingConsumer` を drop すると `MmapMut` が `munmap(2)` を発行する。
//!
//! # Safety
//!
//! `mmap` は本質的に unsafe（kernel が任意のタイミングで内容を書き換え得る）
//! のため、本 module 内で `unsafe { ... }` を 1 箇所だけ使う。`memmap2`
//! crate の `MmapMut::map_mut` は `unsafe fn` であり、その呼び出し点に
//! `# Safety` 注記を付ける。
//!
//! 設計参照: `design/17-driver-comm/01-inline-ring.md`「API スケッチ」
//! 「Bridge 側」「メモリ順序」。

use std::ffi::CString;
use std::fmt;
use std::os::fd::OwnedFd;
use std::sync::atomic::Ordering;

use memmap2::{MmapMut, MmapOptions};
use midori_core::shm::{
    slot_offset_in_shm, ShmHeader, RING_CAPACITY, SHM_LAYOUT_VERSION, SLOT_HEADER_SIZE,
};

use crate::ring_handshake::{page_aligned_shm_size, resolve_requested_slot_size, HandshakeError};

/// [`RingConsumer::create`] が失敗する原因。
#[derive(Debug)]
pub enum CreateError {
    /// driver から受け取った `slot_size` が ABI 制約を満たさない。
    /// 内側の [`HandshakeError`] が具体内容（alignment / 最小値 / 上限）を運ぶ。
    Handshake(HandshakeError),
    /// `memfd_create(2)` / `ftruncate(2)` / `mmap(2)` のいずれかが失敗した。
    /// 内側の `std::io::Error` が OS エラーを運ぶ。
    Os {
        operation: &'static str,
        source: std::io::Error,
    },
}

impl fmt::Display for CreateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Handshake(source) => write!(f, "ring 確保前の handshake で失敗: {source}"),
            Self::Os { operation, source } => {
                write!(f, "shm リング確保中に {operation} が失敗しました: {source}")
            }
        }
    }
}

impl std::error::Error for CreateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Handshake(source) => Some(source),
            Self::Os { source, .. } => Some(source),
        }
    }
}

/// Bridge 側 consumer 1 個分の状態。
///
/// `mmap` 領域と `slot_size` のキャッシュを持つ。`Drop` で `MmapMut` が
/// 自動的に `munmap` する。
///
/// **スレッドモデル**: SPSC の S（single consumer）として扱うこと。本構造体
/// は `Send` だが `Sync` を要求する API は提供しない。consumer ループは
/// 1 スレッド内で `read()` を呼ぶ前提。
pub struct RingConsumer {
    /// shm 全体（ヘッダ + 全スロット）にまたがる mmap region。
    /// `Drop` で `munmap` を発行する。
    mmap: MmapMut,
    /// handshake で確定した slot 全体サイズ（byte）。stride 計算に使う。
    slot_size: u32,
}

impl fmt::Debug for RingConsumer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RingConsumer")
            .field("slot_size", &self.slot_size)
            .field("shm_bytes", &self.mmap.len())
            .finish()
    }
}

impl RingConsumer {
    /// Driver からの handshake `request_ring(requested_slot_size)` を受けて
    /// shm を確保し、`(consumer, fd)` を返す。
    ///
    /// `requested_slot_size = 0` (sentinel) の場合は `DEFAULT_SLOT_SIZE` を
    /// 採用する。返される [`OwnedFd`] は driver subprocess に `SCM_RIGHTS`
    /// で渡す前提で、Bridge 側でも同 fd を mmap している（Bridge と driver
    /// が同じ kernel 領域を共有）。
    ///
    /// # Errors
    ///
    /// - [`CreateError::Handshake`]: alignment / 最小値 / 上限違反
    /// - [`CreateError::Os`]: `memfd_create` / `ftruncate` / `mmap` の失敗
    pub fn create(requested_slot_size: u32) -> Result<(Self, OwnedFd), CreateError> {
        let slot_size =
            resolve_requested_slot_size(requested_slot_size).map_err(CreateError::Handshake)?;
        let shm_bytes = page_aligned_shm_size(slot_size);

        let owned_fd = create_memfd().map_err(|source| CreateError::Os {
            operation: "memfd_create",
            source,
        })?;

        // ftruncate でファイルを目標サイズに拡張する。memfd は初期サイズ 0 の
        // 仮想ファイルなので、mmap する前に必ず必要。
        let truncate_len = i64::try_from(shm_bytes).map_err(|_| CreateError::Os {
            operation: "ftruncate (size_t→i64 cast)",
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "shm size exceeds i64::MAX",
            ),
        })?;
        nix::unistd::ftruncate(&owned_fd, truncate_len).map_err(|errno| CreateError::Os {
            operation: "ftruncate",
            source: std::io::Error::from(errno),
        })?;

        // SAFETY: `owned_fd` は本関数で memfd_create したばかりの fresh な fd。
        // 同一プロセス内で他 mapping は存在しない。len は `shm_bytes`（ftruncate
        // 済みのサイズと一致）。memmap2 の `map_mut` の安全性要件は「他者が
        // mmap 領域を書き換えない、または書き換えに対する整合性を caller が
        // 担保する」だが、本 ring layout 自体が「driver から concurrent に
        // 書かれることを Acquire/Release で同期する」前提で設計されている
        // （詳細: `design/17-driver-comm/01-inline-ring.md`「メモリ順序」）。
        // 残る要件は overflow / null / alignment で、それらは memmap2 内部で
        // 検査される。
        #[allow(unsafe_code)]
        let mmap = unsafe {
            MmapOptions::new()
                .len(shm_bytes)
                .map_mut(&owned_fd)
                .map_err(|source| CreateError::Os {
                    operation: "mmap",
                    source,
                })?
        };

        let mut consumer = Self { mmap, slot_size };
        consumer.init_header();
        Ok((consumer, owned_fd))
    }

    /// このリングが扱う slot 全体サイズ（byte）。テスト / 観測用。
    #[must_use]
    pub fn slot_size(&self) -> u32 {
        self.slot_size
    }

    /// 1 slot 分を pop し、payload バイト列を `Vec<u8>` に複製して返す。
    /// リングが空のときは `None`。
    ///
    /// 戻り値の `Vec<u8>` は新規 allocate する（slot 領域は次の producer 書き込みで
    /// 上書きされ得るため、所有を pop 側に切り離す必要がある）。
    #[must_use]
    pub fn read(&self) -> Option<Vec<u8>> {
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
    fn header(&self) -> &ShmHeader {
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
    pub fn test_push(&mut self, payload: &[u8]) -> bool {
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

/// `memfd_create(2)` の薄いラッパー。close-on-exec を立てる。
fn create_memfd() -> std::io::Result<OwnedFd> {
    use nix::sys::memfd::{memfd_create, MFdFlags};
    let name = CString::new("midori-ring").expect("static C string is non-NUL");
    memfd_create(name.as_c_str(), MFdFlags::MFD_CLOEXEC).map_err(std::io::Error::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use midori_core::shm::{DEFAULT_SLOT_SIZE, HARD_SLOT_SIZE};

    #[test]
    fn it_should_create_default_sized_consumer_for_sentinel_request() {
        let (consumer, fd) = RingConsumer::create(0).expect("default sentinel must succeed");
        assert_eq!(consumer.slot_size(), DEFAULT_SLOT_SIZE);
        // fd が valid な OwnedFd であることは Drop で close されることで担保される。
        drop(fd);
        drop(consumer);
    }

    #[test]
    fn it_should_initialize_shm_header_to_layout_version_one() {
        let (consumer, _fd) = RingConsumer::create(0).expect("default sentinel must succeed");
        let header = consumer.header();
        assert_eq!(header.slot_size, DEFAULT_SLOT_SIZE);
        assert_eq!(header.version, SHM_LAYOUT_VERSION);
        assert_eq!(header.write_index.load(Ordering::Relaxed), 0);
        assert_eq!(header.read_index.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn it_should_reject_request_with_unaligned_slot_size() {
        let err = RingConsumer::create(13).expect_err("alignment 違反");
        assert!(matches!(err, CreateError::Handshake(_)), "got {err:?}");
    }

    #[test]
    fn it_should_reject_request_above_hard_limit() {
        let err = RingConsumer::create(HARD_SLOT_SIZE + 4).expect_err("hard 超過");
        assert!(matches!(err, CreateError::Handshake(_)), "got {err:?}");
    }

    #[test]
    fn it_should_round_trip_a_pushed_payload_through_read() {
        let (mut consumer, _fd) = RingConsumer::create(0).expect("default sentinel must succeed");
        // 空 ring の read は None
        assert!(consumer.read().is_none());

        let payload = b"hello-ring".to_vec();
        assert!(
            consumer.test_push(&payload),
            "push must succeed on empty ring"
        );
        let popped = consumer.read().expect("popped");
        assert_eq!(popped, payload);
        // 連続 read で消える
        assert!(consumer.read().is_none());
    }

    #[test]
    fn it_should_round_trip_with_custom_slot_size() {
        // 4 KiB を超える slot_size を要求できる。
        let custom = 4_104_u32; // page-aligned 例
        let (mut consumer, _fd) = RingConsumer::create(custom).expect("custom slot must succeed");
        assert_eq!(consumer.slot_size(), custom);

        let payload = vec![0xAB_u8; 2000];
        assert!(consumer.test_push(&payload));
        let popped = consumer.read().expect("popped");
        assert_eq!(popped, payload);
    }

    #[test]
    fn it_should_render_create_error_display_with_inner_handshake_reason() {
        // alignment 違反は CreateError::Handshake 経由で内側 SlotSizeError の
        // 説明文を含む。
        let err = RingConsumer::create(13).expect_err("alignment");
        let rendered = err.to_string();
        assert!(rendered.contains("handshake"), "got: {rendered}");
        assert!(
            rendered.contains("4 byte 倍数"),
            "Display は内側理由を含む: {rendered}"
        );
    }
}
