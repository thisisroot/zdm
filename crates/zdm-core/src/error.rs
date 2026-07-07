use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("http request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("server returned status {0}")]
    BadStatus(reqwest::StatusCode),
    /// A status worth retrying (429 or 5xx) — distinct from `BadStatus` so
    /// callers can back off and try again instead of failing the whole
    /// download over one connection's transient rejection.
    #[error("server returned status {status}")]
    Retryable { status: reqwest::StatusCode, retry_after: Option<Duration> },
    #[error("server did not report a usable content length")]
    UnknownLength,
    #[error("failed to read or write download metadata: {0}")]
    Meta(#[from] serde_json::Error),
    #[error("no download with id {0}")]
    NotFound(uuid::Uuid),
    #[error("download was canceled")]
    Canceled,
}

impl DownloadError {
    /// Whether retrying is worth attempting, and if the server told us how
    /// long to wait first (`Retry-After`, only present on 429/503 responses).
    pub fn retry_hint(&self) -> Option<Option<Duration>> {
        match self {
            DownloadError::Retryable { retry_after, .. } => Some(*retry_after),
            DownloadError::Http(_) | DownloadError::Io(_) => Some(None),
            _ => None,
        }
    }

    /// Whether this failure means "the server can't handle this many
    /// concurrent connections" — an explicit 429, or a bare connection-level
    /// failure (refused/reset/timed out). Servers that cap concurrency often
    /// don't bother with a 429; they just drop the extra sockets, which
    /// surfaces here as a generic `Http`/`Io` error indistinguishable from a
    /// one-off network blip. Treating it the same as a 429 means a download
    /// against a single-connection-only server converges down to one
    /// connection after the first failed batch instead of re-attempting the
    /// same oversized fan-out on every subsequent chunk.
    pub fn signals_too_many_connections(&self) -> bool {
        matches!(self, DownloadError::Retryable { status, .. } if *status == reqwest::StatusCode::TOO_MANY_REQUESTS)
            || matches!(self, DownloadError::Http(_) | DownloadError::Io(_))
    }
}
