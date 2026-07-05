use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::batch::parse_batch_pattern;
use crate::filename::filename_from_url;
use crate::queue::{publish, resume_or_restart, try_promote_queue};
use crate::state::{AppState, DownloadRecord, DownloadStatus, QueueInfo, Settings};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UrlProbe {
    pub url: String,
    pub total_size: Option<u64>,
    pub error: Option<String>,
}

fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut last_was_dash = true; // suppresses a leading dash
    for ch in input.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_was_dash = false;
        } else if !last_was_dash {
            out.push('-');
            last_was_dash = true;
        }
    }
    let trimmed = out.trim_end_matches('-').to_string();
    if trimmed.is_empty() {
        format!("queue-{}", Uuid::new_v4())
    } else {
        trimmed
    }
}

async fn ensure_queue(state: &AppState, queue_id: &str, queue_name: &str) {
    let mut queues = state.queues.lock().await;
    if !queues.iter().any(|q| q.id == queue_id) {
        let queue = QueueInfo { id: queue_id.to_string(), name: queue_name.to_string() };
        state.db.upsert_queue(&queue);
        queues.push(queue);
    }
}

#[tauri::command]
pub async fn list_downloads(state: State<'_, AppState>) -> Result<Vec<DownloadRecord>, String> {
    let mut records: Vec<_> = state.records.lock().await.values().cloned().collect();
    records.sort_by_key(|r| r.seq);
    Ok(records)
}

#[tauri::command]
pub async fn list_queues(state: State<'_, AppState>) -> Result<Vec<QueueInfo>, String> {
    Ok(state.queues.lock().await.clone())
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    Ok(state.settings.lock().await.clone())
}

#[tauri::command]
pub async fn update_settings(state: State<'_, AppState>, app: AppHandle, settings: Settings) -> Result<(), String> {
    state.db.save_settings(&settings);
    *state.settings.lock().await = settings;
    try_promote_queue(&app).await;
    Ok(())
}

#[tauri::command]
pub async fn choose_directory(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folder(move |folder| {
        let _ = tx.send(folder);
    });
    let picked = rx.await.map_err(|e| e.to_string())?;
    Ok(picked.map(|p| p.to_string()))
}

/// Checks whether a download's computed filename already exists in the
/// destination folder, so the frontend can offer Replace/Rename before the
/// transfer starts rather than silently overwriting or erroring mid-flight.
/// `filename` lets the frontend re-check a user-typed alternate name too.
#[tauri::command]
pub async fn check_conflict(destination_dir: String, url: String, filename: Option<String>) -> Result<Option<String>, String> {
    let name = match filename {
        Some(f) => crate::filename::sanitize(&f),
        None => filename_from_url(&url),
    };
    let path = PathBuf::from(&destination_dir).join(&name);
    Ok(if tokio::fs::try_exists(&path).await.unwrap_or(false) { Some(name) } else { None })
}

#[tauri::command]
pub async fn add_download(
    state: State<'_, AppState>,
    app: AppHandle,
    url: String,
    destination_dir: String,
    connections: usize,
    category: String,
    queue: String,
    filename: Option<String>,
) -> Result<String, String> {
    let id = Uuid::new_v4();
    let name = match filename {
        Some(f) => crate::filename::sanitize(&f),
        None => filename_from_url(&url),
    };
    let destination = PathBuf::from(&destination_dir).join(&name);
    let seq = state.next_seq();

    ensure_queue(&state, &queue, &queue).await;

    let record = DownloadRecord {
        id,
        seq,
        url,
        name,
        destination: destination.to_string_lossy().into_owned(),
        category,
        queue,
        connections: connections.max(1),
        status: DownloadStatus::Queued,
        downloaded: 0,
        total_size: None,
        speed_bps: 0.0,
        error: None,
        active_chunks: Vec::new(),
    };
    state.records.lock().await.insert(id, record.clone());
    publish(&app, &state, &record);

    try_promote_queue(&app).await;
    Ok(id.to_string())
}

async fn add_batch_from_urls(
    state: &AppState,
    app: &AppHandle,
    urls: Vec<String>,
    destination_dir: String,
    connections: usize,
    category: String,
    queue_name: String,
) -> Result<Vec<String>, String> {
    let queue_id = slugify(&queue_name);
    ensure_queue(state, &queue_id, &queue_name).await;

    let mut ids = Vec::with_capacity(urls.len());
    for url in urls {
        let id = Uuid::new_v4();
        let name = filename_from_url(&url);
        let destination = PathBuf::from(&destination_dir).join(&name);
        let seq = state.next_seq();
        let record = DownloadRecord {
            id,
            seq,
            url,
            name,
            destination: destination.to_string_lossy().into_owned(),
            category: category.clone(),
            queue: queue_id.clone(),
            connections: connections.max(1),
            status: DownloadStatus::Queued,
            downloaded: 0,
            total_size: None,
            speed_bps: 0.0,
            error: None,
            active_chunks: Vec::new(),
        };
        state.records.lock().await.insert(id, record.clone());
        publish(app, state, &record);
        ids.push(id.to_string());
    }

    try_promote_queue(app).await;
    Ok(ids)
}

#[tauri::command]
pub async fn add_batch(
    state: State<'_, AppState>,
    app: AppHandle,
    url_pattern: String,
    destination_dir: String,
    connections: usize,
    category: String,
    queue_name: String,
) -> Result<Vec<String>, String> {
    let urls = parse_batch_pattern(&url_pattern)?;
    add_batch_from_urls(&state, &app, urls, destination_dir, connections, category, queue_name).await
}

/// Same as `add_batch`, but for a batch whose URLs were already resolved and
/// validated by the frontend (e.g. after probing a pattern's expansion and
/// letting the user drop the broken links) — skips pattern parsing entirely.
#[tauri::command]
pub async fn add_batch_urls(
    state: State<'_, AppState>,
    app: AppHandle,
    urls: Vec<String>,
    destination_dir: String,
    connections: usize,
    category: String,
    queue_name: String,
) -> Result<Vec<String>, String> {
    add_batch_from_urls(&state, &app, urls, destination_dir, connections, category, queue_name).await
}

/// Checks reachability and size for a batch of URLs concurrently (bounded, so
/// a large batch doesn't hammer the remote server with dozens of simultaneous
/// requests), preserving input order so the frontend can map results back to
/// specific rows.
#[tauri::command]
pub async fn probe_urls(state: State<'_, AppState>, urls: Vec<String>) -> Result<Vec<UrlProbe>, String> {
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(6));
    let mut handles = Vec::with_capacity(urls.len());
    for url in urls {
        let engine = state.engine.clone();
        let semaphore = semaphore.clone();
        handles.push(tokio::spawn(async move {
            let _permit = semaphore.acquire_owned().await.expect("semaphore is never closed");
            match engine.probe_url(&url).await {
                Ok(result) => UrlProbe { url, total_size: result.total_size, error: None },
                Err(e) => UrlProbe { url, total_size: None, error: Some(e.to_string()) },
            }
        }));
    }

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        results.push(handle.await.map_err(|e| e.to_string())?);
    }
    Ok(results)
}

#[tauri::command]
pub async fn pause_download(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    state.engine.pause(id).await.map_err(|e| e.to_string())
}

/// Resuming a manually-paused download is a direct, immediate action (unlike
/// the scheduler's own promotions, it bypasses `max_simultaneous_downloads`)
/// but still needs the same live-task-or-fall-back-to-disk robustness, since
/// a download can now be Paused without ever having had an engine task at
/// all (e.g. `toggle_queue` pausing a not-yet-started queued download).
#[tauri::command]
pub async fn resume_download(app: AppHandle, id: String) -> Result<(), String> {
    let uid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    resume_or_restart(&app, uid).await
}

/// Retries a failed download: continues from its sidecar if one survived, or
/// starts fresh if the failure happened before any chunk ever landed on disk.
#[tauri::command]
pub async fn retry_download(app: AppHandle, id: String) -> Result<(), String> {
    let uid = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    resume_or_restart(&app, uid).await
}

#[tauri::command]
pub async fn cancel_download(state: State<'_, AppState>, app: AppHandle, id: String) -> Result<(), String> {
    let id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let was_only_queued = {
        let records = state.records.lock().await;
        matches!(records.get(&id).map(|r| r.status), Some(DownloadStatus::Queued))
    };

    if was_only_queued {
        // Never handed to the engine yet, so there's no task to cancel — just
        // drop it out of the queue and let the scheduler fill the freed slot.
        let updated = {
            let mut records = state.records.lock().await;
            records.get_mut(&id).map(|r| {
                r.status = DownloadStatus::Canceled;
                r.clone()
            })
        };
        if let Some(record) = updated {
            publish(&app, &state, &record);
        }
        try_promote_queue(&app).await;
        Ok(())
    } else {
        state.engine.cancel(id).await.map_err(|e| e.to_string())
    }
}

/// Applies a drag-and-drop reorder: `ids` is the full new order, so each
/// entry's index becomes its new `seq`. Only rows whose seq actually changed
/// are republished, to avoid emitting a flood of no-op updates.
#[tauri::command]
pub async fn reorder_downloads(state: State<'_, AppState>, app: AppHandle, ids: Vec<String>) -> Result<(), String> {
    let mut changed = Vec::new();
    {
        let mut records = state.records.lock().await;
        for (i, id_str) in ids.iter().enumerate() {
            let id = Uuid::parse_str(id_str).map_err(|e| e.to_string())?;
            let seq = i as u64;
            if let Some(r) = records.get_mut(&id) {
                if r.seq != seq {
                    r.seq = seq;
                    changed.push(r.clone());
                }
            }
        }
    }
    state.set_seq_floor(ids.len() as u64);
    for record in &changed {
        publish(&app, &state, record);
    }
    // The reorder may have pushed a running download out of the active
    // window (or pulled one in) — resync who's actually transferring.
    try_promote_queue(&app).await;
    Ok(())
}

/// Stops or starts a group of downloads (a queue, or everything) as one unit.
/// Stopping pauses the active member(s) and holds any not-yet-started ones so
/// the scheduler can't immediately refill their slot from the same group.
/// Starting makes held downloads queued again and lets the scheduler pick up
/// to `max_simultaneous_downloads` of them — it does not force all of them to
/// run at once.
async fn toggle_group(state: &AppState, app: &AppHandle, members: Vec<(Uuid, DownloadStatus)>) {
    let any_active = members.iter().any(|(_, s)| matches!(s, DownloadStatus::Downloading | DownloadStatus::Queued));

    if any_active {
        for (id, status) in &members {
            match status {
                DownloadStatus::Downloading => {
                    let _ = state.engine.pause_silent(*id).await;
                    let updated = {
                        let mut records = state.records.lock().await;
                        records.get_mut(id).map(|r| {
                            r.status = DownloadStatus::Paused;
                            r.speed_bps = 0.0;
                            r.active_chunks.clear();
                            r.clone()
                        })
                    };
                    if let Some(record) = updated {
                        publish(app, state, &record);
                    }
                }
                DownloadStatus::Queued => {
                    let updated = {
                        let mut records = state.records.lock().await;
                        records.get_mut(id).map(|r| {
                            r.status = DownloadStatus::Paused;
                            r.clone()
                        })
                    };
                    if let Some(record) = updated {
                        publish(app, state, &record);
                    }
                }
                _ => {}
            }
        }
    } else {
        for (id, status) in &members {
            if *status == DownloadStatus::Paused {
                let updated = {
                    let mut records = state.records.lock().await;
                    records.get_mut(id).map(|r| {
                        r.status = DownloadStatus::Queued;
                        r.clone()
                    })
                };
                if let Some(record) = updated {
                    publish(app, state, &record);
                }
            }
        }
    }

    // Stopping frees slots for other groups; starting fills up to the limit
    // with this group's now-queued downloads — either way the scheduler
    // decides what actually runs.
    try_promote_queue(app).await;
}

#[tauri::command]
pub async fn toggle_queue(state: State<'_, AppState>, app: AppHandle, queue_id: String) -> Result<(), String> {
    let members: Vec<(Uuid, DownloadStatus)> = {
        let records = state.records.lock().await;
        records.values().filter(|r| r.queue == queue_id).map(|r| (r.id, r.status)).collect()
    };
    toggle_group(&state, &app, members).await;
    Ok(())
}

/// The topbar's global pause/resume-all control — same semantics as
/// `toggle_queue` but across every download regardless of queue.
#[tauri::command]
pub async fn toggle_all(state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    let members: Vec<(Uuid, DownloadStatus)> = {
        let records = state.records.lock().await;
        records.values().map(|r| (r.id, r.status)).collect()
    };
    toggle_group(&state, &app, members).await;
    Ok(())
}

/// Deleting a queue never deletes its downloads — members are reassigned to
/// the default queue so nothing silently disappears from the list.
#[tauri::command]
pub async fn delete_queue(state: State<'_, AppState>, app: AppHandle, id: String) -> Result<(), String> {
    if id == "default" {
        return Err("the default queue can't be deleted".to_string());
    }

    let reassigned = {
        let mut records = state.records.lock().await;
        records
            .values_mut()
            .filter(|r| r.queue == id)
            .map(|r| {
                r.queue = "default".to_string();
                r.clone()
            })
            .collect::<Vec<_>>()
    };
    for record in &reassigned {
        publish(&app, &state, record);
    }

    state.queues.lock().await.retain(|q| q.id != id);
    state.db.delete_queue(&id);
    let _ = app.emit("queue-removed", &id);
    Ok(())
}

#[tauri::command]
pub async fn remove_download(state: State<'_, AppState>, app: AppHandle, id: String, delete_file: bool) -> Result<(), String> {
    let id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    let _ = state.engine.cancel(id).await; // harmless no-op if it wasn't an active engine task
    let record = state.records.lock().await.remove(&id);
    state.db.delete_download(id);
    let _ = app.emit("download-removed", id.to_string());
    if delete_file {
        if let Some(record) = &record {
            let path = PathBuf::from(&record.destination);
            let _ = tokio::fs::remove_file(&path).await;
            zdm_core::DownloadMeta::delete(&path).await;
        }
    }
    // The removed download may have been occupying an active slot or sitting
    // queued ahead of others — either way the scheduler needs a nudge, since
    // nothing else triggers it on this path.
    if record.is_some() {
        try_promote_queue(&app).await;
    }
    Ok(())
}
