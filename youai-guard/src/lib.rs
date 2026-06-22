//! YouAI Resource Guard library.

/// Guard poll interval in milliseconds (MVP default).
pub const DEFAULT_POLL_MS: u64 = 500;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_poll_interval() {
        assert_eq!(DEFAULT_POLL_MS, 500);
    }
}