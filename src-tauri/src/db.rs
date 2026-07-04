use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::state::{DownloadRecord, QueueInfo, Settings};

/// Local cache of what downloads/queues/settings exist, so the app remembers
/// its state across restarts. This is deliberately not the resume mechanism —
/// zdm-core's own `.zdm.json` sidecars own byte-level resume state; this DB
/// only tracks lifecycle metadata (queue, category, last known status), so
/// it's written to on transitions, not on every progress tick.
///
/// Each row stores its record as a JSON blob rather than individual columns:
/// every value here is always read back as a whole `DownloadRecord` /
/// `QueueInfo` / `Settings`, so a relational schema would just be overhead.
pub struct Db(Mutex<Connection>);

impl Db {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS downloads (id TEXT PRIMARY KEY, data TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS queues (id TEXT PRIMARY KEY, data TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, data TEXT NOT NULL);",
        )?;
        Ok(Self(Mutex::new(conn)))
    }

    pub fn load_downloads(&self) -> Vec<DownloadRecord> {
        let conn = self.0.lock().unwrap();
        let Ok(mut stmt) = conn.prepare("SELECT data FROM downloads") else { return Vec::new() };
        let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) else { return Vec::new() };
        rows.filter_map(Result::ok).filter_map(|json| serde_json::from_str(&json).ok()).collect()
    }

    pub fn load_queues(&self) -> Vec<QueueInfo> {
        let conn = self.0.lock().unwrap();
        let Ok(mut stmt) = conn.prepare("SELECT data FROM queues") else { return Vec::new() };
        let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(0)) else { return Vec::new() };
        rows.filter_map(Result::ok).filter_map(|json| serde_json::from_str(&json).ok()).collect()
    }

    pub fn load_settings(&self) -> Option<Settings> {
        let conn = self.0.lock().unwrap();
        conn.query_row("SELECT data FROM settings WHERE key = 'settings'", [], |row| row.get::<_, String>(0))
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
    }

    pub fn upsert_download(&self, record: &DownloadRecord) {
        let conn = self.0.lock().unwrap();
        let data = serde_json::to_string(record).expect("DownloadRecord always serializes");
        let _ = conn.execute(
            "INSERT INTO downloads (id, data) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET data = excluded.data",
            params![record.id.to_string(), data],
        );
    }

    pub fn delete_download(&self, id: Uuid) {
        let conn = self.0.lock().unwrap();
        let _ = conn.execute("DELETE FROM downloads WHERE id = ?1", params![id.to_string()]);
    }

    pub fn upsert_queue(&self, queue: &QueueInfo) {
        let conn = self.0.lock().unwrap();
        let data = serde_json::to_string(queue).expect("QueueInfo always serializes");
        let _ = conn.execute(
            "INSERT INTO queues (id, data) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET data = excluded.data",
            params![queue.id, data],
        );
    }

    pub fn delete_queue(&self, id: &str) {
        let conn = self.0.lock().unwrap();
        let _ = conn.execute("DELETE FROM queues WHERE id = ?1", params![id]);
    }

    pub fn save_settings(&self, settings: &Settings) {
        let conn = self.0.lock().unwrap();
        let data = serde_json::to_string(settings).expect("Settings always serializes");
        let _ = conn.execute(
            "INSERT INTO settings (key, data) VALUES ('settings', ?1) ON CONFLICT(key) DO UPDATE SET data = excluded.data",
            params![data],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::DownloadStatus;

    fn sample_record() -> DownloadRecord {
        DownloadRecord {
            id: Uuid::new_v4(),
            seq: 0,
            url: "https://example.com/file.zip".to_string(),
            name: "file.zip".to_string(),
            destination: "/tmp/file.zip".to_string(),
            category: "archive".to_string(),
            queue: "default".to_string(),
            connections: 8,
            status: DownloadStatus::Downloading,
            downloaded: 1024,
            total_size: Some(2048),
            speed_bps: 512.0,
            error: None,
            active_chunks: Vec::new(),
        }
    }

    #[test]
    fn round_trips_downloads_queues_and_settings() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(&dir.path().join("test.sqlite3")).unwrap();

        let record = sample_record();
        db.upsert_download(&record);
        let loaded = db.load_downloads();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, record.id);
        assert_eq!(loaded[0].downloaded, 1024);

        db.delete_download(record.id);
        assert!(db.load_downloads().is_empty());

        let queue = QueueInfo { id: "batch-1".to_string(), name: "Batch One".to_string() };
        db.upsert_queue(&queue);
        assert_eq!(db.load_queues(), vec![queue]);

        db.delete_queue("batch-1");
        assert!(db.load_queues().is_empty());

        assert!(db.load_settings().is_none());
        let settings = Settings::with_default_dir("/home/user/Downloads".to_string());
        db.save_settings(&settings);
        let reloaded = db.load_settings().unwrap();
        assert_eq!(reloaded.default_dir, settings.default_dir);
        assert_eq!(reloaded.category_dirs, settings.category_dirs);
    }
}
