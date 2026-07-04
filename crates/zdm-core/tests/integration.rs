//! End-to-end tests against a real local HTTP server: no mocking of reqwest or
//! the filesystem, so these exercise the exact code path production traffic hits.

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use tokio::sync::mpsc::UnboundedReceiver;

use zdm_core::{DownloadEngine, DownloadEvent, DownloadMeta, DownloadOptions};

struct ServerState {
    data: Vec<u8>,
    delay: Duration,
}

struct FlakyState {
    data: Vec<u8>,
    remaining_429s: std::sync::atomic::AtomicUsize,
}

/// Deterministic filler so the test can regenerate and compare without keeping
/// a second in-memory copy alive across await points.
fn make_bytes(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 256) as u8).collect()
}

fn parse_range(header: &str, total: u64) -> Option<(u64, u64)> {
    let spec = header.strip_prefix("bytes=")?;
    let mut parts = spec.splitn(2, '-');
    let start: u64 = parts.next()?.parse().ok()?;
    let end_str = parts.next()?;
    let end = if end_str.is_empty() { total - 1 } else { end_str.parse().ok()? };
    Some((start, end.min(total - 1)))
}

/// A range-aware endpoint: honors `Range`, reports `Accept-Ranges: bytes`, and
/// can be told to sleep before each response so tests can reliably catch a
/// download mid-flight (e.g. to pause it).
async fn serve_ranged(headers: HeaderMap, State(state): State<Arc<ServerState>>) -> Response {
    let total = state.data.len() as u64;
    if let Some(range) = headers.get(header::RANGE).and_then(|v| v.to_str().ok()) {
        if let Some((start, end)) = parse_range(range, total) {
            if state.delay > Duration::ZERO {
                tokio::time::sleep(state.delay).await;
            }
            let slice = state.data[start as usize..=end as usize].to_vec();
            return Response::builder()
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{total}"))
                .header(header::CONTENT_LENGTH, slice.len())
                .header(header::ACCEPT_RANGES, "bytes")
                .body(Body::from(slice))
                .unwrap();
        }
    }
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_LENGTH, total)
        .header(header::ACCEPT_RANGES, "bytes")
        .body(Body::from(state.data.clone()))
        .unwrap()
}

/// An endpoint that ignores `Range` entirely and never advertises support for
/// it, so the engine must fall back to a single sequential connection.
async fn serve_unranged(State(state): State<Arc<ServerState>>) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_LENGTH, state.data.len())
        .body(Body::from(state.data.clone()))
        .unwrap()
}

/// Rate-limits real chunk fetches (any ranged request past the first byte)
/// for a fixed number of requests, then serves normally — simulating a
/// server that 429s under concurrent load. The initial probe (`HEAD`, or the
/// `bytes=0-0` fallback range) is deliberately exempt so `start()` itself can
/// still succeed; the throttling behavior under test is what happens once
/// real chunk downloads are underway.
async fn serve_flaky(headers: HeaderMap, State(state): State<Arc<FlakyState>>) -> Response {
    let total = state.data.len() as u64;
    let Some(range) = headers.get(header::RANGE).and_then(|v| v.to_str().ok()) else {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_LENGTH, total)
            .header(header::ACCEPT_RANGES, "bytes")
            .body(Body::from(state.data.clone()))
            .unwrap();
    };
    let Some((start, end)) = parse_range(range, total) else {
        return Response::builder().status(StatusCode::BAD_REQUEST).body(Body::empty()).unwrap();
    };
    let is_probe = start == 0 && end == 0;

    if !is_probe {
        let prev = state.remaining_429s.fetch_update(std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst, |n| {
            if n > 0 { Some(n - 1) } else { None }
        });
        if prev.is_ok() {
            return Response::builder()
                .status(StatusCode::TOO_MANY_REQUESTS)
                .header(header::RETRY_AFTER, "0")
                .body(Body::empty())
                .unwrap();
        }
    }

    let slice = state.data[start as usize..=end as usize].to_vec();
    Response::builder()
        .status(StatusCode::PARTIAL_CONTENT)
        .header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{total}"))
        .header(header::CONTENT_LENGTH, slice.len())
        .header(header::ACCEPT_RANGES, "bytes")
        .body(Body::from(slice))
        .unwrap()
}

async fn spawn_flaky_server(data: Vec<u8>, remaining_429s: usize) -> String {
    let state = Arc::new(FlakyState { data, remaining_429s: std::sync::atomic::AtomicUsize::new(remaining_429s) });
    let app = Router::new().route("/flaky.bin", get(serve_flaky)).with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

async fn spawn_server(data: Vec<u8>, delay: Duration) -> String {
    let state = Arc::new(ServerState { data, delay });
    let app = Router::new()
        .route("/ranged.bin", get(serve_ranged))
        .route("/unranged.bin", get(serve_unranged))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

async fn wait_for_completion(events: &mut UnboundedReceiver<DownloadEvent>) {
    loop {
        match tokio::time::timeout(Duration::from_secs(10), events.recv()).await {
            Ok(Some(DownloadEvent::Completed { .. })) => return,
            Ok(Some(DownloadEvent::Failed { error, .. })) => panic!("download failed: {error}"),
            Ok(Some(_)) => continue,
            Ok(None) => panic!("event channel closed before completion"),
            Err(_) => panic!("timed out waiting for download to complete"),
        }
    }
}

#[tokio::test]
async fn segmented_download_reassembles_byte_identical_file() {
    let source = make_bytes(3 * 1024 * 1024 + 777); // deliberately not a round chunk multiple
    let base_url = spawn_server(source.clone(), Duration::ZERO).await;

    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("downloaded.bin");

    let (engine, mut events) = DownloadEngine::new();
    let id = engine
        .start(DownloadOptions { id: uuid::Uuid::new_v4(), url: format!("{base_url}/ranged.bin"), destination: destination.clone(), connections: 6 })
        .await
        .unwrap();

    match events.recv().await.unwrap() {
        DownloadEvent::Started { id: started_id, total_size, resumable } => {
            assert_eq!(started_id, id);
            assert_eq!(total_size, Some(source.len() as u64));
            assert!(resumable, "server advertises range support, engine should have detected it");
        }
        other => panic!("expected Started, got {other:?}"),
    }

    wait_for_completion(&mut events).await;

    let downloaded = tokio::fs::read(&destination).await.unwrap();
    assert_eq!(downloaded, source, "reassembled file must match the source byte-for-byte");
    assert!(DownloadMeta::load(&destination).await.is_none(), "sidecar metadata should be cleaned up on success");
}

#[tokio::test]
async fn falls_back_to_sequential_when_server_ignores_ranges() {
    let source = make_bytes(512 * 1024);
    let base_url = spawn_server(source.clone(), Duration::ZERO).await;

    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("downloaded.bin");

    let (engine, mut events) = DownloadEngine::new();
    let id = engine
        .start(DownloadOptions { id: uuid::Uuid::new_v4(), url: format!("{base_url}/unranged.bin"), destination: destination.clone(), connections: 8 })
        .await
        .unwrap();

    match events.recv().await.unwrap() {
        DownloadEvent::Started { id: started_id, resumable, .. } => {
            assert_eq!(started_id, id);
            assert!(!resumable, "server without Accept-Ranges must not be treated as resumable");
        }
        other => panic!("expected Started, got {other:?}"),
    }

    wait_for_completion(&mut events).await;

    let downloaded = tokio::fs::read(&destination).await.unwrap();
    assert_eq!(downloaded, source);
}

#[tokio::test]
async fn pausing_then_resuming_from_disk_finishes_without_corruption() {
    // Slow enough that the test can reliably catch it mid-transfer and pause.
    let source = make_bytes(1_600_000);
    let base_url = spawn_server(source.clone(), Duration::from_millis(250)).await;

    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("downloaded.bin");

    let (engine_one, mut events_one) = DownloadEngine::new();
    let id = engine_one
        .start(DownloadOptions { id: uuid::Uuid::new_v4(), url: format!("{base_url}/ranged.bin"), destination: destination.clone(), connections: 2 })
        .await
        .unwrap();

    // Drain Started, then wait for real progress before pausing.
    assert!(matches!(events_one.recv().await.unwrap(), DownloadEvent::Started { .. }));
    loop {
        match tokio::time::timeout(Duration::from_secs(10), events_one.recv()).await {
            Ok(Some(DownloadEvent::Progress { downloaded, .. })) if downloaded > 0 => break,
            Ok(Some(_)) => continue,
            _ => panic!("never observed progress before timeout"),
        }
    }

    engine_one.pause(id).await.unwrap();
    // Give any in-flight chunk time to land before we inspect the sidecar —
    // pausing only takes effect between chunks, not mid-stream.
    tokio::time::sleep(Duration::from_millis(600)).await;

    let meta = DownloadMeta::load(&destination).await.expect("sidecar must exist for a paused, resumable download");
    assert!(!meta.completed_chunks.is_empty(), "expected at least one chunk to have completed before pausing");
    assert!(
        meta.completed_chunks.len() < meta.chunks.len(),
        "test is only meaningful if the download was genuinely interrupted, not finished"
    );

    // Simulate an app restart: a fresh engine, independent of engine_one (whose
    // workers are left parked in the paused state and never touch the file again).
    let (engine_two, mut events_two) = DownloadEngine::new();
    let resumed_id = engine_two.resume_from_disk(destination.clone()).await.unwrap();
    assert_eq!(resumed_id, id, "resuming must preserve the original download's id");

    wait_for_completion(&mut events_two).await;

    let downloaded = tokio::fs::read(&destination).await.unwrap();
    assert_eq!(downloaded, source, "resumed download must still reassemble to the exact source bytes");
    assert!(DownloadMeta::load(&destination).await.is_none());
}

#[tokio::test]
async fn survives_rate_limiting_instead_of_failing_outright() {
    let source = make_bytes(600_000);
    // More 429s than any single chunk retry budget alone would tolerate if
    // they all landed on one connection — only recoverable because failing
    // chunks go back to a shared queue other (or the same, later) workers
    // keep draining, backed by retry-with-backoff.
    let base_url = spawn_flaky_server(source.clone(), 10).await;

    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("downloaded.bin");

    let (engine, mut events) = DownloadEngine::new();
    engine
        .start(DownloadOptions { id: uuid::Uuid::new_v4(), url: format!("{base_url}/flaky.bin"), destination: destination.clone(), connections: 4 })
        .await
        .unwrap();

    assert!(matches!(events.recv().await.unwrap(), DownloadEvent::Started { .. }));
    wait_for_completion(&mut events).await;

    let downloaded = tokio::fs::read(&destination).await.unwrap();
    assert_eq!(downloaded, source, "must still reassemble correctly after recovering from rate limiting");
}
