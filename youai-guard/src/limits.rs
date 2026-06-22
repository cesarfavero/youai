use anyhow::{bail, Context, Result};
use std::time::Duration;

/// Parsed resource limits for the guard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceLimits {
    pub ram_max_bytes: u64,
    pub cpu_percent: u8,
    pub poll_interval: Duration,
}

/// Parse a human-readable byte size (`8g`, `512m`, `1024k`).
pub fn parse_byte_size(input: &str) -> Result<u64> {
    let input = input.trim().to_lowercase();
    if input.is_empty() {
        bail!("empty size");
    }

    let (num, unit) = if input.ends_with("gb") {
        (&input[..input.len() - 2], "gb")
    } else if input.ends_with("mb") {
        (&input[..input.len() - 2], "mb")
    } else if input.ends_with("kb") {
        (&input[..input.len() - 2], "kb")
    } else if input.ends_with('g') {
        (&input[..input.len() - 1], "g")
    } else if input.ends_with('m') {
        (&input[..input.len() - 1], "m")
    } else if input.ends_with('k') {
        (&input[..input.len() - 1], "k")
    } else {
        (input.as_str(), "b")
    };

    let value: f64 = num
        .trim()
        .parse()
        .with_context(|| format!("invalid size number: {num}"))?;

    if !value.is_finite() || value <= 0.0 {
        bail!("size must be positive");
    }

    let multiplier = match unit {
        "gb" | "g" => 1024u64.pow(3),
        "mb" | "m" => 1024u64.pow(2),
        "kb" | "k" => 1024,
        "b" => 1,
        _ => bail!("unknown unit"),
    };

    let bytes = (value * multiplier as f64).round() as u64;
    if bytes == 0 {
        bail!("size must be at least 1 byte");
    }

    Ok(bytes)
}

pub fn parse_limits(ram_max: &str, cpu_percent: u8, poll_ms: u64) -> Result<ResourceLimits> {
    if cpu_percent == 0 || cpu_percent > 100 {
        bail!("cpu-percent must be between 1 and 100");
    }
    if poll_ms == 0 {
        bail!("poll-ms must be greater than 0");
    }

    Ok(ResourceLimits {
        ram_max_bytes: parse_byte_size(ram_max)?,
        cpu_percent,
        poll_interval: Duration::from_millis(poll_ms),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_sizes() {
        assert_eq!(parse_byte_size("8g").unwrap(), 8 * 1024 * 1024 * 1024);
        assert_eq!(parse_byte_size("512m").unwrap(), 512 * 1024 * 1024);
        assert_eq!(parse_byte_size("1024k").unwrap(), 1024 * 1024);
        assert_eq!(parse_byte_size("4096").unwrap(), 4096);
    }

    #[test]
    fn rejects_invalid_cpu() {
        assert!(parse_limits("8g", 0, 500).is_err());
        assert!(parse_limits("8g", 101, 500).is_err());
    }
}
