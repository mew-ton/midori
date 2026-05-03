//! Inline-tier IPC primitives for the Midori bridge.
//!
//! The Midori bridge consumes events from each driver subprocess via two
//! tiers (see `design/17-driver-comm/`):
//!
//! - **Inline tier**: Bridge allocates an SPSC ring in `memfd`-backed
//!   shared memory and hands the fd to the driver via `SCM_RIGHTS`. The
//!   driver writes encoded events into ring slots; the Bridge pops them
//!   on a dedicated poll thread.
//! - **Streamed tier**: planned, not yet implemented.
//!
//! This crate owns the safe API boundary around the inline-tier OS
//! primitives. The `unsafe` operations involved (`mmap`, `memfd_create`,
//! and SCM_RIGHTS-bearing `recvmsg`) are all confined inside this crate;
//! `midori-runtime` consumes only the safe surface re-exported below and
//! therefore retains the workspace-wide `unsafe_code = "forbid"` posture.
//! This crate downgrades the lint to `deny` (see `Cargo.toml`).
//!
//! # Modules and platform gating
//!
//! - [`ring_handshake`]: pure, host-platform-independent validation /
//!   page-alignment math for the `request_ring(slot_size)` handshake.
//! - [`ring_consumer`] and [`fd_socket`]: Linux-only OS-backed primitives.
//!   They depend on `memfd_create(2)` (Linux/Android-gated in `nix`) and
//!   `SCM_RIGHTS` over UNIX domain sockets. macOS (`shm_open` ベース) /
//!   Windows (`CreateFileMapping` ベース) の backend は未実装。
//!
//! Until the macOS / Windows backends arrive, callers that need to compile
//! on those targets must wrap their use of [`RingConsumer`], [`send_fd`],
//! and [`recv_fd`] in `#[cfg(target_os = "linux")]` themselves — this
//! crate does not paper over the platform gap with stubs.
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

pub mod ring_handshake;

#[cfg(target_os = "linux")]
pub mod fd_socket;
#[cfg(target_os = "linux")]
pub mod ring_consumer;

pub use ring_handshake::{
    page_aligned_shm_size, resolve_requested_slot_size, HandshakeError, PAGE_SIZE,
    REQUEST_DEFAULT_SLOT_SIZE,
};

#[cfg(target_os = "linux")]
pub use fd_socket::{recv_fd, send_fd};
#[cfg(target_os = "linux")]
pub use ring_consumer::{CreateError, RingConsumer};
