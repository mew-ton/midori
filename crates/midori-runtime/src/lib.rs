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
//!
//! The CLI dispatch layer remains binary-private until there is a concrete
//! need to expose it.

pub mod driver_proc;
pub mod logging;
