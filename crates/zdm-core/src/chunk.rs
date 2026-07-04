use serde::{Deserialize, Serialize};

/// An inclusive byte range, matching HTTP `Range: bytes=start-end` semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByteRange {
    pub start: u64,
    pub end: u64,
}

impl ByteRange {
    pub fn len(&self) -> u64 {
        self.end - self.start + 1
    }

    pub fn is_empty(&self) -> bool {
        false
    }
}

const MIN_CHUNK_SIZE: u64 = 256 * 1024; // 256 KiB
const MAX_CHUNK_SIZE: u64 = 4 * 1024 * 1024; // 4 MiB

/// Splits a file into small chunks rather than one static range per connection.
///
/// Workers pull chunks from a shared queue until it's empty, so a fast connection
/// naturally does more work than a slow one instead of sitting idle once its
/// fixed share finishes — this is what lets total throughput approach the sum of
/// each connection's real capacity instead of being capped by the slowest one.
pub fn plan_chunks(total_size: u64, connections: usize) -> Vec<ByteRange> {
    if total_size == 0 {
        return Vec::new();
    }
    let target_chunk_count = (connections.max(1) as u64) * 4;
    let chunk_size = (total_size / target_chunk_count).clamp(MIN_CHUNK_SIZE, MAX_CHUNK_SIZE);

    let mut chunks = Vec::new();
    let mut offset = 0u64;
    while offset < total_size {
        let end = (offset + chunk_size - 1).min(total_size - 1);
        chunks.push(ByteRange { start: offset, end });
        offset = end + 1;
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covers_whole_file_with_no_gaps_or_overlaps() {
        for total_size in [1u64, 100, 4096, 10_000_000, 987_654_321] {
            for connections in [1usize, 4, 8, 16] {
                let chunks = plan_chunks(total_size, connections);
                assert_eq!(chunks.first().unwrap().start, 0);
                assert_eq!(chunks.last().unwrap().end, total_size - 1);
                for pair in chunks.windows(2) {
                    assert_eq!(pair[0].end + 1, pair[1].start, "gap or overlap between chunks");
                }
                let sum: u64 = chunks.iter().map(|c| c.len()).sum();
                assert_eq!(sum, total_size);
            }
        }
    }

    #[test]
    fn zero_size_yields_no_chunks() {
        assert!(plan_chunks(0, 8).is_empty());
    }

    #[test]
    fn more_connections_than_bytes_still_terminates() {
        let chunks = plan_chunks(3, 16);
        let sum: u64 = chunks.iter().map(|c| c.len()).sum();
        assert_eq!(sum, 3);
    }
}
