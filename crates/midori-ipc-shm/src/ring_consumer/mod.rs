//! Bridge 側の SPSC リング consumer 公開 API。
//!
//! driver からの handshake `request_ring(slot_size)` を受けて:
//!
//! 1. [`crate::ring_handshake::resolve_requested_slot_size`] で受領値を validate
//!    （sentinel `0` → `DEFAULT_SLOT_SIZE`、alignment / 上限を検査）
//! 2. OS 別 backend で anonymous shm を確保
//!    - Linux: `memfd_create(2)` ベース ([`linux`])
//!    - macOS: `shm_open(2)` + `shm_unlink(2)` ベース ([`macos`])
//! 3. [`crate::ring_handshake::page_aligned_shm_size`] のページ整列サイズへ truncate
//! 4. `mmap(2)` で Bridge プロセス内に書き込み可能でマップ
//! 5. `ShmHeader` の `slot_size` / `version` / 両 index を初期化
//! 6. driver に渡すための [`OwnedFd`] と本構造体 [`RingConsumer`] を返す
//!
//! consumer 側 API は [`RingConsumer::read`] のみ:
//!
//! - 1 slot 分の payload を pop して `Vec<u8>` に複製して返す
//! - リングが空のときは `None`（caller は spin / sleep 戦略を上に被せる）
//!
//! `RingConsumer` を drop すると `MmapMut` が `munmap(2)` を発行する。
//!
//! # Module 構造
//!
//! - [`common`]: OS 非依存の `RingConsumerCore` (`MmapMut` + `slot_size`
//!   + `read` / `init_header` / `test_push`)
//! - [`shared`]: `mmap` 呼び出し (`unsafe` 集中点)
//! - [`linux`] / [`macos`]: OS 別 shm 確保経路。両者とも
//!   `create_shm_for_ring(shm_bytes) -> (MmapMut, OwnedFd)` の同一
//!   シグネチャを公開し、本 module の [`RingConsumer::create`] が
//!   cfg dispatch する。
//!
//! Windows backend (`CreateFileMapping` ベース) は未実装（将来対応予定）。
//!
//! # Safety
//!
//! `mmap` は本質的に unsafe（kernel が任意のタイミングで内容を書き換え得る）
//! のため、本サブツリー内で `unsafe { ... }` を 2 箇所だけ使う:
//!
//! - [`shared::map_shared_fd`] の `MmapOptions::map_mut` 呼び出し
//! - [`common::RingConsumerCore::header`] の `&*ptr.cast::<ShmHeader>()`
//!
//! それぞれ `# Safety` 注記を付与してある。
//!
//! 設計参照: ring layout / memory ordering は `midori_core::shm` の
//! `ShmHeader` doc 参照。

use std::fmt;
use std::os::fd::OwnedFd;

use crate::ring_handshake::{page_aligned_shm_size, resolve_requested_slot_size, HandshakeError};

mod common;
mod shared;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

use common::RingConsumerCore;

/// [`RingConsumer::create`] が失敗する原因。
#[derive(Debug)]
pub enum CreateError {
    /// driver から受け取った `slot_size` が ABI 制約を満たさない。
    /// 内側の [`HandshakeError`] が具体内容（alignment / 最小値 / 上限）を運ぶ。
    Handshake(HandshakeError),
    /// shm 確保 / `ftruncate` / `mmap` のいずれかが失敗した。
    /// `operation` は失敗した syscall 名 (`"memfd_create"` / `"shm_open"` /
    /// `"ftruncate"` / `"mmap"` / `"shm_unlink"` 等)、`source` は OS error。
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
    core: RingConsumerCore,
}

impl fmt::Debug for RingConsumer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RingConsumer")
            .field("slot_size", &self.core.slot_size())
            .field("shm_bytes", &self.core.shm_bytes())
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
    /// - [`CreateError::Os`]: shm 確保 / `ftruncate` / `mmap` の失敗
    pub fn create(requested_slot_size: u32) -> Result<(Self, OwnedFd), CreateError> {
        let slot_size =
            resolve_requested_slot_size(requested_slot_size).map_err(CreateError::Handshake)?;
        let shm_bytes = page_aligned_shm_size(slot_size);

        let (mmap, owned_fd) = create_shm_for_ring(shm_bytes)?;
        let core = RingConsumerCore::from_mmap(mmap, slot_size);
        Ok((Self { core }, owned_fd))
    }

    /// このリングが扱う slot 全体サイズ（byte）。テスト / 観測用。
    #[must_use]
    pub fn slot_size(&self) -> u32 {
        self.core.slot_size()
    }

    /// 1 slot 分を pop し、payload バイト列を `Vec<u8>` に複製して返す。
    /// リングが空のときは `None`。
    #[must_use]
    pub fn read(&self) -> Option<Vec<u8>> {
        self.core.read()
    }

    /// テスト専用: producer 側のように 1 件 push する。in-process
    /// での単体テスト用ヘルパー。
    ///
    /// 本 crate のテスト (`cfg(test)`) と、`feature = "test-helpers"` を
    /// dev-dependencies で有効化した上位 crate のテストからのみ可視。
    /// プロダクション経路では使わない。
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn test_push(&mut self, payload: &[u8]) -> bool {
        self.core.test_push(payload)
    }
}

/// OS 別 shm 確保経路への dispatch。Linux と macOS で同一シグネチャ。
#[cfg(target_os = "linux")]
fn create_shm_for_ring(shm_bytes: usize) -> Result<(memmap2::MmapMut, OwnedFd), CreateError> {
    linux::create_shm_for_ring(shm_bytes)
}

#[cfg(target_os = "macos")]
fn create_shm_for_ring(shm_bytes: usize) -> Result<(memmap2::MmapMut, OwnedFd), CreateError> {
    macos::create_shm_for_ring(shm_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use midori_core::shm::{DEFAULT_SLOT_SIZE, HARD_SLOT_SIZE, SHM_LAYOUT_VERSION};
    use std::sync::atomic::Ordering;

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
        let header = consumer.core.header();
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
        // 4 KiB を超える slot_size を要求できる。`validate_slot_size` の
        // alignment 要件は 4 byte 倍数なので、4_104 (= 4096 + 8) のように
        // page boundary ではない値も受け付けられる。`shm_bytes` 全体は
        // `page_aligned_shm_size` が 4 KiB に切り上げて mmap する。
        let custom = 4_104_u32; // 4-byte aligned (page boundary は不要)
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

    #[test]
    fn it_should_create_independent_consumers_back_to_back() {
        // 同一プロセス内で複数の consumer を続けて確保できることを確認する。
        // macOS 経路では shm 名が衝突しないこと、および shm_unlink で名前
        // 空間が綺麗になっていることを担保する回帰テスト。Linux 経路は
        // memfd で名前空間を持たないが、共通 API として同じテストが pass する。
        let (a, _fd_a) = RingConsumer::create(0).expect("first consumer");
        let (b, _fd_b) = RingConsumer::create(0).expect("second consumer");
        assert_eq!(a.slot_size(), b.slot_size());
    }
}
