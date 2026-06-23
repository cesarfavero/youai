//! Pre-network response cache — avoids hitting workers for repeated prompts.

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use youai_common::ChatResponse;

const DEFAULT_MAX_ENTRIES: usize = 500;
const DEFAULT_TTL_CONTRIBUTOR: Duration = Duration::from_secs(24 * 3600);
const DEFAULT_TTL_ANONYMOUS: Duration = Duration::from_secs(3600);

#[derive(Clone)]
struct Entry {
    response: ChatResponse,
    expires_at: Instant,
}

pub struct ResponseCache {
    entries: HashMap<String, Entry>,
    max_entries: usize,
    ttl_contributor: Duration,
    ttl_anonymous: Duration,
}

impl ResponseCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            max_entries: DEFAULT_MAX_ENTRIES,
            ttl_contributor: DEFAULT_TTL_CONTRIBUTOR,
            ttl_anonymous: DEFAULT_TTL_ANONYMOUS,
        }
    }

    pub fn cache_key(prompt: &str, max_tokens: u32, tier: &str, mode: &str) -> String {
        let normalized = prompt.trim().to_lowercase();
        let mut hasher = Sha256::new();
        hasher.update(normalized.as_bytes());
        hasher.update(b"|");
        hasher.update(max_tokens.to_le_bytes());
        hasher.update(b"|");
        hasher.update(tier.as_bytes());
        hasher.update(b"|");
        hasher.update(mode.as_bytes());
        hex::encode(hasher.finalize())
    }

    pub fn get(&mut self, key: &str) -> Option<ChatResponse> {
        self.evict_expired();
        let entry = self.entries.get(key)?;
        if Instant::now() >= entry.expires_at {
            self.entries.remove(key);
            return None;
        }
        let mut resp = entry.response.clone();
        resp.cached = true;
        Some(resp)
    }

    pub fn put(&mut self, key: String, mut response: ChatResponse, contributor: bool) {
        self.evict_expired();
        if self.entries.len() >= self.max_entries {
            self.evict_oldest();
        }
        let ttl = if contributor {
            self.ttl_contributor
        } else {
            self.ttl_anonymous
        };
        response.cached = false;
        self.entries.insert(
            key,
            Entry {
                response,
                expires_at: Instant::now() + ttl,
            },
        );
    }

    fn evict_expired(&mut self) {
        let now = Instant::now();
        self.entries.retain(|_, e| e.expires_at > now);
    }

    fn evict_oldest(&mut self) {
        if let Some(oldest_key) = self
            .entries
            .iter()
            .min_by_key(|(_, e)| e.expires_at)
            .map(|(k, _)| k.clone())
        {
            self.entries.remove(&oldest_key);
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

impl Default for ResponseCache {
    fn default() -> Self {
        Self::new()
    }
}