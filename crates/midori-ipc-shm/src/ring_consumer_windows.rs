//! Windows 版 SPSC リング consumer 実装。
//!
//! Linux 版 (`ring_consumer.rs`) が `memfd_create(2)` + `mmap(2)` を使うのに
//! 対し、Windows では:
//!
//! 1. `CreateFileMappingW(INVALID_HANDLE_VALUE, ...)` で **page-file backed の
//!    名前なし shared memory section** を作る（fd / inode に相当する HANDLE
//!    を取得）
//! 2. `MapViewOfFile` で Bridge プロセスの仮想アドレス空間にマップ
//! 3. `ShmHeader` の `slot_size` / `version` / 両 index を初期化
//! 4. driver subprocess に渡すための [`OwnedHandle`] と本構造体
//!    [`RingConsumer`] を返す
//!
//! 名前なし section にしているのは、Windows でも別プロセスから section を
//! 見つける手段として **HANDLE 受け渡し** を採るため（global namespace の
//! `Global\` 名前空間 section だと衝突 / 権限の問題があり、また driver
//! subprocess 起動順とのレースを避けたい）。HANDLE は
//! [`crate::handle_pipe_windows`] が Named Pipe 経由で `DuplicateHandle` 値として
//! 送信する。
//!
//! consumer 側 API は [`RingConsumer::read`] のみで、Linux 版と同じセマン
//! ティクス（1 slot 分の payload を pop して `Vec<u8>` に複製、空なら
//! `None`）。`Drop` で `UnmapViewOfFile` + `CloseHandle` を発行する。
//!
//! # Safety
//!
//! 本 module は `mmap` 相当の Win32 API を 4 つ（`CreateFileMappingW` /
//! `MapViewOfFile` / `UnmapViewOfFile` / `CloseHandle`）を呼ぶ。各呼び出し点
//! に `# SAFETY` 注記を付け、crate-local の `unsafe_code = "deny"` posture
//! を守る。

use std::ffi::c_void;
use std::fmt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
use std::ptr;
use std::sync::atomic::Ordering;

use windows_sys::Win32::Foundation::{GetLastError, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Memory::{
    CreateFileMappingW, MapViewOfFile, UnmapViewOfFile, FILE_MAP_ALL_ACCESS,
    MEMORY_MAPPED_VIEW_ADDRESS, PAGE_READWRITE,
};

use midori_core::shm::{
    slot_offset_in_shm, ShmHeader, RING_CAPACITY, SHM_LAYOUT_VERSION, SLOT_HEADER_SIZE,
};

use crate::ring_handshake::{page_aligned_shm_size, resolve_requested_slot_size, HandshakeError};

/// [`RingConsumer::create`] が失敗する原因。
///
/// Linux 版の `CreateError` と同じ shape に揃えてあり、Os variant のみ
/// 内側の operation 名が Win32 API 名（`CreateFileMappingW` 等）になる。
#[derive(Debug)]
pub enum CreateError {
    /// driver から受け取った `slot_size` が ABI 制約を満たさない。
    /// 内側の [`HandshakeError`] が具体内容（alignment / 最小値 / 上限）を運ぶ。
    Handshake(HandshakeError),
    /// `CreateFileMappingW` / `MapViewOfFile` のいずれかが失敗した。
    /// `operation` は失敗した Win32 API 名、`source` は `GetLastError` を
    /// 元に組み立てた [`std::io::Error`]。
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

/// マップ済み shm view の所有を 1 ヶ所に閉じ込める RAII ハンドル。
///
/// `MapViewOfFile` で得た base ポインタを保持し、`Drop` で
/// `UnmapViewOfFile` を 1 度だけ呼ぶ。非 `Send` / 非 `Sync` を保証する
/// ため `*mut c_void` を field に持つ（生ポインタは Send/Sync を実装しない）。
struct MappedView {
    /// `MapViewOfFile` 戻り値。`Drop` 時に `UnmapViewOfFile(base)` する。
    base: *mut c_void,
    /// マップ長（byte）。安全アクセスのために slice を切り出す際に使う。
    len: usize,
}

impl MappedView {
    /// マップ済み領域を `&mut [u8]` として借用する。
    fn as_mut_slice(&mut self) -> &mut [u8] {
        // SAFETY: `self.base` は `MapViewOfFile` が返した有効な仮想アドレスで、
        // 長さ `self.len` byte が `MapViewOfFile` の dwNumberOfBytesToMap で
        // 要求した byte 数（page-aligned `shm_bytes`）と一致する。
        // `MappedView` は非 `Send` / 非 `Sync` であり、本 method は `&mut self`
        // 経由でしか呼べないため、戻り slice の lifetime 中は他者が同じ
        // 領域を読み書きできない（Bridge プロセス内では）。driver プロセスは
        // 別仮想アドレス空間で同じ shm section を mmap しており、kernel が
        // 物理 frame を共有させているが、それは memmap2 / Linux 版と同じ
        // 「concurrent producer / consumer を Acquire/Release で同期」モデル
        // に従うため、Rust の aliasing 規律違反にはしない。
        #[allow(unsafe_code)]
        unsafe {
            std::slice::from_raw_parts_mut(self.base.cast::<u8>(), self.len)
        }
    }

    /// マップ済み領域を `&[u8]` として借用する。
    fn as_slice(&self) -> &[u8] {
        // SAFETY: `as_mut_slice` と同じ理由。`&self` 経由なので戻り slice
        // も immutable で済む。
        #[allow(unsafe_code)]
        unsafe {
            std::slice::from_raw_parts(self.base.cast::<u8>(), self.len)
        }
    }
}

impl Drop for MappedView {
    fn drop(&mut self) {
        // SAFETY: `self.base` は `MapViewOfFile` が返した有効な base で、
        // 本構造体が唯一の所有者（cloning しない、Send/Sync しない）。
        // `UnmapViewOfFile` は同じ base をちょうど 1 度 unmap する契約。
        #[allow(unsafe_code)]
        unsafe {
            let _ = UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS { Value: self.base });
        }
    }
}

/// Bridge 側 consumer 1 個分の状態（Windows 版）。
///
/// shm view と `slot_size` のキャッシュを持つ。`Drop` で `MappedView` が
/// 自動的に `UnmapViewOfFile` を発行する（section HANDLE は別途
/// `OwnedHandle` で driver 側に渡されており、Bridge 側は driver 起動後は
/// 保持し続けるか close するかを caller が選ぶ）。
///
/// **スレッドモデル**: SPSC の S（single consumer）として扱うこと。
/// `MappedView` は非 `Send` / 非 `Sync` であり、結果として本構造体も
/// `!Send` / `!Sync`。consumer ループは 1 スレッド内で `read()` を
/// 呼ぶ前提。
pub struct RingConsumer {
    /// マップ済み shm 領域（ヘッダ + 全スロット）。`Drop` で
    /// `UnmapViewOfFile` を発行する。
    view: MappedView,
    /// handshake で確定した slot 全体サイズ（byte）。stride 計算に使う。
    slot_size: u32,
}

impl fmt::Debug for RingConsumer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RingConsumer")
            .field("slot_size", &self.slot_size)
            .field("shm_bytes", &self.view.len)
            .finish()
    }
}

impl RingConsumer {
    /// Driver からの handshake `request_ring(requested_slot_size)` を受けて
    /// shm を確保し、`(consumer, section_handle)` を返す。
    ///
    /// `requested_slot_size = 0` (sentinel) の場合は `DEFAULT_SLOT_SIZE` を
    /// 採用する。返される [`OwnedHandle`] は **page-file backed の名前なし
    /// section HANDLE**。Bridge は本 HANDLE を [`crate::handle_pipe_windows`] 経由で
    /// driver subprocess に `DuplicateHandle` で複製して渡す前提。
    ///
    /// # Errors
    ///
    /// - [`CreateError::Handshake`]: alignment / 最小値 / 上限違反
    /// - [`CreateError::Os`]: `CreateFileMappingW` / `MapViewOfFile` の失敗
    pub fn create(requested_slot_size: u32) -> Result<(Self, OwnedHandle), CreateError> {
        let slot_size =
            resolve_requested_slot_size(requested_slot_size).map_err(CreateError::Handshake)?;
        let shm_bytes = page_aligned_shm_size(slot_size);

        let section_handle = create_file_mapping(shm_bytes)?;
        let view = map_view_of_file(section_handle.as_raw_handle(), shm_bytes)?;

        let mut consumer = Self { view, slot_size };
        consumer.init_header();
        Ok((consumer, section_handle))
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

    /// shm 領域先頭の `ShmHeader` への参照を取り出す。`view.base` から
    /// 直接組み立てる必要があり、`unsafe` を 1 箇所封じ込めるためのヘルパー。
    fn header(&self) -> &ShmHeader {
        let bytes = self.view.as_slice();
        // SAFETY: `view` は本構造体作成時に `shm_bytes` 以上の領域を確保
        // しており、先頭 `size_of::<ShmHeader>()` byte は `ShmHeader` の
        // memory layout として `init_header()` で書き込み済み。alignment は
        // `MapViewOfFile` がアロケーション粒度（典型 64 KiB）に揃え、
        // `ShmHeader` の `align_of` は 8 byte なので満たす（`cast_ptr_alignment`
        // lint はそれを静的に検出できないので個別 allow）。lifetime は
        // `&self` に紐付くので、戻り値の参照中に view が unmap されることは
        // ない。
        #[allow(unsafe_code, clippy::cast_ptr_alignment)]
        unsafe {
            &*bytes.as_ptr().cast::<ShmHeader>()
        }
    }

    /// 指定 slot index の `payload_len` / `occupied` を読み、payload バイト列を
    /// `Vec<u8>` に複製して返す。`payload_len == 0` または `occupied == 0` の
    /// ときは `None`（防衛的: producer 側 race で実質空のスロットを pop した）。
    fn copy_slot_payload(&self, slot_index: usize) -> Option<Vec<u8>> {
        let slot_offset = slot_offset_in_shm(self.slot_size, slot_index);
        let header_bytes = SLOT_HEADER_SIZE as usize;
        let slot_bytes_total = self.slot_size as usize;
        let bytes = self.view.as_slice();
        let slot_bytes = &bytes[slot_offset..slot_offset + slot_bytes_total];

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
        let slot_size = self.slot_size;
        let bytes = self.view.as_mut_slice();
        let header_bytes = std::mem::size_of::<ShmHeader>();
        debug_assert!(bytes.len() >= header_bytes, "view < ShmHeader size");

        // write_index = 0 (offset 0..8), read_index = 0 (offset 8..16)
        bytes[0..16].fill(0);
        // slot_size (offset 16..20)
        bytes[16..20].copy_from_slice(&slot_size.to_ne_bytes());
        // version (offset 20..24)
        bytes[20..24].copy_from_slice(&SHM_LAYOUT_VERSION.to_ne_bytes());
        // _pad (offset 24..56)
        bytes[24..header_bytes].fill(0);

        // 各 slot の `occupied` / `payload_len` を 0 で初期化する。これで
        // 万が一 `read()` が write 前に走っても確実に空判定される。
        for slot_index in 0..RING_CAPACITY {
            let off = slot_offset_in_shm(slot_size, slot_index);
            bytes[off..off + (SLOT_HEADER_SIZE as usize)].fill(0);
        }
    }

    /// テスト専用: producer 側のように 1 件 push する。in-process
    /// での単体テスト用ヘルパー。
    ///
    /// 本 crate のテスト (`cfg(test)`) と、`feature = "test-helpers"` を
    /// dev-dependencies で有効化した上位 crate のテストからのみ可視。
    /// プロダクション経路では使わない。
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn test_push(&mut self, payload: &[u8]) -> bool {
        let slot_size = self.slot_size;
        let bytes = self.view.as_mut_slice();
        let header_bytes_view = ShmHeaderView::from_slice(bytes);
        let write_index = header_bytes_view.write_index;
        let read_index = header_bytes_view.read_index;
        if write_index.wrapping_sub(read_index) >= RING_CAPACITY as u64 {
            return false;
        }
        // テスト経路。剰余を取るため上位ビットの破棄は影響なし。
        #[allow(clippy::cast_possible_truncation)]
        let slot_index = (write_index as usize) % RING_CAPACITY;
        let off = slot_offset_in_shm(slot_size, slot_index);
        let header_bytes = SLOT_HEADER_SIZE as usize;
        let cap = slot_size as usize - header_bytes;
        if payload.len() > cap {
            return false;
        }

        let payload_len_bytes =
            (u32::try_from(payload.len()).expect("payload < u32::MAX")).to_ne_bytes();
        bytes[off] = 1; // occupied
        bytes[off + 1..off + 4].fill(0); // _pad
        bytes[off + 4..off + 8].copy_from_slice(&payload_len_bytes);
        bytes[off + header_bytes..off + header_bytes + payload.len()].copy_from_slice(payload);

        // write_index を更新する。test-only 経路（in-process round-trip 検証）
        // で他スレッド observer が居ないため、Release を使った atomic store
        // ではなく plain byte write で十分。
        let new_index = write_index.wrapping_add(1);
        bytes[0..8].copy_from_slice(&new_index.to_ne_bytes());
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

/// `CreateFileMappingW(INVALID_HANDLE_VALUE, ...)` の薄いラッパー。
///
/// page-file backed の **名前なし** section を `shm_bytes` byte 確保し、
/// HANDLE を [`OwnedHandle`] で包んで返す。失敗時は `GetLastError` を
/// 元に [`CreateError::Os`] を返す。
fn create_file_mapping(shm_bytes: usize) -> Result<OwnedHandle, CreateError> {
    // dwMaximumSizeHigh / dwMaximumSizeLow に分割するため u64 にしてから
    // 上位 / 下位 32 bit を取る。`shm_bytes` は `HARD_SLOT_SIZE *
    // RING_CAPACITY + ヘッダ + ページ整列` の上限内（高々 ~16 MiB）なので
    // 32 bit に収まるが、API シグネチャに合わせて分割表現する。
    let size_u64 = shm_bytes as u64;
    let size_high = (size_u64 >> 32) as u32;
    // size_low は下位 32 bit を取り出す純粋なビット演算。`as u32` は意図的に
    // 上位を捨てる挙動なので clippy::cast_possible_truncation を許可。
    #[allow(clippy::cast_possible_truncation)]
    let size_low = (size_u64 & 0xFFFF_FFFF) as u32;

    // SAFETY: `INVALID_HANDLE_VALUE` を hFile に渡すことで page-file backed
    // section を要求する（公式契約）。lpFileMappingAttributes = NULL は
    // default security descriptor（呼び出しスレッドの primary token から
    // 派生）。flProtect = PAGE_READWRITE で read/write 可能。lpName = NULL
    // で名前なし section（global namespace に出さない）。戻り値が NULL なら
    // 失敗、その場合は GetLastError でエラーコードを取り出す。
    #[allow(unsafe_code)]
    let raw = unsafe {
        CreateFileMappingW(
            INVALID_HANDLE_VALUE,
            ptr::null_mut(),
            PAGE_READWRITE,
            size_high,
            size_low,
            ptr::null(),
        )
    };

    if raw.is_null() {
        return Err(CreateError::Os {
            operation: "CreateFileMappingW",
            source: last_os_error(),
        });
    }

    // SAFETY: `raw` は `CreateFileMappingW` が返した有効な HANDLE で、
    // 本関数内で他者に複製していない。`OwnedHandle` で包むことで Drop
    // 時に `CloseHandle` が 1 度だけ呼ばれることを保証する。`raw` は
    // `*mut c_void` 型で `RawHandle` に互換。
    #[allow(unsafe_code)]
    let owned = unsafe { OwnedHandle::from_raw_handle(raw as RawHandle) };
    Ok(owned)
}

/// `MapViewOfFile` の薄いラッパー。
///
/// `section_handle` の section 全体を `shm_bytes` byte 分マップし、
/// `MappedView` を返す。失敗時は `GetLastError` を元に [`CreateError::Os`]
/// を返す。
fn map_view_of_file(
    section_handle: RawHandle,
    shm_bytes: usize,
) -> Result<MappedView, CreateError> {
    // SAFETY: `section_handle` は `CreateFileMappingW` が返した有効な
    // section HANDLE。FILE_MAP_ALL_ACCESS で read/write/copy 全て可能。
    // dwFileOffsetHigh = 0 / dwFileOffsetLow = 0 で section 先頭から、
    // dwNumberOfBytesToMap = shm_bytes byte をマップ要求。戻りの
    // MEMORY_MAPPED_VIEW_ADDRESS::Value が NULL なら失敗、GetLastError で
    // エラーコードを取り出す。
    #[allow(unsafe_code)]
    let view = unsafe {
        MapViewOfFile(
            section_handle as HANDLE,
            FILE_MAP_ALL_ACCESS,
            0,
            0,
            shm_bytes,
        )
    };

    if view.Value.is_null() {
        return Err(CreateError::Os {
            operation: "MapViewOfFile",
            source: last_os_error(),
        });
    }

    Ok(MappedView {
        base: view.Value,
        len: shm_bytes,
    })
}

/// 直近の Win32 エラー（`GetLastError`）を [`std::io::Error`] に変換する。
fn last_os_error() -> std::io::Error {
    // SAFETY: `GetLastError` は副作用なく直近スレッド local の last-error 値
    // を返すだけで、引数も持たない。スレッド local なので race も無い。
    #[allow(unsafe_code)]
    let code = unsafe { GetLastError() };
    std::io::Error::from_raw_os_error(code as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use midori_core::shm::{DEFAULT_SLOT_SIZE, HARD_SLOT_SIZE};

    #[test]
    fn it_should_create_default_sized_consumer_for_sentinel_request() {
        let (consumer, handle) = RingConsumer::create(0).expect("default sentinel must succeed");
        assert_eq!(consumer.slot_size(), DEFAULT_SLOT_SIZE);
        // handle が valid な OwnedHandle であることは Drop で CloseHandle
        // されることで担保される。
        drop(handle);
        drop(consumer);
    }

    #[test]
    fn it_should_initialize_shm_header_to_layout_version_one() {
        let (consumer, _handle) = RingConsumer::create(0).expect("default sentinel must succeed");
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
        let (mut consumer, _handle) =
            RingConsumer::create(0).expect("default sentinel must succeed");
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
        // resolve_requested_slot_size が要求するのは 4 byte 倍数性のみで、
        // 4_104 はその条件を満たす（ページ整列とは別）。
        let custom = 4_104_u32;
        let (mut consumer, _handle) =
            RingConsumer::create(custom).expect("custom slot must succeed");
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
