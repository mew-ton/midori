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
//! - [`ring_ingest`]: glue that pumps the ring consumer into
//!   [`events_pipeline::process_inline_payload`] on a dedicated thread.
//!   Linux / macOS only because it owns a [`midori_ipc_shm::RingConsumer`]
//!   built from the `OwnedFd` / `SCM_RIGHTS` backend of `midori-ipc-shm`.
//!   Windows は同 crate の `OwnedHandle` / Named Pipe backend に対応する
//!   ingest glue が未整備のため、本 cfg からは外している。
//!
//! Inline-tier IPC primitives themselves (`RingConsumer`, `send_fd` /
//! `recv_fd`, ring handshake validation) live in the `midori-ipc-shm`
//! crate. They were extracted so that the `unsafe` blocks required by
//! `mmap` and `SCM_RIGHTS` reception stay confined to that crate, and this
//! crate can keep `unsafe_code = "forbid"` via `lints.workspace = true`.
//!
//! `midori-ipc-shm` 側は inline-tier の OS 別実装をすべて公開している
//! （Linux: `memfd_create(2)` + `SCM_RIGHTS` / macOS: `shm_open(2)` +
//! `shm_unlink(2)` + `SCM_RIGHTS` / Windows: `CreateFileMappingW` +
//! Named Pipe + `DuplicateHandle`）。本 crate の [`ring_ingest`] は
//! Linux / macOS の同名 type (`midori_ipc_shm::RingConsumer`) を 1 本の
//! 配線でカバーし、Windows backend (`OwnedHandle` + Named Pipe handoff)
//! を取り込む差分は別途必要なため、本 lib では Windows を cfg から外す。
//!
//! The CLI dispatch layer remains binary-private until there is a concrete
//! need to expose it.

pub mod driver_proc;
pub mod logging;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub mod ring_ingest;

// `events_pipeline` was previously binary-private. It is now a dependency
// of `ring_ingest`, which the lib surface re-exports — so the module
// itself must move to lib scope too. It stays `pub` because the `midori`
// binary lives in a separate crate and reaches it via `midori_runtime::*`.
pub mod events_pipeline;
pub mod events_schema;
