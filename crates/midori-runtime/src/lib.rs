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
//!   Linux-only because it owns a [`midori_ipc_shm::RingConsumer`].
//!
//! Inline-tier IPC primitives themselves (`RingConsumer`, `send_fd` /
//! `recv_fd`, ring handshake validation) live in the `midori-ipc-shm`
//! crate. They were extracted so that the `unsafe` blocks required by
//! `mmap` and `SCM_RIGHTS` reception stay confined to that crate, and this
//! crate can keep `unsafe_code = "forbid"` via `lints.workspace = true`.
//!
//! 現状 inline-tier IPC の **公開 API** は Linux 専用（`memfd_create(2)`
//! 依存）。macOS 向け実装 (`shm_open(2)` + `shm_unlink(2)` ベース) は
//! `midori-ipc-shm` 内部に存在しビルド / テストされているが、Bridge
//! runtime 層の cfg ゲート整理が完了するまで公開 surface としての解禁は
//! 保留している。Windows backend (`CreateFileMapping` ベース) は未実装。
//! それまでは [`ring_ingest`] および `midori_ipc_shm` の公開 API を
//! `cfg(target_os = "linux")` で囲って他 OS では参照しない運用。
//!
//! The CLI dispatch layer remains binary-private until there is a concrete
//! need to expose it.

pub mod driver_proc;
pub mod logging;
#[cfg(target_os = "linux")]
pub mod ring_ingest;

// `events_pipeline` was previously binary-private. It is now a dependency
// of `ring_ingest`, which the lib surface re-exports — so the module
// itself must move to lib scope too. It stays `pub` because the `midori`
// binary lives in a separate crate and reaches it via `midori_runtime::*`.
pub mod events_pipeline;
pub mod events_schema;
