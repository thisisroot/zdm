//! Reproduces the "new downloads never start" report against a real Tauri
//! `App` (real Wry runtime, so `AppHandle`/`commands`/`queue` line up exactly
//! with production types) with a genuine `AppState` and a real local HTTP
//! server — no IPC layer involved, so this isolates the backend scheduler
//! from any frontend explanation.
//!
//! This is a manual repro/regression tool, not a `cargo test` target: Wry's
//! event loop refuses to initialize off the process's actual main thread,
//! and `cargo test` always runs test functions on worker threads, so a real
//! `AppHandle<Wry>` can't be built inside `#[tokio::test]`. Run explicitly
//! with `SCENARIO=1|2|3 cargo run -p zdm --example repro_scheduler`.
//! Scenario 3 is the one that mattered: a record left marked `Downloading`
//! with no live engine task behind it (e.g. from the startup-recovery race
//! documented in docs/PROGRESS.md) used to permanently occupy the sole
//! concurrency slot, so every subsequently added download sat `Queued`
//! forever — matching the "does not start" report exactly.

use std::time::Duration;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use tauri::Manager;

use app_lib::state::{AppState, DownloadRecord, DownloadStatus, Settings};
use app_lib::{commands, queue, Db};

async fn serve_file(Path(_name): Path<String>, State(hits): State<Arc<AtomicUsize>>) -> Response {
    hits.fetch_add(1, Ordering::SeqCst);
    let body = vec![7u8; 4096];
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_LENGTH, body.len())
        .header(header::ACCEPT_RANGES, "bytes")
        .body(Body::from(body))
        .unwrap()
}

async fn spawn_server() -> (String, Arc<AtomicUsize>) {
    let hits = Arc::new(AtomicUsize::new(0));
    let app = Router::new().route("/{name}", get(serve_file)).with_state(hits.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), hits)
}

async fn wait_for<F: Fn(&DownloadRecord) -> bool>(
    state: &tauri::State<'_, AppState>,
    id: uuid::Uuid,
    label: &str,
    cond: F,
) -> DownloadStatus {
    let mut last = None;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let records = state.records.lock().await;
        let record = records.get(&id).expect("record must exist");
        last = Some(record.status);
        if cond(record) {
            return record.status;
        }
    }
    println!("FAIL[{label}]: never satisfied condition, last status = {:?}", last);
    std::process::exit(1);
}

#[tokio::main]
async fn main() {
    // Only one tauri::Builder<Wry> app can exist per process (GTK singleton),
    // so scenarios are gated behind an env var and run as separate processes.
    let scenario = std::env::var("SCENARIO").unwrap_or_else(|_| "1".to_string());
    let (base_url, hits) = spawn_server().await;
    let dir = tempfile::tempdir().unwrap();

    if scenario == "1" {
        let (engine, events_rx) = zdm_core::DownloadEngine::new();
        let db = Db::open(&dir.path().join("s1.sqlite3")).unwrap();
        let mut settings = Settings::with_default_dir(dir.path().to_string_lossy().into_owned());
        settings.notify_on_completion = false; // notification plugin isn't managed in this harness
        let state = AppState::new(engine, db, Vec::new(), Vec::new(), settings);
        let app = tauri::Builder::<tauri::Wry>::new()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("failed to build app");
        app.manage(state);
        queue::spawn_event_forwarder(app.handle().clone(), events_rx);

        let id = commands::add_download(
            app.state::<AppState>(),
            app.handle().clone(),
            format!("{base_url}/one.bin"),
            dir.path().to_string_lossy().into_owned(),
            4,
            "other".to_string(),
            "default".to_string(),
            None,
        )
        .await
        .expect("add_download should succeed");
        let uuid = uuid::Uuid::parse_str(&id).unwrap();

        let s = wait_for(&app.state::<AppState>(), uuid, "scenario1-single", |r| {
            matches!(r.status, DownloadStatus::Downloading | DownloadStatus::Completed)
        })
        .await;
        println!("scenario1 (fresh single): PASS, reached {:?}", s);
    } else if scenario == "2" {
        let (engine, events_rx) = zdm_core::DownloadEngine::new();
        let db = Db::open(&dir.path().join("s2.sqlite3")).unwrap();
        let mut settings = Settings::with_default_dir(dir.path().to_string_lossy().into_owned());
        settings.notify_on_completion = false; // notification plugin isn't managed in this harness
        let state = AppState::new(engine, db, Vec::new(), Vec::new(), settings);
        let app = tauri::Builder::<tauri::Wry>::new()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("failed to build app");
        app.manage(state);
        queue::spawn_event_forwarder(app.handle().clone(), events_rx);

        let urls = vec![format!("{base_url}/b1.bin"), format!("{base_url}/b2.bin"), format!("{base_url}/b3.bin")];
        let ids = commands::add_batch_urls(
            app.state::<AppState>(),
            app.handle().clone(),
            urls,
            dir.path().to_string_lossy().into_owned(),
            4,
            "other".to_string(),
            "My Batch".to_string(),
        )
        .await
        .expect("add_batch_urls should succeed");
        assert_eq!(ids.len(), 3);
        let first = uuid::Uuid::parse_str(&ids[0]).unwrap();

        let s = wait_for(&app.state::<AppState>(), first, "scenario2-batch-first", |r| {
            matches!(r.status, DownloadStatus::Downloading | DownloadStatus::Completed)
        })
        .await;
        println!("scenario2 (fresh batch, first item): PASS, reached {:?}", s);
    } else if scenario == "3" {
        let (engine, events_rx) = zdm_core::DownloadEngine::new();
        let db = Db::open(&dir.path().join("s3.sqlite3")).unwrap();
        let mut settings = Settings::with_default_dir(dir.path().to_string_lossy().into_owned());
        settings.notify_on_completion = false; // notification plugin isn't managed in this harness

        let stale = DownloadRecord {
            id: uuid::Uuid::new_v4(),
            seq: 0,
            url: format!("{base_url}/stale.bin"),
            name: "stale.bin".to_string(),
            destination: dir.path().join("stale.bin").to_string_lossy().into_owned(),
            category: "other".to_string(),
            queue: "default".to_string(),
            connections: 1,
            status: DownloadStatus::Downloading,
            downloaded: 0,
            total_size: None,
            speed_bps: 0.0,
            error: None,
            active_chunks: Vec::new(),
        };
        let state = AppState::new(engine, db, vec![stale], Vec::new(), settings);
        let app = tauri::Builder::<tauri::Wry>::new()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .expect("failed to build app");
        app.manage(state);
        queue::spawn_event_forwarder(app.handle().clone(), events_rx);

        // Deliberately do NOT call resume_interrupted_downloads — add_download
        // straight away, the way a UI click racing app startup would.
        let id = commands::add_download(
            app.state::<AppState>(),
            app.handle().clone(),
            format!("{base_url}/new.bin"),
            dir.path().to_string_lossy().into_owned(),
            4,
            "other".to_string(),
            "default".to_string(),
            None,
        )
        .await
        .expect("add_download should succeed");
        let uuid = uuid::Uuid::parse_str(&id).unwrap();

        let mut last = None;
        let mut ok = false;
        for _ in 0..30 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let app_state = app.state::<AppState>();
            let records = app_state.records.lock().await;
            let record = records.get(&uuid).unwrap();
            last = Some(record.status);
            if matches!(record.status, DownloadStatus::Downloading | DownloadStatus::Completed) {
                ok = true;
                break;
            }
        }
        if ok {
            println!("scenario3 (stale Downloading blocks new): PASS (unexpected!), reached {:?}", last);
        } else {
            println!(
                "scenario3 (stale Downloading blocks new): REPRODUCED BUG — new download stuck at {:?} forever, blocked by a stale non-running 'Downloading' record occupying the only slot",
                last
            );
        }
    }

    println!("total http hits served: {}", hits.load(Ordering::SeqCst));
}

