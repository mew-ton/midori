//! Inline-tier IPC primitives for the Midori bridge.
//!
//! The Midori bridge consumes events from each driver subprocess via two
//! tiers:
//!
//! - **Inline tier**: Bridge allocates an SPSC ring in OS-backed shared
//!   memory and hands the underlying handle (Linux: `memfd` fd; Windows:
//!   `CreateFileMapping` HANDLE) to the driver via the platform's IPC
//!   handoff mechanism (Linux: `SCM_RIGHTS`; Windows: Named Pipe +
//!   `DuplicateHandle`). The driver writes encoded events into ring slots;
//!   the Bridge pops them on a dedicated poll thread.
//! - **Streamed tier**: planned, not yet implemented.
//!
//! This crate owns the safe API boundary around the inline-tier OS
//! primitives. The `unsafe` operations involved (`mmap` / `MapViewOfFile`,
//! `memfd_create` / `CreateFileMappingW`, SCM_RIGHTS-bearing `recvmsg` /
//! `DuplicateHandle`-bearing pipe transport) are all confined inside this
//! crate; 上位 crate（bridge runtime / driver SDK 等）は本 crate が
//! 再エクスポートする safe surface のみを利用することで、各々の crate は
//! workspace 既定の `unsafe_code = "forbid"` posture を維持できる。本 crate
//! のみ自身の manifest で `unsafe_code = "deny"` に下げて escape hatch を
//! 有効にしている。
//!
//! # Modules and platform gating
//!
//! - [`ring_handshake`]: pure, host-platform-independent validation /
//!   page-alignment math for the `request_ring(slot_size)` handshake.
//! - Linux backend: [`ring_consumer`] + [`fd_socket`]. Depends on
//!   `memfd_create(2)` (Linux/Android-gated in `nix`) and `SCM_RIGHTS` over
//!   UNIX domain sockets.
//! - Windows backend: [`ring_consumer_windows`] + [`handle_pipe_windows`].
//!   Depends on `CreateFileMappingW` / `MapViewOfFile` for shared memory
//!   and Named Pipe + `DuplicateHandle` for HANDLE handoff.
//! - macOS backend: 未実装（macOS 上で本 crate を使うには Linux/Windows
//!   いずれかと同等の backend が必要）。
//!
//! Until the macOS backend arrives, callers that need to compile on macOS
//! must wrap their use of the OS-specific backends with the matching cfg
//! gate themselves. Linux 経路は `RingConsumer` (`ring_consumer` から
//! 再エクスポート) / `send_fd` / `recv_fd` を `cfg(target_os = "linux")`
//! で、Windows 経路は同名 `RingConsumer` (`ring_consumer_windows` から
//! 再エクスポート) / `PipeName` / `create_pipe_server` / `accept_and_send`
//! / `connect_and_recv` を `cfg(target_os = "windows")` で囲む。本 crate
//! は platform gap を stub で埋めない。
//!
//! # Public surface
//!
//! From [`ring_handshake`] (always available):
//!
//! - [`HandshakeError`], [`REQUEST_DEFAULT_SLOT_SIZE`],
//!   [`resolve_requested_slot_size`], [`page_aligned_shm_size`],
//!   [`PAGE_SIZE`]
//!
//! From [`ring_consumer`] (Linux only):
//!
//! - [`RingConsumer`], [`CreateError`]
//!
//! From [`fd_socket`] (Linux only):
//!
//! - [`send_fd`], [`recv_fd`]
//!
//! From [`ring_consumer_windows`] (Windows only):
//!
//! - `RingConsumer` (Windows 専用 type、Linux 版と同名), `CreateError`
//!
//! From [`handle_pipe_windows`] (Windows only):
//!
//! - `PipeName`, `create_pipe_server`, `accept_and_send`,
//!   `connect_and_recv`, `HandlePipeError`

pub mod ring_handshake;

#[cfg(target_os = "linux")]
pub mod fd_socket;
#[cfg(target_os = "linux")]
pub mod ring_consumer;

#[cfg(target_os = "windows")]
pub mod handle_pipe_windows;
#[cfg(target_os = "windows")]
pub mod ring_consumer_windows;

pub use ring_handshake::{
    page_aligned_shm_size, resolve_requested_slot_size, HandshakeError, PAGE_SIZE,
    REQUEST_DEFAULT_SLOT_SIZE,
};

#[cfg(target_os = "linux")]
pub use fd_socket::{recv_fd, send_fd};
#[cfg(target_os = "linux")]
pub use ring_consumer::{CreateError, RingConsumer};

#[cfg(target_os = "windows")]
pub use handle_pipe_windows::{
    accept_and_send, connect_and_recv, create_pipe_server, HandlePipeError, PipeName,
};
#[cfg(target_os = "windows")]
pub use ring_consumer_windows::{CreateError, RingConsumer};
