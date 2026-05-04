//! Inline-tier IPC primitives for the Midori bridge.
//!
//! The Midori bridge consumes events from each driver subprocess via two
//! tiers:
//!
//! - **Inline tier**: Bridge allocates an SPSC ring in OS-backed shared
//!   memory and hands the underlying fd / HANDLE to the driver. 各 OS の
//!   実装は:
//!   - Linux: `memfd_create(2)` で anonymous shm を確保し、`SCM_RIGHTS`
//!     over Unix domain socket で fd を渡す
//!   - macOS: `shm_open(2)` + `shm_unlink(2)` で名前付き shm を確保 (open
//!     直後に unlink して名前空間を綺麗にする) し、`SCM_RIGHTS` over Unix
//!     domain socket で fd を渡す
//!   - Windows: `CreateFileMappingW` + `MapViewOfFile` で page-file backed
//!     shm を確保し、Named Pipe + `DuplicateHandle` で HANDLE を渡す
//!
//!   driver は受け取った fd / HANDLE で同じ shm をマップし、ring slot に
//!   encoded events を書く。Bridge は dedicated poll thread で pop する。
//! - **Streamed tier**: planned, not yet implemented.
//!
//! This crate owns the safe API boundary around the inline-tier OS
//! primitives. The `unsafe` operations involved (`mmap` / `MapViewOfFile`,
//! `memfd_create` / `shm_open` / `CreateFileMappingW`, SCM_RIGHTS-bearing
//! `recvmsg` / `DuplicateHandle`-bearing pipe transport) are all confined
//! inside this crate; 上位 crate（bridge runtime / driver SDK 等）は本 crate
//! が再エクスポートする safe surface のみを利用することで、各々の crate は
//! workspace 既定の `unsafe_code = "forbid"` posture を維持できる。本 crate
//! のみ自身の manifest で `unsafe_code = "deny"` に下げて escape hatch を
//! 有効にしている。
//!
//! # Modules and platform gating
//!
//! - [`ring_handshake`]: pure, host-platform-independent validation /
//!   page-alignment math for the `request_ring(slot_size)` handshake.
//! - `ring_consumer` + `fd_socket`: Linux / macOS backend。`memfd_create`
//!   または `shm_open` ベースの shm 確保と `SCM_RIGHTS` ベースの fd handoff
//!   を実装する。両 OS で内部 module はビルドされ、CI で単体テストが走る。
//! - [`ring_consumer_windows`] + [`handle_pipe_windows`]: Windows backend。
//!   `CreateFileMappingW` ベースの shm 確保と Named Pipe + `DuplicateHandle`
//!   ベースの HANDLE handoff を実装する。
//!
//! 公開 re-export は OS 別の cfg ゲートで露出する。Linux / macOS 上では
//! `RingConsumer` / `CreateError` (`ring_consumer` から) と `send_fd` /
//! `recv_fd` (`fd_socket` から) を、Windows 上では `RingConsumer` /
//! `CreateError` (`ring_consumer_windows` から) と `PipeName` /
//! `create_pipe_server` / `accept_and_send` / `connect_and_recv` /
//! `HandlePipeError` (`handle_pipe_windows` から) を export する。
//!
//! Linux と macOS は同名 type (`RingConsumer` / `CreateError` / `send_fd` /
//! `recv_fd`) を共有するため、`pub use` 文を 1 本にまとめて
//! `cfg(any(target_os = "linux", target_os = "macos"))` で gating する。
//! Windows backend は型名は同じだが内部 namespace が独立しているので
//! `pub use` 文も別建て (`cfg(target_os = "windows")`) で持つ。
//!
//! callers that need to compile across OSes must wrap their use of
//! OS-specific symbols in the matching `#[cfg(target_os = "...")]`
//! themselves — this crate does not paper over the platform gap with
//! stubs.
//!
//! # Public surface
//!
//! From [`ring_handshake`] (always available):
//!
//! - [`HandshakeError`], [`REQUEST_DEFAULT_SLOT_SIZE`],
//!   [`resolve_requested_slot_size`], [`page_aligned_shm_size`],
//!   [`PAGE_SIZE`]
//!
//! From `ring_consumer` (Linux / macOS):
//!
//! - [`RingConsumer`], [`CreateError`]
//!
//! From `fd_socket` (Linux / macOS):
//!
//! - [`send_fd`], [`recv_fd`]
//!
//! From [`ring_consumer_windows`] (Windows only):
//!
//! - `RingConsumer`, `CreateError`
//!
//! From [`handle_pipe_windows`] (Windows only):
//!
//! - `PipeName`, `create_pipe_server`, `accept_and_send`,
//!   `connect_and_recv`, `HandlePipeError`

pub mod ring_handshake;

// 内部 module は Linux / macOS の両方でビルドする。これにより両 OS で
// 単体テストが走り、CI macos-latest job が macOS 固有経路 (shm_open /
// shm_unlink / 名前生成) を実機検証できる。
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod fd_socket;
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod ring_consumer;

#[cfg(target_os = "windows")]
pub mod handle_pipe_windows;
#[cfg(target_os = "windows")]
pub mod ring_consumer_windows;

pub use ring_handshake::{
    page_aligned_shm_size, resolve_requested_slot_size, HandshakeError, PAGE_SIZE,
    REQUEST_DEFAULT_SLOT_SIZE,
};

// 公開 re-export は Linux と macOS で同一 (`fd_socket` / `ring_consumer`
// は両 OS で同名 type を露出する単一実装)。Windows backend は別 module
// (`handle_pipe_windows` / `ring_consumer_windows`) に分かれているので
// `pub use` 文を別建てで持つ。
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use fd_socket::{recv_fd, send_fd};
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use ring_consumer::{CreateError, RingConsumer};

#[cfg(target_os = "windows")]
pub use handle_pipe_windows::{
    accept_and_send, connect_and_recv, create_pipe_server, HandlePipeError, PipeName,
};
#[cfg(target_os = "windows")]
pub use ring_consumer_windows::{CreateError, RingConsumer};
