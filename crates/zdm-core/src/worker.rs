use std::collections::HashMap;
use std::io::SeekFrom;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use futures_util::StreamExt;
use reqwest::header::RANGE;
use reqwest::Client;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::Mutex;

use crate::chunk::ByteRange;
use crate::error::DownloadError;

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
        return Err(DownloadError::BadStatus(resp.status()));
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
        return Err(DownloadError::BadStatus(resp.status()));
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
