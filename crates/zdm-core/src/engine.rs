use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use tokio::sync::{mpsc, Mutex, Notify};
use uuid::Uuid;

use crate::chunk::{plan_chunks, ByteRange};
use crate::error::DownloadError;
use crate::events::{ActiveChunk, DownloadEvent};
use crate::meta::DownloadMeta;
use crate::probe::probe;
use crate::progress::SpeedTracker;
use crate::worker::{download_chunk, download_whole_file_sequential};

const RUNNING: u8 = 0;
const PAUSED: u8 = 1;
const CANCELED: u8 = 2;

/// Pause/resume/cancel signaling shared between the engine and a task's workers,
/// plus the live byte counter workers report into.
struct TaskControl {
    state: AtomicU8,
    notify: Notify,
    downloaded: AtomicU64,
}

impl TaskControl {
    fn new(initial_downloaded: u64) -> Self {
        Self { state: AtomicU8::new(RUNNING), notify: Notify::new(), downloaded: AtomicU64::new(initial_downloaded) }
    }

    fn pause(&self) {
        self.state.store(PAUSED, Ordering::SeqCst);
    }

    fn resume(&self) {
        self.state.store(RUNNING, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    fn cancel(&self) {
        self.state.store(CANCELED, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    /// Blocks while paused. A worker only checks this between chunks rather than
    /// mid-stream, so pausing costs at most one in-flight chunk (≤4 MiB) of
    /// re-download on resume — a deliberate trade for not needing byte-level
    /// resume bookkeeping inside a single HTTP response.
    async fn wait_if_paused(&self) -> Result<(), DownloadError> {
        loop {
            match self.state.load(Ordering::SeqCst) {
                RUNNING => return Ok(()),
                CANCELED => return Err(DownloadError::Canceled),
                _ => self.notify.notified().await,
            }
        }
    }
}

pub struct DownloadOptions {
    /// Caller-assigned id, so app-level code can track a download (e.g. while it
    /// sits queued, before the engine has actually started it) under the same
    /// identity the engine will later use in every emitted event.
    pub id: Uuid,
    pub url: String,
    pub destination: PathBuf,
    pub connections: usize,
}

#[derive(Clone)]
pub struct DownloadEngine {
    client: Client,
    tasks: Arc<Mutex<HashMap<Uuid, Arc<TaskControl>>>>,
    events_tx: mpsc::UnboundedSender<DownloadEvent>,
}

impl DownloadEngine {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<DownloadEvent>) {
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let engine = Self {
            client: Client::builder().build().expect("reqwest client builds with rustls-tls enabled"),
            tasks: Arc::new(Mutex::new(HashMap::new())),
            events_tx,
        };
        (engine, events_rx)
    }

    /// Probes the URL, preallocates the destination file, and hands back an id
    /// immediately — the transfer itself runs in a background task so callers
    /// (e.g. a Tauri command) never block on a multi-gigabyte download.
    pub async fn start(&self, opts: DownloadOptions) -> Result<Uuid, DownloadError> {
        let id = opts.id;
        let connections = opts.connections.max(1);
        let probe_result = probe(&self.client, &opts.url).await?;
        let resumable = probe_result.supports_ranges && probe_result.total_size.is_some();
        let total_size = probe_result.total_size.unwrap_or(0);

        if let Some(parent) = opts.destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let chunks = if resumable { plan_chunks(total_size, connections) } else { Vec::new() };

        if resumable {
            let file = tokio::fs::OpenOptions::new().write(true).create(true).open(&opts.destination).await?;
            file.set_len(total_size).await?;
        }

        let meta = DownloadMeta {
            id,
            url: opts.url,
            destination: opts.destination,
            total_size,
            connections,
            chunks,
            completed_chunks: Default::default(),
            etag: probe_result.etag,
            last_modified: probe_result.last_modified,
        };
        if resumable {
            meta.save().await?;
        }

        self.spawn_task(id, meta, 0, resumable, probe_result.total_size).await;
        Ok(id)
    }

    /// Resumes a download from its `.zdm.json` sidecar after an app restart.
    /// If the remote file's validators (etag / last-modified) no longer match
    /// what we recorded, the remote content changed underneath us — stitching
    /// old chunks onto new bytes would silently corrupt the file, so we discard
    /// the partial download and start over instead.
    pub async fn resume_from_disk(&self, destination: PathBuf) -> Result<Uuid, DownloadError> {
        let meta = DownloadMeta::load(&destination).await.ok_or(DownloadError::NotFound(Uuid::nil()))?;
        let probe_result = probe(&self.client, &meta.url).await?;

        if !meta.matches_remote(probe_result.etag.as_deref(), probe_result.last_modified.as_deref(), probe_result.total_size) {
            DownloadMeta::delete(&destination).await;
            return self
                .start(DownloadOptions { id: meta.id, url: meta.url, destination, connections: meta.connections })
                .await;
        }

        let id = meta.id;
        let already_downloaded: u64 =
            meta.completed_chunks.iter().filter_map(|i| meta.chunks.get(*i)).map(ByteRange::len).sum();
        let total_size = meta.total_size;

        self.spawn_task(id, meta, already_downloaded, true, Some(total_size)).await;
        Ok(id)
    }

    async fn spawn_task(
        &self,
        id: Uuid,
        meta: DownloadMeta,
        initial_downloaded: u64,
        resumable: bool,
        total_size_for_event: Option<u64>,
    ) {
        let control = Arc::new(TaskControl::new(initial_downloaded));
        self.tasks.lock().await.insert(id, control.clone());

        let _ = self.events_tx.send(DownloadEvent::Started { id, total_size: total_size_for_event, resumable });

        let engine = self.clone();
        tokio::spawn(async move { engine.run_download(id, meta, control, resumable).await });
    }

    async fn run_download(&self, id: Uuid, mut meta: DownloadMeta, control: Arc<TaskControl>, resumable: bool) {
        let result = if resumable {
            self.run_segmented(&id, &mut meta, &control).await
        } else {
            self.run_sequential_with_progress(&id, &meta, &control).await
        };

        self.tasks.lock().await.remove(&id);

        match result {
            Ok(()) => {
                DownloadMeta::delete(&meta.destination).await;
                let _ = self.events_tx.send(DownloadEvent::Completed { id });
            }
            Err(DownloadError::Canceled) => {
                let _ = self.events_tx.send(DownloadEvent::Canceled { id });
            }
            Err(e) => {
                let _ = self.events_tx.send(DownloadEvent::Failed { id, error: e.to_string() });
            }
        }
    }

    /// Wraps a non-resumable, single-connection download with the same
    /// periodic Progress reporting the segmented path gets — otherwise these
    /// downloads would show 0% right up until they jump straight to Completed.
    async fn run_sequential_with_progress(
        &self,
        id: &Uuid,
        meta: &DownloadMeta,
        control: &Arc<TaskControl>,
    ) -> Result<(), DownloadError> {
        let events_tx = self.events_tx.clone();
        let ticker_control = control.clone();
        let ticker_id = *id;
        let total_size = meta.total_size;

        let ticker = tokio::spawn(async move {
            let mut speed_tracker = SpeedTracker::new();
            let mut interval = tokio::time::interval(Duration::from_millis(400));
            loop {
                interval.tick().await;
                let downloaded = ticker_control.downloaded.load(Ordering::Relaxed);
                let speed = speed_tracker.sample(downloaded);
                let _ = events_tx.send(DownloadEvent::Progress {
                    id: ticker_id,
                    downloaded,
                    total_size: if total_size > 0 { Some(total_size) } else { None },
                    speed_bps: speed,
                    active_chunks: Vec::new(),
                });
            }
        });

        let result =
            download_whole_file_sequential(&self.client, &meta.url, &meta.destination, &control.downloaded).await;
        ticker.abort();
        result
    }

    async fn run_segmented(
        &self,
        id: &Uuid,
        meta: &mut DownloadMeta,
        control: &Arc<TaskControl>,
    ) -> Result<(), DownloadError> {
        let already_done = meta.completed_chunks.clone();
        let pending: VecDeque<(usize, ByteRange)> = meta
            .chunks
            .iter()
            .enumerate()
            .filter(|(i, _)| !already_done.contains(i))
            .map(|(i, r)| (i, *r))
            .collect();
        let queue = Arc::new(Mutex::new(pending));
        let completed = Arc::new(Mutex::new(already_done));
        let active_progress: Arc<Mutex<HashMap<usize, u64>>> = Arc::new(Mutex::new(HashMap::new()));

        let mut worker_handles = Vec::with_capacity(meta.connections);
        for _ in 0..meta.connections {
            let client = self.client.clone();
            let url = meta.url.clone();
            let destination = meta.destination.clone();
            let queue = queue.clone();
            let completed = completed.clone();
            let control = control.clone();
            let active_progress = active_progress.clone();

            worker_handles.push(tokio::spawn(async move {
                loop {
                    control.wait_if_paused().await?;
                    let next = queue.lock().await.pop_front();
                    let Some((index, range)) = next else { break };
                    active_progress.lock().await.insert(index, 0);
                    let result =
                        download_chunk(&client, &url, &destination, index, range, &control.downloaded, &active_progress)
                            .await;
                    active_progress.lock().await.remove(&index);
                    result?;
                    completed.lock().await.insert(index);
                }
                Ok::<(), DownloadError>(())
            }));
        }

        let mut speed_tracker = SpeedTracker::new();
        let total_size = meta.total_size;
        let mut ticker = tokio::time::interval(Duration::from_millis(400));
        loop {
            ticker.tick().await;
            let downloaded = control.downloaded.load(Ordering::Relaxed);
            let speed = speed_tracker.sample(downloaded);
            let active_chunks = active_progress
                .lock()
                .await
                .iter()
                .filter_map(|(index, bytes_done)| {
                    meta.chunks.get(*index).map(|r| ActiveChunk { start: r.start, end: r.end, bytes_done: *bytes_done })
                })
                .collect();
            let _ = self.events_tx.send(DownloadEvent::Progress {
                id: *id,
                downloaded,
                total_size: Some(total_size),
                speed_bps: speed,
                active_chunks,
            });
            meta.completed_chunks = completed.lock().await.clone();
            let _ = meta.save().await;

            if worker_handles.iter().all(|h| h.is_finished()) {
                break;
            }
        }

        for handle in worker_handles {
            handle.await.expect("chunk worker task panicked")?;
        }
        Ok(())
    }

    pub async fn pause(&self, id: Uuid) -> Result<(), DownloadError> {
        let tasks = self.tasks.lock().await;
        let control = tasks.get(&id).ok_or(DownloadError::NotFound(id))?;
        control.pause();
        let _ = self.events_tx.send(DownloadEvent::Paused { id });
        Ok(())
    }

    pub async fn resume(&self, id: Uuid) -> Result<(), DownloadError> {
        let tasks = self.tasks.lock().await;
        let control = tasks.get(&id).ok_or(DownloadError::NotFound(id))?;
        control.resume();
        Ok(())
    }

    pub async fn cancel(&self, id: Uuid) -> Result<(), DownloadError> {
        let tasks = self.tasks.lock().await;
        let control = tasks.get(&id).ok_or(DownloadError::NotFound(id))?;
        control.cancel();
        Ok(())
    }
}

impl Default for DownloadEngine {
    fn default() -> Self {
        Self::new().0
    }
}
