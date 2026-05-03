//! Inline-tier IPC primitives for the Midori bridge.
//!
//! The Midori bridge consumes events from each driver subprocess via two
//! tiers:
//!
//! - **Inline tier**: Bridge allocates an SPSC ring in shared memory
//!   (`memfd_create(2)` on Linux, `shm_open(2)` + `shm_unlink(2)` on macOS)
//!   and hands the fd to the driver via `SCM_RIGHTS`. The driver writes
//!   encoded events into ring slots; the Bridge pops them on a dedicated
//!   poll thread.
//! - **Streamed tier**: planned, not yet implemented.
//!
//! This crate owns the safe API boundary around the inline-tier OS
//! primitives. The `unsafe` operations involved (`mmap`, and `SCM_RIGHTS`-
//! bearing `recvmsg`) are all confined inside this crate;
//! 上位 crate（bridge runtime / driver SDK 等）は本 crate が再エクスポート
//! する safe surface のみを利用することで、各々の crate は workspace 既定の
//! `unsafe_code = "forbid"` posture を維持できる。本 crate のみ自身の
//! manifest で `unsafe_code = "deny"` に下げて escape hatch を有効にしている。
//!
//! # Modules and platform gating
//!
//! - [`ring_handshake`]: pure, host-platform-independent validation /
//!   page-alignment math for the `request_ring(slot_size)` handshake.
//! - `ring_consumer` and `fd_socket`: OS-backed primitives. Built on Linux
//!   (`memfd_create(2)` + `SCM_RIGHTS`) and macOS (`shm_open(2)` +
//!   `shm_unlink(2)` + `SCM_RIGHTS`). Windows (`CreateFileMapping` ベース)
//!   backend は未実装。
//!
//! Public re-exports (`RingConsumer`, `CreateError`, `send_fd`, `recv_fd`)
//! は当面 Linux のみで提供する。macOS でも内部 module はビルド / テスト
//! されるが、Bridge runtime 層の cfg ゲート整理が完了するまで公開 API
//! としての解禁は保留する。callers that need to compile on those targets
//! must wrap their use of [`RingConsumer`], [`send_fd`], and [`recv_fd`]
//! in `#[cfg(target_os = "linux")]` themselves — this crate does not
//! paper over the platform gap with stubs.
//!
//! # Public surface
//!
//! From [`ring_handshake`] (always available):
//!
//! - [`HandshakeError`], [`REQUEST_DEFAULT_SLOT_SIZE`],
//!   [`resolve_requested_slot_size`], [`page_aligned_shm_size`],
//!   [`PAGE_SIZE`]
//!
//! From `ring_consumer` (Linux only, until macOS public surface is unlocked):
//!
//! - [`RingConsumer`], [`CreateError`]
//!
//! From `fd_socket` (Linux only, until macOS public surface is unlocked):
//!
//! - [`send_fd`], [`recv_fd`]

pub mod ring_handshake;

// 内部 module は Linux / macOS の両方でビルドする。これにより両 OS で
// 単体テストが走り、CI macos-latest job が macOS 固有経路 (shm_open /
// shm_unlink / 名前生成) を実機検証できる。Windows backend が入るまで
// `cfg(any(...))` の右辺を拡張しない。
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod fd_socket;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod ring_consumer;

pub use ring_handshake::{
    page_aligned_shm_size, resolve_requested_slot_size, HandshakeError, PAGE_SIZE,
    REQUEST_DEFAULT_SLOT_SIZE,
};

// 公開 re-export は当面 Linux のみ。macOS でも内部 module はビルド /
// テストされているが、Bridge runtime 層の cfg ゲート整理が完了するまで
// 外部公開は保留する（その整理が完了したら下記 cfg を `any(linux, macos)`
// に広げて symbol を解禁する）。
#[cfg(target_os = "linux")]
pub use fd_socket::{recv_fd, send_fd};
#[cfg(target_os = "linux")]
pub use ring_consumer::{CreateError, RingConsumer};
