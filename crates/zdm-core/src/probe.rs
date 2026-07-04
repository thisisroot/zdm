use reqwest::header::{HeaderMap, HeaderName, ACCEPT_RANGES, CONTENT_LENGTH, CONTENT_RANGE, ETAG, LAST_MODIFIED, RANGE};
use reqwest::Client;

use crate::error::DownloadError;

#[derive(Debug, Clone)]
pub struct ProbeResult {
    pub total_size: Option<u64>,
    pub supports_ranges: bool,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

/// Determines whether a URL can be split into parallel ranged requests, without
/// risking a full-body download during the check.
///
/// HEAD is tried first since it's the standard, zero-body way to read
/// `Accept-Ranges` and `Content-Length`. Some servers refuse or misreport HEAD,
/// so if it's inconclusive we fall back to a single-byte ranged GET: a `206`
/// response proves range support and `Content-Range` reveals the real size.
pub async fn probe(client: &Client, url: &str) -> Result<ProbeResult, DownloadError> {
    if let Ok(resp) = client.head(url).send().await {
        if resp.status().is_success() {
            let total_size = header_u64(resp.headers(), CONTENT_LENGTH);
            if total_size.is_some() {
                let supports_ranges = header_str(resp.headers(), ACCEPT_RANGES)
                    .map(|v| v.eq_ignore_ascii_case("bytes"))
                    .unwrap_or(false);
                return Ok(ProbeResult {
                    total_size,
                    supports_ranges,
                    etag: header_string(resp.headers(), ETAG),
                    last_modified: header_string(resp.headers(), LAST_MODIFIED),
                });
            }
        }
    }

    let resp = client.get(url).header(RANGE, "bytes=0-0").send().await?;
    let etag = header_string(resp.headers(), ETAG);
    let last_modified = header_string(resp.headers(), LAST_MODIFIED);

    if resp.status() == reqwest::StatusCode::PARTIAL_CONTENT {
        let total_size = header_str(resp.headers(), CONTENT_RANGE)
            .and_then(|v| v.rsplit('/').next())
            .and_then(|v| v.parse::<u64>().ok());
        return Ok(ProbeResult { total_size, supports_ranges: true, etag, last_modified });
    }

    if resp.status().is_success() {
        return Ok(ProbeResult {
            total_size: header_u64(resp.headers(), CONTENT_LENGTH),
            supports_ranges: false,
            etag,
            last_modified,
        });
    }

    Err(DownloadError::BadStatus(resp.status()))
}

fn header_str(headers: &HeaderMap, name: HeaderName) -> Option<&str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}

fn header_string(headers: &HeaderMap, name: HeaderName) -> Option<String> {
    header_str(headers, name).map(str::to_string)
}

fn header_u64(headers: &HeaderMap, name: HeaderName) -> Option<u64> {
    header_str(headers, name).and_then(|v| v.parse().ok())
}
