use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A chunk currently being fetched by one connection, and how far it's gotten.
/// This is what lets the UI draw genuine per-connection progress instead of a
/// single aggregate bar — every value here is read straight from a live worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveChunk {
    pub start: u64,
    pub end: u64,
    pub bytes_done: u64,
}

/// Emitted over an unbounded channel as downloads progress; the Tauri layer
/// forwards these to the frontend as-is (they already derive `Serialize`).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DownloadEvent {
    Started { id: Uuid, total_size: Option<u64>, resumable: bool },
    Progress { id: Uuid, downloaded: u64, total_size: Option<u64>, speed_bps: f64, active_chunks: Vec<ActiveChunk> },
    Paused { id: Uuid },
    Resumed { id: Uuid },
    Completed { id: Uuid },
    Canceled { id: Uuid },
    Failed { id: Uuid, error: String },
}
