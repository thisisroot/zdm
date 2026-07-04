use thiserror::Error;

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("http request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("server returned status {0}")]
    BadStatus(reqwest::StatusCode),
    #[error("server did not report a usable content length")]
    UnknownLength,
    #[error("failed to read or write download metadata: {0}")]
    Meta(#[from] serde_json::Error),
    #[error("no download with id {0}")]
    NotFound(uuid::Uuid),
    #[error("download was canceled")]
    Canceled,
}
