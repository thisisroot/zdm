use std::path::PathBuf;

use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::batch::parse_batch_pattern;
use crate::filename::filename_from_url;
use crate::queue::{publish, try_promote_queue};
use crate::state::{AppState, DownloadRecord, DownloadStatus, QueueInfo, Settings};

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
    let queue_id = slugify(&queue_name);
    ensure_queue(&state, &queue_id, &queue_name).await;

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
        publish(&app, &state, &record);
        ids.push(id.to_string());
    }

    try_promote_queue(&app).await;
    Ok(ids)
}

#[tauri::command]
pub async fn pause_download(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    state.engine.pause(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn resume_download(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
    state.engine.resume(id).await.map_err(|e| e.to_string())
}

/// Retries a failed download: continues from its sidecar if one survived, or
/// starts fresh if the failure happened before any chunk ever landed on disk.
#[tauri::command]
pub async fn retry_download(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let id = Uuid::parse_str(&id).map_err(|e| e.to_string())?;
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
            zdm_core::DownloadOptions { id, url: r.url.clone(), destination: PathBuf::from(&r.destination), connections: r.connections }
        };
        state.engine.start(opts).await
    };

    result.map(|_| ()).map_err(|e| e.to_string())
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
