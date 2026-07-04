use std::collections::HashMap;
use std::io::SeekFrom;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::header::RANGE;
use reqwest::{Client, Response, StatusCode};
use tokio::fs::OpenOptions;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::Mutex;

use crate::chunk::ByteRange;
use crate::error::DownloadError;

/// Classifies a non-success response: 429/5xx are worth retrying (with the
/// server's own `Retry-After` if it gave one), everything else — 403, 404,
/// etc. — won't resolve by trying again.
fn classify_bad_status(resp: &Response) -> DownloadError {
    let status = resp.status();
    if status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
        let retry_after = resp
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .map(Duration::from_secs);
        DownloadError::Retryable { status, retry_after }
    } else {
        DownloadError::BadStatus(status)
    }
}

/// Downloads one byte range and writes it at the matching offset in the
/// (already fully-allocated) destination file.
///
/// Each chunk gets its own file handle rather than sharing one across workers:
/// every `open()` call gets an independent OS-level file position, so N workers
/// can seek-and-write into disjoint regions of the same file with no locking or
/// coordination beyond the byte ranges themselves not overlapping.
///
/// `active` tracks bytes-written-so-far per in-flight chunk index purely for
/// UI reporting — the engine's progress ticker reads it to show genuine
/// per-connection progress instead of a single aggregate bar.
pub async fn download_chunk(
    client: &Client,
    url: &str,
    destination: &Path,
    chunk_index: usize,
    range: ByteRange,
    downloaded: &AtomicU64,
    active: &Mutex<HashMap<usize, u64>>,
) -> Result<(), DownloadError> {
    let resp = client
        .get(url)
        .header(RANGE, format!("bytes={}-{}", range.start, range.end))
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(classify_bad_status(&resp));
    }

    let mut file = OpenOptions::new().write(true).open(destination).await?;
    file.seek(SeekFrom::Start(range.start)).await?;

    let mut written = 0u64;
    let mut stream = resp.bytes_stream();
    while let Some(next) = stream.next().await {
        let bytes = next?;
        file.write_all(&bytes).await?;
        downloaded.fetch_add(bytes.len() as u64, Ordering::Relaxed);
        written += bytes.len() as u64;
        active.lock().await.insert(chunk_index, written);
    }
    Ok(())
}

/// Fallback for servers that don't support ranged requests: one connection,
/// streamed straight to disk in order. Still fast (network-bound, not
/// disk-bound), just not parallelizable.
pub async fn download_whole_file_sequential(
    client: &Client,
    url: &str,
    destination: &Path,
    downloaded: &AtomicU64,
) -> Result<(), DownloadError> {
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(classify_bad_status(&resp));
    }

    let mut file = OpenOptions::new().write(true).create(true).truncate(true).open(destination).await?;
    let mut stream = resp.bytes_stream();
    while let Some(next) = stream.next().await {
        let bytes = next?;
        file.write_all(&bytes).await?;
        downloaded.fetch_add(bytes.len() as u64, Ordering::Relaxed);
    }
    Ok(())
}
