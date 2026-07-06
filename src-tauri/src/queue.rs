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

/// Resumes a download that has a live (paused) engine task, or — if none
/// exists, e.g. it was never started, or the app restarted and the in-memory
/// task registry was cleared — falls back to resuming from its on-disk
/// sidecar, or starting fresh if there isn't one either.
pub async fn resume_or_restart(app: &AppHandle, id: uuid::Uuid) -> Result<(), String> {
    let state = app.state::<AppState>();
    if state.engine.resume_silent(id).await.is_ok() {
        return Ok(());
    }

    let destination = {
        let records = state.records.lock().await;
        records.get(&id).map(|r| PathBuf::from(&r.destination))
    }
    .ok_or("unknown download")?;

    let has_sidecar = zdm_core::DownloadMeta::load(&destination).await.is_some();
    let result = if has_sidecar {
        state.engine.resume_from_disk(destination).await
    } else {
        let opts = {
            let records = state.records.lock().await;
            let r = records.get(&id).ok_or("unknown download")?;
            DownloadOptions { id, url: r.url.clone(), destination: PathBuf::from(&r.destination), connections: r.connections }
        };
        state.engine.start(opts).await
    };
    result.map(|_| ()).map_err(|e| e.to_string())
}

/// Keeps the top `max_simultaneous_downloads` downloads (by seq, among
/// queued/downloading ones) actually running, and everything past that
/// window queued — called after anything that could change which downloads
/// belong in that window: one added, removed, finished, reordered, or the
/// limit itself changing. Unlike a plain "fill empty slots" scheduler, this
/// also demotes a running download that a drag-and-drop reorder pushed out
/// of the window, so a higher-priority one can take its place.
pub async fn try_promote_queue(app: &AppHandle) {
    let state = app.state::<AppState>();
    let max = state.settings.lock().await.max_simultaneous_downloads.max(1);

    let mut candidates: Vec<DownloadRecord> = {
        let records = state.records.lock().await;
        records.values().filter(|r| matches!(r.status, DownloadStatus::Downloading | DownloadStatus::Queued)).cloned().collect()
    };
    candidates.sort_by_key(|r| r.seq);
    let split_at = candidates.len().min(max);
    let (should_run, should_wait) = candidates.split_at(split_at);

    for record in should_wait {
        if record.status != DownloadStatus::Downloading {
            continue;
        }
        let _ = state.engine.pause_silent(record.id).await;
        let updated = {
            let mut records = state.records.lock().await;
            records.get_mut(&record.id).map(|r| {
                r.status = DownloadStatus::Queued;
                r.speed_bps = 0.0;
                r.active_chunks.clear();
                r.clone()
            })
        };
        if let Some(record) = updated {
            publish(app, &state, &record);
        }
    }

    for record in should_run {
        match record.status {
            DownloadStatus::Queued => {}
            // A record can say Downloading while its engine task is actually
            // gone (e.g. it was added — or its recovery was still in flight —
            // while another record wrongly held this slot, or its task ended
            // without the resulting event having been applied yet). Trusting
            // the status blindly here means a single stale record permanently
            // occupies the slot and every future download sits Queued forever;
            // verify liveness and fall through to (re)start it if it's dead.
            DownloadStatus::Downloading if !state.engine.is_active(record.id).await => {}
            _ => continue,
        }
        match resume_or_restart(app, record.id).await {
            Ok(()) => {
                let updated = {
                    let mut records = state.records.lock().await;
                    records.get_mut(&record.id).map(|r| {
                        r.status = DownloadStatus::Downloading;
                        r.clone()
                    })
                };
                // resume_or_restart may have taken the "resume a live paused
                // task" path, which deliberately emits no event of its own
                // (see resume_silent) — publish here so the frontend actually
                // finds out, instead of showing this download stuck Queued
                // while it's genuinely transferring again.
                if let Some(record) = updated {
                    publish(app, &state, &record);
                }
            }
            Err(e) => {
                let updated = {
                    let mut records = state.records.lock().await;
                    records.get_mut(&record.id).map(|r| {
                        r.status = DownloadStatus::Failed;
                        r.error = Some(e);
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
        | DownloadEvent::Resumed { id }
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
                    DownloadEvent::Resumed { .. } => {
                        record.status = DownloadStatus::Downloading;
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
