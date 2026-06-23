//! YouAI inference worker — runs llama.cpp and exposes a small HTTP API.

pub mod llama;
pub mod server;
pub mod shards;

pub use llama::{run_inference, InferenceConfig};
pub use server::{serve, WorkerState};
