use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::chunk::ByteRange;

/// Sidecar file written next to the destination (`<file>.zdm.json`) so a download
/// can resume after the app restarts: which chunks already landed on disk, and the
/// validators needed to confirm the remote file hasn't changed since we started.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadMeta {
    pub id: Uuid,
    pub url: String,
    pub destination: PathBuf,
    pub total_size: u64,
    pub connections: usize,
    pub chunks: Vec<ByteRange>,
    pub completed_chunks: HashSet<usize>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

impl DownloadMeta {
    pub fn sidecar_path(destination: &Path) -> PathBuf {
        let mut name = destination.as_os_str().to_owned();
        name.push(".zdm.json");
        PathBuf::from(name)
    }

    pub async fn load(destination: &Path) -> Option<Self> {
        let bytes = tokio::fs::read(Self::sidecar_path(destination)).await.ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub async fn save(&self) -> std::io::Result<()> {
        let bytes = serde_json::to_vec_pretty(self).expect("DownloadMeta always serializes");
        tokio::fs::write(Self::sidecar_path(&self.destination), bytes).await
    }

    pub async fn delete(destination: &Path) {
        let _ = tokio::fs::remove_file(Self::sidecar_path(destination)).await;
    }

    /// A resumed download is only safe if the remote file hasn't changed since
    /// we started — otherwise stitching old chunks onto new bytes would silently
    /// corrupt the result. Prefer a strong validator (etag, then last-modified)
    /// when both sides report one; a mismatch there is a definitive "changed".
    /// If neither side has a validator to compare, there's nothing to catch a
    /// real change with — fall back to a size check, and failing that, trust
    /// the resume rather than discarding valid progress on a technicality.
    pub fn matches_remote(&self, remote_etag: Option<&str>, remote_last_modified: Option<&str>, remote_size: Option<u64>) -> bool {
        if let (Some(a), Some(b)) = (self.etag.as_deref(), remote_etag) {
            return a == b;
        }
        if let (Some(a), Some(b)) = (self.last_modified.as_deref(), remote_last_modified) {
            return a == b;
        }
        match remote_size {
            Some(size) => size == self.total_size,
            None => true,
        }
    }
}
