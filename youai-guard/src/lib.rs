//! YouAI Resource Guard — enforces RAM/CPU limits on child processes.

pub mod limits;
pub mod logging;
pub mod monitor;
pub mod platform;

/// Guard poll interval in milliseconds (MVP default).
pub const DEFAULT_POLL_MS: u64 = 500;

pub use limits::{parse_byte_size, parse_limits, ResourceLimits};
pub use platform::run;
