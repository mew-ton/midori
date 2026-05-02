//! Library surface of the Midori Bridge runtime.
//!
//! The runtime ships primarily as the `midori` binary (`src/main.rs`), but
//! certain subsystems are exposed here so integration tests and downstream
//! consumers can drive them directly without going through argv.
//!
//! Today only the driver process supervisor is exposed. The CLI dispatch
//! layer remains binary-private until there is a concrete need to expose it.

pub mod driver_proc;
