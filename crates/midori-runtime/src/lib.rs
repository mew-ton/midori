//! Library surface of the Midori Bridge runtime.
//!
//! The runtime ships primarily as the `midori` binary (`src/main.rs`), but
//! certain subsystems are exposed here so integration tests and downstream
//! consumers can drive them directly without going through argv.
//!
//! Exposed:
//!
//! - [`driver_proc`]: driver subprocess supervisor (spawn / handshake /
//!   lifecycle).
//! - [`logging`]: bridge logger entrypoints. Re-exported so `driver_proc`'s
//!   stdout-forwarding path can hand non-JSON driver lines to the same
//!   logger the CLI uses, and so integration tests can observe the layer
//!   string used for forwarded lines.
//! - [`ring_consumer`]: per-driver shm allocator + SPSC consumer used by
//!   the inline-tier ingest path. Linux-only — depends on `memfd_create(2)`.
//! - [`fd_socket`]: SCM_RIGHTS-based fd handoff helper used to deliver the
//!   shm fd from Bridge to driver. Linux-only as part of the inline-tier
//!   chain that includes [`ring_consumer`].
//! - [`ring_ingest`]: glue that pumps the ring consumer into
//!   [`events_pipeline::process_inline_payload`] on a dedicated thread.
//!   Linux-only as part of the same chain.
//!
//! macOS / Windows 対応の inline-tier IPC は別 Issue で実装する（macOS は
//! `shm_open(2)` ベース、Windows は `CreateFileMapping` ベースの予定）。
//! 現状はその 3 つの module ごと `cfg(target_os = "linux")` で囲って
//! 他 OS では存在しない扱いにしている。
//!
//! The CLI dispatch layer remains binary-private until there is a concrete
//! need to expose it.

pub mod driver_proc;
#[cfg(target_os = "linux")]
pub mod fd_socket;
pub mod logging;
#[cfg(target_os = "linux")]
pub mod ring_consumer;
#[cfg(target_os = "linux")]
pub mod ring_ingest;

// `events_pipeline` / `ring_handshake` were previously binary-private. They
// are now dependencies of `ring_consumer` / `ring_ingest`, which the lib
// surface re-exports — so the modules themselves must move to lib scope too.
// They stay `pub` because the `midori` binary lives in a separate crate and
// reaches them via `midori_runtime::*`.
pub mod events_pipeline;
pub mod events_schema;
pub mod ring_handshake;
