use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;
use zdm_core::{ActiveChunk, DownloadEngine};

use crate::db::Db;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadStatus {
    Queued,
    Downloading,
    Paused,
    Completed,
    Failed,
    Canceled,
}

/// App-level view of a download: everything zdm-core doesn't know about
/// (display name, category, which queue it belongs to) plus the live transfer
/// stats mirrored in from `DownloadEvent`s as they arrive.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadRecord {
    pub id: Uuid,
    pub seq: u64,
    pub url: String,
    pub name: String,
    pub destination: String,
    pub category: String,
    pub queue: String,
    pub connections: usize,
    pub status: DownloadStatus,
    pub downloaded: u64,
    pub total_size: Option<u64>,
    pub speed_bps: f64,
    pub error: Option<String>,
    /// Chunks currently in flight, straight from the engine's live workers —
    /// empty whenever nothing is actively transferring (paused/queued/done).
    pub active_chunks: Vec<ActiveChunk>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueInfo {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub max_simultaneous_downloads: usize,
    pub default_connections: usize,
    pub notify_on_completion: bool,
    pub category_dirs: HashMap<String, String>,
    pub default_dir: String,
    /// One of the ids in `src/lib/accents.ts` — the app only reads this back
    /// to apply the matching CSS custom properties, so its accepted values
    /// live entirely on the frontend.
    #[serde(default = "default_accent")]
    pub accent_color: String,
}

fn default_accent() -> String {
    "amber".to_string()
}

impl Settings {
    /// Seeds category folders under the platform's real Downloads directory
    /// (resolved by the caller via Tauri's path API) rather than a placeholder.
    pub fn with_default_dir(default_dir: String) -> Self {
        let sub = |name: &str| format!("{default_dir}/{name}");
        let category_dirs = [
            ("video", "Video"),
            ("audio", "Audio"),
            ("archive", "Compressed"),
            ("docs", "Documents"),
            ("disc", "Disc Images"),
            ("software", "Software"),
            ("image", "Images"),
            ("ebook", "Ebooks"),
            ("font", "Fonts"),
            ("torrent", "Torrents"),
            ("other", "Other"),
        ]
        .into_iter()
        .map(|(cat, folder)| (cat.to_string(), sub(folder)))
        .collect();

        Self {
            max_simultaneous_downloads: 1,
            default_connections: 8,
            notify_on_completion: true,
            category_dirs,
            default_dir,
            accent_color: default_accent(),
        }
    }
}

pub struct AppState {
    pub engine: DownloadEngine,
    pub db: Db,
    pub records: Mutex<HashMap<Uuid, DownloadRecord>>,
    pub queues: Mutex<Vec<QueueInfo>>,
    pub settings: Mutex<Settings>,
    seq_counter: AtomicU64,
}

impl AppState {
    /// `initial_downloads`/`initial_queues` come from the DB (empty on first
    /// launch); `default_settings` is used only when nothing was persisted yet.
    pub fn new(
        engine: DownloadEngine,
        db: Db,
        initial_downloads: Vec<DownloadRecord>,
        initial_queues: Vec<QueueInfo>,
        settings: Settings,
    ) -> Self {
        let next_seq = initial_downloads.iter().map(|r| r.seq).max().map(|s| s + 1).unwrap_or(0);
        let mut queues = initial_queues;
        if !queues.iter().any(|q| q.id == "default") {
            queues.insert(0, QueueInfo { id: "default".to_string(), name: "Default Queue".to_string() });
        }

        Self {
            engine,
            db,
            records: Mutex::new(initial_downloads.into_iter().map(|r| (r.id, r)).collect()),
            queues: Mutex::new(queues),
            settings: Mutex::new(settings),
            seq_counter: AtomicU64::new(next_seq),
        }
    }

    pub fn next_seq(&self) -> u64 {
        self.seq_counter.fetch_add(1, Ordering::Relaxed)
    }

    /// After a full resequence (drag-and-drop reorder), later additions must
    /// still land after every existing row instead of colliding with one.
    pub fn set_seq_floor(&self, floor: u64) {
        self.seq_counter.fetch_max(floor, Ordering::Relaxed);
    }
}
