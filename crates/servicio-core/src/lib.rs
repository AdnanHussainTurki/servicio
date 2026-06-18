//! servicio-core: headless supervisor engine.
//!
//! Pure library — no UI, no SQLite, no service install. Spawns and monitors
//! worker processes, restarts them per policy, and captures their logs.

pub mod backoff;
pub mod error;
pub mod event;
pub mod logsink;
pub mod manager;
pub mod process;
pub mod schedule;
pub mod state;
pub mod supervisor;
pub mod worker;

pub use error::CoreError;
