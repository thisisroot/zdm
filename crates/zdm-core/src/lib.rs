//! ZDM's download engine: probes a URL for range support, splits the file into
//! small chunks pulled from a shared queue by N concurrent connections, and
//! persists enough state to resume after the app restarts.

mod chunk;
mod engine;
mod error;
mod events;
mod meta;
mod probe;
mod progress;
mod worker;

pub use chunk::ByteRange;
pub use engine::{DownloadEngine, DownloadOptions};
pub use error::DownloadError;
pub use events::{ActiveChunk, DownloadEvent};
pub use meta::DownloadMeta;
pub use probe::{probe, ProbeResult};
