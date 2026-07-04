use std::path::PathBuf;

use tauri::{AppHandle, Emitter, Manager};
use zdm_core::{DownloadEvent, DownloadOptions};

use crate::state::{AppState, DownloadRecord, DownloadStatus};

/// Emits the updated record to the frontend and writes it through to the DB —
/// the one place these two always happen together, so no call site forgets one.
pub fn publish(app: &AppHandle, state: &AppState, record: &DownloadRecord) {
    state.db.upsert_download(record);
    let _ = app.emit("download-updated", record);
}

/// Pulls queued downloads into active transfers up to the configured
/// concurrency limit. Called after anything that could free or fill a slot:
/// a new download being added, one finishing, or the limit itself changing.
pub async fn try_promote_queue(app: &AppHandle) {
    let state = app.state::<AppState>();
    loop {
        let max = state.settings.lock().await.max_simultaneous_downloads.max(1);
        let active = state.records.lock().await.values().filter(|r| r.status == DownloadStatus::Downloading).count();
        if active >= max {
            break;
        }

        let candidate = {
            let records = state.records.lock().await;
            records.values().filter(|r| r.status == DownloadStatus::Queued).min_by_key(|r| r.seq).cloned()
        };
        let Some(record) = candidate else { break };

        let opts = DownloadOptions {
            id: record.id,
            url: record.url.clone(),
            destination: PathBuf::from(&record.destination),
            connections: record.connections,
        };

        match state.engine.start(opts).await {
            Ok(_) => {
                let mut records = state.records.lock().await;
                if let Some(r) = records.get_mut(&record.id) {
                    r.status = DownloadStatus::Downloading;
                }
            }
            Err(e) => {
                let updated = {
                    let mut records = state.records.lock().await;
                    records.get_mut(&record.id).map(|r| {
                        r.status = DownloadStatus::Failed;
                        r.error = Some(e.to_string());
                        r.clone()
                    })
                };
                if let Some(record) = updated {
                    publish(app, &state, &record);
                }
            }
        }
    }
}

/// Runs once at startup for every download that was mid-flight when the app
/// last closed. Resuming reads the same `.zdm.json` sidecar the engine itself
/// writes, so this only works for downloads that got at least one chunk in —
/// anything else is marked Failed with an actionable message rather than
/// silently reappearing stuck at 0%.
pub async fn resume_interrupted_downloads(app: &AppHandle) {
    let state = app.state::<AppState>();
    let candidates: Vec<DownloadRecord> = {
        let records = state.records.lock().await;
        records.values().filter(|r| matches!(r.status, DownloadStatus::Downloading | DownloadStatus::Paused)).cloned().collect()
    };

    for record in candidates {
        let destination = PathBuf::from(&record.destination);
        if state.engine.resume_from_disk(destination).await.is_err() {
            let mut updated = record;
            updated.status = DownloadStatus::Failed;
            updated.error = Some("Interrupted by app restart — click Retry to resume.".to_string());
            publish(app, &state, &updated);
        }
    }
}

fn event_id(event: &DownloadEvent) -> uuid::Uuid {
    match event {
        DownloadEvent::Started { id, .. }
        | DownloadEvent::Progress { id, .. }
        | DownloadEvent::Paused { id }
        | DownloadEvent::Completed { id }
        | DownloadEvent::Canceled { id }
        | DownloadEvent::Failed { id, .. } => *id,
    }
}

/// Forwards every engine event into the matching `DownloadRecord`, re-emits the
/// updated record to the frontend, and re-runs the scheduler whenever a slot
/// might have opened up.
pub fn spawn_event_forwarder(app: AppHandle, mut events_rx: tokio::sync::mpsc::UnboundedReceiver<DownloadEvent>) {
    tauri::async_runtime::spawn(async move {
        while let Some(event) = events_rx.recv().await {
            let id = event_id(&event);
            let state = app.state::<AppState>();
            let mut frees_a_slot = false;

            let updated = {
                let mut records = state.records.lock().await;
                let Some(record) = records.get_mut(&id) else { continue };
                match &event {
                    DownloadEvent::Started { total_size, .. } => {
                        record.status = DownloadStatus::Downloading;
                        record.total_size = *total_size;
                    }
                    DownloadEvent::Progress { downloaded, total_size, speed_bps, active_chunks, .. } => {
                        record.downloaded = *downloaded;
                        record.total_size = *total_size;
                        record.speed_bps = *speed_bps;
                        record.active_chunks = active_chunks.clone();
                    }
                    DownloadEvent::Paused { .. } => {
                        record.status = DownloadStatus::Paused;
                        record.speed_bps = 0.0;
                        record.active_chunks.clear();
                        frees_a_slot = true;
                    }
                    DownloadEvent::Completed { .. } => {
                        record.status = DownloadStatus::Completed;
                        record.speed_bps = 0.0;
                        record.active_chunks.clear();
                        if let Some(total) = record.total_size {
                            record.downloaded = total;
                        }
                        frees_a_slot = true;
                    }
                    DownloadEvent::Canceled { .. } => {
                        record.status = DownloadStatus::Canceled;
                        record.speed_bps = 0.0;
                        record.active_chunks.clear();
                        frees_a_slot = true;
                    }
                    DownloadEvent::Failed { error, .. } => {
                        record.status = DownloadStatus::Failed;
                        record.speed_bps = 0.0;
                        record.active_chunks.clear();
                        record.error = Some(error.clone());
                        frees_a_slot = true;
                    }
                }
                record.clone()
            };

            // Every event is emitted live, but only lifecycle transitions are
            // written to disk — persisting on every ~400ms Progress tick would
            // hammer SQLite for data that's already recoverable from the
            // engine's own resume sidecar.
            let is_progress_only = matches!(event, DownloadEvent::Progress { .. });
            if is_progress_only {
                let _ = app.emit("download-updated", &updated);
            } else {
                publish(&app, &state, &updated);
            }

            let notify_worthy = matches!(event, DownloadEvent::Completed { .. });
            if notify_worthy && state.settings.lock().await.notify_on_completion {
                notify_completion(&app, &updated.name);
            }
            if frees_a_slot {
                try_promote_queue(&app).await;
            }
        }
    });
}

fn notify_completion(app: &AppHandle, file_name: &str) {
    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification().builder().title("Download complete").body(file_name).show();
}
