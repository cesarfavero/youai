//! Network compute units — tier selection by aggregate power, not headcount.

use anyhow::{Context, Result};

/// Effective compute units for one contributor node.
///
/// `cpu_percent` caps how much of `ram_max_mb` the node actually offers the network.
/// Example: 30% CPU × 2048 MB RAM = 614 CU.
pub fn node_compute_units(cpu_percent: u8, ram_max_mb: u32) -> u64 {
    let cpu = u64::from(cpu_percent.min(100));
    let ram = u64::from(ram_max_mb);
    cpu.saturating_mul(ram) / 100
}

/// Parse `ram_max` strings like `2g`, `512m`, `8192`.
pub fn parse_ram_max_mb(raw: &str) -> Result<u32> {
    let s = raw.trim().to_lowercase();
    if s.is_empty() {
        anyhow::bail!("empty ram_max");
    }
    let (num, unit) = if let Some(stripped) = s.strip_suffix('g') {
        (stripped, "g")
    } else if let Some(stripped) = s.strip_suffix("gb") {
        (stripped, "g")
    } else if let Some(stripped) = s.strip_suffix('m') {
        (stripped, "m")
    } else if let Some(stripped) = s.strip_suffix("mb") {
        (stripped, "m")
    } else {
        (s.as_str(), "m")
    };
    let value: f64 = num
        .trim()
        .parse()
        .with_context(|| format!("invalid ram_max: {raw}"))?;
    let mb = match unit {
        "g" => value * 1024.0,
        _ => value,
    };
    if mb <= 0.0 || mb > f64::from(u32::MAX) {
        anyhow::bail!("ram_max out of range: {raw}");
    }
    Ok(mb.round() as u32)
}

/// Sum compute units across online nodes.
pub fn network_compute_units(nodes: &[(u8, u32)]) -> u64 {
    nodes
        .iter()
        .map(|(cpu, ram)| node_compute_units(*cpu, *ram))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_mini_defaults() {
        let cu = node_compute_units(30, 2048);
        assert_eq!(cu, 614);
    }

    #[test]
    fn parse_ram() {
        assert_eq!(parse_ram_max_mb("2g").unwrap(), 2048);
        assert_eq!(parse_ram_max_mb("512m").unwrap(), 512);
    }
}