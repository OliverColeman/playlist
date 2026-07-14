//! Library target for the playlist CLI. The binary (`src/main.rs`) is a thin wrapper
//! around this; the commands are exposed as a library so integration tests can drive
//! them directly.

pub mod commands;
