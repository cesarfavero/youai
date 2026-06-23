//! Contributor-priority scheduling before hitting the network.

use std::time::Duration;

/// Contributors skip the fairness delay; others wait briefly when the network is busy.
pub async fn apply_chat_priority(contributor: bool, contributor_score: f64) -> Duration {
    if contributor {
        return Duration::ZERO;
    }
    let base_ms = 40u64;
    let score_bonus = contributor_score.min(100.0) as u64;
    let delay = Duration::from_millis(base_ms.saturating_sub(score_bonus / 10));
    if !delay.is_zero() {
        tokio::time::sleep(delay).await;
    }
    delay
}