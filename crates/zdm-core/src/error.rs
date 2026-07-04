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

    pub fn is_rate_limited(&self) -> bool {
        matches!(self, DownloadError::Retryable { status, .. } if *status == reqwest::StatusCode::TOO_MANY_REQUESTS)
    }
}
